//! Mono voice: wavetable osc(s), ADSR, SVF filter, mod matrix.

use crate::patch::{Envelope, Lfo, ModSlot, Patch};
use crate::wavetable::WavetableBank;

pub struct Voice {
    phase: f32,
    env_level: f32,
    env_stage: u8,
    env_time: f32,
    svf_low: f32,
    svf_band: f32,
}

impl Default for Voice {
    fn default() -> Self {
        Self {
            phase: 0.0,
            env_level: 0.0,
            env_stage: 0,
            env_time: 0.0,
            svf_low: 0.0,
            svf_band: 0.0,
        }
    }
}

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
    let mut voice = Voice::default();
    let release_samples = (patch.envelope.release * sr) as usize;
    let tail_release = release_samples
        .min(num_samples.saturating_sub(1))
        .min(num_samples / 2)
        .max(1);

    for i in 0..num_samples {
        let t = i as f32 / sr;
        let gate = i < num_samples.saturating_sub(tail_release);
        let env = voice.advance_envelope(&patch.envelope, gate, 1.0 / sr);

        let lfo = lfo_value(&patch.lfo, t);
        let mods = compute_mods(&patch.mod_matrix, lfo, env, 1.0);

        let mut sample = 0.0f32;
        for (oi, osc) in patch.oscillators.iter().enumerate() {
            let pos_mod = mods.get(&format!("osc{}_position", oi + 1)).copied().unwrap_or(0.0);
            let wt_pos = (osc.position + pos_mod + lfo_for_target(&patch.lfo, lfo, "wt_position"))
                .clamp(0.0, (bank.num_frames - 1) as f32);
            let det = osc.detune + mods.get(&format!("osc{}_detune", oi + 1)).copied().unwrap_or(0.0);
            let osc_freq = freq * 2.0f32.powf(det / 1200.0);
            voice.phase += osc_freq / sr;
            if voice.phase >= 1.0 {
                voice.phase -= 1.0;
            }
            sample += bank.sample(wt_pos, voice.phase) * osc.level * env;
        }

        if patch.sub_level > 0.0 {
            let sub_phase = voice.phase * 0.5;
            sample += (sub_phase * std::f32::consts::TAU).sin() * patch.sub_level * env * 0.5;
        }
        if patch.noise_level > 0.0 {
            let noise = pseudo_noise(i as u32) * patch.noise_level * env;
            sample += noise;
        }

        let cutoff_mod = mods.get("filter_cutoff").copied().unwrap_or(0.0)
            + lfo_for_target(&patch.lfo, lfo, "cutoff") * patch.filter.cutoff;
        let cutoff = (patch.filter.cutoff + cutoff_mod).max(25.0).min(sr * 0.45);
        let res_mod = mods.get("filter_resonance").copied().unwrap_or(0.0);
        let resonance = (patch.filter.resonance + res_mod).clamp(0.0, 0.95);

        sample = voice.svf_filter(sample, cutoff, resonance, &patch.filter.filter_type, sr);
        out[i] = sample.clamp(-1.0, 1.0);
    }
    out
}

impl Voice {
    fn advance_envelope(&mut self, env: &Envelope, gate: bool, dt: f32) -> f32 {
        // stages: 0=attack, 1=decay, 2=sustain, 3=release
        if gate {
            match self.env_stage {
                0 => {
                    self.env_time += dt;
                    let a = env.attack.max(1e-4);
                    self.env_level = (self.env_time / a).min(1.0);
                    if self.env_level >= 1.0 {
                        self.env_stage = 1;
                        self.env_time = 0.0;
                    }
                }
                1 => {
                    self.env_time += dt;
                    let d = env.decay.max(1e-4);
                    let t = (self.env_time / d).min(1.0);
                    self.env_level = 1.0 + t * (env.sustain - 1.0);
                    if t >= 1.0 {
                        self.env_stage = 2;
                    }
                }
                2 => self.env_level = env.sustain,
                3 => {
                    self.env_stage = 0;
                    self.env_time = 0.0;
                }
                _ => {}
            }
        } else if self.env_stage != 3 {
            self.env_stage = 3;
            self.env_time = 0.0;
        } else {
            self.env_time += dt;
            let r = env.release.max(1e-4);
            let t = (self.env_time / r).min(1.0);
            self.env_level *= 1.0 - t;
        }
        self.env_level
    }

    /// Chamberlin state-variable filter (LP/HP/BP/notch).
    fn svf_filter(&mut self, input: f32, cutoff: f32, resonance: f32, mode: &str, sr: f32) -> f32 {
        let fc = cutoff.clamp(20.0, sr * 0.49);
        let f = 2.0 * (std::f32::consts::PI * fc / sr).sin();
        let q = 1.0 - resonance.clamp(0.0, 0.95);

        self.svf_low += f * self.svf_band;
        let high = input - self.svf_low - q * self.svf_band;
        self.svf_band += f * high;

        match mode.to_ascii_lowercase().as_str() {
            "highpass" | "hp" => high,
            "bandpass" | "bp" => self.svf_band,
            "notch" => self.svf_low + high,
            _ => self.svf_low,
        }
    }
}

fn lfo_value(lfo: &Lfo, t: f32) -> f32 {
    (t * lfo.rate * std::f32::consts::TAU * 2.0).sin() * lfo.depth
}

fn lfo_for_target(lfo: &Lfo, value: f32, target: &str) -> f32 {
    if lfo.target == target {
        value
    } else {
        0.0
    }
}

fn compute_mods(slots: &[ModSlot], lfo: f32, env: f32, velocity: f32) -> std::collections::HashMap<String, f32> {
    let mut out = std::collections::HashMap::new();
    for slot in slots {
        let src = match slot.source.as_str() {
            "lfo1" | "lfo" => lfo,
            "env1" | "env" => env,
            "velocity" | "vel" => velocity,
            "modwheel" => 0.0,
            _ => 0.0,
        };
        *out.entry(slot.target.clone()).or_insert(0.0) += src * slot.amount;
    }
    out
}

fn pseudo_noise(seed: u32) -> f32 {
    let x = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    ((x >> 16) as f32 / 32768.0) - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::Patch;

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
        // High-cut filter reduces high-frequency variation (lower zero-crossing rate proxy)
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
