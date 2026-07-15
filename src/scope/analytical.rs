//! Analytical + spectrum helpers for the four-tap scope strip.

use crate::engine::note_to_freq;
use crate::fx::FxChain;
use crate::patch::Patch;
use super::ring_buffer::SCOPE_DISPLAY_LEN;
use crate::voice::{process_sample_stages, VoiceSampleContext, VoiceState};
use crate::wavetable::WavetableBank;

const PREVIEW_SR: f32 = 48_000.0;
/// Default idle preview root (C3).
pub const PREVIEW_ROOT_NOTE: u8 = 48;
/// Perfect fifth above root.
pub const PREVIEW_FIFTH_NOTE: u8 = 55;

/// One scope tap buffer (normalized waveform samples).
#[derive(Clone, Debug, Default)]
pub struct ScopeTap {
    pub samples: Vec<f32>,
}

/// Four-tap signal chain preview (Osc → Filter → FX → Out).
#[derive(Clone, Debug, Default)]
pub struct ScopePreviews {
    pub osc: ScopeTap,
    pub filter: ScopeTap,
    pub fx: ScopeTap,
    pub out: ScopeTap,
}

/// Render analytical previews for the scope strip UI.
pub fn render_scope_previews(
    banks: &[WavetableBank],
    bank_for_osc: impl Fn(usize) -> usize + Copy,
    patch: &Patch,
    sample_count: usize,
) -> ScopePreviews {
    let count = sample_count.max(SCOPE_DISPLAY_LEN);
    let gate_samples = (count as f32 * 0.75) as usize;
    let preview_notes = [note_to_freq(PREVIEW_ROOT_NOTE), note_to_freq(PREVIEW_FIFTH_NOTE)];

    let mut osc_buf = vec![0.0f32; count];
    let mut filt_buf = vec![0.0f32; count];
    let mut fx_buf = vec![0.0f32; count];
    let mut out_buf = vec![0.0f32; count];

    let mut voices: Vec<VoiceState> = preview_notes
        .iter()
        .map(|_| VoiceState::new(patch))
        .collect();
    let mut fx = FxChain::new(PREVIEW_SR as u32);
    fx.set_effects(patch.effects.clone());
    let wt_ids = patch.wavetable_ids();

    for i in 0..count {
        let t = i as f32 / PREVIEW_SR;
        let gate = i < gate_samples;

        let mut osc_sum = 0.0f32;
        let mut filt_l = 0.0f32;
        let mut filt_r = 0.0f32;
        for (vi, &freq) in preview_notes.iter().enumerate() {
            let ctx = VoiceSampleContext {
                banks,
                bank_for_osc: &bank_for_osc,
                wt_ids: &wt_ids,
                patch,
                freq,
                gate,
                velocity: 0.85,
                time: t,
                sample_index: i as u32,
                dt: 1.0 / PREVIEW_SR,
                sr: PREVIEW_SR,
                modwheel: 0.0,
                mpe: crate::engine::VoiceMpe::default(),
                bend_range_semitones: 48.0,
            };
            let stages = process_sample_stages(&mut voices[vi], &ctx);
            osc_sum += stages.osc_mono;
            filt_l += stages.filtered[0];
            filt_r += stages.filtered[1];
        }

        osc_buf[i] = osc_sum.clamp(-1.0, 1.0);
        let filt_mono = ((filt_l + filt_r) * 0.5).clamp(-1.0, 1.0);
        filt_buf[i] = filt_mono;
        let [fx_l, fx_r] = fx.process_stereo(filt_l, filt_r);
        let fx_mono = ((fx_l + fx_r) * 0.5).clamp(-1.0, 1.0);
        fx_buf[i] = fx_mono;
        out_buf[i] = ((fx_l + fx_r) * 0.5 * 0.98 + filt_mono * 0.02).clamp(-1.0, 1.0);
    }

    // Osc tap: single-cycle snapshot at the root pitch (power chord composite).
    let osc_cycle = render_combined_osc_cycle(banks, bank_for_osc, patch, count);

    ScopePreviews {
        osc: ScopeTap { samples: osc_cycle },
        filter: ScopeTap { samples: filt_buf },
        fx: ScopeTap { samples: fx_buf },
        out: ScopeTap { samples: out_buf },
    }
}

