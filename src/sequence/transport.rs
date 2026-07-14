//! Transport state: play / record / playhead / loop.

use serde::{Deserialize, Serialize};

/// Shared transport snapshot (readable from UI thread).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct TransportState {
    pub playing: bool,
    pub recording: bool,
    pub bpm: f32,
    pub playhead_beats: f32,
    pub loop_start: f32,
    pub loop_end: f32,
    pub loop_enabled: bool,
}

impl TransportState {
    pub fn new(bpm: f32) -> Self {
        Self {
            bpm,
            loop_end: 16.0,
            loop_enabled: true,
            ..Default::default()
        }
    }

    pub fn play(&mut self) {
        self.playing = true;
    }

    pub fn stop(&mut self) {
        self.playing = false;
        self.recording = false;
    }

    pub fn start_record(&mut self) {
        self.recording = true;
        self.playing = true;
    }

    pub fn seek(&mut self, beats: f32) {
        self.playhead_beats = beats.max(0.0);
    }

    pub fn set_bpm(&mut self, bpm: f32) {
        self.bpm = bpm.max(20.0).min(999.0);
    }

    pub fn wrap_loop(&mut self) {
        if !self.loop_enabled || self.loop_end <= self.loop_start {
            return;
        }
        let len = self.loop_end - self.loop_start;
        while self.playhead_beats >= self.loop_end {
            self.playhead_beats = self.loop_start + (self.playhead_beats - self.loop_end) % len;
        }
    }
}
