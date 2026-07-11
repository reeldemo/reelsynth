//! Optional parameter smoothing for block-based realtime processing.

/// One-pole exponential smoother for control-rate targets.
#[derive(Clone, Debug)]
pub struct Smoother {
    current: f32,
    target: f32,
    coeff: f32,
}

impl Smoother {
    pub fn new(initial: f32, smooth_ms: f32, sample_rate: f32) -> Self {
        let smooth_s = (smooth_ms / 1000.0).max(1e-6);
        let coeff = (-1.0 / (smooth_s * sample_rate)).exp();
        Self {
            current: initial,
            target: initial,
            coeff,
        }
    }

    pub fn set_immediate(&mut self, value: f32) {
        self.current = value;
        self.target = value;
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn current(&self) -> f32 {
        self.current
    }

    pub fn process(&mut self) -> f32 {
        self.current = self.target + self.coeff * (self.current - self.target);
        self.current
    }

    pub fn is_settled(&self, epsilon: f32) -> bool {
        (self.current - self.target).abs() <= epsilon
    }
}

/// Smoothed engine-level parameters (filter cutoff, master gain).
#[derive(Clone, Debug)]
pub struct EngineParams {
    pub filter_cutoff: Smoother,
    pub master_gain: Smoother,
}

impl EngineParams {
    pub fn new(patch: &crate::patch::Patch, sample_rate: f32) -> Self {
        Self {
            filter_cutoff: Smoother::new(patch.filter.cutoff, 10.0, sample_rate),
            master_gain: Smoother::new(1.0, 5.0, sample_rate),
        }
    }

    pub fn sync_from_patch(&mut self, patch: &crate::patch::Patch) {
        self.filter_cutoff.set_target(patch.filter.cutoff);
        self.master_gain.set_target(1.0);
    }
}
