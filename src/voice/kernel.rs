//! Shared per-sample voice DSP kernel (offline + realtime).

use crate::fm::{fm_mod_signal, sample_carrier_with_fm, FmSource};
use crate::lfo::{lfo_for_target, lfo_value, LfoRuntime};
use crate::modulation::{compute_macro_mods, compute_mods, merge_mods, ModSources};
use crate::osc::WtWarpMode;
use crate::oversample::{process_os, OS_FACTOR};
use crate::patch::{Envelope, Filter, Lfo, Oscillator, Patch};
use crate::wavetable::WavetableBank;
use crate::engine::VoiceMpe;

/// Per-voice DSP state shared by offline `render_note` and realtime voices.
#[derive(Clone, Debug)]
pub struct VoiceState {
    pub phases: Vec<f32>,
    pub amp_env_level: f32,
    pub amp_env_stage: u8,
    pub amp_env_time: f32,
    pub filt_env_level: f32,
    pub filt_env_stage: u8,
    pub filt_env_time: f32,
    pub svf_low: f32,
    pub svf_band: f32,
    pub svf2_low: f32,
    pub svf2_band: f32,
    pub noise_seed: u32,
    /// Previous-sample feedback for self-FM per osc slot.
    pub fm_feedback: [f32; 3],
    pub lfo1_rt: LfoRuntime,
    pub lfo2_rt: LfoRuntime,
    /// Per-voice random mod source (latched on note on).
    pub rand_mod: f32,
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
            amp_env_level: 0.0,
            amp_env_stage: 0,
            amp_env_time: 0.0,
            filt_env_level: 0.0,
            filt_env_stage: 0,
            filt_env_time: 0.0,
            svf_low: 0.0,
            svf_band: 0.0,
            svf2_low: 0.0,
            svf2_band: 0.0,
            noise_seed: 1,
            fm_feedback: [0.0; 3],
            lfo1_rt: LfoRuntime::default(),
            lfo2_rt: LfoRuntime::default(),
            rand_mod: 0.0,
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
        self.amp_env_level = 0.0;
        self.amp_env_stage = 0;
        self.amp_env_time = 0.0;
        self.filt_env_level = 0.0;
        self.filt_env_stage = 0;
        self.filt_env_time = 0.0;
        self.svf_low = 0.0;
        self.svf_band = 0.0;
        self.svf2_low = 0.0;
        self.svf2_band = 0.0;
        self.noise_seed = self.noise_seed.wrapping_add(1);
        self.fm_feedback = [0.0; 3];
        self.lfo1_rt.reset();
        self.lfo2_rt.reset();
        self.rand_mod = pseudo_noise(self.noise_seed);
    }
}

pub struct VoiceSampleContext<'a> {
    pub banks: &'a [WavetableBank],
    pub bank_for_osc: &'a dyn Fn(usize) -> usize,
    pub patch: &'a Patch,
    pub freq: f32,
    pub gate: bool,
    pub velocity: f32,
    pub time: f32,
    pub sample_index: u32,
    pub dt: f32,
    pub sr: f32,
    pub modwheel: f32,
    pub mpe: VoiceMpe,
    pub bend_range_semitones: f32,
}

/// Per-voice signal-chain taps before the FX bus.
#[derive(Clone, Copy, Debug, Default)]
pub struct VoiceStageSample {
    /// Mono sum of oscillator/sub/noise output before filtering.
    pub osc_mono: f32,
    /// Stereo output after SVF filtering.
    pub filtered: [f32; 2],
}

/// Process one output frame for a single voice (stereo).
pub fn process_sample(state: &mut VoiceState, ctx: &VoiceSampleContext<'_>) -> [f32; 2] {
    process_sample_stages(state, ctx).filtered
}

