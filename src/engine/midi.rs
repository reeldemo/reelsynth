//! MIDI-style note events for the realtime engine.

/// Note on/off events routed into [`crate::engine::SynthEngine`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MidiEvent {
    NoteOn { note: u8, velocity: f32 },
    NoteOff { note: u8 },
}

impl MidiEvent {
    pub fn note_on(note: u8, velocity: f32) -> Self {
        Self::NoteOn {
            note,
            velocity: velocity.clamp(0.0, 1.0),
        }
    }

    pub fn note_off(note: u8) -> Self {
        Self::NoteOff { note }
    }
}

/// Equal-temperament frequency for a MIDI note number (A4 = 69).
pub fn note_to_freq(note: u8) -> f32 {
    440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_is_440() {
        assert!((note_to_freq(69) - 440.0).abs() < 1e-3);
    }
}
