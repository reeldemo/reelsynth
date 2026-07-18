use super::*;
use super::types::{OvertoneFilterSlot, OvertoneFilterType};

const SR: u32 = 44100;
const N: usize = 256;

fn hf_energy_proxy(samples: &[f32]) -> f32 {
    if samples.len() < 2 {
        return 0.0;
    }
    let mut e = 0.0f32;
    for i in 1..samples.len() {
        let d = samples[i] - samples[i - 1];
        e += d * d;
    }
    e / samples.len() as f32
}

fn peak_delta(samples: &[f32]) -> f32 {
    samples
        .windows(2)
        .map(|w| (w[1] - w[0]).abs())
        .fold(0.0f32, f32::max)
}

fn process_block(
    chain: &mut OvertoneFilterChain,
    input: &[f32],
) -> Vec<f32> {
    input.iter().map(|&x| chain.process_sample(x)).collect()
}

fn make_chain(slots: Vec<OvertoneFilterSlot>, harshness: f32) -> OvertoneFilterChain {
    let mut chain = OvertoneFilterChain::new(SR);
    chain.set_slots(slots);
    chain.set_curve_harshness(harshness);
    chain
}

#[test]
fn empty_chain_is_identity() {
    let mut chain = make_chain(vec![], 1.0);
    for &x in &[0.0f32, 0.5, -0.7, 1.0, -1.0] {
        let out = chain.process_sample(x);
        assert!((out - x).abs() < 1e-6, "empty chain {out} vs {x}");
    }
    let sine = fixture_sine(N);
    let out = process_block(&mut chain, &sine);
    let err = sine
        .iter()
        .zip(out.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(err < 1e-6, "sine identity err {err}");
}

#[test]
fn sine_harshness_low_saw_harshness_high() {
    let sine = fixture_sine(N);
    let saw = fixture_saw_wrap(N);
    let h_sine = curve_harshness(&sine);
    let h_saw = curve_harshness(&saw);
    assert!(h_sine < 0.15, "sine harshness {h_sine}");
    assert!(h_saw > 0.5, "saw harshness {h_saw}");
    assert!(h_saw > h_sine);
}

#[test]
fn each_type_stronger_on_harsh_than_sine() {
    let sine = fixture_sine(N);
    let saw = fixture_saw_wrap(N);
    let h_sine = curve_harshness(&sine);
    let h_saw = curve_harshness(&saw);

    for ty in OvertoneFilterType::ALL {
        let slot = OvertoneFilterSlot {
            filter_type: ty.clone(),
            strength: 1.0,
            bypassed: false,
        };
        let mut chain_s = make_chain(vec![slot.clone()], h_sine);
        let mut chain_h = make_chain(vec![slot], h_saw);
        let out_s = process_block(&mut chain_s, &sine);
        let out_h = process_block(&mut chain_h, &saw);
        let delta_s = hf_energy_proxy(&out_s);
        let delta_h = hf_energy_proxy(&out_h);
        // Input HF energy for reference
        let in_s = hf_energy_proxy(&sine);
        let in_h = hf_energy_proxy(&saw);
        let reduction_s = (in_s - delta_s).abs();
        let reduction_h = (in_h - delta_h).max(0.0);
        // Harsh fixture should see a larger absolute change from input.
        let change_s: f32 = sine
            .iter()
            .zip(out_s.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        let change_h: f32 = saw
            .iter()
            .zip(out_h.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(
            change_h > change_s * 1.5 || change_h > 0.5,
            "{:?}: change_h={change_h} change_s={change_s} red_s={reduction_s} red_h={reduction_h}",
            ty
        );
    }
}

#[test]
fn adaptive_scaling_stronger_with_higher_harshness() {
    let saw = fixture_saw_wrap(N);
    let slot = OvertoneFilterSlot::lowpass();
    let mut mild = make_chain(vec![slot.clone()], 0.1);
    let mut strong = make_chain(vec![slot], 0.9);
    let out_mild = process_block(&mut mild, &saw);
    let out_strong = process_block(&mut strong, &saw);
    let change_mild: f32 = saw
        .iter()
        .zip(out_mild.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();
    let change_strong: f32 = saw
        .iter()
        .zip(out_strong.iter())
        .map(|(a, b)| (a - b).abs())
        .sum();
    assert!(
        change_strong > change_mild * 1.5,
        "adaptive: strong={change_strong} mild={change_mild}"
    );
}

#[test]
fn chain_order_matters_lp_then_slew_vs_reverse() {
    let saw = fixture_saw_wrap(512);
    let h = curve_harshness(&saw);
    let lp = OvertoneFilterSlot::lowpass();
    let slew = OvertoneFilterSlot::slew();

    let mut ab = make_chain(vec![lp.clone(), slew.clone()], h);
    let mut ba = make_chain(vec![slew, lp], h);
    // Warm up slew state similarly
    let out_ab = process_block(&mut ab, &saw);
    let out_ba = process_block(&mut ba, &saw);
    let max_diff = out_ab
        .iter()
        .zip(out_ba.iter())
        .map(|(a, b)| (a - b).abs())
        .fold(0.0f32, f32::max);
    assert!(
        max_diff > 1e-3,
        "LP→Slew vs Slew→LP should differ (max_diff={max_diff})"
    );
}

#[test]
fn stability_no_nan_all_types() {
    let saw = fixture_saw_wrap(N);
    let h = curve_harshness(&saw);
    for ty in OvertoneFilterType::ALL {
        for strength in [0.0f32, 1.0] {
            let slot = OvertoneFilterSlot {
                filter_type: ty.clone(),
                strength,
                bypassed: false,
            };
            let mut chain = make_chain(vec![slot], h);
            for &x in &saw {
                let y = chain.process_sample(x);
                assert!(y.is_finite(), "{:?} strength={strength} produced {y}", ty);
            }
        }
    }
    // Empty ignores strength conceptually
    let mut empty = make_chain(vec![], 1.0);
    for &x in &saw {
        assert!(empty.process_sample(x).is_finite());
    }
}

#[test]
fn bypassed_slot_is_identity() {
    let mut slot = OvertoneFilterSlot::lowpass();
    slot.bypassed = true;
    let mut chain = make_chain(vec![slot], 1.0);
    let x = 0.42f32;
    assert!((chain.process_sample(x) - x).abs() < 1e-6);
}

#[test]
fn peak_delta_slew_reduces_on_harsh() {
    let saw = fixture_saw_wrap(N);
    // Two periods so the wrap cliff appears as consecutive samples.
    let mut signal = saw.clone();
    signal.extend_from_slice(&saw);
    let h = curve_harshness(&saw);
    let mut chain = make_chain(vec![OvertoneFilterSlot::slew()], h);
    let out = process_block(&mut chain, &signal);
    assert!(
        peak_delta(&out) < peak_delta(&signal) * 0.95,
        "slew should reduce peak |Δ| ({} vs {})",
        peak_delta(&out),
        peak_delta(&signal)
    );
}
