//! Patch schema parsing (reelsynth-preset-v2 with v1 migration).

use crate::performance::PerformanceSettings;
use crate::sequence::SequenceProject;
use serde::{Deserialize, Serialize};

pub const SCHEMA_V1: &str = "reelsynth-preset-v1";
pub const SCHEMA_V2: &str = "reelsynth-preset-v2";

/// One entry in the per-oscillator wave slot map (frame index + label).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct WaveSlot {
    pub frame: f32,
    #[serde(default)]
    pub label: String,
}

/// One layer in a live wave stack (additive overlay inside a single oscillator).
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct WaveLayer {
    #[serde(default = "default_saw_type", rename = "type")]
    pub source_type: String,
    #[serde(default = "one")]
    pub level: f32,
    /// Detune offset in cents relative to the carrier.
    #[serde(default)]
    pub detune: f32,
    /// Wavetable frame index for `wavetable` layers.
    #[serde(default)]
    pub wt_position: f32,
    #[serde(default = "default_pulse_width")]
    pub pulse_width: f32,
    #[serde(default)]
    pub wavetable_id: Option<String>,
    /// Phase offset in radians at layer phase origin (sine layers).
    #[serde(default)]
    pub phase: f32,
    /// When true, layer contributes with inverted sign (−).
    #[serde(default)]
    pub invert: bool,
    /// Curve-wide default quant interp (hold|linear|spline|poly|expo|ma).
    #[serde(default = "default_quant_interp")]
    pub quant_interp: String,
    /// Per-segment interp modes (len = max(0, quant-1)).
    #[serde(default)]
    pub quant_segment_interps: Vec<String>,
}

