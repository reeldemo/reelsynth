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
}

impl StackMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "avg" | "average" => Self::Avg,
            _ => Self::Add,
        }
    }
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
        bank.sample_warped(effective_pos, layer_phase, warp, warp_amount)
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
        sum += sample * layer.level;
        weight += layer.level;
    }

    if weight <= 0.0 {
        return 0.0;
    }
    match mode {
        StackMode::Add => sum,
        StackMode::Avg => sum / weight,
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
}
