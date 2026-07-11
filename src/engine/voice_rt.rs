//! Realtime voice state wrapping the shared DSP kernel.

use crate::patch::Patch;
use crate::voice::{process_sample, VoiceSampleContext, VoiceState};

/// One realtime voice with note metadata.
#[derive(Clone, Debug)]
pub struct RtVoice {
    pub state: VoiceState,
    pub note: u8,
    pub freq: f32,
    pub velocity: f32,
    pub gate: bool,
    pub active: bool,
    pub age: u64,
    pub sample_counter: u32,
    pub start_time: f32,
}

impl RtVoice {
    pub fn new(patch: &Patch) -> Self {
        Self {
            state: VoiceState::new(patch),
            note: 0,
            freq: 0.0,
            velocity: 0.0,
            gate: false,
            active: false,
            age: 0,
            sample_counter: 0,
            start_time: 0.0,
        }
    }

    pub fn trigger(&mut self, patch: &Patch, note: u8, freq: f32, velocity: f32, start_time: f32) {
        self.state.reset(patch);
        self.note = note;
        self.freq = freq;
        self.velocity = velocity;
        self.gate = true;
        self.active = true;
        self.sample_counter = 0;
        self.start_time = start_time;
    }

    pub fn release(&mut self) {
        self.gate = false;
    }

    pub fn is_audible(&self) -> bool {
        self.active && (self.gate || self.state.env_level > 1e-5)
    }

    pub fn process_sample(
        &mut self,
        bank: &crate::wavetable::WavetableBank,
        patch: &Patch,
        global_time: f32,
        dt: f32,
        sr: f32,
    ) -> f32 {
        if !self.active {
            return 0.0;
        }

        let ctx = VoiceSampleContext {
            bank,
            patch,
            freq: self.freq,
            gate: self.gate,
            velocity: self.velocity,
            time: global_time - self.start_time,
            sample_index: self.sample_counter,
            dt,
            sr,
        };
        let sample = process_sample(&mut self.state, &ctx);
        self.sample_counter = self.sample_counter.wrapping_add(1);

        if !self.gate && self.state.env_level <= 1e-6 && self.state.env_stage == 3 {
            self.active = false;
        }

        sample
    }
}
