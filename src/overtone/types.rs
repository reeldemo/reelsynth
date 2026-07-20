//! Overtone filter slot types — master-bus anti-crackle chain (not musical FX).

use serde::{Deserialize, Serialize};

/// Insertable overtone filter kinds (chainable; duplicates allowed).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OvertoneFilterType {
    Lowpass,
    Harmonic,
    Slew,
}

impl OvertoneFilterType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Lowpass => "Lowpass",
            Self::Harmonic => "Harmonic",
            Self::Slew => "Slew",
        }
    }

    pub const ALL: [Self; 3] = [Self::Lowpass, Self::Harmonic, Self::Slew];
}

/// One slot in the master overtone suppression chain.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct OvertoneFilterSlot {
    pub filter_type: OvertoneFilterType,
    /// 0.0 ..= 1.0 — ceiling; effective = strength × curveHarshness.
    #[serde(default = "default_strength")]
    pub strength: f32,
    #[serde(default)]
    pub bypassed: bool,
}

fn default_strength() -> f32 {
    1.0
}

impl OvertoneFilterSlot {
    pub fn lowpass() -> Self {
        Self {
            filter_type: OvertoneFilterType::Lowpass,
            strength: 1.0,
            bypassed: false,
        }
    }

    pub fn harmonic() -> Self {
        Self {
            filter_type: OvertoneFilterType::Harmonic,
            strength: 1.0,
            bypassed: false,
        }
    }

    pub fn slew() -> Self {
        Self {
            filter_type: OvertoneFilterType::Slew,
            strength: 1.0,
            bypassed: false,
        }
    }

    pub fn for_type(filter_type: OvertoneFilterType) -> Self {
        match filter_type {
            OvertoneFilterType::Lowpass => Self::lowpass(),
            OvertoneFilterType::Harmonic => Self::harmonic(),
            OvertoneFilterType::Slew => Self::slew(),
        }
    }
}
