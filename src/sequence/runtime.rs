//! Sequencer runtime — transport, clock, scheduler, recorder.

use super::automation::compute_automation_mods;
use super::clock::SampleClock;
use super::recorder::MidiRecorder;
use super::scheduler::{NoteScheduler, SchedEvent, SEQ_CHANNEL};
use super::schema::SequenceProject;
use super::transport::TransportState;
use std::collections::HashMap;

/// Callback trait for scheduled MIDI into the synth engine (extension point).
pub trait SequencerEngineSink {
    fn seq_note_on(&mut self, channel: u8, note: u8, velocity: f32);
    fn seq_note_off(&mut self, channel: u8, note: u8);
}

/// Combined sequencer state owned by `SynthEngine`.
#[derive(Clone, Debug)]
pub struct SequencerRuntime {
    pub transport: TransportState,
    clock: SampleClock,
    scheduler: NoteScheduler,
    pub recorder: MidiRecorder,
    pending_events: Vec<SchedEvent>,
    event_cursor: usize,
}

impl Default for SequencerRuntime {
    fn default() -> Self {
        Self::new(120.0)
    }
}

impl SequencerRuntime {
    pub fn new(bpm: f32) -> Self {
        Self {
            transport: TransportState::new(bpm),
            clock: SampleClock,
            scheduler: NoteScheduler::new(),
            recorder: MidiRecorder::default(),
            pending_events: Vec::new(),
            event_cursor: 0,
        }
    }

    pub fn sync_from_project(&mut self, project: &SequenceProject) {
        self.transport.bpm = project.bpm;
        self.transport.loop_start = project.loop_region.start_beats;
        self.transport.loop_end = project.loop_region.end_beats;
        self.transport.loop_enabled = project.loop_region.enabled;
    }

    pub fn transport_play(&mut self) {
        self.transport.play();
    }

    pub fn transport_stop(&mut self) {
        self.scheduler.clear_active();
        self.transport.stop();
    }

    pub fn transport_record(
        &mut self,
        project: &mut SequenceProject,
        track_idx: Option<usize>,
    ) {
        self.transport.start_record();
        if let Some(ti) = track_idx.or_else(|| project.armed_track_index()) {
            if let Some(target) = MidiRecorder::ensure_clip_at_playhead(
                project,
                ti,
                self.transport.playhead_beats,
                4.0,
            ) {
                let clip_start = project.tracks[ti]
                    .clips
                    .get(target.clip)
                    .map(|c| c.start_beats)
                    .unwrap_or(0.0);
                self.recorder.arm(target, clip_start);
            }
        }
    }

    pub fn set_bpm(&mut self, bpm: f32) {
        self.transport.set_bpm(bpm);
    }

    pub fn seek_playhead(&mut self, beats: f32) {
        self.transport.seek(beats);
    }

    /// Stop recording and commit notes to clip with project quantize grid.
    pub fn stop_record_and_commit(&mut self, project: &mut SequenceProject) -> bool {
        let was_recording = self.transport.recording;
        self.transport.recording = false;
        if was_recording && self.recorder.is_armed() {
            let quantize = project.quantize.clone();
            self.recorder.commit(project, &quantize);
            return true;
        }
        false
    }

    /// Prepare scheduler events for the next audio buffer.
    pub fn begin_buffer(
        &mut self,
        project: &SequenceProject,
        frames: usize,
        sample_rate: f32,
    ) {
        let range = self
            .clock
            .tick(&mut self.transport, frames, sample_rate);
        self.pending_events = self.scheduler.process(
            project,
            &self.transport,
            range,
            frames,
        );
        self.event_cursor = 0;
    }

    /// Fire events due at `frame`; returns events to dispatch.
    pub fn events_at_frame(&mut self, frame: usize) -> Vec<SchedEvent> {
        let mut fired = Vec::new();
        while self.event_cursor < self.pending_events.len() {
            let ev = &self.pending_events[self.event_cursor];
            let offset = match ev {
                SchedEvent::NoteOn { sample_offset, .. }
                | SchedEvent::NoteOff { sample_offset, .. } => *sample_offset,
            };
            if offset > frame {
                break;
            }
            fired.push(self.pending_events[self.event_cursor].clone());
            self.event_cursor += 1;
        }
        fired
    }

    pub fn stop_note_offs(&self) -> Vec<SchedEvent> {
        self.scheduler.all_note_offs()
    }

    /// Live performance note while recording.
    pub fn live_note_on(&mut self, pitch: u8, velocity: f32) {
        if self.transport.recording && self.recorder.is_armed() {
            self.recorder
                .note_on(self.transport.playhead_beats, pitch, velocity);
        }
    }

    pub fn live_note_off(&mut self, pitch: u8) {
        if self.transport.recording && self.recorder.is_armed() {
            self.recorder
                .note_off(self.transport.playhead_beats, pitch);
        }
    }

    pub fn automation_mods(&self, project: &SequenceProject) -> HashMap<String, f32> {
        compute_automation_mods(project, &self.transport)
    }

    pub fn seq_channel() -> u8 {
        SEQ_CHANNEL
    }
}
