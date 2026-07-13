//! MIDI-style note events for the realtime engine.

/// Note on/off and expressive MIDI events routed into [`crate::engine::SynthEngine`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MidiEvent {
    NoteOn {
        channel: u8,
        note: u8,
        velocity: f32,
    },
    NoteOff { channel: u8, note: u8 },
    /// 14-bit pitch wheel mapped to -1..1.
    PitchBend { channel: u8, value: f32 },
    /// Channel aftertouch 0..1.
    ChannelPressure { channel: u8, pressure: f32 },
    /// Poly aftertouch 0..1.
    PolyAftertouch {
        channel: u8,
        note: u8,
        pressure: f32,
    },
    ControlChange {
        channel: u8,
        cc: u8,
        value: f32,
    },
}

impl MidiEvent {
    pub fn note_on(channel: u8, note: u8, velocity: f32) -> Self {
        Self::NoteOn {
            channel,
            note,
            velocity: velocity.clamp(0.0, 1.0),
        }
    }

    pub fn note_off(channel: u8, note: u8) -> Self {
        Self::NoteOff { channel, note }
    }
}

/// Equal-temperament frequency for a MIDI note number (A4 = 69).
pub fn note_to_freq(note: u8) -> f32 {
    440.0 * 2.0f32.powf((note as f32 - 69.0) / 12.0)
}

/// Map 14-bit pitch wheel (0..16383) to -1..1.
pub fn pitch_bend_from_raw(lo: u8, hi: u8) -> f32 {
    let raw = (hi as u16) << 7 | (lo as u16);
    (raw as f32 / 8192.0) - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a4_is_440() {
        assert!((note_to_freq(69) - 440.0).abs() < 1e-3);
    }

    #[test]
    fn pitch_bend_center() {
        assert!(pitch_bend_from_raw(0, 64).abs() < 0.01);
    }
}
