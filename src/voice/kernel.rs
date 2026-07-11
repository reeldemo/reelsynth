//! Shared per-sample voice DSP kernel (offline + realtime).

use crate::patch::{Envelope, Lfo, ModSlot, Patch};
use crate::wavetable::WavetableBank;

/// Per-voice DSP state shared by offline `render_note` and realtime voices.
#[derive(Clone, Debug)]
pub struct VoiceState {
    pub phases: Vec<f32>,
    pub env_level: f32,
    pub env_stage: u8,
    pub env_time: f32,
    pub svf_low: f32,
    pub svf_band: f32,
    pub noise_seed: u32,
}

impl VoiceState {
    pub fn new(patch: &Patch) -> Self {
        let phase_count: usize = patch
            .oscillators
            .iter()
            .map(|o| o.unison.max(1) as usize)
            .sum();
        Self {
            phases: vec![0.0; phase_count.max(1)],
            env_level: 0.0,
            env_stage: 0,
            env_time: 0.0,
            svf_low: 0.0,
            svf_band: 0.0,
            noise_seed: 1,
        }
    }

    pub fn reset(&mut self, patch: &Patch) {
        let phase_count: usize = patch
            .oscillators
            .iter()
            .map(|o| o.unison.max(1) as usize)
            .sum();
        self.phases.resize(phase_count.max(1), 0.0);
        self.phases.fill(0.0);
        self.env_level = 0.0;
        self.env_stage = 0;
        self.env_time = 0.0;
        self.svf_low = 0.0;
        self.svf_band = 0.0;
        self.noise_seed = self.noise_seed.wrapping_add(1);
    }
}

pub struct VoiceSampleContext<'a> {
    pub bank: &'a WavetableBank,
    pub patch: &'a Patch,
    pub freq: f32,
    pub gate: bool,
    pub velocity: f32,
    pub time: f32,
    pub sample_index: u32,
    pub dt: f32,
    pub sr: f32,
}

/// Process one output sample for a single voice.
pub fn process_sample(state: &mut VoiceState, ctx: &VoiceSampleContext<'_>) -> f32 {
    let env = advance_envelope(
        state,
        &ctx.patch.envelope,
        ctx.gate,
        ctx.dt,
    );

    let lfo = lfo_value(&ctx.patch.lfo, ctx.time);
    let mods = compute_mods(&ctx.patch.mod_matrix, lfo, env, ctx.velocity);

    let mut sample = 0.0f32;
    let mut phase_idx = 0usize;

    for (oi, osc) in ctx.patch.oscillators.iter().enumerate() {
        let pos_mod = mods
            .get(&format!("osc{}_position", oi + 1))
            .copied()
            .unwrap_or(0.0);
        let wt_pos = (osc.position
            + pos_mod
            + lfo_for_target(&ctx.patch.lfo, lfo, "wt_position"))
            .clamp(0.0, (ctx.bank.num_frames - 1) as f32);
        let det_mod = mods
            .get(&format!("osc{}_detune", oi + 1))
            .copied()
            .unwrap_or(0.0);
        let unison = osc.unison.max(1) as usize;
        let spread_cents = 15.0f32;

        for u in 0..unison {
            let spread = if unison > 1 {
                spread_cents * (u as f32 / (unison - 1) as f32 - 0.5) * 2.0
            } else {
                0.0
            };
            let det = osc.detune + det_mod + spread;
            let osc_freq = ctx.freq * 2.0f32.powf(det / 1200.0);
            let phase = &mut state.phases[phase_idx];
            *phase += osc_freq / ctx.sr;
            if *phase >= 1.0 {
                *phase -= 1.0;
            }
            sample += ctx.bank.sample(wt_pos, *phase) * osc.level * env / unison as f32;
            phase_idx += 1;
        }
    }

    if ctx.patch.sub_level > 0.0 {
        let sub_phase = state.phases.first().copied().unwrap_or(0.0) * 0.5;
        sample += (sub_phase * std::f32::consts::TAU).sin() * ctx.patch.sub_level * env * 0.5;
    }
    if ctx.patch.noise_level > 0.0 {
        let noise = pseudo_noise(state.noise_seed) * ctx.patch.noise_level * env;
        state.noise_seed = state.noise_seed.wrapping_add(1);
        sample += noise;
    }

    let cutoff_mod = mods.get("filter_cutoff").copied().unwrap_or(0.0)
        + lfo_for_target(&ctx.patch.lfo, lfo, "cutoff") * ctx.patch.filter.cutoff;
    let cutoff = (ctx.patch.filter.cutoff + cutoff_mod)
        .max(25.0)
        .min(ctx.sr * 0.45);
    let res_mod = mods.get("filter_resonance").copied().unwrap_or(0.0);
    let resonance = (ctx.patch.filter.resonance + res_mod).clamp(0.0, 0.95);

    sample = svf_filter(
        state,
        sample,
        cutoff,
        resonance,
        &ctx.patch.filter.filter_type,
        ctx.sr,
    );
    sample.clamp(-1.0, 1.0)
}

fn advance_envelope(state: &mut VoiceState, env: &Envelope, gate: bool, dt: f32) -> f32 {
    if gate {
        match state.env_stage {
            0 => {
                state.env_time += dt;
                let a = env.attack.max(1e-4);
                state.env_level = (state.env_time / a).min(1.0);
                if state.env_level >= 1.0 {
                    state.env_stage = 1;
                    state.env_time = 0.0;
                }
            }
            1 => {
                state.env_time += dt;
                let d = env.decay.max(1e-4);
                let t = (state.env_time / d).min(1.0);
                state.env_level = 1.0 + t * (env.sustain - 1.0);
                if t >= 1.0 {
                    state.env_stage = 2;
                }
            }
            2 => state.env_level = env.sustain,
            3 => {
                state.env_stage = 0;
                state.env_time = 0.0;
            }
            _ => {}
        }
    } else if state.env_stage != 3 {
        state.env_stage = 3;
        state.env_time = 0.0;
    } else {
        state.env_time += dt;
        let r = env.release.max(1e-4);
        let t = (state.env_time / r).min(1.0);
        state.env_level *= 1.0 - t;
    }
    state.env_level
}

fn svf_filter(
    state: &mut VoiceState,
    input: f32,
    cutoff: f32,
    resonance: f32,
    mode: &str,
    sr: f32,
) -> f32 {
    let fc = cutoff.clamp(20.0, sr * 0.49);
    let f = 2.0 * (std::f32::consts::PI * fc / sr).sin();
    let q = 1.0 - resonance.clamp(0.0, 0.95);

    state.svf_low += f * state.svf_band;
    let high = input - state.svf_low - q * state.svf_band;
    state.svf_band += f * high;

    match mode.to_ascii_lowercase().as_str() {
        "highpass" | "hp" => high,
        "bandpass" | "bp" => state.svf_band,
        "notch" => state.svf_low + high,
        _ => state.svf_low,
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

fn compute_mods(
    slots: &[ModSlot],
    lfo: f32,
    env: f32,
    velocity: f32,
) -> std::collections::HashMap<String, f32> {
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
