//! Mono voice: wavetable osc(s), ADSR, SVF filter, mod matrix.

mod kernel;

pub use kernel::{process_sample, process_sample_stages, VoiceSampleContext, VoiceStageSample, VoiceState};

use crate::patch::Patch;
use crate::wavetable::WavetableBank;

pub fn render_note(
    banks: &[WavetableBank],
    bank_for_osc: impl Fn(usize) -> usize + Copy,
    freq: f32,
    duration: f32,
    sample_rate: u32,
    patch: &Patch,
) -> Vec<f32> {
    let sr = sample_rate as f32;
    let num_samples = (duration * sr).ceil() as usize;
    let mut out = vec![0.0f32; num_samples];
    let mut voice = VoiceState::new(patch);
    let release_samples = (patch.envelope.release * sr) as usize;
    let tail_release = release_samples
        .min(num_samples.saturating_sub(1))
        .min(num_samples / 2)
        .max(1);

    for i in 0..num_samples {
        let t = i as f32 / sr;
        let gate = i < num_samples.saturating_sub(tail_release);
        let ctx = VoiceSampleContext {
            banks,
            bank_for_osc: &bank_for_osc,
            patch,
            freq,
            gate,
            velocity: 1.0,
            time: t,
            sample_index: i as u32,
            dt: 1.0 / sr,
            sr,
            modwheel: 0.0,
            mpe: crate::engine::VoiceMpe::default(),
            bend_range_semitones: 48.0,
        };
        let [l, r] = process_sample(&mut voice, &ctx);
        out[i] = (l + r) * 0.5;
    }
    out
}

