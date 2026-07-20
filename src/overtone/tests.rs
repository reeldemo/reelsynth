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

/// Runtime evidence: overlay Add + long sustain crackle from wrap / overtones.
#[test]
fn diagnose_overlay_and_long_tone_crackle() {
    let sine = fixture_sine(N);
    let saw = fixture_saw_wrap(N);
    // StackMode::Add with two in-phase unit saws → peaks ±2 (clip → odd harmonics).
    let overlay: Vec<f32> = saw.iter().map(|&b| b + b).collect();
    let h_sine = curve_harshness(&sine);
    let h_saw = curve_harshness(&saw);
    let h_overlay = curve_harshness(&overlay);
    let peak_sine = sine.iter().copied().map(f32::abs).fold(0.0f32, f32::max);
    let peak_overlay = overlay.iter().copied().map(f32::abs).fold(0.0f32, f32::max);
    let hf_sine = hf_harshness(&sine);
    let hf_overlay = hf_harshness(&overlay);
    let wrap_saw = wrap_harshness(&saw);
    let wrap_overlay = wrap_harshness(&overlay);

    let periods = 32usize;
    let mut long_saw = Vec::with_capacity(N * periods);
    for _ in 0..periods {
        long_saw.extend_from_slice(&saw);
    }
    let mut wrap_events = 0u32;
    let mut max_wrap_step = 0.0f32;
    for p in 0..periods.saturating_sub(1) {
        let i1 = p * N + N - 1;
        let step = (long_saw[i1 + 1] - long_saw[i1]).abs();
        if step > 0.5 {
            wrap_events += 1;
            max_wrap_step = max_wrap_step.max(step);
        }
    }
    let samples_above_unity = overlay.iter().filter(|&&x| x.abs() > 1.0).count();

    // #region agent log
    let payload = serde_json::json!({
        "sessionId": "0ab8f9",
        "runId": "crackle-diagnose",
        "hypothesisId": "H-overtones-overlay-long",
        "location": "overtone/tests.rs:diagnose_overlay_and_long_tone_crackle",
        "message": "overlay vs sine harshness; long-tone wrap events",
        "data": {
            "h_sine": h_sine,
            "h_saw": h_saw,
            "h_overlay": h_overlay,
            "hf_sine": hf_sine,
            "hf_overlay": hf_overlay,
            "wrap_saw": wrap_saw,
            "wrap_overlay": wrap_overlay,
            "peak_sine": peak_sine,
            "peak_overlay": peak_overlay,
            "samples_above_unity": samples_above_unity,
            "long_periods": periods,
            "wrap_events_over_05": wrap_events,
            "max_wrap_step": max_wrap_step,
            "user_hyp_overtones": true
        },
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    });
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("debug-0ab8f9.log")
    {
        use std::io::Write;
        let _ = writeln!(f, "{payload}");
    }
    // #endregion

    assert!(hf_overlay > hf_sine, "overlay HF {hf_overlay} vs sine {hf_sine}");
    assert!(peak_overlay > peak_sine + 0.5, "peak overlay {peak_overlay}");
    assert!(samples_above_unity > 0, "must exceed ±1 for clip overtones");
    assert!(wrap_overlay >= wrap_saw, "wrap {wrap_overlay} vs {wrap_saw}");
    assert_eq!(wrap_events, periods as u32 - 1);
    assert!(max_wrap_step > 1.0, "wrap step {max_wrap_step}");
    let _ = h_overlay;
    let _ = h_sine;
    let _ = h_saw;
}

