//! Q&A — 4-bar sequence clip offline render.

use reelsynth::patch::Patch;
use reelsynth::sequence::schema::{Clip, MidiNote, SequenceProject};
use reelsynth::sequence::{SchedEvent, SequencerRuntime};
use reelsynth::SynthEngine;

use super::helpers::*;

/// Pin: transport play + begin_buffer must emit NoteOn at the expected sample frame.
#[test]
fn transport_play_emits_note_on_at_expected_frame() {
    let mut project = SequenceProject::default_template();
    project.bpm = 120.0;
    project.loop_region.enabled = false;
    // Note at beat 1.0 → at 120 BPM, 1 beat = 0.5s → frame 22050 @ 44.1kHz
    project.tracks[0].clips.push(Clip {
        start_beats: 0.0,
        length_beats: 8.0,
        notes: vec![MidiNote {
            pitch: 60,
            start_beats: 1.0,
            duration_beats: 0.25,
            velocity: 0.9,
        }],
        r#loop: false,
        automation: vec![],
    });

    let sr = 44_100.0_f32;
    let mut seq = SequencerRuntime::new(120.0);
    seq.sync_from_project(&project);
    seq.seek_playhead(0.0);
    seq.transport_play();

    // Cover beats 0..2 in one buffer (~1 second).
    let frames = (sr as usize) * 1;
    seq.begin_buffer(&project, frames, sr);

    let mut found_on: Option<usize> = None;
    let mut found_off = false;
    for frame in 0..frames {
        for ev in seq.events_at_frame(frame) {
            match ev {
                SchedEvent::NoteOn {
                    note: 60,
                    sample_offset,
                    ..
                } => found_on = Some(sample_offset),
                SchedEvent::NoteOff { note: 60, .. } => found_off = true,
                _ => {}
            }
        }
    }

    let on_frame = found_on.expect("scheduled NoteOn for pitch 60");
    let expected = (1.0 / ((120.0 / 60.0) / sr)).round() as usize; // beat 1.0 → samples
    assert!(
        (on_frame as i64 - expected as i64).abs() <= 2,
        "NoteOn at frame {on_frame}, expected ~{expected}"
    );
    assert!(found_off, "scheduled NoteOff for pitch 60");
}

#[test]
fn four_bar_clip_render_finite_output() {
    let mut patch = Patch::default_mono();
    let mut seq = SequenceProject::default_template();
    seq.bpm = 120.0;
    seq.loop_region.enabled = false;
    seq.tracks[0].clips.push(Clip {
        start_beats: 0.0,
        length_beats: 16.0,
        notes: vec![
            MidiNote {
                pitch: 60,
                start_beats: 0.0,
                duration_beats: 0.5,
                velocity: 0.85,
            },
            MidiNote {
                pitch: 64,
                start_beats: 1.0,
                duration_beats: 0.5,
                velocity: 0.8,
            },
            MidiNote {
                pitch: 67,
                start_beats: 2.0,
                duration_beats: 0.5,
                velocity: 0.8,
            },
            MidiNote {
                pitch: 72,
                start_beats: 3.0,
                duration_beats: 1.0,
                velocity: 0.9,
            },
        ],
        r#loop: false,
        automation: vec![],
    });
    patch.sequence = seq;

    let bank = primary_bank_for_patch(&patch);
    let mut engine = SynthEngine::new(bank, patch, QA_SR);
    engine.sequencer_mut().transport_play();
    engine.sequencer_mut().seek_playhead(0.0);

    // 4 bars at 120 BPM = 8 seconds
    let frames = (8.0 * QA_SR as f32) as usize;
    let mut out = vec![0.0f32; frames];
    engine.process(&mut out);

    assert_rms_in_range(&out, 0.001, 0.5);
    assert!(peak(&out) < 2.0, "output should stay bounded");
    assert!(
        out.iter().any(|s| s.abs() > 0.01),
        "sequenced clip should produce audible output"
    );
}
