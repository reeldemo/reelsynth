//! MPE zone routing — per-note pitch bend, pressure, timbre (CC74).

/// One MPE zone (lower or upper).
#[derive(Clone, Debug)]
pub struct MpeZone {
    /// Master channel (global messages).
    pub master: u8,
    /// Per-note member channels (1-based MIDI channels).
    pub members: Vec<u8>,
    pub note_lo: u8,
    pub note_hi: u8,
}

impl MpeZone {
    pub fn default_lower() -> Self {
        Self {
            master: 1,
            members: (2..=8).collect(),
            note_lo: 0,
            note_hi: 63,
        }
    }

    pub fn default_upper() -> Self {
        Self {
            master: 16,
            members: (9..=15).collect(),
            note_lo: 64,
            note_hi: 127,
        }
    }

    pub fn contains_note(&self, note: u8) -> bool {
        note >= self.note_lo && note <= self.note_hi
    }

    pub fn is_member_channel(&self, channel: u8) -> bool {
        self.members.contains(&channel)
    }

    pub fn is_master_channel(&self, channel: u8) -> bool {
        channel == self.master
    }
}

/// Dual-zone MPE configuration (enabled by default).
#[derive(Clone, Debug)]
pub struct MpeConfig {
    pub enabled: bool,
    pub lower: MpeZone,
    pub upper: MpeZone,
    /// Pitch bend range in semitones (MPE default 48).
    pub bend_range_semitones: f32,
}

impl Default for MpeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            lower: MpeZone::default_lower(),
            upper: MpeZone::default_upper(),
            bend_range_semitones: 48.0,
        }
    }
}

impl MpeConfig {
    pub fn zone_for_note(&self, note: u8) -> Option<&MpeZone> {
        if !self.enabled {
            return None;
        }
        if self.lower.contains_note(note) {
            Some(&self.lower)
        } else if self.upper.contains_note(note) {
            Some(&self.upper)
        } else {
            None
        }
    }

    pub fn zone_for_channel(&self, channel: u8) -> Option<&MpeZone> {
        if !self.enabled {
            return None;
        }
        if self.lower.is_master_channel(channel) || self.lower.is_member_channel(channel) {
            Some(&self.lower)
        } else if self.upper.is_master_channel(channel) || self.upper.is_member_channel(channel) {
            Some(&self.upper)
        } else {
            None
        }
    }

    pub fn is_global_channel(&self, channel: u8) -> bool {
        if !self.enabled {
            return channel == 0;
        }
        self.lower.is_master_channel(channel) || self.upper.is_master_channel(channel)
    }
}

/// Per-note expressive state carried on a voice.
#[derive(Clone, Copy, Debug, Default)]
pub struct VoiceMpe {
    pub channel: u8,
    pub pitch_bend: f32,
    pub pressure: f32,
    pub timbre: f32,
}

impl VoiceMpe {
    pub fn pitch_bend_semitones(&self, bend_range: f32) -> f32 {
        self.pitch_bend * bend_range
    }
}

/// Channel-level expressive state (master channel CCs).
#[derive(Clone, Copy, Debug, Default)]
pub struct GlobalMidi {
    pub modwheel: f32,
    pub pitch_bend: f32,
}

/// Per-member-channel expressive state for active notes.
#[derive(Clone, Debug, Default)]
pub struct MpeState {
    pub config: MpeConfig,
    pub global: GlobalMidi,
    /// Per-channel pitch bend (-1..1).
    pub channel_bend: [f32; 16],
    /// Per-channel pressure (0..1).
    pub channel_pressure: [f32; 16],
    /// Per-channel timbre / CC74 (0..1).
    pub channel_timbre: [f32; 16],
}

impl MpeState {
    pub fn new() -> Self {
        Self {
            config: MpeConfig::default(),
            ..Default::default()
        }
    }

    pub fn set_pitch_bend(&mut self, channel: u8, value: f32) {
        let ch = channel.min(15) as usize;
        let v = value.clamp(-1.0, 1.0);
        if self.config.is_global_channel(channel) {
            self.global.pitch_bend = v;
        } else {
            self.channel_bend[ch] = v;
        }
    }

    pub fn set_pressure(&mut self, channel: u8, value: f32) {
        let ch = channel.min(15) as usize;
        self.channel_pressure[ch] = value.clamp(0.0, 1.0);
    }

    pub fn set_timbre(&mut self, channel: u8, value: f32) {
        let ch = channel.min(15) as usize;
        self.channel_timbre[ch] = value.clamp(0.0, 1.0);
    }

    pub fn set_modwheel(&mut self, value: f32) {
        self.global.modwheel = value.clamp(0.0, 1.0);
    }

    pub fn voice_mpe(&self, channel: u8) -> VoiceMpe {
        let ch = channel.min(15) as usize;
        VoiceMpe {
            channel,
            pitch_bend: self.channel_bend[ch],
            pressure: self.channel_pressure[ch],
            timbre: self.channel_timbre[ch],
        }
    }

    pub fn modwheel(&self) -> f32 {
        self.global.modwheel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zones_partition_notes() {
        let cfg = MpeConfig::default();
        assert!(cfg.lower.contains_note(60));
        assert!(cfg.upper.contains_note(80));
        assert!(!cfg.lower.contains_note(80));
    }

    #[test]
    fn bend_converts_to_semitones() {
        let mpe = VoiceMpe {
            pitch_bend: 0.5,
            ..Default::default()
        };
        assert!((mpe.pitch_bend_semitones(48.0) - 24.0).abs() < 1e-3);
    }
}
