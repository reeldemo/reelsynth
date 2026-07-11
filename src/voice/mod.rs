//! Mono voice: wavetable osc(s), ADSR, SVF filter, mod matrix.

mod kernel;

pub use kernel::{process_sample, VoiceSampleContext, VoiceState};

use crate::patch::Patch;
use crate::wavetable::WavetableBank;

pub fn render_note(
    bank: &WavetableBank,
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
            bank,
            patch,
            freq,
            gate,
            velocity: 1.0,
            time: t,
            sample_index: i as u32,
            dt: 1.0 / sr,
            sr,
        };
        out[i] = process_sample(&mut voice, &ctx);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{Envelope, Patch};

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
        let audio = render_note(&bank, 220.0, 0.2, 44100, &patch);
        assert!(audio[100].abs() > audio[10].abs());
    }

    #[test]
    fn filter_darkens() {
        let bank = WavetableBank::factory_saw_morph();
        let mut bright = Patch::default_mono();
        bright.filter.cutoff = 8000.0;
        let mut dark = Patch::default_mono();
        dark.filter.cutoff = 200.0;
        let a_bright = render_note(&bank, 440.0, 0.5, 44100, &bright);
        let a_dark = render_note(&bank, 440.0, 0.5, 44100, &dark);
        let zc_bright = zero_crossings(&a_bright[4410..]);
        let zc_dark = zero_crossings(&a_dark[4410..]);
        assert!(zc_bright > zc_dark, "bright={zc_bright} dark={zc_dark}");
    }

    #[test]
    fn filter_highpass_passes_highs() {
        let bank = WavetableBank::factory_saw_morph();
        let mut lp = Patch::default_mono();
        lp.filter.cutoff = 200.0;
        lp.filter.filter_type = "lowpass".into();
        let mut hp = Patch::default_mono();
        hp.filter.cutoff = 200.0;
        hp.filter.filter_type = "highpass".into();
        let a_lp = render_note(&bank, 440.0, 0.5, 44100, &lp);
        let a_hp = render_note(&bank, 440.0, 0.5, 44100, &hp);
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
        let audio = render_note(&bank, 220.0, 0.2, 44100, &patch);
        let peak: f32 = audio.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.01, "default_mono peak was {peak}");
    }

    #[test]
    fn render_from_json_patch() {
        let bank = WavetableBank::factory_saw_morph();
        let json = r#"{"oscillators":[{"type":"wavetable","level":1.0,"position":0.0}],"filter":{"type":"lowpass","cutoff":1200,"resonance":0.3},"envelope":{"attack":0.01,"decay":0.2,"sustain":0.6,"release":0.4}}"#;
        let patch = Patch::from_json(json).unwrap();
        assert!(!patch.oscillators.is_empty(), "oscillators empty");
        let audio = render_note(&bank, 220.0, 0.2, 44100, &patch);
        let peak: f32 = audio.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
        assert!(peak > 0.01, "peak was {peak}");
    }
}
