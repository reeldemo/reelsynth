//! Sequence / arrangement data model for Compose mode.

use serde::{Deserialize, Serialize};

/// One MIDI note in beat time.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct MidiNote {
    pub pitch: u8,
    pub start_beats: f32,
    pub duration_beats: f32,
    #[serde(default = "default_velocity")]
    pub velocity: f32,
}

fn default_velocity() -> f32 {
    0.8
}

/// Arrangement clip holding note data.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Clip {
    pub start_beats: f32,
    pub length_beats: f32,
    #[serde(default)]
    pub notes: Vec<MidiNote>,
    #[serde(default)]
    pub r#loop: bool,
    #[serde(default)]
    pub automation: Vec<AutomationLane>,
}

impl Clip {
    pub fn new(start_beats: f32, length_beats: f32) -> Self {
        Self {
            start_beats,
            length_beats,
            notes: Vec::new(),
            r#loop: false,
            automation: Vec::new(),
        }
    }
}

/// Reference to a clip slot in a scene.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct ClipRef {
    pub track: usize,
    pub clip: usize,
}

/// One arrangement / session track.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Track {
    pub name: String,
    #[serde(default)]
    pub mute: bool,
    #[serde(default)]
    pub solo: bool,
    #[serde(default)]
    pub arm: bool,
    #[serde(default)]
    pub clips: Vec<Clip>,
    /// Optional oscillator index override (uses patch sound when None).
    #[serde(default)]
    pub target_osc: Option<usize>,
}

impl Track {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            mute: false,
            solo: false,
            arm: false,
            clips: Vec::new(),
            target_osc: None,
        }
    }
}

/// Scene slot grid — one optional clip per track.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Scene {
    pub name: String,
    #[serde(default)]
    pub slots: Vec<Option<ClipRef>>,
}

impl Scene {
    pub fn new(name: impl Into<String>, track_count: usize) -> Self {
        Self {
            name: name.into(),
            slots: vec![None; track_count],
        }
    }
}

/// Automation breakpoint in beat time (value 0..1).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AutomationPoint {
    pub beats: f32,
    pub value: f32,
}

/// Mod-matrix target automation lane.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct AutomationLane {
    pub target: String,
    #[serde(default)]
    pub points: Vec<AutomationPoint>,
}

/// Quantize grid division for record / edit snap.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum QuantizeDivision {
    #[default]
    Quarter,
    Eighth,
    Sixteenth,
    EighthTriplet,
    SixteenthTriplet,
}

impl QuantizeDivision {
    pub fn beats_per_step(&self) -> f32 {
        match self {
            Self::Quarter => 1.0,
            Self::Eighth => 0.5,
            Self::Sixteenth => 0.25,
            Self::EighthTriplet => 1.0 / 3.0,
            Self::SixteenthTriplet => 1.0 / 6.0,
        }
    }
}

/// Active quantize grid settings.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct QuantizeGrid {
    #[serde(default)]
    pub division: QuantizeDivision,
    #[serde(default)]
    pub triplet: bool,
}

impl Default for QuantizeGrid {
    fn default() -> Self {
        Self {
            division: QuantizeDivision::Sixteenth,
            triplet: false,
        }
    }
}

/// Loop region in beats.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct LoopRegion {
    #[serde(default)]
    pub start_beats: f32,
    #[serde(default = "default_loop_end")]
    pub end_beats: f32,
    #[serde(default)]
    pub enabled: bool,
}

fn default_loop_end() -> f32 {
    16.0
}

impl Default for LoopRegion {
    fn default() -> Self {
        Self {
            start_beats: 0.0,
            end_beats: 16.0,
            enabled: true,
        }
    }
}

/// Root sequence project embedded in a patch.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct SequenceProject {
    #[serde(default = "default_bpm")]
    pub bpm: f32,
    #[serde(default = "default_time_sig_num")]
    pub time_sig_num: u8,
    #[serde(default = "default_time_sig_den")]
    pub time_sig_den: u8,
    #[serde(default)]
    pub loop_region: LoopRegion,
    #[serde(default)]
    pub tracks: Vec<Track>,
    #[serde(default)]
    pub scenes: Vec<Scene>,
    #[serde(default)]
    pub quantize: QuantizeGrid,
}

fn default_bpm() -> f32 {
    120.0
}
fn default_time_sig_num() -> u8 {
    4
}
fn default_time_sig_den() -> u8 {
    4
}

impl Default for SequenceProject {
    fn default() -> Self {
        Self::default_template()
    }
}

impl SequenceProject {
    /// Empty 4-track template with 8 scenes.
    pub fn default_template() -> Self {
        let tracks = vec![
            Track::new("Track 1"),
            Track::new("Track 2"),
            Track::new("Track 3"),
            Track::new("Track 4"),
        ];
        let scenes = (1..=8)
            .map(|i| Scene::new(format!("Scene {i}"), tracks.len()))
            .collect();
        Self {
            bpm: default_bpm(),
            time_sig_num: default_time_sig_num(),
            time_sig_den: default_time_sig_den(),
            loop_region: LoopRegion::default(),
            tracks,
            scenes,
            quantize: QuantizeGrid::default(),
        }
    }

    pub fn armed_track_index(&self) -> Option<usize> {
        self.tracks.iter().position(|t| t.arm)
    }
}
