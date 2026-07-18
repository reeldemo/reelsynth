//! Live wave stack: additive overlay of multiple wave sources per oscillator.

use crate::osc::{sample_va, VaWaveform, WtWarpMode};
use crate::patch::{Oscillator, WaveLayer};
use crate::wavetable::WavetableBank;

/// How stacked layer samples are combined.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StackMode {
    #[default]
    Add,
    Avg,
    AvgEqual,
}

impl StackMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "avg" | "average" => Self::Avg,
            "avg_equal" | "avgequal" | "avg equal" => Self::AvgEqual,
            _ => Self::Add,
        }
    }
}

/// Signed contribution multiplier for a stack layer.
pub fn layer_sign(layer: &WaveLayer) -> f32 {
    if layer.invert { -1.0 } else { 1.0 }
}

/// Resolve a wavetable bank for a stack layer (falls back to the osc default bank).
pub fn bank_for_layer<'a>(
    layer: &WaveLayer,
    default_bank: &'a WavetableBank,
    banks: &'a [WavetableBank],
    wt_ids: &[String],
) -> &'a WavetableBank {
    if let Some(id) = layer.wavetable_id.as_deref() {
        if let Some(idx) = wt_ids.iter().position(|s| s == id) {
            return banks.get(idx).unwrap_or(default_bank);
        }
    }
    default_bank
}

fn detune_ratio(cents: f32) -> f32 {
    2.0_f32.powf(cents / 1200.0)
}

/// Sample one stack layer at the given carrier phase.
pub fn sample_layer(
    layer: &WaveLayer,
    bank: &WavetableBank,
    phase: f32,
    phase_inc: f32,
    wt_pos_offset: f32,
    warp: WtWarpMode,
    warp_amount: f32,
    phase_off: f32,
    wt_pos_off: f32,
    freq_mult: f32,
) -> f32 {
    let ratio = detune_ratio(layer.detune);
    let layer_inc = phase_inc * ratio * freq_mult;
    let layer_phase = (phase * ratio + phase_off).fract();

    if let Some(wave) = VaWaveform::from_osc_type(&layer.source_type) {
        if wave == VaWaveform::Sine {
            return (layer_phase.fract() * std::f32::consts::TAU + layer.phase).sin();
        }
        sample_va(wave, layer_phase, layer_inc, layer.pulse_width)
    } else if layer.source_type.eq_ignore_ascii_case("wavetable") {
        let max_pos = (bank.num_frames.saturating_sub(1)).max(1) as f32;
        let effective_pos = (layer.wt_position + wt_pos_offset + wt_pos_off).clamp(0.0, max_pos);
        bank.sample_warped_inc(effective_pos, layer_phase, warp, warp_amount, layer_inc)
    } else {
        0.0
    }
}

/// Sum (or average) all active wave layers for an oscillator.
pub fn sample_stack(
    osc: &Oscillator,
    default_bank: &WavetableBank,
    banks: &[WavetableBank],
    wt_ids: &[String],
    phase: f32,
    phase_inc: f32,
    wt_pos_offset: f32,
    warp: WtWarpMode,
    warp_amount: f32,
    phase_off: f32,
    wt_pos_off: f32,
    freq_mult: f32,
) -> f32 {
    let mode = StackMode::from_str(&osc.stack_mode);
    let mut sum = 0.0f32;
    let mut weight = 0.0f32;
    let mut count = 0u32;

    for layer in &osc.wave_layers {
        if layer.level <= 0.0 {
            continue;
        }
        let bank = bank_for_layer(layer, default_bank, banks, wt_ids);
        let sample = sample_layer(
            layer,
            bank,
            phase,
            phase_inc,
            wt_pos_offset,
            warp,
            warp_amount,
            phase_off,
            wt_pos_off,
            freq_mult,
        );
        let sign = layer_sign(layer);
        let signed = sign * sample * layer.level;
        match mode {
            StackMode::Add => sum += signed,
            StackMode::Avg => {
                sum += signed;
                weight += layer.level.abs();
            }
            StackMode::AvgEqual => {
                sum += sign * sample;
                count += 1;
            }
        }
    }

    match mode {
        StackMode::Add => sum,
        StackMode::Avg => {
            if weight <= 0.0 {
                0.0
            } else {
                sum / weight
            }
        }
        StackMode::AvgEqual => {
            if count == 0 {
                0.0
            } else {
                sum / count as f32
            }
        }
    }
}