/// Like [`process_sample`] but also exposes pre-filter osc and post-filter stereo taps.
pub fn process_sample_stages(state: &mut VoiceState, ctx: &VoiceSampleContext<'_>) -> VoiceStageSample {
    let amp_env = advance_envelope(
        &mut state.amp_env_level,
        &mut state.amp_env_stage,
        &mut state.amp_env_time,
        &ctx.patch.envelope,
        ctx.gate,
        ctx.dt,
    );
    let filt_env = advance_envelope(
        &mut state.filt_env_level,
        &mut state.filt_env_stage,
        &mut state.filt_env_time,
        &ctx.patch.filter_envelope,
        ctx.gate,
        ctx.dt,
    );

    let lfo1 = lfo_value(&ctx.patch.lfo, ctx.time, &mut state.lfo1_rt);
    let lfo2 = lfo_value(&ctx.patch.lfo2, ctx.time, &mut state.lfo2_rt);
    let step = (ctx.time * 2.0).fract() * 2.0 - 1.0;

    let macro_vals: [f32; 4] = std::array::from_fn(|i| {
        ctx.patch
            .macros
            .get(i)
            .map(|m| (m.value - 0.5) * 2.0)
            .unwrap_or(0.0)
    });

    let sources = ModSources {
        lfo1,
        lfo2,
        amp_env,
        filt_env,
        velocity: ctx.velocity,
        modwheel: ctx.modwheel,
        aftertouch: ctx.mpe.pressure,
        pressure: ctx.mpe.pressure,
        timbre: ctx.mpe.timbre,
        pitch_bend: ctx.mpe.pitch_bend,
        step,
        rand: state.rand_mod,
        macros: macro_vals,
    };

    let matrix_mods = compute_mods(&ctx.patch.mod_matrix, &sources);
    let macro_mods = compute_macro_mods(&ctx.patch.macros);
    let mods = merge_mods(matrix_mods, macro_mods);

    let amp_mod = mods.get("amp").copied().unwrap_or(0.0);
    let amplitude = (ctx.velocity + amp_mod).clamp(0.0, 1.0) * amp_env;

    let pitch_bend_semi = ctx.mpe.pitch_bend_semitones(ctx.bend_range_semitones);
    let pitch_bend_mod = mods
        .get("pitch_bend")
        .or_else(|| mods.get("osc1_detune"))
        .copied()
        .unwrap_or(0.0);
    let base_freq = ctx.freq
        * 2.0f32.powf((pitch_bend_semi + pitch_bend_mod / 1200.0) / 12.0);

    let mut left = 0.0f32;
    let mut right = 0.0f32;
    let mut phase_idx = 0usize;
    let spread_cents = 15.0f32;
    let stereo_spread = ctx.patch.unison_stereo_spread.clamp(0.0, 1.0);

    for (oi, osc) in ctx.patch.oscillators.iter().enumerate() {
        if osc.level <= 0.0 {
            phase_idx += osc.unison.max(1) as usize;
            continue;
        }

        let bank_idx = (ctx.bank_for_osc)(oi);
        let bank = ctx
            .banks
            .get(bank_idx)
            .unwrap_or_else(|| ctx.banks.first().expect("at least one bank"));

        let pos_mod = mods
            .get(&format!("osc{}_position", oi + 1))
            .copied()
            .unwrap_or(0.0);
        let fm_index_mod = mods
            .get(&format!("osc{}_fm_index", oi + 1))
            .copied()
            .unwrap_or(0.0);
        let wt_pos = wt_position(osc, pos_mod, lfo1, lfo2, &ctx.patch.lfo, &ctx.patch.lfo2, bank.num_frames);
        let det_mod = mods
            .get(&format!("osc{}_detune", oi + 1))
            .copied()
            .unwrap_or(0.0);
        let pan_mod = mods
            .get(&format!("osc{}_pan", oi + 1))
            .copied()
            .unwrap_or(0.0);
        let level_mod = mods
            .get(&format!("osc{}_level", oi + 1))
            .copied()
            .unwrap_or(0.0);
        let unison = osc.unison.max(1) as usize;
        let warp = WtWarpMode::from_str(&osc.warp_mode);
        let warp_amount = osc.warp_amount;
        let fm_source = FmSource::from_str(&osc.fm_source);
        let fm_ratio = osc.fm_ratio.clamp(0.5, 16.0);
        let fm_index = (osc.fm_index + fm_index_mod
            + lfo_for_target(&ctx.patch.lfo, lfo1, &format!("osc{}_fm_index", oi + 1))
            + lfo_for_target(&ctx.patch.lfo2, lfo2, &format!("osc{}_fm_index", oi + 1)))
        .clamp(0.0, 10.0);

        let osc_level = (osc.level + level_mod).clamp(0.0, 1.0);

        for u in 0..unison {
            let det_spread = if unison > 1 {
                spread_cents * (u as f32 / (unison - 1) as f32 - 0.5) * 2.0
            } else {
                0.0
            };
            let pan_spread = if unison > 1 {
                (u as f32 / (unison - 1) as f32 - 0.5) * 2.0 * stereo_spread
            } else {
                0.0
            };
            let det = osc.detune + det_mod + det_spread;
            let osc_freq = base_freq * 2.0f32.powf(det / 1200.0);
            let phase_inc = osc_freq / ctx.sr;
            let phase = &mut state.phases[phase_idx];
            *phase += phase_inc;
            if *phase >= 1.0 {
                *phase -= 1.0;
            }

            let raw = process_os_fm(
                osc,
                bank,
                *phase,
                phase_inc,
                wt_pos,
                warp,
                warp_amount,
                fm_source,
                oi,
                ctx,
                fm_ratio,
                fm_index,
                state,
            );
            state.fm_feedback[oi] = raw;

            let osc_sample = raw * osc_level * amplitude / unison as f32;

            let (pan_l, pan_r) = equal_power_pan(osc.pan + pan_mod + pan_spread);
            left += osc_sample * pan_l;
            right += osc_sample * pan_r;
            phase_idx += 1;
        }
    }

    if ctx.patch.sub_level > 0.0 {
        let sub_phase = state.phases.first().copied().unwrap_or(0.0) * 0.5;
        let sub = (sub_phase * std::f32::consts::TAU).sin() * ctx.patch.sub_level * amplitude * 0.5;
        left += sub;
        right += sub;
    }
    if ctx.patch.noise_level > 0.0 {
        let noise = pseudo_noise(state.noise_seed) * ctx.patch.noise_level * amplitude;
        state.noise_seed = state.noise_seed.wrapping_add(1);
        left += noise;
        right += noise;
    }

    let filt_env_level = filt_env;

    let cutoff_mod = mods.get("filter_cutoff").copied().unwrap_or(0.0)
        + lfo_for_target(&ctx.patch.lfo, lfo1, "cutoff") * ctx.patch.filter.cutoff
        + lfo_for_target(&ctx.patch.lfo2, lfo2, "cutoff") * ctx.patch.filter.cutoff
        + ctx.mpe.timbre * 2000.0;
    let res_mod = mods.get("filter_resonance").copied().unwrap_or(0.0)
        + ctx.mpe.timbre * 0.15;

    let cutoff1 = compute_cutoff(
        &ctx.patch.filter,
        cutoff_mod,
        base_freq,
        filt_env_level,
        ctx.sr,
    );
    let resonance1 = (ctx.patch.filter.resonance + res_mod).clamp(0.0, 0.95);

    let cutoff2 = compute_cutoff(
        &ctx.patch.filter2,
        cutoff_mod * 0.5,
        base_freq,
        filt_env_level,
        ctx.sr,
    );
    let resonance2 = (ctx.patch.filter2.resonance + res_mod * 0.5).clamp(0.0, 0.95);

    let driven_l = process_os(left, |sample, _| soft_drive(sample, ctx.patch.filter.drive));
    let driven_r = process_os(
        right,
        |sample, _| soft_drive(sample, ctx.patch.filter2.drive.max(ctx.patch.filter.drive)),
    );
    let filtered_l = svf_filter(
        &mut state.svf_low,
        &mut state.svf_band,
        driven_l,
        cutoff1,
        resonance1,
        &ctx.patch.filter.filter_type,
        ctx.sr,
        ctx.patch.filter.drive,
        ctx.dt,
        0,
    );
    let filtered_r = svf_filter(
        &mut state.svf2_low,
        &mut state.svf2_band,
        driven_r,
        cutoff2,
        resonance2,
        &ctx.patch.filter2.filter_type,
        ctx.sr,
        ctx.patch.filter2.drive,
        ctx.dt,
        0,
    );

    let osc_mono = (left + right) * 0.5;
    VoiceStageSample {
        osc_mono,
        filtered: [
            filtered_l.clamp(-1.0, 1.0),
            filtered_r.clamp(-1.0, 1.0),
        ],
    }
}

