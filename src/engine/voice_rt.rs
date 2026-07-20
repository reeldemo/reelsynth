//! Realtime voice state wrapping the shared DSP kernel.

use crate::engine::mpe::VoiceMpe;
use crate::patch::Patch;
use crate::voice::VoiceState;

/// One realtime voice with note metadata.
#[derive(Clone, Debug)]
pub struct RtVoice {
    pub state: VoiceState,
    pub note: u8,
    pub channel: u8,
    pub freq: f32,
    pub velocity: f32,
    pub gate: bool,
    pub active: bool,
    pub age: u64,
    pub sample_counter: u32,
    pub start_time: f32,
    pub mpe: VoiceMpe,
}

impl RtVoice {
    pub fn new(patch: &Patch) -> Self {
        Self {
            state: VoiceState::new(patch),
            note: 0,
            channel: 0,
            freq: 0.0,
            velocity: 0.0,
            gate: false,
            active: false,
            age: 0,
            sample_counter: 0,
            start_time: 0.0,
            mpe: VoiceMpe::default(),
        }
    }

    pub fn trigger(
        &mut self,
        patch: &Patch,
        channel: u8,
        note: u8,
        freq: f32,
        velocity: f32,
        start_time: f32,
        mpe: VoiceMpe,
    ) {
        // Soft-continue when the voice is still audible — including the common
        // "brief note-off then note-on" path while releasing. A hard reset here
        // zeroed filter_fade and caused a held-note silence gap before sustain
        // resumed (user-reported "stops shortly then keeps playing").
        let audible = self.active && self.state.amp_env_level > 1e-5;
        let releasing = self.state.amp_env_stage == 3;
        if audible && (self.gate || releasing) {
            if releasing || !self.gate {
                self.state.amp_env_stage = 2;
                self.state.amp_env_level =
                    patch.envelope.sustain.max(self.state.amp_env_level);
                self.state.filt_env_stage = 2;
                self.state.filt_env_level = patch
                    .filter_envelope
                    .sustain
                    .max(self.state.filt_env_level);
            }
            // Keep filter_fade / slew state — no soft-start dropout.
        } else {
            self.state.reset(patch);
        }
        self.note = note;
        self.channel = channel;
        self.freq = freq;
        self.velocity = velocity;
        self.gate = true;
        self.active = true;
        self.sample_counter = 0;
        self.start_time = start_time;
        self.mpe = mpe;
    }

    pub fn release(&mut self) {
        self.gate = false;
    }

    pub fn is_audible(&self) -> bool {
        self.active && (self.gate || self.state.amp_env_level > 1e-5)
    }

    pub fn process_sample(
        &mut self,
        banks: &[crate::wavetable::WavetableBank],
        bank_for_osc: &dyn Fn(usize) -> usize,
        patch: &Patch,
        global_time: f32,
        dt: f32,
        sr: f32,
        modwheel: f32,
        bend_range_semitones: f32,
    ) -> [f32; 2] {
        self.process_sample_stages(
            banks,
            bank_for_osc,
            patch,
            global_time,
            dt,
            sr,
            modwheel,
            bend_range_semitones,
        )
        .filtered
    }

    pub fn process_sample_stages(
        &mut self,
        banks: &[crate::wavetable::WavetableBank],
        bank_for_osc: &dyn Fn(usize) -> usize,
        patch: &Patch,
        global_time: f32,
        dt: f32,
        sr: f32,
        modwheel: f32,
        bend_range_semitones: f32,
    ) -> crate::voice::VoiceStageSample {
        if !self.active {
            return crate::voice::VoiceStageSample::default();
        }

        let ctx = crate::voice::VoiceSampleContext {
            banks,
            bank_for_osc,
            wt_ids: &patch.wavetable_ids(),
            patch,
            freq: self.freq,
            gate: self.gate,
            velocity: self.velocity,
            time: global_time - self.start_time,
            sample_index: self.sample_counter,
            dt,
            sr,
            modwheel,
            mpe: self.mpe,
            bend_range_semitones,
        };
        let sample = crate::voice::process_sample_stages(&mut self.state, &ctx);
        self.sample_counter = self.sample_counter.wrapping_add(1);

        if !self.gate && self.state.amp_env_level <= 1e-6 && self.state.amp_env_stage == 3 {
            self.active = false;
        }

        sample
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::{Envelope, Patch};

    #[test]
    fn retrigger_while_releasing_keeps_filter_fade() {
        let mut patch = Patch::default_mono();
        patch.envelope = Envelope {
            attack: 0.001,
            decay: 0.05,
            sustain: 0.8,
            release: 0.4,
        };
        let mut voice = RtVoice::new(&patch);
        voice.trigger(&patch, 0, 60, 440.0, 1.0, 0.0, VoiceMpe::default());
        // Advance into sustain so amp is high and fade is complete.
        voice.state.filter_fade = 1.0;
        voice.state.amp_env_stage = 2;
        voice.state.amp_env_level = 0.8;
        voice.release();
        voice.state.amp_env_stage = 3;
        voice.state.amp_env_level = 0.55;

        voice.trigger(&patch, 0, 60, 440.0, 1.0, 0.1, VoiceMpe::default());

        assert!(
            voice.state.filter_fade > 0.99,
            "retrigger while releasing must not soft-start from silence (fade={})",
            voice.state.filter_fade
        );
        assert_eq!(voice.state.amp_env_stage, 2);
        assert!(voice.gate);
        assert!(voice.state.amp_env_level >= 0.55);
    }

    #[test]
    fn fresh_note_still_soft_starts() {
        let patch = Patch::default_mono();
        let mut voice = RtVoice::new(&patch);
        voice.trigger(&patch, 0, 60, 440.0, 1.0, 0.0, VoiceMpe::default());
        assert!(
            voice.state.filter_fade < 0.01,
            "cold note-on must soft-start (fade={})",
            voice.state.filter_fade
        );
    }
}
