//! Performance input settings persisted in presets.

use serde::{Deserialize, Serialize};

use super::{ArpSettings, ChordSet, ChordVoicing, PerformanceLayout, Scale, ScaleBehavior};

fn default_root() -> u8 {
    0
}

fn default_base_octave() -> i8 {
    4
}

/// Key + scale + layout options for the performance layer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PerformanceSettings {
    /// Pitch class of the key root (0 = C … 11 = B).
    #[serde(default = "default_root")]
    pub root: u8,
    #[serde(default)]
    pub scale: Scale,
    #[serde(default)]
    pub scale_behavior: ScaleBehavior,
    #[serde(default)]
    pub layout: PerformanceLayout,
    #[serde(default)]
    pub chord_set: ChordSet,
    #[serde(default)]
    pub voicing: ChordVoicing,
    /// Octave for scale-degree and chord-row mapping (4 = middle C octave).
    #[serde(default = "default_base_octave")]
    pub base_octave: i8,
    #[serde(default)]
    pub arp: ArpSettings,
}

impl Default for PerformanceSettings {
    fn default() -> Self {
        Self {
            root: default_root(),
            scale: Scale::default(),
            scale_behavior: ScaleBehavior::default(),
            layout: PerformanceLayout::default(),
            chord_set: ChordSet::default(),
            voicing: ChordVoicing::default(),
            base_octave: default_base_octave(),
            arp: ArpSettings::default(),
        }
    }
}
