//! Oscillator engines (VA with MinBLEP, wavetable helpers).

pub mod stack;
pub mod va;

pub use stack::{bank_for_layer, layer_sign, sample_layer, sample_stack, uses_wave_stack, StackMode};
pub use va::{VaWaveform, sample_va};

/// Wavetable warp modes applied before lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum WtWarpMode {
    #[default]
    None,
    Sync,
    Bend,
}

impl WtWarpMode {
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "sync" => Self::Sync,
            "bend" => Self::Bend,
            _ => Self::None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Sync => "sync",
            Self::Bend => "bend",
        }
    }
}

/// Apply phase warp before wavetable lookup.
pub fn warp_phase(phase: f32, mode: WtWarpMode, amount: f32) -> f32 {
    let amount = amount.clamp(0.0, 1.0);
    match mode {
        WtWarpMode::None => phase.fract(),
        WtWarpMode::Sync => {
            let ratio = 1.0 + amount * 7.0;
            (phase * ratio).fract()
        }
        WtWarpMode::Bend => {
            let p = phase.fract();
            let exponent = 0.25 + amount * 1.75;
            p.powf(exponent)
        }
    }
}