/// Why Soft/Adaptive still crackle: residual end-slope + Seam doesn't touch VA layers.
#[test]
fn diagnose_seam_modes_residual_and_va_unaffected() {
    use crate::osc::{sample_va, VaWaveform};

    let n = 2048usize;
    // Harsh Quant-like open cycle (ends opposite start).
    let open: Vec<f32> = (0..n)
        .map(|i| {
            let t = i as f32 / (n - 1) as f32;
            -0.9 + 1.8 * t // -0.9 → +0.9
        })
        .collect();

    let measure = |frame: &[f32]| -> (f32, f32, f32) {
        let wrap = (frame[frame.len() - 1] - frame[0]).abs();
        let mut max_step = 0.0f32;
        for w in frame.windows(2) {
            max_step = max_step.max((w[1] - w[0]).abs());
        }
        max_step = max_step.max((frame[0] - frame[frame.len() - 1]).abs());
        // Steepness in last 1% of cycle (where Soft fades).
        let tail0 = frame.len() * 99 / 100;
        let mut tail_step = 0.0f32;
        for w in frame[tail0..].windows(2) {
            tail_step = tail_step.max((w[1] - w[0]).abs());
        }
        (wrap, max_step, tail_step)
    };

    let modes = [
        ("off", {
            let f = open.clone();
            // Simulate Off: no fade (ui Off returns early).
            f
        }),
        ("soft", {
            let mut f = open.clone();
            // Mirror Soft fade from quant_handles (fade = n/16 capped).
            let fade = (n / 16).max(16).min(128);
            let start = f[0];
            for i in 0..fade {
                let w = (i as f32 + 1.0) / (fade as f32 + 1.0);
                let w = w * w;
                let idx = n - fade + i;
                f[idx] = f[idx] * (1.0 - w) + start * w;
            }
            f[n - 1] = start;
            f
        }),
        ("adaptive", {
            let mut f = open.clone();
            let seam = (f[n - 1] - f[0]).abs();
            let t = (seam / 2.0).clamp(0.0, 1.0);
            let min_f = 4usize;
            let max_f = (n / 12).max(24).min(96);
            let fade = (min_f as f32 + t * (max_f - min_f) as f32).round() as usize;
            let fade = fade.min(n / 2).max(1);
            let start = f[0];
            for i in 0..fade {
                let w = (i as f32 + 1.0) / (fade as f32 + 1.0);
                let w = w * w;
                let idx = n - fade + i;
                f[idx] = f[idx] * (1.0 - w) + start * w;
            }
            f[n - 1] = start;
            f
        }),
    ];

    let mut rows = Vec::new();
    for (name, frame) in &modes {
        let (wrap, max_step, tail_step) = measure(frame);
        rows.push(serde_json::json!({
            "mode": name,
            "wrap": wrap,
            "max_step": max_step,
            "tail_step": tail_step,
        }));
    }

    // VA saw: Seam never runs — wrap cliff is inherent every period.
    let mut va_cycle = Vec::with_capacity(n);
    let dt = 1.0 / n as f32;
    for i in 0..n {
        let phase = i as f32 / n as f32;
        va_cycle.push(sample_va(VaWaveform::Saw, phase, dt, 0.5));
    }
    let (va_wrap, va_max, va_tail) = measure(&va_cycle);

    // Add two soft-periodized frames → still peaks > 1 (clip overtones).
    let soft = {
        let mut f = open.clone();
        let fade = (n / 16).max(16).min(128);
        let start = f[0];
        for i in 0..fade {
            let w = ((i as f32 + 1.0) / (fade as f32 + 1.0)).powi(2);
            let idx = n - fade + i;
            f[idx] = f[idx] * (1.0 - w) + start * w;
        }
        f[n - 1] = start;
        f
    };
    let stacked: Vec<f32> = soft.iter().map(|&x| x + x).collect();
    let peak_stacked = stacked.iter().copied().map(f32::abs).fold(0.0f32, f32::max);
    let above1 = stacked.iter().filter(|&&x| x.abs() > 1.0).count();

    // #region agent log
    let payload = serde_json::json!({
        "sessionId": "0ab8f9",
        "runId": "seam-all-modes-cracks",
        "hypothesisId": "H-seam-incomplete",
        "location": "overtone/tests.rs:diagnose_seam_modes_residual_and_va_unaffected",
        "message": "Soft/Adaptive close wrap but leave steep tail; VA saw ignores Seam; Add still clips",
        "data": {
            "quant_modes": rows,
            "va_saw": { "wrap": va_wrap, "max_step": va_max, "tail_step": va_tail, "seam_applies": false },
            "soft_then_add_two": { "peak": peak_stacked, "samples_above_1": above1 },
            "why_all_modes_crack": [
                "Seam only runs on Quant-resampled WT frames, not VA saw/sine/square",
                "Soft/Adaptive pin last=first but fade can still be steep (audible)",
                "Stack Add can still exceed ±1 after Seam → clip overtones"
            ]
        },
        "timestamp": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    });
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open("debug-0ab8f9.log")
    {
        use std::io::Write;
        let _ = writeln!(f, "{payload}");
    }
    // #endregion

    let soft_row = rows.iter().find(|r| r["mode"] == "soft").unwrap();
    let off_row = rows.iter().find(|r| r["mode"] == "off").unwrap();
    assert!(
        soft_row["wrap"].as_f64().unwrap() < 1e-5,
        "Soft should pin wrap closed"
    );
    assert!(
        off_row["wrap"].as_f64().unwrap() > 1.0,
        "Off leaves open wrap"
    );
    // BLEP softens VA saw but Seam UI never touches it — still a per-period step.
    assert!(
        va_max > 0.1,
        "VA saw still has audible per-period steps regardless of Seam UI ({va_max})"
    );
    assert!(peak_stacked > 1.0 && above1 > 0, "Add after Soft still clips");
}
