//! Shared per-sample voice DSP kernel (offline + realtime).

use crate::fm::{fm_mod_signal, sample_carrier_with_fm, FmSource};
use crate::lfo::{lfo_for_target, lfo_value, LfoRuntime};
use crate::modulation::{compute_macro_mods, compute_mods, merge_mods, ModSources};
use crate::osc::WtWarpMode;
use crate::oversample::{process_os, OS_FACTOR};
use crate::patch::{Lfo, Oscillator, Patch};
use crate::wavetable::WavetableBank;
use crate::engine::VoiceMpe;
use super::envelope::advance_envelope;
use super::filter_svf::{compute_cutoff, equal_power_pan, svf_filter};

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

fn pseudo_noise(seed: u32) -> f32 {
    let x = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    ((x >> 16) as f32 / 32768.0) - 1.0
}

