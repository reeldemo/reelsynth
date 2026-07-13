//! LFO waveform shapes for mod matrix sources.

use crate::patch::Lfo;

/// LFO waveform shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum LfoShape {
    #[default]
    Sine,
    Tri,
    Saw,
    /// Sample & hold — steps on phase wrap.
    Sh,
}

impl LfoShape {
    pub fn from_str(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "tri" | "triangle" => Self::Tri,
            "saw" => Self::Saw,
            "sh" | "s&h" | "samplehold" | "sample_hold" => Self::Sh,
            _ => Self::Sine,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sine => "sine",
            Self::Tri => "tri",
            Self::Saw => "saw",
            Self::Sh => "sh",
        }
    }
}

/// Per-voice LFO runtime state (S&H latch).
#[derive(Clone, Debug, Default)]
pub struct LfoRuntime {
    pub sh_value: f32,
    pub last_phase: f32,
}

impl LfoRuntime {
    pub fn reset(&mut self) {
        self.sh_value = 0.0;
        self.last_phase = 0.0;
    }
}

/// Unit bipolar LFO waveform in [-1, 1] before depth scaling.
pub fn lfo_wave_unit(shape: LfoShape, phase: f32, runtime: &mut LfoRuntime) -> f32 {
    let p = phase.fract();
    match shape {
        LfoShape::Sine => (p * std::f32::consts::TAU).sin(),
        LfoShape::Tri => {
            if p < 0.5 {
                4.0 * p - 1.0
            } else {
                3.0 - 4.0 * p
            }
        }
        LfoShape::Saw => 2.0 * p - 1.0,
        LfoShape::Sh => {
            if p < runtime.last_phase {
                runtime.sh_value = pseudo_noise((p * 1_000_000.0) as u32);
            }
            runtime.last_phase = p;
            runtime.sh_value
        }
    }
}

/// Scaled LFO output for a patch LFO block at time `t` seconds.
pub fn lfo_value(lfo: &Lfo, time: f32, runtime: &mut LfoRuntime) -> f32 {
    let shape = LfoShape::from_str(&lfo.shape);
    let phase = time * lfo.rate.max(0.0);
    lfo_wave_unit(shape, phase, runtime) * lfo.depth
}

/// Legacy helper: sine-only, no runtime state.
pub fn lfo_value_legacy(lfo: &Lfo, time: f32) -> f32 {
    let mut rt = LfoRuntime::default();
    lfo_value(lfo, time, &mut rt)
}

/// Returns modulation when `lfo.target` matches `target` string.
pub fn lfo_for_target(lfo: &Lfo, value: f32, target: &str) -> f32 {
    if lfo.target == target {
        value
    } else {
        0.0
    }
}

fn pseudo_noise(seed: u32) -> f32 {
    let x = seed.wrapping_mul(1664525).wrapping_add(1013904223);
    ((x >> 16) as f32 / 32768.0) - 1.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_differ() {
        let mut rt = LfoRuntime::default();
        let sine = lfo_wave_unit(LfoShape::Sine, 0.25, &mut rt);
        let mut rt2 = LfoRuntime::default();
        let tri = lfo_wave_unit(LfoShape::Tri, 0.25, &mut rt2);
        assert!((sine - tri).abs() > 0.01);
    }

    #[test]
    fn saw_ramps() {
        let mut rt = LfoRuntime::default();
        let a = lfo_wave_unit(LfoShape::Saw, 0.1, &mut rt);
        let b = lfo_wave_unit(LfoShape::Saw, 0.4, &mut rt);
        assert!(b > a);
    }
}
