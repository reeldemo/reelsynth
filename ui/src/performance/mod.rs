//! UI-facing performance settings (indices for dropdowns + labels).

mod arp_panel;
mod chord_grid;
mod header;

pub use arp_panel::{draw_arp_panel, ArpPanelActions};
pub use chord_grid::{draw_chord_grid, ChordGridActions};
pub use header::{draw_performance_header, PerformanceHeaderActions};

use reelsynth::{
    ArpDirection, ArpInputMode, ArpRate, ArpSettings, ChordSet, ChordVoicing, PerformanceLayout,
    PerformanceSettings, Scale, ScaleBehavior,
};

pub const INPUT_MODE_NAMES: &[&str] = &["Single", "Chord", "Scale"];
pub const STYLE_NAMES: &[&str] = &[
    "Up",
    "Down",
    "Up-Down",
    "Down-Up",
    "Random",
    "As Played",
    "Converge",
];
pub const RATE_NAMES: &[&str] = &["1/4", "1/8", "1/16", "1/32", "1/8T", "1/16T"];

pub const ROOT_NAMES: &[&str] = &[
    "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
];

pub const SCALE_NAMES: &[&str] = &[
    "Major",
    "Minor",
    "Natural Minor",
    "Harmonic Minor",
    "Melodic Minor",
    "Dorian",
    "Phrygian",
    "Lydian",
    "Mixolydian",
    "Locrian",
    "Major Pent",
    "Minor Pent",
    "Blues",
    "Whole Tone",
    "Chromatic",
];

pub const LAYOUT_NAMES: &[&str] = &["Piano", "Scale", "Chords"];

pub const CHORD_DEGREE_LABELS: &[&str] = &["I", "ii", "iii", "IV", "V", "vi", "vii°"];

/// Mirror of [`ArpSettings`] using dropdown indices.
#[derive(Clone, Debug, PartialEq)]
pub struct ArpUi {
    pub enabled: bool,
    pub input_mode: usize,
    pub direction: usize,
    pub rate: usize,
    pub gate: f32,
    pub octave_spread: u8,
    pub latch: bool,
}

impl Default for ArpUi {
    fn default() -> Self {
        Self {
            enabled: false,
            input_mode: 0,
            direction: 0,
            rate: 2,
            gate: 0.85,
            octave_spread: 1,
            latch: false,
        }
    }
}

impl ArpUi {
    pub fn from_settings(s: &ArpSettings) -> Self {
        Self {
            enabled: s.enabled,
            input_mode: arp_input_index(s.input_mode),
            direction: arp_direction_index(s.direction),
            rate: arp_rate_index(s.rate),
            gate: s.gate,
            octave_spread: s.octave_spread.clamp(1, 4),
            latch: s.latch,
        }
    }

    pub fn to_settings(&self) -> ArpSettings {
        ArpSettings {
            enabled: self.enabled,
            input_mode: arp_input_from_index(self.input_mode),
            direction: arp_direction_from_index(self.direction),
            rate: arp_rate_from_index(self.rate),
            gate: self.gate.clamp(0.05, 1.0),
            octave_spread: self.octave_spread.clamp(1, 4),
            latch: self.latch,
        }
    }
}

pub fn arp_input_index(mode: ArpInputMode) -> usize {
    match mode {
        ArpInputMode::SingleNote => 0,
        ArpInputMode::HeldChord => 1,
        ArpInputMode::ScaleDegrees => 2,
    }
}

pub fn arp_input_from_index(idx: usize) -> ArpInputMode {
    match idx {
        1 => ArpInputMode::HeldChord,
        2 => ArpInputMode::ScaleDegrees,
        _ => ArpInputMode::SingleNote,
    }
}

pub fn arp_direction_index(dir: ArpDirection) -> usize {
    match dir {
        ArpDirection::Up => 0,
        ArpDirection::Down => 1,
        ArpDirection::UpDown => 2,
        ArpDirection::DownUp => 3,
        ArpDirection::Random => 4,
        ArpDirection::AsPlayed => 5,
        ArpDirection::Converge => 6,
    }
}

pub fn arp_direction_from_index(idx: usize) -> ArpDirection {
    match idx {
        1 => ArpDirection::Down,
        2 => ArpDirection::UpDown,
        3 => ArpDirection::DownUp,
        4 => ArpDirection::Random,
        5 => ArpDirection::AsPlayed,
        6 => ArpDirection::Converge,
        _ => ArpDirection::Up,
    }
}

pub fn arp_rate_index(rate: ArpRate) -> usize {
    match rate {
        ArpRate::Quarter => 0,
        ArpRate::Eighth => 1,
        ArpRate::Sixteenth => 2,
        ArpRate::ThirtySecond => 3,
        ArpRate::EighthTriplet => 4,
        ArpRate::SixteenthTriplet => 5,
    }
}

pub fn arp_rate_from_index(idx: usize) -> ArpRate {
    match idx {
        1 => ArpRate::Eighth,
        2 => ArpRate::Sixteenth,
        3 => ArpRate::ThirtySecond,
        4 => ArpRate::EighthTriplet,
        5 => ArpRate::SixteenthTriplet,
        _ => ArpRate::Quarter,
    }
}

