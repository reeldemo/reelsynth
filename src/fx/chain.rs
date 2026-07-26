use serde::{Deserialize, Serialize};

use super::processors::{soft_clip, EffectProcessor};
use super::types::{default_effects, EffectSlot, EffectType};

#[derive(Clone, Debug, Serialize, Deserialize)]
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

pub fn effects_from_bypass(bypass: &FxBypass) -> Vec<EffectSlot> {
    let mut effects = default_effects();
    if let Some(slot) = effects
        .iter_mut()
        .find(|s| s.effect_type == EffectType::Chorus)
    {
        slot.bypassed = bypass.chorus_bypassed;
    }
    if let Some(slot) = effects
        .iter_mut()
        .find(|s| s.effect_type == EffectType::Delay)
    {
        slot.bypassed = bypass.delay_bypassed;
    }
    if let Some(slot) = effects
        .iter_mut()
        .find(|s| s.effect_type == EffectType::Reverb)
    {
        slot.bypassed = bypass.reverb_bypassed;
    }
    effects
}

/// Post-voice FX chain with reorderable slots.
#[derive(Clone, Debug)]
pub struct FxChain {
    slots: Vec<EffectSlot>,
    processors: Vec<EffectProcessor>,
    sample_rate: f32,
}

impl Default for FxChain {
    fn default() -> Self {
        Self::new(44100)
    }
}

impl FxChain {
    pub fn new(sample_rate: u32) -> Self {
        let slots = default_effects();
        let processors = slots
            .iter()
            .map(|s| EffectProcessor::new(s, sample_rate as f32))
            .collect();
        Self {
            slots,
            processors,
            sample_rate: sample_rate as f32,
        }
    }

    pub fn set_effects(&mut self, slots: Vec<EffectSlot>) {
        self.slots = slots;
        self.processors = self
            .slots
            .iter()
            .map(|s| EffectProcessor::new(s, self.sample_rate))
            .collect();
    }

    pub fn slots(&self) -> &[EffectSlot] {
        &self.slots
    }

    /// Legacy API — maps fixed chorus/delay/reverb bypass flags.
    pub fn set_bypass(&mut self, bypass: FxBypass) {
        self.set_effects(effects_from_bypass(&bypass));
    }

    pub fn process_sample(&mut self, input: f32) -> f32 {
        self.process_stereo(input, input)[0]
    }

    pub fn process_stereo(&mut self, left: f32, right: f32) -> [f32; 2] {
        let mut l = left;
        let mut r = right;

        for (slot, proc) in self.slots.iter().zip(self.processors.iter_mut()) {
            if slot.bypassed {
                continue;
            }
            let mix = slot.mix.clamp(0.0, 1.0);
            if mix <= 0.0001 {
                continue;
            }
            let [wet_l, wet_r] = proc.process_stereo(l, r, slot);
            l = l * (1.0 - mix) + wet_l * mix;
            r = r * (1.0 - mix) + wet_r * mix;
        }

        [soft_clip(l), soft_clip(r)]
    }
}
