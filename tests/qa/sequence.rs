//! Q&A — 4-bar sequence clip offline render.

use reelsynth::patch::Patch;
use reelsynth::sequence::schema::{Clip, MidiNote, SequenceProject};
use reelsynth::SynthEngine;

use super::helpers::*;

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
