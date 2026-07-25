//! Block-based realtime synthesizer engine (S0).

mod bank_set;
mod midi;
mod mpe;
mod params;
mod voice_pool;
mod voice_rt;

pub use crate::scope::ScopeMonitor;
pub use bank_set::BankSet;
pub use midi::{note_to_freq, pitch_bend_from_raw, MidiEvent};
pub use mpe::{MpeConfig, MpeState, VoiceMpe};
pub use params::{EngineParams, Smoother};
pub use voice_pool::{VoicePool, MAX_VOICES};
pub use voice_rt::RtVoice;

use crate::fx::FxChain;
use crate::modulation::apply_mods_to_patch;
use crate::overtone::OvertoneFilterChain;
use crate::patch::Patch;
use crate::seam::CrackleVoice;
use crate::sequence::{SequencerRuntime, TransportState};
use crate::voice::render_note;
use crate::wavetable::WavetableBank;

/// Internal block size for voice summing (64–128 samples).
pub const BLOCK_SIZE: usize = 64;

/// Polyphonic wavetable synth engine with shared offline/realtime DSP.
pub struct SynthEngine {
    banks: BankSet,
    patch: Patch,
    scratch_patch: Patch,
    pool: VoicePool,
    params: EngineParams,
    fx: FxChain,
    overtone: OvertoneFilterChain,
    /// Artistic crackle character (0 = clean; modulatable via patch.crackle).
    crackle_l: CrackleVoice,
    crackle_r: CrackleVoice,
    /// Cached frame index used for overtone harshness (osc 0).
    overtone_harsh_frame: Option<usize>,
    sample_rate: u32,
    global_time: f32,
    scope: ScopeMonitor,
    mpe: MpeState,
    sequencer: SequencerRuntime,
    /// Smoothed √N headroom — avoids a hard gain jump when a second voice starts.
    headroom_smooth: f32,
}

fn voice_headroom(active_voices: usize) -> f32 {
    if active_voices <= 1 {
        1.0
    } else {
        1.0 / (active_voices as f32).sqrt()
    }
}

/// Smooth poly headroom: duck quickly when voices are added (avoid bus clip),
/// restore slowly when voices leave (avoid swell clicks).
const HEADROOM_DOWN_SECONDS: f32 = 0.004;
const HEADROOM_UP_SECONDS: f32 = 0.045;
/// Cap how far smooth may sit above the target so a 5-note chord can't clip for tens of ms.
const HEADROOM_ABOVE_TARGET_MAX: f32 = 0.06;

fn sanitize_sample(sample: f32) -> f32 {
    if sample.is_finite() {
        sample
    } else {
        0.0
    }
}

impl SynthEngine {
    pub fn new(bank: WavetableBank, patch: Patch, sample_rate: u32) -> Self {
        let params = EngineParams::new(&patch, sample_rate as f32);
        let pool = VoicePool::new(&patch);
        let fx = FxChain::new(sample_rate);
        let overtone = OvertoneFilterChain::new(sample_rate);
        let crackle_l = CrackleVoice::default();
        let crackle_r = CrackleVoice::default();
        let banks = BankSet::from_primary(bank, &patch);
        let bpm = patch.sequence.bpm;
        let mut sequencer = SequencerRuntime::new(bpm);
        sequencer.sync_from_project(&patch.sequence);
        let scratch_patch = patch.clone();
        Self {
            banks,
            patch,
            scratch_patch,
            pool,
            params,
            fx,
            overtone,
            crackle_l,
            crackle_r,
            overtone_harsh_frame: None,
            sample_rate,
            global_time: 0.0,
            scope: ScopeMonitor::new(),
            mpe: MpeState::new(),
            sequencer,
            headroom_smooth: 1.0,
        }
    }

