//! Master FX chain stubs (S6) — bypass flags wired from UI; DSP placeholders.

use serde::{Deserialize, Serialize};

/// Per-slot bypass state persisted in presets.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FxBypass {
    #[serde(default)]
    pub chorus_bypassed: bool,
    #[serde(default)]
    pub delay_bypassed: bool,
    #[serde(default = "default_true")]
    pub reverb_bypassed: bool,
}

fn default_true() -> bool {
    true
}

impl Default for FxBypass {
    fn default() -> Self {
        Self {
            chorus_bypassed: false,
            delay_bypassed: false,
            reverb_bypassed: true,
        }
    }
}

/// Minimal post-voice FX chain (stub DSP until S7).
#[derive(Clone, Debug, Default)]
pub struct FxChain {
    pub bypass: FxBypass,
    delay_buf: Vec<f32>,
    delay_pos: usize,
}

impl FxChain {
    pub fn new(sample_rate: u32) -> Self {
        let delay_len = (sample_rate as f32 * 0.18) as usize;
        Self {
            bypass: FxBypass::default(),
            delay_buf: vec![0.0; delay_len.max(1)],
            delay_pos: 0,
        }
    }

    pub fn set_bypass(&mut self, bypass: FxBypass) {
        self.bypass = bypass;
    }

    pub fn process_sample(&mut self, input: f32) -> f32 {
        let mut sample = input;

        if !self.bypass.chorus_bypassed {
            sample = chorus_stub(sample);
        }

        if !self.bypass.delay_bypassed {
            sample = self.delay_stub(sample);
        }

        if !self.bypass.reverb_bypassed {
            sample = reverb_stub(sample);
        }

        sample.clamp(-1.0, 1.0)
    }
}

fn chorus_stub(sample: f32) -> f32 {
    sample * 1.02
}

fn reverb_stub(sample: f32) -> f32 {
    sample * 0.98 + sample * 0.04
}

impl FxChain {
    fn delay_stub(&mut self, sample: f32) -> f32 {
        if self.delay_buf.is_empty() {
            return sample;
        }
        let delayed = self.delay_buf[self.delay_pos];
        self.delay_buf[self.delay_pos] = sample + delayed * 0.32;
        self.delay_pos = (self.delay_pos + 1) % self.delay_buf.len();
        sample * 0.72 + delayed * 0.28
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bypass_skips_fx() {
        let mut chain = FxChain::new(44100);
        chain.set_bypass(FxBypass {
            chorus_bypassed: true,
            delay_bypassed: true,
            reverb_bypassed: true,
        });
        assert_eq!(chain.process_sample(0.5), 0.5);
    }

    #[test]
    fn chorus_changes_sample_when_active() {
        let mut chain = FxChain::new(44100);
        chain.set_bypass(FxBypass {
            chorus_bypassed: false,
            delay_bypassed: true,
            reverb_bypassed: true,
        });
        assert_ne!(chain.process_sample(0.5), 0.5);
    }
}