/// Mirror of [`PerformanceSettings`] using dropdown indices in [`UiState`].
#[derive(Clone, Debug, PartialEq)]
pub struct PerformanceUi {
    pub root: usize,
    pub scale: usize,
    pub layout: usize,
    pub chord_set: usize,
    pub voicing: usize,
    pub base_octave: i8,
    pub scale_behavior: ScaleBehavior,
    pub arp: ArpUi,
}

impl Default for PerformanceUi {
    fn default() -> Self {
        Self {
            root: 0,
            scale: 0,
            layout: 0,
            chord_set: 0,
            voicing: 0,
            base_octave: 4,
            scale_behavior: ScaleBehavior::default(),
            arp: ArpUi::default(),
        }
    }
}

impl PerformanceUi {
    pub fn from_settings(s: &PerformanceSettings) -> Self {
        Self {
            root: s.root.min(11) as usize,
            scale: scale_index(s.scale),
            layout: layout_index(s.layout),
            chord_set: match s.chord_set {
                ChordSet::Triads => 0,
                ChordSet::Sevenths => 1,
            },
            voicing: match s.voicing {
                ChordVoicing::Close => 0,
                ChordVoicing::Spread => 1,
                ChordVoicing::Root => 2,
            },
            base_octave: s.base_octave,
            scale_behavior: s.scale_behavior,
            arp: ArpUi::from_settings(&s.arp),
        }
    }

    pub fn to_settings(&self) -> PerformanceSettings {
        PerformanceSettings {
            root: self.root.min(ROOT_NAMES.len().saturating_sub(1)) as u8,
            scale: scale_from_index(self.scale),
            scale_behavior: self.scale_behavior,
            layout: layout_from_index(self.layout),
            chord_set: if self.chord_set == 1 {
                ChordSet::Sevenths
            } else {
                ChordSet::Triads
            },
            voicing: match self.voicing {
                1 => ChordVoicing::Spread,
                2 => ChordVoicing::Root,
                _ => ChordVoicing::Close,
            },
            base_octave: self.base_octave,
            arp: self.arp.to_settings(),
        }
    }
}

pub fn scale_index(scale: Scale) -> usize {
    match scale {
        Scale::Major => 0,
        Scale::Minor => 1,
        Scale::NaturalMinor => 2,
        Scale::HarmonicMinor => 3,
        Scale::MelodicMinor => 4,
        Scale::Dorian => 5,
        Scale::Phrygian => 6,
        Scale::Lydian => 7,
        Scale::Mixolydian => 8,
        Scale::Locrian => 9,
        Scale::MajorPent => 10,
        Scale::MinorPent => 11,
        Scale::Blues => 12,
        Scale::WholeTone => 13,
        Scale::Chromatic => 14,
    }
}

pub fn scale_from_index(idx: usize) -> Scale {
    match idx {
        1 => Scale::Minor,
        2 => Scale::NaturalMinor,
        3 => Scale::HarmonicMinor,
        4 => Scale::MelodicMinor,
        5 => Scale::Dorian,
        6 => Scale::Phrygian,
        7 => Scale::Lydian,
        8 => Scale::Mixolydian,
        9 => Scale::Locrian,
        10 => Scale::MajorPent,
        11 => Scale::MinorPent,
        12 => Scale::Blues,
        13 => Scale::WholeTone,
        14 => Scale::Chromatic,
        _ => Scale::Major,
    }
}

pub fn layout_index(layout: PerformanceLayout) -> usize {
    match layout {
        PerformanceLayout::Piano => 0,
        PerformanceLayout::Scale => 1,
        PerformanceLayout::Chords => 2,
    }
}

pub fn layout_from_index(idx: usize) -> PerformanceLayout {
    match idx {
        1 => PerformanceLayout::Scale,
        2 => PerformanceLayout::Chords,
        _ => PerformanceLayout::Piano,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn performance_ui_roundtrip() {
        let settings = PerformanceSettings {
            root: 7,
            scale: Scale::Dorian,
            layout: PerformanceLayout::Chords,
            arp: ArpSettings {
                enabled: true,
                input_mode: ArpInputMode::ScaleDegrees,
                direction: ArpDirection::UpDown,
                rate: ArpRate::Sixteenth,
                ..ArpSettings::default()
            },
            ..PerformanceSettings::default()
        };
        let ui = PerformanceUi::from_settings(&settings);
        let back = ui.to_settings();
        assert_eq!(back.root, 7);
        assert_eq!(back.scale, Scale::Dorian);
        assert_eq!(back.layout, PerformanceLayout::Chords);
        assert!(back.arp.enabled);
        assert_eq!(back.arp.input_mode, ArpInputMode::ScaleDegrees);
        assert_eq!(back.arp.direction, ArpDirection::UpDown);
        assert_eq!(back.arp.rate, ArpRate::Sixteenth);
    }
}
