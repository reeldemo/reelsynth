//! SMF Type-1 MIDI writer from `SequenceProject`.

use crate::export::ExportReport;
use crate::patch::Patch;
use crate::sequence::schema::{MidiNote, Track};
use std::path::Path;

const TICKS_PER_QUARTER: u16 = 480;

pub fn export_midi(preset: &Patch, out_path: &Path, opts: &crate::export::ExportOptions) -> ExportReport {
    let _ = opts;
    let project = &preset.sequence;
    if project.tracks.is_empty() {
        return export_demo_fallback(out_path, opts);
    }

    let tracks: Vec<Vec<u8>> = project
        .tracks
        .iter()
        .map(|track| build_arrangement_track(track, project.bpm))
        .collect();

    let has_notes = project.tracks.iter().any(|t| t.clips.iter().any(|c| !c.notes.is_empty()));
    if !has_notes {
        return export_demo_fallback(out_path, opts);
    }

    let file = build_smf_type1(&tracks, TICKS_PER_QUARTER);
    write_bytes(out_path, file)
}

fn export_demo_fallback(out_path: &Path, opts: &crate::export::ExportOptions) -> ExportReport {
    let duration_ticks = (opts.duration * TICKS_PER_QUARTER as f32) as u32;
    let track = build_single_note_track(opts.midi_note, duration_ticks);
    let file = build_smf_type1(&[track], TICKS_PER_QUARTER);
    write_bytes(out_path, file)
}

fn write_bytes(out_path: &Path, file: Vec<u8>) -> ExportReport {
    if let Some(parent) = out_path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            return ExportReport::fail("midi", e.to_string());
        }
    }
    match std::fs::write(out_path, file) {
        Ok(()) => ExportReport::ok("midi", out_path.display().to_string()),
        Err(e) => ExportReport::fail("midi", e.to_string()),
    }
}

fn build_arrangement_track(track: &Track, bpm: f32) -> Vec<u8> {
    let mut events: Vec<(u32, Vec<u8>)> = Vec::new();

    let us_per_quarter = (60_000_000.0 / bpm.max(1.0)) as u32;
    events.push((
        0,
        vec![
            0xFF,
            0x51,
            0x03,
            ((us_per_quarter >> 16) & 0xFF) as u8,
            ((us_per_quarter >> 8) & 0xFF) as u8,
            (us_per_quarter & 0xFF) as u8,
        ],
    ));

    for clip in &track.clips {
        let clip_start_ticks = beats_to_ticks(clip.start_beats);
        for note in &clip.notes {
            push_note_events(&mut events, note, clip_start_ticks);
        }
    }

    events.sort_by_key(|(t, _)| *t);

    let mut track_data = Vec::new();
    let mut prev_tick = 0u32;
    for (tick, data) in events {
        let delta = tick.saturating_sub(prev_tick);
        track_data.extend(vlq(delta));
        track_data.extend(data);
        prev_tick = tick;
    }
    track_data.extend(vlq(0));
    track_data.extend([0xFF, 0x2F, 0x00]);

    let mut track = Vec::new();
    track.extend(b"MTrk");
    track.extend(&(track_data.len() as u32).to_be_bytes());
    track.extend(track_data);
    track
}

fn push_note_events(events: &mut Vec<(u32, Vec<u8>)>, note: &MidiNote, clip_start_ticks: u32) {
    let start = clip_start_ticks + beats_to_ticks(note.start_beats);
    let end = start + beats_to_ticks(note.duration_beats);
    let vel = (note.velocity.clamp(0.0, 1.0) * 127.0).round() as u8;
    events.push((start, vec![0x90, note.pitch, vel.max(1)]));
    events.push((end, vec![0x80, note.pitch, 0x00]));
}

fn build_single_note_track(note: u8, duration_ticks: u32) -> Vec<u8> {
    let mut events = Vec::new();
    events.extend(vlq(0));
    events.extend([0x90, note, 0x64]);
    events.extend(vlq(duration_ticks));
    events.extend([0x80, note, 0x00]);
    events.extend(vlq(0));
    events.extend([0xFF, 0x2F, 0x00]);

    let mut track = Vec::new();
    track.extend(b"MTrk");
    track.extend(&(events.len() as u32).to_be_bytes());
    track.extend(events);
    track
}

fn build_smf_type1(tracks: &[Vec<u8>], ticks_per_quarter: u16) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend(b"MThd");
    out.extend(&6u32.to_be_bytes());
    out.extend(&1u16.to_be_bytes()); // type 1
    out.extend(&(tracks.len() as u16).to_be_bytes());
    out.extend(&ticks_per_quarter.to_be_bytes());
    for track in tracks {
        out.extend(track);
    }
    out
}

fn beats_to_ticks(beats: f32) -> u32 {
    (beats * TICKS_PER_QUARTER as f32).round() as u32
}

fn vlq(value: u32) -> Vec<u8> {
    let mut buffer = value;
    let mut bytes = Vec::new();
    bytes.push((buffer & 0x7F) as u8);
    buffer >>= 7;
    while buffer > 0 {
        bytes.insert(0, ((buffer & 0x7F) as u8) | 0x80);
        buffer >>= 7;
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence::schema::{Clip, SequenceProject};

    #[test]
    fn type1_header() {
        let mut project = SequenceProject::default_template();
        project.tracks[0].clips.push(Clip {
            start_beats: 0.0,
            length_beats: 4.0,
            notes: vec![crate::sequence::MidiNote {
                pitch: 60,
                start_beats: 0.0,
                duration_beats: 1.0,
                velocity: 0.8,
            }],
            r#loop: false,
            automation: vec![],
        });
        let mut patch = Patch::default_mono();
        patch.sequence = project;
        let dir = std::env::temp_dir().join("reelsynth_midi_export");
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("test.mid");
        let report = export_midi(&patch, &path, &crate::export::ExportOptions::default());
        assert!(report.success);
        let data = std::fs::read(&path).unwrap();
        assert_eq!(&data[0..4], b"MThd");
        assert_eq!(u16::from_be_bytes([data[8], data[9]]), 1); // SMF type 1
        assert_eq!(u16::from_be_bytes([data[10], data[11]]), 4); // four tracks
    }
}
