//! Quantize note starts and durations to grid.

use super::schema::{MidiNote, QuantizeGrid, QuantizeDivision};

/// Snap a beat position to the nearest grid line.
pub fn snap_beat(beat: f32, grid: &QuantizeGrid) -> f32 {
    let step = effective_step(grid);
    if step <= 0.0 {
        return beat;
    }
    (beat / step).round() * step
}

/// Quantize note start and duration (minimum one step).
pub fn quantize_note(note: &MidiNote, grid: &QuantizeGrid) -> MidiNote {
    let step = effective_step(grid);
    let start = snap_beat(note.start_beats, grid);
    let mut duration = snap_beat(note.duration_beats, grid).max(step);
    if duration < step {
        duration = step;
    }
    MidiNote {
        pitch: note.pitch,
        start_beats: start.max(0.0),
        duration_beats: duration,
        velocity: note.velocity,
    }
}

/// Quantize all notes in place.
pub fn quantize_notes(notes: &mut [MidiNote], grid: &QuantizeGrid) {
    for note in notes.iter_mut() {
        *note = quantize_note(note, grid);
    }
}

fn effective_step(grid: &QuantizeGrid) -> f32 {
    let mut div = grid.division;
    if grid.triplet {
        div = match div {
            QuantizeDivision::Quarter => QuantizeDivision::EighthTriplet,
            QuantizeDivision::Eighth => QuantizeDivision::EighthTriplet,
            QuantizeDivision::Sixteenth => QuantizeDivision::SixteenthTriplet,
            other => other,
        };
    }
    div.beats_per_step()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snaps_to_sixteenth() {
        let grid = QuantizeGrid {
            division: QuantizeDivision::Sixteenth,
            triplet: false,
        };
        assert!((snap_beat(0.13, &grid) - 0.25).abs() < 1e-5);
        assert!((snap_beat(0.12, &grid) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn quantize_note_minimum_duration() {
        let grid = QuantizeGrid::default();
        let note = MidiNote {
            pitch: 60,
            start_beats: 0.1,
            duration_beats: 0.05,
            velocity: 0.9,
        };
        let q = quantize_note(&note, &grid);
        assert!((q.start_beats - 0.0).abs() < 1e-5 || (q.start_beats - 0.25).abs() < 1e-5);
        assert!(q.duration_beats >= 0.25);
    }
}