/// True when the oscillator uses live wave stacking instead of legacy single-source playback.
pub fn uses_wave_stack(osc: &Oscillator) -> bool {
    !osc.wave_layers.is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::Oscillator;

    #[test]
    fn stack_saw_plus_sine_nonzero() {
        let bank = WavetableBank::factory_saw_morph();
        let osc = Oscillator {
            wave_layers: vec![
                WaveLayer {
                    source_type: "saw".into(),
                    level: 0.65,
                    ..WaveLayer::default()
                },
                WaveLayer {
                    source_type: "sine".into(),
                    level: 0.35,
                    ..WaveLayer::default()
                },
            ],
            stack_mode: "add".into(),
            ..Oscillator::default_va()
        };
        let s = sample_stack(
            &osc,
            &bank,
            std::slice::from_ref(&bank),
            &["saw_morph".into()],
            0.25,
            0.01,
            0.0,
            WtWarpMode::None,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        assert!(s.abs() > 0.01, "stack sample was {s}");
    }

    #[test]
    fn stack_wt_layer_responds_to_position_offset() {
        let bank = WavetableBank::factory_saw_morph();
        let osc = Oscillator {
            wave_layers: vec![WaveLayer {
                source_type: "wavetable".into(),
                level: 1.0,
                wt_position: 0.0,
                ..WaveLayer::default()
            }],
            ..Oscillator::default_va()
        };
        let low = sample_stack(
            &osc,
            &bank,
            std::slice::from_ref(&bank),
            &[],
            0.1,
            0.01,
            0.0,
            WtWarpMode::None,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        let high = sample_stack(
            &osc,
            &bank,
            std::slice::from_ref(&bank),
            &[],
            0.1,
            0.01,
            200.0,
            WtWarpMode::None,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        assert!(
            (low - high).abs() > 1e-4,
            "wt stack should morph: low={low} high={high}"
        );
    }

    #[test]
    fn stack_invert_cancels_in_add_mode() {
        let bank = WavetableBank::factory_saw_morph();
        let osc = Oscillator {
            wave_layers: vec![
                WaveLayer {
                    source_type: "sine".into(),
                    level: 1.0,
                    ..WaveLayer::default()
                },
                WaveLayer {
                    source_type: "sine".into(),
                    level: 1.0,
                    invert: true,
                    ..WaveLayer::default()
                },
            ],
            stack_mode: "add".into(),
            ..Oscillator::default_va()
        };
        let s = sample_stack(
            &osc,
            &bank,
            std::slice::from_ref(&bank),
            &[],
            0.25,
            0.01,
            0.0,
            WtWarpMode::None,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        assert!(s.abs() < 1e-4, "inverted sine should cancel: {s}");
    }

    #[test]
    fn avg_equal_differs_from_weighted_avg() {
        let bank = WavetableBank::factory_saw_morph();
        let layers = Oscillator {
            wave_layers: vec![
                WaveLayer {
                    source_type: "saw".into(),
                    level: 1.0,
                    ..WaveLayer::default()
                },
                WaveLayer {
                    source_type: "sine".into(),
                    level: 0.25,
                    ..WaveLayer::default()
                },
            ],
            ..Oscillator::default_va()
        };
        let mut avg_osc = layers.clone();
        avg_osc.stack_mode = "avg".into();
        let mut eq_osc = layers;
        eq_osc.stack_mode = "avg_equal".into();
        let avg = sample_stack(
            &avg_osc,
            &bank,
            std::slice::from_ref(&bank),
            &[],
            0.25,
            0.01,
            0.0,
            WtWarpMode::None,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        let eq = sample_stack(
            &eq_osc,
            &bank,
            std::slice::from_ref(&bank),
            &[],
            0.25,
            0.01,
            0.0,
            WtWarpMode::None,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        assert!(
            (avg - eq).abs() > 1e-4,
            "avg ({avg}) vs avg_equal ({eq}) should differ"
        );
    }

    /// Overlay is same-sample-time Add (not async mistiming). Crackle tracks waveform
    /// shape / wrap cliffs / HF — different signal types behave differently.
    #[test]
    fn diagnose_signal_types_and_simultaneous_overlay() {
        use crate::overtone::{hf_harshness, wrap_harshness};

        let bank = WavetableBank::factory_saw_morph();
        let n = 256usize;
        let phase_inc = 1.0 / n as f32;

        let render = |types: &[&str], detune_cents: f32| -> Vec<f32> {
            let osc = Oscillator {
                wave_layers: types
                    .iter()
                    .enumerate()
                    .map(|(i, ty)| WaveLayer {
                        source_type: (*ty).into(),
                        level: 1.0,
                        detune: if i == 1 { detune_cents } else { 0.0 },
                        ..WaveLayer::default()
                    })
                    .collect(),
                stack_mode: "add".into(),
                ..Oscillator::default_va()
            };
            (0..n)
                .map(|i| {
                    let phase = i as f32 / n as f32;
                    sample_stack(
                        &osc,
                        &bank,
                        std::slice::from_ref(&bank),
                        &[],
                        phase,
                        phase_inc,
                        0.0,
                        WtWarpMode::None,
                        0.0,
                        0.0,
                        0.0,
                        1.0,
                    )
                })
                .collect()
        };

        let l0 = WaveLayer {
            source_type: "sine".into(),
            level: 1.0,
            ..WaveLayer::default()
        };
        let l1 = WaveLayer {
            source_type: "sine".into(),
            level: 1.0,
            ..WaveLayer::default()
        };
        let phase = 0.3f32;
        let a = sample_layer(
            &l0,
            &bank,
            phase,
            phase_inc,
            0.0,
            WtWarpMode::None,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        let b = sample_layer(
            &l1,
            &bank,
            phase,
            phase_inc,
            0.0,
            WtWarpMode::None,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        let sum = sample_stack(
            &Oscillator {
                wave_layers: vec![l0, l1],
                stack_mode: "add".into(),
                ..Oscillator::default_va()
            },
            &bank,
            std::slice::from_ref(&bank),
            &[],
            phase,
            phase_inc,
            0.0,
            WtWarpMode::None,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        let simultaneous_add_ok = (sum - (a + b)).abs() < 1e-5;

        let pairs: &[(&str, &[&str], f32)] = &[
            ("sine+sine", &["sine", "sine"], 0.0),
            ("sine+saw", &["sine", "saw"], 0.0),
            ("saw+saw", &["saw", "saw"], 0.0),
            ("square+square", &["square", "square"], 0.0),
            ("saw+saw_detune7c", &["saw", "saw"], 7.0),
        ];

        let mut rows = Vec::new();
        for (name, types, detune) in pairs {
            let sig = render(types, *detune);
            let peak = sig.iter().copied().map(f32::abs).fold(0.0f32, f32::max);
            let wrap = wrap_harshness(&sig);
            let hf = hf_harshness(&sig);
            let mut max_step = 0.0f32;
            for w in sig.windows(2) {
                max_step = max_step.max((w[1] - w[0]).abs());
            }
            max_step = max_step.max((sig[0] - sig[sig.len() - 1]).abs());
            let above1 = sig.iter().filter(|&&x| x.abs() > 1.0).count();
            rows.push(serde_json::json!({
                "pair": name,
                "peak": peak,
                "wrap": wrap,
                "hf": hf,
                "max_step": max_step,
                "samples_above_1": above1,
            }));
        }

        // #region agent log
        let payload = serde_json::json!({
            "sessionId": "0ab8f9",
            "runId": "signal-types",
            "hypothesisId": "H-timing-vs-shape",
            "location": "osc/stack.rs:diagnose_signal_types_and_simultaneous_overlay",
            "message": "overlay is same-time Add; crackle from shape/wrap not async delay",
            "data": {
                "simultaneous_add_equals_a_plus_b": simultaneous_add_ok,
                "carrier_phase_shared": true,
                "async_mistiming": false,
                "detune_shifts_layer_phase": true,
                "pairs": rows
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

        assert!(simultaneous_add_ok, "layers must be summed at the same sample instant");
        let sine_sine = render(&["sine", "sine"], 0.0);
        let saw_saw = render(&["saw", "saw"], 0.0);
        assert!(
            wrap_harshness(&saw_saw) > wrap_harshness(&sine_sine) * 5.0,
            "saw+saw wrap much worse than sine+sine"
        );
        assert!(hf_harshness(&saw_saw) > hf_harshness(&sine_sine));
    }

    /// Result / composite curve must not have a near-vertical wrap cliff at A4.
    #[test]
    fn factory_lead_stack_wrap_not_steep() {
        let bank = WavetableBank::factory_saw_morph();
        let patch = crate::patch::Patch::factory_lead();
        let osc = &patch.oscillators[0];
        let dt = 440.0 / 44_100.0;
        let mut phase = 1.0 - 8.0 * dt;
        let mut prev = sample_stack(
            osc,
            &bank,
            std::slice::from_ref(&bank),
            &[],
            phase,
            dt,
            0.0,
            WtWarpMode::None,
            0.0,
            0.0,
            0.0,
            1.0,
        );
        let mut max_jump = 0.0f32;
        for _ in 0..16 {
            phase = (phase + dt).fract();
            let cur = sample_stack(
                osc,
                &bank,
                std::slice::from_ref(&bank),
                &[],
                phase,
                dt,
                0.0,
                WtWarpMode::None,
                0.0,
                0.0,
                0.0,
                1.0,
            );
            max_jump = max_jump.max((cur - prev).abs());
            prev = cur;
        }
        assert!(
            max_jump < 0.22,
            "result curve wrap too steep: {max_jump}"
        );
    }
}