impl Default for WaveLayer {
    fn default() -> Self {
        Self {
            source_type: default_saw_type(),
            level: 1.0,
            detune: 0.0,
            wt_position: 0.0,
            pulse_width: default_pulse_width(),
            wavetable_id: None,
            phase: 0.0,
            invert: false,
            quant_interp: default_quant_interp(),
            quant_segment_interps: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Oscillator {
    #[serde(default = "default_wt_type", rename = "type")]
    pub osc_type: String,
    #[serde(default = "one")]
    pub level: f32,
    #[serde(default)]
    pub position: f32,
    #[serde(default)]
    pub detune: f32,
    #[serde(default = "default_unison")]
    pub unison: u32,
    #[serde(default)]
    pub pan: f32,
    #[serde(default)]
    pub wavetable_id: Option<String>,
    /// Square/pulse duty cycle (0.05..0.95).
    #[serde(default = "default_pulse_width")]
    pub pulse_width: f32,
    /// Morph endpoint A (frame index).
    #[serde(default)]
    pub morph_a: f32,
    /// Morph endpoint B (frame index).
    #[serde(default = "default_morph_b")]
    pub morph_b: f32,
    /// Morph blend 0..1 between A and B.
    #[serde(default)]
    pub morph_amount: f32,
    /// Wavetable warp: none | sync | bend.
    #[serde(default = "default_warp_none")]
    pub warp_mode: String,
    #[serde(default)]
    pub warp_amount: f32,
    /// FM modulator source: none | osc2 | osc3 | osc2_osc3 | feedback.
    #[serde(default = "default_fm_none")]
    pub fm_source: String,
    /// Modulator frequency ratio relative to carrier (0.5..16).
    #[serde(default = "default_fm_ratio")]
    pub fm_ratio: f32,
    /// FM modulation depth (0..10).
    #[serde(default)]
    pub fm_index: f32,
    /// Wave slot count: 8 | 16 | 32 | 64 | 0 = smooth (legacy continuous position).
    #[serde(default = "default_wave_quant")]
    pub wave_quant: u8,
    /// Current wave slot index (0..wave_quant-1).
    #[serde(default = "default_wave_slot")]
    pub wave_slot: u8,
    /// Fine blend 0..1 toward the next slot.
    #[serde(default)]
    pub wave_slot_fine: f32,
    /// User slot map; auto-generated from `wave_quant` when empty.
    #[serde(default)]
    pub wave_slots: Vec<WaveSlot>,
    /// Live wave stack layers (empty = legacy single-source via `osc_type`).
    #[serde(default)]
    pub wave_layers: Vec<WaveLayer>,
    /// Stack combine mode: `add` (default) or `avg`.
    #[serde(default = "default_stack_mode")]
    pub stack_mode: String,
}

impl Oscillator {
    pub fn default_va() -> Self {
        Self {
            osc_type: "saw".into(),
            level: 0.0,
            position: 0.0,
            detune: 0.0,
            unison: 1,
            pan: 0.0,
            wavetable_id: None,
            pulse_width: default_pulse_width(),
            morph_a: 0.0,
            morph_b: default_morph_b(),
            morph_amount: 0.0,
            warp_mode: default_warp_none(),
            warp_amount: 0.0,
            fm_source: default_fm_none(),
            fm_ratio: default_fm_ratio(),
            fm_index: 0.0,
            wave_quant: default_wave_quant(),
            wave_slot: default_wave_slot(),
            wave_slot_fine: 0.0,
            wave_slots: Vec::new(),
            wave_layers: Vec::new(),
            stack_mode: default_stack_mode(),
        }
    }

    /// Effective slot count (explicit quant or derived from custom map).
    pub fn effective_wave_quant(&self) -> u8 {
        if self.wave_quant > 0 {
            self.wave_quant
        } else if !self.wave_slots.is_empty() {
            self.wave_slots.len().min(255) as u8
        } else {
            default_wave_quant()
        }
    }
}

pub(crate) fn default_pulse_width() -> f32 {
    0.5
}
pub(crate) fn default_morph_b() -> f32 {
    255.0
}
pub(crate) fn default_warp_none() -> String {
    "none".into()
}
pub(crate) fn default_fm_none() -> String {
    "none".into()
}
pub(crate) fn default_fm_ratio() -> f32 {
    1.0
}

pub(crate) fn default_wt_type() -> String {
    "wavetable".into()
}
pub(crate) fn default_saw_type() -> String {
    "saw".into()
}
pub(crate) fn default_stack_mode() -> String {
    "add".into()
}
pub(crate) fn default_quant_interp() -> String {
    "hold".into()
}
fn one() -> f32 {
    1.0
}
pub(crate) fn default_unison() -> u32 {
    1
}
pub(crate) fn default_wave_quant() -> u8 {
    16
}
pub(crate) fn default_wave_slot() -> u8 {
    7
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Filter {
    #[serde(default = "default_lp", rename = "type")]
    pub filter_type: String,
    #[serde(default = "default_cutoff")]
    pub cutoff: f32,
    #[serde(default)]
    pub resonance: f32,
    /// 0 = no tracking, 1 = cutoff follows pitch 1:1 in semitones.
    #[serde(default = "default_key_tracking")]
    pub key_tracking: f32,
    /// Soft tanh drive before the SVF (0..1).
    #[serde(default)]
    pub drive: f32,
}

/// One slot in the musical voice filter chain (serial SVF).
/// Distinct from master-bus [`crate::overtone::OvertoneFilterSlot`].
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct FilterSlot {
    #[serde(default = "default_lp", rename = "type")]
    pub filter_type: String,
    #[serde(default = "default_cutoff")]
    pub cutoff: f32,
    #[serde(default)]
    pub resonance: f32,
    #[serde(default = "default_key_tracking")]
    pub key_tracking: f32,
    #[serde(default)]
    pub drive: f32,
    #[serde(default)]
    pub bypassed: bool,
}

impl FilterSlot {
    pub const MAX_SLOTS: usize = 8;

    pub fn from_filter(f: &Filter) -> Self {
        Self {
            filter_type: f.filter_type.clone(),
            cutoff: f.cutoff,
            resonance: f.resonance,
            key_tracking: f.key_tracking,
            drive: f.drive,
            bypassed: false,
        }
    }

    pub fn to_filter(&self) -> Filter {
        Filter {
            filter_type: self.filter_type.clone(),
            cutoff: self.cutoff,
            resonance: self.resonance,
            key_tracking: self.key_tracking,
            drive: self.drive,
        }
    }

    pub fn lowpass() -> Self {
        Self::from_filter(&Filter::default())
    }

    pub fn for_type(filter_type: &str) -> Self {
        let mut slot = Self::lowpass();
        slot.filter_type = normalize_filter_type(filter_type).into();
        slot
    }

    pub fn is_active(&self) -> bool {
        !self.bypassed
    }
}

/// Canonical filter type strings used by the SVF and UI.
pub fn normalize_filter_type(filter_type: &str) -> &'static str {
    match filter_type.to_ascii_lowercase().as_str() {
        "highpass" | "hp" => "highpass",
        "bandpass" | "bp" => "bandpass",
        "notch" => "notch",
        _ => "lowpass",
    }
}

pub const FILTER_TYPES: [&str; 4] = ["lowpass", "highpass", "bandpass", "notch"];

pub fn filter_type_label(filter_type: &str) -> &'static str {
    match normalize_filter_type(filter_type) {
        "highpass" => "Highpass",
        "bandpass" => "Bandpass",
        "notch" => "Notch",
        _ => "Lowpass",
    }
}

/// Build the legacy dual-filter chain (Filter 1 → Filter 2).
pub fn legacy_filter_slots(filter: &Filter, filter2: &Filter) -> Vec<FilterSlot> {
    vec![FilterSlot::from_filter(filter), FilterSlot::from_filter(filter2)]
}

pub(crate) fn default_lp() -> String {
    "lowpass".into()
}
pub(crate) fn default_cutoff() -> f32 {
    1200.0
}
pub(crate) fn default_key_tracking() -> f32 {
    0.5
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Envelope {
    #[serde(default = "default_attack")]
    pub attack: f32,
    #[serde(default = "default_decay")]
    pub decay: f32,
    #[serde(default = "default_sustain")]
    pub sustain: f32,
    #[serde(default = "default_release")]
    pub release: f32,
}

pub(crate) fn default_attack() -> f32 {
    0.01
}
pub(crate) fn default_decay() -> f32 {
    0.2
}
pub(crate) fn default_sustain() -> f32 {
    0.6
}
pub(crate) fn default_release() -> f32 {
    0.4
}

impl Default for Envelope {
    fn default() -> Self {
        Self {
            attack: default_attack(),
            decay: default_decay(),
            sustain: default_sustain(),
            release: default_release(),
        }
    }
}

pub(crate) fn default_filter_envelope() -> Envelope {
    Envelope {
        attack: 0.005,
        decay: 0.35,
        sustain: 0.2,
        release: 0.5,
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Lfo {
    #[serde(default = "default_lfo_rate")]
    pub rate: f32,
    #[serde(default)]
    pub depth: f32,
    #[serde(default = "default_lfo_target")]
    pub target: String,
    /// Waveform: sine | tri | saw | sh
    #[serde(default = "default_lfo_shape")]
    pub shape: String,
}

pub(crate) fn default_lfo_shape() -> String {
    "sine".into()
}

pub(crate) fn default_lfo_rate() -> f32 {
    0.5
}
pub(crate) fn default_lfo_target() -> String {
    "wt_position".into()
}

impl Default for Lfo {
    fn default() -> Self {
        Self {
            rate: default_lfo_rate(),
            depth: 0.0,
            target: default_lfo_target(),
            shape: default_lfo_shape(),
        }
    }
}

/// Macro knob with direct destination routing (S6).
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Macro {
    #[serde(default = "default_macro_value")]
    pub value: f32,
    #[serde(default = "default_macro_target")]
    pub target: String,
    #[serde(default = "default_macro_amount")]
    pub amount: f32,
}

pub(crate) fn default_macro_value() -> f32 {
    0.5
}
pub(crate) fn default_macro_target() -> String {
    "filter_cutoff".into()
}
pub(crate) fn default_macro_amount() -> f32 {
    0.5
}

impl Default for Macro {
    fn default() -> Self {
        Self {
            value: default_macro_value(),
            target: default_macro_target(),
            amount: default_macro_amount(),
        }
    }
}

pub(crate) fn default_macros() -> Vec<Macro> {
    vec![
        Macro {
            target: "filter_cutoff".into(),
            amount: 0.6,
            ..Macro::default()
        },
        Macro {
            target: "osc1_position".into(),
            amount: 0.5,
            ..Macro::default()
        },
        Macro {
            target: "osc1_fm_index".into(),
            amount: 0.4,
            ..Macro::default()
        },
        Macro {
            target: "filter_resonance".into(),
            amount: 0.35,
            ..Macro::default()
        },
    ]
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ModSlot {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub amount: f32,
    /// When false the route is ignored by the engine (S6 UI On/Off).
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

pub(crate) fn default_enabled() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Patch {
    #[serde(default)]
    pub schema: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub wavetable_id: Option<String>,
    #[serde(default)]
    pub oscillators: Vec<Oscillator>,
    #[serde(default)]
    pub filter: Filter,
    /// Second serial filter (legacy mirror of `filters[1]` when chain is set).
    #[serde(default = "default_filter2")]
    pub filter2: Filter,
    /// Musical voice filter chain (serial SVF). `None` = legacy `filter`+`filter2`.
    /// `Some([])` = bypass. Distinct from header Overtone (master-bus anti-crackle).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub filters: Option<Vec<FilterSlot>>,
    #[serde(default)]
    pub envelope: Envelope,
    #[serde(default = "default_filter_envelope")]
    pub filter_envelope: Envelope,
    #[serde(default)]
    pub lfo: Lfo,
    #[serde(default)]
    pub lfo2: Lfo,
    #[serde(default = "default_macros")]
    pub macros: Vec<Macro>,
    #[serde(default)]
    pub mod_matrix: Vec<ModSlot>,
    #[serde(default)]
    pub effects: Vec<crate::fx::EffectSlot>,
    /// Legacy field — migrated into `effects` on load.
    #[serde(default, skip_serializing)]
    pub fx_bypass: crate::fx::FxBypass,
    #[serde(default)]
    pub sub_level: f32,
    #[serde(default)]
    pub noise_level: f32,
    /// Unison voice pan spread (0 = mono, 1 = full L/R).
    #[serde(default = "default_unison_spread")]
    pub unison_stereo_spread: f32,
    /// Key, scale, and performance keyboard layout.
    #[serde(default)]
    pub performance: PerformanceSettings,
    /// Compose-mode arrangement (clips, scenes, transport defaults).
    #[serde(default)]
    pub sequence: SequenceProject,
    /// Artistic wrap / edge crackle amount (0 = eliminate / clean, 1 = full cliff grit).
    /// Modulatable via mod-matrix target `crackle`. Default 0 = professional clean.
    #[serde(default)]
    pub crackle: f32,
}

pub(crate) fn default_filter2() -> Filter {
    Filter {
        filter_type: default_lp(),
        cutoff: 2400.0,
        resonance: 0.25,
        key_tracking: default_key_tracking(),
        drive: 0.0,
    }
}

pub(crate) fn default_unison_spread() -> f32 {
    0.7
}

impl Default for Filter {
    fn default() -> Self {
        Self {
            filter_type: default_lp(),
            cutoff: default_cutoff(),
            resonance: 0.3,
            key_tracking: default_key_tracking(),
            drive: 0.0,
        }
    }
}