/// Hann-windowed magnitude spectrum for the Out scope (bar display).
pub fn spectrum_magnitudes(samples: &[f32], bar_count: usize) -> Vec<f32> {
    let bar_count = bar_count.clamp(4, SCOPE_DISPLAY_LEN);
    if samples.is_empty() {
        return vec![0.0; bar_count];
    }

    let n = samples.len();
    let mut bars = vec![0.0f32; bar_count];
    for k in 0..bar_count {
        let mut re = 0.0f32;
        let mut im = 0.0f32;
        for (i, &sample) in samples.iter().enumerate() {
            let w = if n > 1 {
                0.5 * (1.0 - (std::f32::consts::TAU * i as f32 / (n - 1) as f32).cos())
            } else {
                1.0
            };
            let angle = -std::f32::consts::TAU * k as f32 * i as f32 / n as f32;
            re += sample * w * angle.cos();
            im += sample * w * angle.sin();
        }
        bars[k] = (re * re + im * im).sqrt();
    }

    let peak = bars.iter().copied().fold(0.0f32, f32::max).max(1e-6);
    for bar in &mut bars {
        *bar = (*bar / peak).clamp(0.0, 1.0);
    }
    bars
}

/// Single-cycle preview of one oscillator (power-chord voicing for audibility).
pub fn render_osc_cycle_at_index(
    banks: &[WavetableBank],
    bank_for_osc: impl Fn(usize) -> usize + Copy,
    patch: &Patch,
    osc_index: usize,
    sample_count: usize,
) -> Vec<f32> {
    let count = sample_count.max(16);
    let root = note_to_freq(PREVIEW_ROOT_NOTE);
    let fifth = note_to_freq(PREVIEW_FIFTH_NOTE);
    let mut out = vec![0.0f32; count];

    let Some(osc) = patch.oscillators.get(osc_index) else {
        return out;
    };

    for i in 0..count {
        let phase = i as f32 / count as f32;
        out[i] = preview_osc_sample_at_phase(
            banks,
            bank_for_osc,
            patch,
            osc_index,
            osc,
            phase,
            root,
            fifth,
        );
    }
    out
}

/// Sum of all active oscillators (matches scope strip Osc tap).
pub fn render_combined_osc_cycle(
    banks: &[WavetableBank],
    bank_for_osc: impl Fn(usize) -> usize + Copy,
    patch: &Patch,
    sample_count: usize,
) -> Vec<f32> {
    let count = sample_count.max(16);
    let root = note_to_freq(PREVIEW_ROOT_NOTE);
    let fifth = note_to_freq(PREVIEW_FIFTH_NOTE);
    let mut out = vec![0.0f32; count];

    for i in 0..count {
        let phase = i as f32 / count as f32;
        let mut sum = 0.0f32;
        for (oi, osc) in patch.oscillators.iter().enumerate() {
            if osc.level <= 0.0 {
                continue;
            }
            sum += preview_osc_sample_at_phase(
                banks,
                bank_for_osc,
                patch,
                oi,
                osc,
                phase,
                root,
                fifth,
            );
        }
        out[i] = sum.clamp(-1.0, 1.0);
    }

    out
}