fn process_os_fm(
    osc: &Oscillator,
    bank: &WavetableBank,
    phase: f32,
    phase_inc: f32,
    wt_pos: f32,
    warp: WtWarpMode,
    warp_amount: f32,
    fm_source: FmSource,
    oi: usize,
    ctx: &VoiceSampleContext<'_>,
    fm_ratio: f32,
    fm_index: f32,
    state: &mut VoiceState,
) -> f32 {
    process_os(0.0, |_, os_idx| {
        let sub_inc = phase_inc / OS_FACTOR as f32;
        let sub_phase = (phase + sub_inc * os_idx as f32).fract();
        let mod_signal = fm_mod_signal(
            fm_source,
            oi,
            &ctx.patch.oscillators,
            ctx.banks,
            ctx.bank_for_osc,
            sub_phase,
            fm_ratio,
            sub_inc,
            state.fm_feedback[oi],
        );
        sample_carrier_with_fm(
            osc,
            bank,
            sub_phase,
            sub_inc,
            wt_pos,
            warp,
            warp_amount,
            mod_signal,
            fm_index,
        )
    })
}

fn wt_position(
    osc: &Oscillator,
    pos_mod: f32,
    lfo1: f32,
    lfo2: f32,
    lfo1_cfg: &Lfo,
    lfo2_cfg: &Lfo,
    num_frames: usize,
) -> f32 {
    let max_pos = (num_frames.saturating_sub(1)).max(1) as f32;
    let morph_pos = if osc.morph_amount > 0.0 {
        osc.morph_a + (osc.morph_b - osc.morph_a) * osc.morph_amount.clamp(0.0, 1.0)
    } else {
        osc.position
    };
    (morph_pos
        + pos_mod
        + lfo_for_target(lfo1_cfg, lfo1, "wt_position")
        + lfo_for_target(lfo2_cfg, lfo2, "wt_position"))
    .clamp(0.0, max_pos)
}