/// Convenience wrapper when only a single bank is available.
pub fn render_note_single_bank(
    bank: &WavetableBank,
    freq: f32,
    duration: f32,
    sample_rate: u32,
    patch: &Patch,
) -> Vec<f32> {
    render_note(
        std::slice::from_ref(bank),
        |_| 0,
        freq,
        duration,
        sample_rate,
        patch,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{Envelope, Patch};

    fn closed_filter_env() -> Envelope {
        Envelope {
            attack: 0.001,
            decay: 0.001,
            sustain: 0.0,
            release: 0.001,
        }
    }

    #[test]
    fn adsr_attack_rises() {
        let bank = WavetableBank::factory_sine();
        let mut patch = Patch::default_mono();
        patch.envelope = Envelope {
            attack: 0.05,
            decay: 0.1,
            sustain: 0.5,
            release: 0.1,
        };
        let audio = render_note_single_bank(&bank, 220.0, 0.2, 44100, &patch);
        assert!(audio[100].abs() > audio[10].abs());
    }

    #[test]
    fn filter_darkens() {
        let bank = WavetableBank::factory_saw_morph();
        let mut bright = Patch::default_mono();
        bright.filter.cutoff = 8000.0;
        bright.filter.key_tracking = 0.0;
        let mut dark = Patch::default_mono();
        dark.filter.cutoff = 200.0;
        dark.filter.key_tracking = 0.0;
        let a_bright = render_note_single_bank(&bank, 440.0, 0.5, 44100, &bright);
        let a_dark = render_note_single_bank(&bank, 440.0, 0.5, 44100, &dark);
        let zc_bright = zero_crossings(&a_bright[4410..]);
        let zc_dark = zero_crossings(&a_dark[4410..]);
        assert!(zc_bright > zc_dark, "bright={zc_bright} dark={zc_dark}");
    }

    #[test]
    fn filter_highpass_passes_highs() {
        let bank = WavetableBank::factory_saw_morph();
        let mut lp = Patch::default_mono();
        lp.filter.cutoff = 200.0;
        lp.filter.key_tracking = 0.0;
        lp.filter.filter_type = "lowpass".into();
        lp.filter_envelope = closed_filter_env();
        let mut hp = Patch::default_mono();
        hp.filter.cutoff = 200.0;
        hp.filter.key_tracking = 0.0;
        hp.filter.filter_type = "highpass".into();
        hp.filter_envelope = closed_filter_env();
        let a_lp = render_note_single_bank(&bank, 440.0, 0.5, 44100, &lp);
        let a_hp = render_note_single_bank(&bank, 440.0, 0.5, 44100, &hp);
        let rms_lp = rms(&a_lp[4410..]);
        let rms_hp = rms(&a_hp[4410..]);
        assert!(rms_hp > rms_lp * 1.2, "hp={rms_hp} lp={rms_lp}");
    }

    fn rms(samples: &[f32]) -> f32 {
        let mean = samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32;
        mean.sqrt()
    }

    fn zero_crossings(samples: &[f32]) -> usize {
        samples
            .windows(2)
            .filter(|w| w[0].signum() != w[1].signum())
            .count()
    }

    #[test]
    fn default_mono_has_signal() {
        let bank = WavetableBank::factory_saw_morph();
        let patch = Patch::default_mono();
        let audio = render_note_single_bank(&bank, 220.0, 0.2, 44100, &patch);
        let peak: f32 = audio.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.01, "default_mono peak was {peak}");
    }

    #[test]
    fn render_from_json_patch() {
        let bank = WavetableBank::factory_saw_morph();
        let json = r#"{"oscillators":[{"type":"wavetable","level":1.0,"position":0.0}],"filter":{"type":"lowpass","cutoff":1200,"resonance":0.3},"envelope":{"attack":0.01,"decay":0.2,"sustain":0.6,"release":0.4}}"#;
        let patch = Patch::from_json(json).unwrap();
        assert!(!patch.oscillators.is_empty(), "oscillators empty");
        let audio = render_note_single_bank(&bank, 220.0, 0.2, 44100, &patch);
        let peak: f32 = audio.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.01, "peak was {peak}");
    }

    #[test]
    fn filter_envelope_opens_cutoff() {
        let bank = WavetableBank::factory_saw_morph();
        let mut patch = Patch::default_mono();
        patch.filter.cutoff = 200.0;
        patch.filter.key_tracking = 0.0;
        patch.envelope = Envelope {
            attack: 0.001,
            decay: 0.001,
            sustain: 1.0,
            release: 0.001,
        };
        patch.filter_envelope = Envelope {
            attack: 0.001,
            decay: 0.001,
            sustain: 1.0,
            release: 0.001,
        };
        let audio = render_note_single_bank(&bank, 440.0, 0.15, 44100, &patch);
        let early = zero_crossings(&audio[50..200]);
        let late = zero_crossings(&audio[4000..6500]);
        assert!(late > early, "early zc={early} late zc={late}");
    }

    #[test]
    fn factory_va_bass_renders() {
        let bank = WavetableBank::factory_saw_morph();
        let patch = Patch::factory_va_bass();
        let audio = render_note_single_bank(&bank, 55.0, 0.3, 44100, &patch);
        let peak: f32 = audio.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.05, "va bass peak={peak}");
    }

    #[test]
    fn factory_wt_lead_renders() {
        let bank = WavetableBank::factory_saw_morph();
        let patch = Patch::factory_wt_lead();
        let audio = render_note_single_bank(&bank, 440.0, 0.3, 44100, &patch);
        let peak: f32 = audio.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.05, "wt lead peak={peak}");
    }

    #[test]
    fn factory_fm_bell_renders() {
        let bank = WavetableBank::factory_sine();
        let patch = Patch::factory_fm_bell();
        let audio = render_note_single_bank(&bank, 880.0, 0.5, 44100, &patch);
        let peak: f32 = audio.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.02, "fm bell peak={peak}");
    }

    #[test]
    fn factory_fm_pluck_renders() {
        let bank = WavetableBank::factory_metallic();
        let patch = Patch::factory_fm_pluck();
        let audio = render_note_single_bank(&bank, 440.0, 0.4, 44100, &patch);
        let peak: f32 = audio.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.01, "fm pluck peak={peak}");
    }

    #[test]
    fn fm_bell_differs_from_no_fm() {
        let bank = WavetableBank::factory_sine();
        let fm = Patch::factory_fm_bell();
        let mut no_fm = fm.clone();
        no_fm.oscillators[0].fm_index = 0.0;
        no_fm.oscillators[0].fm_source = "none".into();
        let a_fm = render_note_single_bank(&bank, 880.0, 0.3, 44100, &fm);
        let a_dry = render_note_single_bank(&bank, 880.0, 0.3, 44100, &no_fm);
        assert!(a_fm.iter().all(|s| s.is_finite()));
        let start = 2000.min(a_fm.len());
        let end = a_fm.len().min(12000);
        let diff: f32 = a_fm[start..end]
            .iter()
            .zip(&a_dry[start..end])
            .map(|(a, b)| (a - b).abs())
            .sum();
        assert!(diff > 0.5, "fm bell diff sum={diff}");
    }

    #[test]
    fn fm_bell_render_finite() {
        let bank = WavetableBank::factory_sine();
        let patch = Patch::factory_fm_bell();
        let audio = render_note_single_bank(&bank, 880.0, 0.5, 44100, &patch);
        assert!(
            audio.iter().all(|s| s.is_finite()),
            "non-finite sample in fm bell render"
        );
    }
}