fn preview_osc_sample_at_phase(
    banks: &[WavetableBank],
    bank_for_osc: impl Fn(usize) -> usize + Copy,
    patch: &Patch,
    oi: usize,
    osc: &crate::patch::Oscillator,
    phase: f32,
    root: f32,
    fifth: f32,
) -> f32 {
    use crate::osc::{sample_stack, sample_va, uses_wave_stack, VaWaveform, WtWarpMode};

    if osc.level <= 0.0 {
        return 0.0;
    }

    let unison = osc.unison.max(1) as usize;
    let spread_cents = 15.0f32;
    let stereo_spread = patch.unison_stereo_spread.clamp(0.0, 1.0);
    let wt_ids = patch.wavetable_ids();
    let mut sum = 0.0f32;

    for u in 0..unison {
        let det_spread = if unison > 1 {
            spread_cents * (u as f32 / (unison - 1) as f32 - 0.5) * 2.0
        } else {
            0.0
        };
        let det = osc.detune + det_spread;
        let ratio = 2.0f32.powf(det / 1200.0);

        let sample_at = |base_freq: f32| -> f32 {
            let voice_phase = (phase * base_freq * ratio / root).fract();
            let phase_inc = base_freq * ratio / root / 2048.0;

            if uses_wave_stack(osc) {
                let bank_idx = bank_for_osc(oi);
                let default_bank = banks.get(bank_idx).or_else(|| banks.first());
                let Some(default_bank) = default_bank else {
                    return 0.0;
                };
                let warp = WtWarpMode::from_str(&osc.warp_mode);
                let wt_pos = preview_wt_position(osc, default_bank.num_frames);
                return sample_stack(
                    osc,
                    default_bank,
                    banks,
                    &wt_ids,
                    voice_phase,
                    phase_inc,
                    wt_pos,
                    warp,
                    osc.warp_amount,
                    0.0,
                    0.0,
                    1.0,
                );
            }

            if let Some(wave) = VaWaveform::from_osc_type(&osc.osc_type) {
                return sample_va(wave, voice_phase, phase_inc, osc.pulse_width);
            }
            let bank_idx = bank_for_osc(oi);
            let bank = banks.get(bank_idx).or_else(|| banks.first());
            let Some(bank) = bank else {
                return 0.0;
            };
            let wt_pos = preview_wt_position(osc, bank.num_frames);
            let warp = WtWarpMode::from_str(&osc.warp_mode);
            bank.sample_warped(wt_pos, voice_phase, warp, osc.warp_amount)
        };

        sum += sample_at(root) * osc.level / unison as f32;
        sum += sample_at(fifth) * osc.level * 0.85 / unison as f32;
        let _ = stereo_spread;
    }

    sum.clamp(-1.0, 1.0)
}

fn preview_wt_position(osc: &crate::patch::Oscillator, num_frames: usize) -> f32 {
    crate::wt_quant::resolve_wt_position(osc, 0.0, 0.0, num_frames)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::Patch;

    #[test]
    fn analytical_previews_are_non_silent() {
        let bank = WavetableBank::factory_saw_morph();
        let patch = Patch::default_mono();
        let previews = render_scope_previews(std::slice::from_ref(&bank), |_| 0, &patch, 64);
        let peak = previews
            .out
            .samples
            .iter()
            .map(|s| s.abs())
            .fold(0.0f32, f32::max);
        assert!(peak > 0.009, "out peak was {peak}");
    }

    #[test]
    fn filter_cutoff_changes_filter_tap() {
        let bank = WavetableBank::factory_saw_morph();
        let mut bright = Patch::default_mono();
        bright.filter.cutoff = 9000.0;
        bright.filter.key_tracking = 0.0;
        let mut dark = Patch::default_mono();
        dark.filter.cutoff = 180.0;
        dark.filter.key_tracking = 0.0;

        let bright_prev = render_scope_previews(std::slice::from_ref(&bank), |_| 0, &bright, 64);
        let dark_prev = render_scope_previews(std::slice::from_ref(&bank), |_| 0, &dark, 64);

        let zc = |buf: &[f32]| {
            buf.windows(2)
                .filter(|w| w[0].signum() != w[1].signum())
                .count()
        };
        let zc_bright = zc(&bright_prev.filter.samples);
        let zc_dark = zc(&dark_prev.filter.samples);
        assert!(
            zc_bright > zc_dark,
            "bright crossings {zc_bright} should exceed dark {zc_dark}"
        );
    }

    #[test]
    fn spectrum_bars_normalized() {
        let samples: Vec<f32> = (0..64)
            .map(|i| (i as f32 * 0.2).sin())
            .collect();
        let bars = spectrum_magnitudes(&samples, 24);
        assert_eq!(bars.len(), 24);
        assert!(bars.iter().copied().fold(0.0f32, f32::max) <= 1.0);
        assert!(bars.iter().copied().fold(0.0f32, f32::max) > 0.1);
    }
}