fn soft_drive(input: f32, drive: f32) -> f32 {
    if drive <= 0.0 {
        return input;
    }
    (input * (1.0 + drive * 4.0)).tanh()
}

fn compute_cutoff(filter: &Filter, mod_hz: f32, freq: f32, filt_env: f32, sr: f32) -> f32 {
    let base = (filter.cutoff + mod_hz).max(25.0);
    let key_cutoff = key_tracked_cutoff(base, freq, filter.key_tracking);
    filter_env_cutoff(key_cutoff, filt_env, sr).min(sr * 0.45)
}

fn equal_power_pan(pan: f32) -> (f32, f32) {
    let p = pan.clamp(-1.0, 1.0);
    let angle = (p + 1.0) * 0.25 * std::f32::consts::PI;
    (angle.cos(), angle.sin())
}

fn key_tracked_cutoff(base: f32, freq: f32, key_tracking: f32) -> f32 {
    if key_tracking <= 0.0 {
        return base;
    }
    let semitones = 12.0 * (freq / 440.0).log2();
    base * 2.0f32.powf(semitones * key_tracking / 12.0)
}

fn filter_env_cutoff(base: f32, env_level: f32, sr: f32) -> f32 {
    let range = base * 3.0;
    (base + env_level * range).clamp(25.0, sr * 0.45)
}

fn advance_envelope(
    level: &mut f32,
    stage: &mut u8,
    time: &mut f32,
    env: &Envelope,
    gate: bool,
    dt: f32,
) -> f32 {
    if gate {
        match *stage {
            0 => {
                *time += dt;
                let a = env.attack.max(1e-4);
                *level = (*time / a).min(1.0);
                if *level >= 1.0 {
                    *stage = 1;
                    *time = 0.0;
                }
            }
            1 => {
                *time += dt;
                let d = env.decay.max(1e-4);
                let t = (*time / d).min(1.0);
                *level = 1.0 + t * (env.sustain - 1.0);
                if t >= 1.0 {
                    *stage = 2;
                }
            }
            2 => *level = env.sustain,
            3 => {
                *stage = 0;
                *time = 0.0;
            }
            _ => {}
        }
    } else if *stage != 3 {
        *stage = 3;
        *time = 0.0;
    } else {
        *time += dt;
        let r = env.release.max(1e-4);
        let t = (*time / r).min(1.0);
        *level *= 1.0 - t;
    }
    *level
}

