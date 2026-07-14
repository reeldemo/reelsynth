//! Sequence engine unit tests.

#[cfg(test)]
mod scheduler_tests {
    use crate::sequence::clock::BeatRange;
    use crate::sequence::schema::{Clip, MidiNote, SequenceProject};
    use crate::sequence::scheduler::{NoteScheduler, SchedEvent};
    use crate::sequence::transport::TransportState;

    #[test]
    fn scheduler_note_on_off_in_window() {
        let mut project = SequenceProject::default_template();
        project.tracks[0].clips.push(Clip {
            start_beats: 0.0,
            length_beats: 8.0,
            notes: vec![MidiNote {
                pitch: 64,
                start_beats: 2.0,
                duration_beats: 1.0,
                velocity: 0.7,
            }],
            r#loop: false,
            automation: vec![],
        });

        let transport = TransportState {
            playing: true,
            bpm: 120.0,
            ..Default::default()
        };
        let range = BeatRange {
            start_beats: 1.5,
            end_beats: 3.5,
            beats_per_sample: (120.0 / 60.0) / 48000.0,
        };
        let mut sched = NoteScheduler::new();
        let events = sched.process(&project, &transport, range, 512);
        assert!(events.iter().any(|e| matches!(
            e,
            SchedEvent::NoteOn { note: 64, .. }
        )));
        assert!(events.iter().any(|e| matches!(
            e,
            SchedEvent::NoteOff { note: 64, .. }
        )));
    }

    #[test]
    fn muted_track_silent() {
        let mut project = SequenceProject::default_template();
        project.tracks[0].mute = true;
        project.tracks[0].clips.push(Clip {
            start_beats: 0.0,
            length_beats: 4.0,
            notes: vec![MidiNote {
                pitch: 60,
                start_beats: 0.0,
                duration_beats: 1.0,
                velocity: 1.0,
            }],
            r#loop: false,
            automation: vec![],
        });
        let transport = TransportState {
            playing: true,
            bpm: 120.0,
            ..Default::default()
        };
        let range = BeatRange {
            start_beats: 0.0,
            end_beats: 2.0,
            beats_per_sample: 0.001,
        };
        let mut sched = NoteScheduler::new();
        let events = sched.process(&project, &transport, range, 128);
        assert!(events.is_empty());
    }
}

#[cfg(test)]
mod quantize_tests {
    use crate::sequence::quantize::{quantize_note, snap_beat};
    use crate::sequence::schema::{MidiNote, QuantizeDivision, QuantizeGrid};

    #[test]
    fn eighth_grid() {
        let grid = QuantizeGrid {
            division: QuantizeDivision::Eighth,
            triplet: false,
        };
        assert!((snap_beat(0.7, &grid) - 0.5).abs() < 1e-5);
    }

    #[test]
    fn triplet_flag() {
        let grid = QuantizeGrid {
            division: QuantizeDivision::Eighth,
            triplet: true,
        };
        let note = MidiNote {
            pitch: 48,
            start_beats: 0.2,
            duration_beats: 0.4,
            velocity: 0.8,
        };
        let q = quantize_note(&note, &grid);
        let step = QuantizeDivision::EighthTriplet.beats_per_step();
        assert!((q.start_beats / step).fract().abs() < 1e-4);
    }
}

#[cfg(test)]
mod loop_tests {
    use crate::sequence::clock::SampleClock;
    use crate::sequence::transport::TransportState;

    #[test]
    fn playhead_wraps_at_loop_end() {
        let clock = SampleClock;
        let mut t = TransportState {
            playing: true,
            bpm: 120.0,
            playhead_beats: 15.9,
            loop_start: 0.0,
            loop_end: 16.0,
            loop_enabled: true,
            ..Default::default()
        };
        clock.tick(&mut t, 8192, 44100.0);
        assert!(t.playhead_beats < 2.0);
    }
}
