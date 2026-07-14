//! Live MIDI recorder — capture notes with beat timestamps.

use super::quantize::quantize_notes;
use super::schema::{Clip, MidiNote, QuantizeGrid, SequenceProject};

#[derive(Clone, Debug)]
struct PendingNote {
    pitch: u8,
    start_beats: f32,
    velocity: f32,
}

/// Records live note on/off into a clip on commit.
#[derive(Clone, Debug, Default)]
pub struct MidiRecorder {
    pending: Vec<PendingNote>,
    recorded: Vec<MidiNote>,
    clip_start_beats: f32,
    record_target: Option<RecordTarget>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordTarget {
    pub track: usize,
    pub clip: usize,
}

impl MidiRecorder {
    pub fn arm(&mut self, target: RecordTarget, clip_start_beats: f32) {
        self.record_target = Some(target);
        self.clip_start_beats = clip_start_beats;
        self.pending.clear();
        self.recorded.clear();
    }

    pub fn disarm(&mut self) {
        self.record_target = None;
        self.pending.clear();
    }

    pub fn is_armed(&self) -> bool {
        self.record_target.is_some()
    }

    pub fn target(&self) -> Option<&RecordTarget> {
        self.record_target.as_ref()
    }

    /// Note on at absolute transport beat.
    pub fn note_on(&mut self, beat: f32, pitch: u8, velocity: f32) {
        if self.record_target.is_none() {
            return;
        }
        self.pending.push(PendingNote {
            pitch,
            start_beats: beat - self.clip_start_beats,
            velocity,
        });
    }

    /// Note off at absolute transport beat.
    pub fn note_off(&mut self, beat: f32, pitch: u8) {
        if self.record_target.is_none() {
            return;
        }
        if let Some(idx) = self.pending.iter().position(|n| n.pitch == pitch) {
            let pending = self.pending.remove(idx);
            let rel_end = beat - self.clip_start_beats;
            let duration = (rel_end - pending.start_beats).max(0.0);
            if duration > 0.0 {
                self.recorded.push(MidiNote {
                    pitch: pending.pitch,
                    start_beats: pending.start_beats.max(0.0),
                    duration_beats: duration,
                    velocity: pending.velocity,
                });
            }
        }
    }

    /// Commit recorded notes into project clip with quantize.
    pub fn commit(
        &mut self,
        project: &mut SequenceProject,
        grid: &QuantizeGrid,
    ) -> Option<RecordTarget> {
        let target = self.record_target.take()?;
        for pending in self.pending.drain(..) {
            self.recorded.push(MidiNote {
                pitch: pending.pitch,
                start_beats: pending.start_beats.max(0.0),
                duration_beats: grid.division.beats_per_step(),
                velocity: pending.velocity,
            });
        }
        quantize_notes(&mut self.recorded, grid);

        let track = project.tracks.get_mut(target.track)?;
        let clip = track.clips.get_mut(target.clip)?;
        clip.notes.extend(self.recorded.drain(..));
        Some(target)
    }

    /// Ensure a clip exists for recording at playhead; returns target indices.
    pub fn ensure_clip_at_playhead(
        project: &mut SequenceProject,
        track_idx: usize,
        playhead: f32,
        length_beats: f32,
    ) -> Option<RecordTarget> {
        let track = project.tracks.get_mut(track_idx)?;
        let bar = (playhead / 4.0).floor() * 4.0;
        if let Some((ci, _)) = track
            .clips
            .iter()
            .enumerate()
            .find(|(_, c)| (c.start_beats - bar).abs() < 0.001)
        {
            return Some(RecordTarget {
                track: track_idx,
                clip: ci,
            });
        }
        track.clips.push(Clip::new(bar, length_beats));
        Some(RecordTarget {
            track: track_idx,
            clip: track.clips.len() - 1,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence::schema::QuantizeDivision;

    #[test]
    fn commit_quantized_notes() {
        let mut project = SequenceProject::default_template();
        project.tracks[0].clips.push(Clip::new(0.0, 4.0));
        let mut rec = MidiRecorder::default();
        rec.arm(
            RecordTarget { track: 0, clip: 0 },
            0.0,
        );
        rec.note_on(0.12, 60, 0.9);
        rec.note_off(0.38, 60);
        let grid = QuantizeGrid {
            division: QuantizeDivision::Sixteenth,
            triplet: false,
        };
        rec.commit(&mut project, &grid);
        let notes = &project.tracks[0].clips[0].notes;
        assert_eq!(notes.len(), 1);
        assert!((notes[0].start_beats - 0.0).abs() < 0.01 || (notes[0].start_beats - 0.25).abs() < 0.01);
    }
}