    pub fn scope_monitor(&self) -> &ScopeMonitor {
        &self.scope
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn patch(&self) -> &Patch {
        &self.patch
    }

    pub fn patch_mut(&mut self) -> &mut Patch {
        &mut self.patch
    }

    pub fn transport(&self) -> &TransportState {
        &self.sequencer.transport
    }

    pub fn sequencer(&self) -> &SequencerRuntime {
        &self.sequencer
    }

    pub fn sequencer_mut(&mut self) -> &mut SequencerRuntime {
        &mut self.sequencer
    }

    /// Live UI sync — updates patch state without killing active voices or FX tails.
    pub fn apply_patch_hot(&mut self, patch: Patch) {
        self.sequencer.sync_from_project(&patch.sequence);
        self.params.sync_from_patch(&patch);
        self.banks = BankSet::from_primary(self.banks.primary().clone(), &patch);
        if patch.effects != self.patch.effects {
            self.fx.set_effects(patch.effects.clone());
        }
        self.patch = patch;
    }

    /// Hard reset for preset load — clears voices and rebuilds FX processors.
    pub fn reset_from_preset(&mut self, bank: WavetableBank, patch: Patch) {
        self.banks = BankSet::from_primary(bank, &patch);
        self.sequencer.sync_from_project(&patch.sequence);
        self.params.sync_from_patch(&patch);
        self.pool.reset_patch(&patch);
        self.fx.set_effects(patch.effects.clone());
        self.overtone_harsh_frame = None;
        self.headroom_smooth = 1.0;
        self.patch = patch;
    }

    /// Replace the primary wavetable bank without resetting voices or FX.
    pub fn update_bank(&mut self, bank: WavetableBank) {
        self.banks.replace_primary(bank, &self.patch);
        self.overtone_harsh_frame = None;
    }

    pub fn bank(&self) -> &WavetableBank {
        self.banks.primary()
    }

    pub fn banks(&self) -> &[WavetableBank] {
        self.banks.banks()
    }

    /// Hot-swap wavetable bank and patch (preset load).
    pub fn load_preset(&mut self, bank: WavetableBank, patch: Patch) {
        self.reset_from_preset(bank, patch);
    }

    pub fn set_wt_position(&mut self, position: f32) {
        if let Some(osc) = self.patch.oscillators.get_mut(0) {
            osc.position = position.clamp(0.0, 255.0);
        }
    }

    pub fn set_filter_cutoff(&mut self, cutoff: f32) {
        self.patch.filter.cutoff = cutoff;
        self.patch.sync_chain_slot0_from_legacy();
        self.params.filter_cutoff.set_target(cutoff);
    }

    pub fn set_filter_resonance(&mut self, resonance: f32) {
        self.patch.filter.resonance = resonance.clamp(0.0, 0.95);
        self.patch.sync_chain_slot0_from_legacy();
    }

    pub fn set_filter_type(&mut self, filter_type: &str) {
        self.patch.filter.filter_type = filter_type.to_string();
        self.patch.sync_chain_slot0_from_legacy();
    }

    pub fn set_filter_key_tracking(&mut self, key_tracking: f32) {
        self.patch.filter.key_tracking = key_tracking.clamp(0.0, 1.0);
        self.patch.sync_chain_slot0_from_legacy();
    }

    pub fn set_filter_drive(&mut self, drive: f32) {
        self.patch.filter.drive = drive.clamp(0.0, 1.0);
        self.patch.sync_chain_slot0_from_legacy();
    }

    pub fn set_filter2_cutoff(&mut self, cutoff: f32) {
        self.patch.filter2.cutoff = cutoff;
        if let Some(slots) = &mut self.patch.filters {
            if let Some(slot) = slots.get_mut(1) {
                slot.cutoff = cutoff;
            }
        }
    }

    pub fn set_filter2_resonance(&mut self, resonance: f32) {
        self.patch.filter2.resonance = resonance.clamp(0.0, 0.95);
        if let Some(slots) = &mut self.patch.filters {
            if let Some(slot) = slots.get_mut(1) {
                slot.resonance = self.patch.filter2.resonance;
            }
        }
    }

    pub fn set_filter2_type(&mut self, filter_type: &str) {
        self.patch.filter2.filter_type = filter_type.to_string();
        if let Some(slots) = &mut self.patch.filters {
            if let Some(slot) = slots.get_mut(1) {
                slot.filter_type = filter_type.to_string();
            }
        }
    }

    pub fn set_filter2_drive(&mut self, drive: f32) {
        self.patch.filter2.drive = drive.clamp(0.0, 1.0);
        if let Some(slots) = &mut self.patch.filters {
            if let Some(slot) = slots.get_mut(1) {
                slot.drive = self.patch.filter2.drive;
            }
        }
    }

    pub fn set_filter_chain(&mut self, slots: Vec<crate::patch::FilterSlot>) {
        let capped: Vec<_> = slots
            .into_iter()
            .take(crate::patch::FilterSlot::MAX_SLOTS)
            .collect();
        self.patch.filters = Some(capped);
        self.patch.sync_legacy_filters_from_chain();
        self.params
            .filter_cutoff
            .set_target(self.patch.filter.cutoff);
    }

    pub fn set_unison_stereo_spread(&mut self, spread: f32) {
        self.patch.unison_stereo_spread = spread.clamp(0.0, 1.0);
    }

    pub fn set_osc_type(&mut self, index: usize, osc_type: &str) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.osc_type = osc_type.to_string();
        }
    }

    pub fn set_osc_pulse_width(&mut self, index: usize, pw: f32) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.pulse_width = pw.clamp(0.05, 0.95);
        }
    }

    pub fn set_osc_morph(&mut self, index: usize, a: f32, b: f32, amount: f32) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.morph_a = a.clamp(0.0, 255.0);
            osc.morph_b = b.clamp(0.0, 255.0);
            osc.morph_amount = amount.clamp(0.0, 1.0);
            if amount > 0.0 {
                osc.position = osc.morph_a + (osc.morph_b - osc.morph_a) * amount;
            }
        }
    }

    pub fn set_osc_warp(&mut self, index: usize, mode: &str, amount: f32) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.warp_mode = mode.to_string();
            osc.warp_amount = amount.clamp(0.0, 1.0);
        }
    }

    pub fn set_osc_fm(&mut self, index: usize, fm_source: &str, fm_ratio: f32, fm_index: f32) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.fm_source = fm_source.to_string();
            osc.fm_ratio = fm_ratio.clamp(0.5, 16.0);
            osc.fm_index = fm_index.clamp(0.0, 10.0);
        }
    }

    pub fn set_envelope(&mut self, envelope: crate::patch::Envelope) {
        self.patch.envelope = envelope;
    }

    pub fn set_filter_envelope(&mut self, envelope: crate::patch::Envelope) {
        self.patch.filter_envelope = envelope;
    }

    pub fn set_lfo_rate(&mut self, rate: f32) {
        self.patch.lfo.rate = rate.max(0.0);
    }

    pub fn set_lfo_depth(&mut self, depth: f32) {
        self.patch.lfo.depth = depth.clamp(0.0, 1.0);
    }

    pub fn set_lfo_shape(&mut self, shape: &str) {
        self.patch.lfo.shape = shape.to_string();
    }

    pub fn set_lfo2_rate(&mut self, rate: f32) {
        self.patch.lfo2.rate = rate.max(0.0);
    }

    pub fn set_lfo2_depth(&mut self, depth: f32) {
        self.patch.lfo2.depth = depth.clamp(0.0, 1.0);
    }

    pub fn set_lfo2_shape(&mut self, shape: &str) {
        self.patch.lfo2.shape = shape.to_string();
    }

    pub fn set_macro(&mut self, index: usize, value: f32) {
        if let Some(mac) = self.patch.macros.get_mut(index) {
            mac.value = value.clamp(0.0, 1.0);
        }
    }

    pub fn set_macros(&mut self, macros: Vec<crate::patch::Macro>) {
        self.patch.macros = macros;
    }

    pub fn set_osc_level(&mut self, index: usize, level: f32) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.level = level.clamp(0.0, 1.0);
        }
    }

    pub fn set_osc_pan(&mut self, index: usize, pan: f32) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.pan = pan.clamp(-1.0, 1.0);
        }
    }

    pub fn set_osc_detune(&mut self, index: usize, detune: f32) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.detune = detune.clamp(-2400.0, 2400.0);
        }
    }

    pub fn set_osc_unison(&mut self, index: usize, unison: u32) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.unison = unison.clamp(1, 8);
        }
    }

    pub fn set_osc_position(&mut self, index: usize, position: f32) {
        self.patch.ensure_oscillators(index + 1);
        if let Some(osc) = self.patch.oscillators.get_mut(index) {
            osc.position = position.clamp(0.0, 255.0);
        }
    }

    pub fn set_sub_level(&mut self, level: f32) {
        self.patch.sub_level = level.clamp(0.0, 1.0);
    }

    pub fn set_noise_level(&mut self, level: f32) {
        self.patch.noise_level = level.clamp(0.0, 1.0);
    }

    pub fn set_mod_matrix(&mut self, slots: Vec<crate::patch::ModSlot>) {
        self.patch.mod_matrix = slots;
    }

    pub fn set_effects(&mut self, effects: Vec<crate::fx::EffectSlot>) {
        self.patch.effects = effects.clone();
        self.fx.set_effects(effects);
    }

    /// Session-only overtone / anti-crackle chain (not persisted in `.reelpreset`).
    pub fn set_overtone_slots(&mut self, slots: Vec<crate::overtone::OvertoneFilterSlot>) {
        self.overtone.set_slots(slots);
        self.overtone_harsh_frame = None;
    }

    pub fn overtone_slots(&self) -> &[crate::overtone::OvertoneFilterSlot] {
        self.overtone.slots()
    }

    /// Legacy API — maps fixed chorus/delay/reverb bypass flags.
    pub fn set_fx_bypass(&mut self, bypass: crate::fx::FxBypass) {
        self.set_effects(crate::fx::effects_from_bypass(&bypass));
    }

    /// Recompute curve harshness from osc 0's active WT frame when the frame index changes.
    fn refresh_overtone_harshness(&mut self) {
        if self.overtone.slots().is_empty() {
            return;
        }
        let bank = self.banks.primary();
        if bank.num_frames == 0 || bank.frame_size == 0 {
            self.overtone.set_curve_harshness(0.0);
            return;
        }
        let pos = self
            .patch
            .oscillators
            .first()
            .map(|o| o.position)
            .unwrap_or(0.0);
        let idx = pos.round().clamp(0.0, (bank.num_frames - 1) as f32) as usize;
        if self.overtone_harsh_frame == Some(idx) {
            return;
        }
        let harsh = crate::overtone::curve_harshness(bank.frame(idx));
        self.overtone.set_curve_harshness(harsh);
        self.overtone_harsh_frame = Some(idx);
    }

    pub fn note_on(&mut self, channel: u8, note: u8, velocity: f32) {
        if channel != SequencerRuntime::seq_channel() && self.sequencer.transport.recording {
            self.sequencer.live_note_on(note, velocity);
        }
        let freq = note_to_freq(note);
        self.note_on_freq(channel, note, freq, velocity);
    }

    /// Trigger a voice at an arbitrary frequency (custom Hz performance input).
    pub fn note_on_freq(&mut self, channel: u8, note: u8, freq: f32, velocity: f32) {
        let voice_mpe = self.mpe.voice_mpe(channel);
        self.pool.note_on(
            &self.patch,
            channel,
            note,
            freq.max(0.0),
            velocity,
            self.global_time,
            voice_mpe,
        );
    }

    pub fn note_off(&mut self, channel: u8, note: u8) {
        if channel != SequencerRuntime::seq_channel() && self.sequencer.transport.recording {
            self.sequencer.live_note_off(note);
        }
        self.pool.note_off(channel, note);
    }

    pub fn handle_event(&mut self, event: MidiEvent) {
        match event {
            MidiEvent::NoteOn {
                channel,
                note,
                velocity,
            } => self.note_on(channel, note, velocity),
            MidiEvent::NoteOff { channel, note } => self.note_off(channel, note),
            MidiEvent::PitchBend { channel, value } => {
                self.mpe.set_pitch_bend(channel, value);
                let mpe = self.mpe.voice_mpe(channel);
                self.pool.update_channel_mpe(channel, mpe);
            }
            MidiEvent::ChannelPressure { channel, pressure } => {
                self.mpe.set_pressure(channel, pressure);
                let mpe = self.mpe.voice_mpe(channel);
                self.pool.update_channel_mpe(channel, mpe);
            }
            MidiEvent::PolyAftertouch {
                channel,
                note,
                pressure,
            } => {
                self.mpe.set_pressure(channel, pressure);
                let mpe = self.mpe.voice_mpe(channel);
                for voice in self.pool.voices_mut() {
                    if voice.active && voice.channel == channel && voice.note == note {
                        voice.mpe.pressure = pressure;
                    }
                }
                self.pool.update_channel_mpe(channel, mpe);
            }
            MidiEvent::ControlChange { channel, cc, value } => match cc {
                1 => self.mpe.set_modwheel(value),
                74 => {
                    self.mpe.set_timbre(channel, value);
                    let mpe = self.mpe.voice_mpe(channel);
                    self.pool.update_channel_mpe(channel, mpe);
                }
                _ => {}
            },
        }
    }

    /// Legacy note-on without channel (channel 0).
    pub fn note_on_legacy(&mut self, note: u8, velocity: f32) {
        self.note_on(0, note, velocity);
    }

    /// Legacy note-off without channel (channel 0).
    pub fn note_off_legacy(&mut self, note: u8) {
        self.note_off(0, note);
    }

    /// Render one block of mono audio into `out` (L+R average).
    pub fn process(&mut self, out: &mut [f32]) {
        for chunk in out.chunks_mut(BLOCK_SIZE) {
            self.process_block_mono(chunk);
        }
    }

    /// Render interleaved stereo `[L,R,L,R,…]`.
    pub fn process_stereo(&mut self, out: &mut [f32]) {
        let frames = out.len() / 2;
        for chunk_start in (0..frames).step_by(BLOCK_SIZE) {
            let chunk_frames = (frames - chunk_start).min(BLOCK_SIZE);
            self.process_block_stereo(&mut out[chunk_start * 2..(chunk_start + chunk_frames) * 2]);
        }
    }

    fn process_block_mono(&mut self, out: &mut [f32]) {
        let sr = self.sample_rate as f32;
        let dt = 1.0 / sr;
        let bank_slice = self.banks.banks().to_vec();
        let frames = out.len();

        self.sequencer
            .begin_buffer(&self.patch.sequence, frames, sr);

        self.scratch_patch.clone_from(&self.patch);
        self.refresh_overtone_harshness();

        for (frame, sample) in out.iter_mut().enumerate() {
            self.dispatch_seq_events(frame);

            self.params.filter_cutoff.process();
            self.params.master_gain.process();
            self.scratch_patch.filter.cutoff = self.params.filter_cutoff.current();
            self.scratch_patch.sync_chain_slot0_from_legacy();
            let auto_mods = self.sequencer.automation_mods(&self.patch.sequence);
            apply_mods_to_patch(&mut self.scratch_patch, &auto_mods);
            let patch = &self.scratch_patch;
            let bank_for_osc = |oi: usize| self.banks.bank_for_osc(patch, oi);

            let mut acc_osc = 0.0f32;
            let mut acc_l = 0.0f32;
            let mut acc_r = 0.0f32;
            let mut voices_active = 0usize;
            let modwheel = self.mpe.modwheel();
            let bend_range = self.mpe.config.bend_range_semitones;
            for voice in self.pool.voices_mut() {
                if !voice.active {
                    continue;
                }
                if voice.is_audible() {
                    voices_active += 1;
                }
                let stages = voice.process_sample_stages(
                    &bank_slice,
                    &bank_for_osc,
                    patch,
                    self.global_time,
                    dt,
                    sr,
                    modwheel,
                    bend_range,
                );
                acc_osc += stages.osc_mono;
                acc_l += stages.filtered[0];
                acc_r += stages.filtered[1];
            }
            let headroom = self.smooth_headroom(voices_active, dt);
            acc_l = Self::soft_bus(acc_l * headroom);
            acc_r = Self::soft_bus(acc_r * headroom);
            acc_osc *= headroom;
            let gain = self.params.master_gain.current();
            let filt_mono = (acc_l + acc_r) * 0.5 * gain;
            let mixed = (acc_l + acc_r) * 0.5 * gain;
            let cracked = self.crackle_l.process(mixed, self.scratch_patch.crackle);
            let suppressed = self.overtone.process_sample(cracked);
            let mono = self.fx.process_sample(suppressed);
            let fx_mono = mono;
            *sample = sanitize_sample(mono);
            self.scope
                .write_frame(acc_osc * gain, filt_mono, fx_mono, mono, voices_active > 0);
            self.global_time += dt;
        }
    }

    fn process_block_stereo(&mut self, out: &mut [f32]) {
        let sr = self.sample_rate as f32;
        let dt = 1.0 / sr;
        let frames = out.len() / 2;
        let bank_slice = self.banks.banks().to_vec();

        self.sequencer
            .begin_buffer(&self.patch.sequence, frames, sr);

        self.scratch_patch.clone_from(&self.patch);
        self.refresh_overtone_harshness();

        for frame in 0..frames {
            self.dispatch_seq_events(frame);

            self.params.filter_cutoff.process();
            self.params.master_gain.process();
            self.scratch_patch.filter.cutoff = self.params.filter_cutoff.current();
            self.scratch_patch.sync_chain_slot0_from_legacy();
            let auto_mods = self.sequencer.automation_mods(&self.patch.sequence);
            apply_mods_to_patch(&mut self.scratch_patch, &auto_mods);
            let patch = &self.scratch_patch;
            let bank_for_osc = |oi: usize| self.banks.bank_for_osc(patch, oi);

            let mut acc_osc = 0.0f32;
            let mut acc_l = 0.0f32;
            let mut acc_r = 0.0f32;
            let mut voices_active = 0usize;
            let modwheel = self.mpe.modwheel();
            let bend_range = self.mpe.config.bend_range_semitones;
            for voice in self.pool.voices_mut() {
                if !voice.active {
                    continue;
                }
                if voice.is_audible() {
                    voices_active += 1;
                }
                let stages = voice.process_sample_stages(
                    &bank_slice,
                    &bank_for_osc,
                    patch,
                    self.global_time,
                    dt,
                    sr,
                    modwheel,
                    bend_range,
                );
                acc_osc += stages.osc_mono;
                acc_l += stages.filtered[0];
                acc_r += stages.filtered[1];
            }
            let headroom = self.smooth_headroom(voices_active, dt);
            acc_l = Self::soft_bus(acc_l * headroom);
            acc_r = Self::soft_bus(acc_r * headroom);
            acc_osc *= headroom;
            let gain = self.params.master_gain.current();
            let filt_mono = (acc_l + acc_r) * 0.5 * gain;
            let cl = self
                .crackle_l
                .process(acc_l * gain, self.scratch_patch.crackle);
            let cr = self
                .crackle_r
                .process(acc_r * gain, self.scratch_patch.crackle);
            let [sl, sr_] = self.overtone.process_stereo(cl, cr);
            let [l, r] = self.fx.process_stereo(sl, sr_);
            let fx_mono = (l + r) * 0.5;
            let out_mono = fx_mono;
            out[frame * 2] = sanitize_sample(l);
            out[frame * 2 + 1] = sanitize_sample(r);
            self.scope.write_frame(
                acc_osc * gain,
                filt_mono,
                fx_mono,
                out_mono,
                voices_active > 0,
            );
            self.global_time += dt;
        }
    }

    #[inline]
    fn smooth_headroom(&mut self, voices_active: usize, dt: f32) -> f32 {
        let target = voice_headroom(voices_active);
        // Prevent a multi-note chord from riding near-unity gain while √N target is low.
        if self.headroom_smooth > target + HEADROOM_ABOVE_TARGET_MAX {
            self.headroom_smooth = target + HEADROOM_ABOVE_TARGET_MAX;
        }
        let tau = if target < self.headroom_smooth {
            HEADROOM_DOWN_SECONDS
        } else {
            HEADROOM_UP_SECONDS
        };
        let alpha = (dt / tau).clamp(0.0, 1.0);
        self.headroom_smooth += (target - self.headroom_smooth) * alpha;
        self.headroom_smooth
    }

    /// Soft bus clip after voice sum — catches residual poly peaks without hard digital foldover.
    #[inline]
    fn soft_bus(x: f32) -> f32 {
        // Cubic soft-clip: unity gain near 0, gentle compress above ~0.8.
        let a = x.clamp(-1.5, 1.5);
        a - (a * a * a) * (1.0 / 6.75)
    }

    /// Offline reference render using the same patch/bank (for golden tests).
    pub fn render_offline(&self, freq: f32, duration: f32) -> Vec<f32> {
        let bank_for_osc = |oi: usize| self.banks.bank_for_osc(&self.patch, oi);
        let mut audio = render_note(
            self.banks.banks(),
            bank_for_osc,
            freq,
            duration,
            self.sample_rate,
            &self.patch,
        );
        let mut fx = FxChain::new(self.sample_rate);
        fx.set_effects(self.patch.effects.clone());
        for sample in audio.iter_mut() {
            *sample = fx.process_sample(*sample);
        }
        audio
    }

    fn dispatch_seq_events(&mut self, frame: usize) {
        let events = self.sequencer.events_at_frame(frame);
        for ev in events {
            match ev {
                crate::sequence::SchedEvent::NoteOn {
                    channel,
                    note,
                    velocity,
                    ..
                } => {
                    let freq = note_to_freq(note);
                    self.note_on_freq(channel, note, freq, velocity);
                }
                crate::sequence::SchedEvent::NoteOff { channel, note, .. } => {
                    self.pool.note_off(channel, note);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::artifact_reduce::{periodize_with_algo, PeriodizeAlgo};
    use crate::seam::SeamStyle;

    fn max_abs_step(mono: &[f32]) -> f32 {
        mono.windows(2)
            .map(|w| (w[1] - w[0]).abs())
            .fold(0.0f32, f32::max)
    }

    #[test]
    fn second_note_onset_does_not_chop_held_voice() {
        let mut bank = WavetableBank::factory_saw_morph();
        for f in 0..bank.num_frames {
            periodize_with_algo(
                bank.frame_mut(f),
                0.0,
                SeamStyle::Adaptive,
                PeriodizeAlgo::DualCosine,
            );
        }
        let mut patch = Patch::factory_lead();
        patch.effects.clear();
        patch.lfo.depth = 0.0;
        patch.lfo2.depth = 0.0;
        for slot in &mut patch.mod_matrix {
            slot.enabled = false;
        }

        let sr = 44_100u32;
        let mut eng = SynthEngine::new(bank, patch, sr);
        let total = (sr as f32 * 0.55) as usize;
        let n2_at = (sr as f32 * 0.25) as usize;
        let mut stereo = vec![0.0f32; total * 2];
        eng.note_on(0, 60, 0.9);
        let mut fired_n2 = false;
        let mut i = 0;
        while i < total {
            if !fired_n2 && i >= n2_at {
                eng.note_on(0, 64, 0.9);
                fired_n2 = true;
            }
            let end = (i + 128).min(total);
            eng.process_stereo(&mut stereo[i * 2..end * 2]);
            i = end;
        }
        let mono: Vec<f32> = stereo.chunks(2).map(|c| (c[0] + c[1]) * 0.5).collect();

        let pre_a = n2_at.saturating_sub((0.04 * sr as f32) as usize);
        let pre_b = n2_at;
        let post_a = n2_at;
        let post_b = (n2_at + (0.020 * sr as f32) as usize).min(mono.len());
        let pre_step = max_abs_step(&mono[pre_a..pre_b]);
        let post_step = max_abs_step(&mono[post_a..post_b]);
        let peak = mono[post_a..post_b]
            .iter()
            .map(|s| s.abs())
            .fold(0.0f32, f32::max);
        assert!(peak > 0.05, "second note too quiet ({peak})");
        // Instant √N headroom used to spike post_step ≫ pre_step (~0.3+).
        assert!(
            post_step < pre_step * 3.5 + 0.08,
            "second-note headroom click: pre={pre_step:.4} post={post_step:.4}"
        );
    }

    #[test]
    fn five_note_chord_onset_stays_bounded() {
        let mut bank = WavetableBank::factory_saw_morph();
        for f in 0..bank.num_frames {
            periodize_with_algo(
                bank.frame_mut(f),
                0.0,
                SeamStyle::Adaptive,
                PeriodizeAlgo::DualCosine,
            );
        }
        let mut patch = Patch::factory_lead();
        patch.effects.clear();
        patch.lfo.depth = 0.0;
        patch.lfo2.depth = 0.0;
        for slot in &mut patch.mod_matrix {
            slot.enabled = false;
        }

        let sr = 44_100u32;
        let mut eng = SynthEngine::new(bank, patch, sr);
        let total = (sr as f32 * 0.6) as usize;
        let chord_at = (sr as f32 * 0.15) as usize;
        let notes = [60u8, 64, 67, 71, 74];
        let mut stereo = vec![0.0f32; total * 2];
        let mut fired = false;
        let mut i = 0;
        while i < total {
            if !fired && i >= chord_at {
                for n in notes {
                    eng.note_on(0, n, 0.9);
                }
                fired = true;
            }
            let end = (i + 128).min(total);
            eng.process_stereo(&mut stereo[i * 2..end * 2]);
            i = end;
        }
        let mono: Vec<f32> = stereo.chunks(2).map(|c| (c[0] + c[1]) * 0.5).collect();
        let onset_end = (chord_at + (0.04 * sr as f32) as usize).min(mono.len());
        let onset_step = max_abs_step(&mono[chord_at..onset_end]);
        let peak = mono[chord_at..onset_end]
            .iter()
            .map(|s| s.abs())
            .fold(0.0f32, f32::max);
        assert!(peak > 0.05, "chord too quiet ({peak})");
        assert!(
            onset_step < 0.55,
            "five-note chord onset crackle: step={onset_step:.4} peak={peak:.4}"
        );
        assert!(
            mono.iter().all(|s| s.is_finite() && s.abs() <= 1.01),
            "bus clipped to non-finite or hard foldover"
        );
    }
}