fn svf_filter(
    low: &mut f32,
    band: &mut f32,
    input: f32,
    cutoff: f32,
    resonance: f32,
    mode: &str,
    sr: f32,
    drive: f32,
    _dt: f32,
    _os_idx: usize,
) -> f32 {
    let driven = if drive > 0.0 {
        (input * (1.0 + drive * 2.0)).tanh()
    } else {
        input
    };
    let fc = cutoff.clamp(20.0, sr * 0.49);
    let f = 2.0 * (std::f32::consts::PI * fc / sr).sin();
    let q = 1.0 - resonance.clamp(0.0, 0.95);

    *low += f * *band;
    let high = driven - *low - q * *band;
    *band += f * high;
    *low = low.clamp(-8.0, 8.0);
    *band = band.clamp(-8.0, 8.0);

    let out = match mode.to_ascii_lowercase().as_str() {
        "highpass" | "hp" => high,
        "bandpass" | "bp" => *band,
        "notch" => *low + high,
        _ => *low,
    };

    if drive > 0.0 {
        (out * (1.0 + drive)).tanh()
    } else {
        out
    }
}

fn pseudo_noise(seed: u32) -> f32 {
    let x = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    ((x >> 16) as f32 / 32768.0) - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::Patch;

    fn single_bank_ctx<'a>(
        bank: &'a WavetableBank,
        patch: &'a Patch,
        freq: f32,
        gate: bool,
        velocity: f32,
        time: f32,
        dt: f32,
    ) -> VoiceSampleContext<'a> {
        VoiceSampleContext {
            banks: std::slice::from_ref(bank),
            bank_for_osc: &|_| 0,
            patch,
            freq,
            gate,
            velocity,
            time,
            sample_index: 0,
            dt,
            sr: 44100.0,
            modwheel: 0.0,
            mpe: VoiceMpe::default(),
            bend_range_semitones: 48.0,
        }
    }

    #[test]
    fn velocity_scales_amplitude() {
        let bank = WavetableBank::factory_sine();
        let patch = Patch::default_mono();
        let mut low = VoiceState::new(&patch);
        let mut high = VoiceState::new(&patch);
        let dt = 1.0 / 44100.0;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let ctx_low = single_bank_ctx(&bank, &patch, 440.0, true, 0.25, t, dt);
            let ctx_high = single_bank_ctx(&bank, &patch, 440.0, true, 1.0, t, dt);
            let [l_l, _] = process_sample(&mut low, &ctx_low);
            let [l_h, _] = process_sample(&mut high, &ctx_high);
            if i > 2000 {
                assert!(l_h.abs() > l_l.abs());
            }
        }
    }

    #[test]
    fn pan_moves_energy() {
        let bank = WavetableBank::factory_sine();
        let mut patch_left = Patch::default_mono();
        patch_left.oscillators[0].pan = -1.0;
        let mut patch_right = Patch::default_mono();
        patch_right.oscillators[0].pan = 1.0;
        let mut left_voice = VoiceState::new(&patch_left);
        let mut right_voice = VoiceState::new(&patch_right);
        let dt = 1.0 / 44100.0;
        let mut hard_left = 0.0f32;
        let mut soft_left = 0.0f32;
        let mut hard_right = 0.0f32;
        let mut soft_right = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let ctx_l = single_bank_ctx(&bank, &patch_left, 440.0, true, 1.0, t, dt);
            let ctx_r = single_bank_ctx(&bank, &patch_right, 440.0, true, 1.0, t, dt);
            let [l, r] = process_sample(&mut left_voice, &ctx_l);
            hard_left += l.abs();
            soft_left += r.abs();
            let [l2, r2] = process_sample(&mut right_voice, &ctx_r);
            soft_right += l2.abs();
            hard_right += r2.abs();
        }
        assert!(hard_left > soft_left * 2.0, "hard_left={hard_left} soft_left={soft_left}");
        assert!(hard_right > soft_right * 2.0, "hard_right={hard_right} soft_right={soft_right}");
    }

    #[test]
    fn va_saw_produces_signal() {
        let bank = WavetableBank::factory_sine();
        let mut patch = Patch::factory_va_bass();
        patch.oscillators.truncate(1);
        patch.oscillators[0].level = 1.0;
        let mut voice = VoiceState::new(&patch);
        let dt = 1.0 / 44100.0;
        let mut peak = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let ctx = single_bank_ctx(&bank, &patch, 55.0, true, 1.0, t, dt);
            let [l, r] = process_sample(&mut voice, &ctx);
            peak = peak.max(l.abs().max(r.abs()));
        }
        assert!(peak > 0.05, "va saw peak={peak}");
    }

    #[test]
    fn dual_filter_stereo_width() {
        let bank = WavetableBank::factory_sine();
        let mut patch = Patch::default_mono();
        patch.filter.cutoff = 400.0;
        patch.filter2.cutoff = 4000.0;
        patch.filter2.filter_type = "highpass".into();
        patch.oscillators[0].pan = 0.0;
        let mut voice = VoiceState::new(&patch);
        let dt = 1.0 / 44100.0;
        let mut diff = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let ctx = single_bank_ctx(&bank, &patch, 440.0, true, 1.0, t, dt);
            let [l, r] = process_sample(&mut voice, &ctx);
            if l.is_finite() && r.is_finite() {
                diff += (l - r).abs();
            }
        }
        assert!(diff > 5.0, "stereo diff={diff}");
    }

    #[test]
    fn unison_spread_widens_stereo() {
        let bank = WavetableBank::factory_sine();
        let mut narrow = Patch::default_mono();
        narrow.oscillators[0].unison = 4;
        narrow.unison_stereo_spread = 0.0;
        narrow.filter2 = narrow.filter.clone();
        let mut wide = Patch::default_mono();
        wide.oscillators[0].unison = 4;
        wide.unison_stereo_spread = 1.0;
        wide.filter2 = wide.filter.clone();
        let dt = 1.0 / 44100.0;
        let mut narrow_diff = 0.0f32;
        let mut wide_diff = 0.0f32;
        let mut v1 = VoiceState::new(&narrow);
        let mut v2 = VoiceState::new(&wide);
        for i in 0..4410 {
            let t = i as f32 * dt;
            let [l1, r1] = process_sample(
                &mut v1,
                &single_bank_ctx(&bank, &narrow, 440.0, true, 1.0, t, dt),
            );
            let [l2, r2] = process_sample(
                &mut v2,
                &single_bank_ctx(&bank, &wide, 440.0, true, 1.0, t, dt),
            );
            narrow_diff += (l1 - r1).abs();
            wide_diff += (l2 - r2).abs();
        }
        assert!(wide_diff > narrow_diff * 1.2, "narrow={narrow_diff} wide={wide_diff}");
    }

    #[test]
    fn fm_index_changes_output() {
        let bank = WavetableBank::factory_sine();
        let mut wet_patch = Patch::factory_fm_bell();
        wet_patch.mod_matrix.clear();
        wet_patch.lfo.depth = 0.0;
        let mut dry_patch = wet_patch.clone();
        dry_patch.oscillators[0].fm_source = "none".into();
        dry_patch.oscillators[0].fm_index = 0.0;

        let mut dry = VoiceState::new(&dry_patch);
        let mut wet = VoiceState::new(&wet_patch);
        let dt = 1.0 / 44100.0;
        let mut diff = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let ctx_dry = single_bank_ctx(&bank, &dry_patch, 880.0, true, 1.0, t, dt);
            let ctx_wet = single_bank_ctx(&bank, &wet_patch, 880.0, true, 1.0, t, dt);
            let [l_d, _] = process_sample(&mut dry, &ctx_dry);
            let [l_w, _] = process_sample(&mut wet, &ctx_wet);
            assert!(l_d.is_finite(), "dry NaN at {i}");
            assert!(l_w.is_finite(), "wet NaN at {i}");
            if i > 500 {
                diff += (l_d - l_w).abs();
            }
        }
        assert!(diff > 0.5, "fm diff={diff}");
    }

    #[test]
    fn fm_index_mod_matrix_applies() {
        let bank = WavetableBank::factory_sine();
        let mut base = Patch::factory_fm_bell();
        base.mod_matrix.clear();
        base.lfo.depth = 0.0;
        base.lfo.target = "wt_position".into();
        let mut modded = base.clone();
        modded.mod_matrix.push(crate::patch::ModSlot {
            source: "lfo1".into(),
            target: "osc1_fm_index".into(),
            amount: 2.0,
            enabled: true,
        });
        modded.lfo.depth = 1.0;
        modded.lfo.rate = 10.0;
        modded.lfo.target = "wt_position".into();

        let mut v1 = VoiceState::new(&base);
        let mut v2 = VoiceState::new(&modded);
        let dt = 1.0 / 44100.0;
        let mut diff = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let [l1, _] = process_sample(
                &mut v1,
                &single_bank_ctx(&bank, &base, 660.0, true, 1.0, t, dt),
            );
            let [l2, _] = process_sample(
                &mut v2,
                &single_bank_ctx(&bank, &modded, 660.0, true, 1.0, t, dt),
            );
            assert!(l1.is_finite() && l2.is_finite());
            diff += (l1 - l2).abs();
        }
        assert!(diff > 0.01, "mod fm diff={diff}");
    }

    #[test]
    fn lfo2_mod_matrix_applies() {
        let bank = WavetableBank::factory_sine();
        let mut patch = Patch::default_mono();
        patch.lfo2.rate = 8.0;
        patch.lfo2.depth = 1.0;
        patch.mod_matrix.push(crate::patch::ModSlot {
            source: "lfo2".into(),
            target: "filter_cutoff".into(),
            amount: 0.5,
            enabled: true,
        });
        let mut dry = Patch::default_mono();
        dry.mod_matrix.clear();

        let mut v_wet = VoiceState::new(&patch);
        let mut v_dry = VoiceState::new(&dry);
        let dt = 1.0 / 44100.0;
        let mut diff = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let wet = process_sample(&mut v_wet, &single_bank_ctx(&bank, &patch, 440.0, true, 1.0, t, dt));
            let dry_s = process_sample(&mut v_dry, &single_bank_ctx(&bank, &dry, 440.0, true, 1.0, t, dt));
            diff += (wet[0] - dry_s[0]).abs();
        }
        assert!(diff > 0.1, "lfo2 mod diff={diff}");
    }

    #[test]
    fn mpe_pitch_bend_shifts_pitch() {
        let bank = WavetableBank::factory_sine();
        let patch = Patch::default_mono();
        let dt = 1.0 / 44100.0;
        let mut center = VoiceState::new(&patch);
        let mut bent = VoiceState::new(&patch);
        let mut diff = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let mut ctx_c = single_bank_ctx(&bank, &patch, 440.0, true, 1.0, t, dt);
            let mut ctx_b = single_bank_ctx(&bank, &patch, 440.0, true, 1.0, t, dt);
            ctx_b.mpe.pitch_bend = 0.5;
            let [l_c, _] = process_sample(&mut center, &ctx_c);
            let [l_b, _] = process_sample(&mut bent, &ctx_b);
            if i > 500 {
                diff += (l_c - l_b).abs();
            }
        }
        assert!(diff > 0.01, "mpe bend diff={diff}");
    }

    #[test]
    fn macro_changes_cutoff() {
        let bank = WavetableBank::factory_saw_morph();
        let mut patch = Patch::default_mono();
        patch.macros[0].value = 1.0;
        patch.macros[0].target = "filter_cutoff".into();
        patch.macros[0].amount = 1.0;
        let mut dry = patch.clone();
        dry.macros[0].value = 0.0;

        let mut v_wet = VoiceState::new(&patch);
        let mut v_dry = VoiceState::new(&dry);
        let dt = 1.0 / 44100.0;
        let mut diff = 0.0f32;
        for i in 0..4410 {
            let t = i as f32 * dt;
            let wet = process_sample(&mut v_wet, &single_bank_ctx(&bank, &patch, 440.0, true, 1.0, t, dt));
            let dry_s = process_sample(&mut v_dry, &single_bank_ctx(&bank, &dry, 440.0, true, 1.0, t, dt));
            diff += (wet[0] - dry_s[0]).abs();
        }
        assert!(diff > 0.1, "macro diff={diff}");
    }
}
