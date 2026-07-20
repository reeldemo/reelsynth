//! Serial overtone filter chain on the master bus (before musical FxChain).

use super::processors::OvertoneProcessor;
use super::types::OvertoneFilterSlot;

/// Ordered master-bus anti-crackle chain. Empty = identity (Off).
#[derive(Clone, Debug)]
pub struct OvertoneFilterChain {
    slots: Vec<OvertoneFilterSlot>,
    processors: Vec<OvertoneProcessor>,
    sample_rate: f32,
    /// Cached harshness from last `set_curve_harshness` / process call.
    curve_harshness: f32,
}

impl Default for OvertoneFilterChain {
    fn default() -> Self {
        Self::new(44100)
    }
}

impl OvertoneFilterChain {
    pub fn new(sample_rate: u32) -> Self {
        Self {
            slots: Vec::new(),
            processors: Vec::new(),
            sample_rate: sample_rate as f32,
            curve_harshness: 0.0,
        }
    }

    pub fn slots(&self) -> &[OvertoneFilterSlot] {
        &self.slots
    }

    pub fn set_slots(&mut self, slots: Vec<OvertoneFilterSlot>) {
        self.slots = slots;
        self.processors = self
            .slots
            .iter()
            .map(|s| OvertoneProcessor::new(s, self.sample_rate))
            .collect();
    }

    pub fn set_curve_harshness(&mut self, harshness: f32) {
        self.curve_harshness = harshness.clamp(0.0, 1.0);
    }

    pub fn curve_harshness(&self) -> f32 {
        self.curve_harshness
    }

    pub fn process_sample(&mut self, input: f32) -> f32 {
        self.process_stereo(input, input)[0]
    }

    pub fn process_stereo(&mut self, left: f32, right: f32) -> [f32; 2] {
        if self.slots.is_empty() {
            return [left, right];
        }

        let mut l = left;
        let mut r = right;
        let harsh = self.curve_harshness;

        for (slot, proc) in self.slots.iter().zip(self.processors.iter_mut()) {
            if slot.bypassed {
                continue;
            }
            let strength = slot.strength.clamp(0.0, 1.0);
            if strength <= 1e-6 {
                continue;
            }
            let effective = strength * harsh;
            if effective <= 1e-6 {
                continue;
            }
            let [ol, or_] = proc.process_stereo(l, r, effective);
            l = ol;
            r = or_;
        }
        [l, r]
    }
}
