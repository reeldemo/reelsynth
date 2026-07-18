//! Per-slot overtone DSP processors (master bus).

use super::types::{OvertoneFilterSlot, OvertoneFilterType};

const CUTOFF_MIN_HZ: f32 = 800.0;
const SHELF_MIN_DB: f32 = -18.0;
const SLEW_OPEN: f32 = 1.0;
const SLEW_TIGHT: f32 = 0.015;

#[derive(Clone, Debug)]
pub(crate) enum OvertoneProcessor {
    Lowpass(OnePoleLp),
    Harmonic(HighShelf),
    Slew(SlewBus),
}

impl OvertoneProcessor {
    pub(crate) fn new(slot: &OvertoneFilterSlot, sample_rate: f32) -> Self {
        match slot.filter_type {
            OvertoneFilterType::Lowpass => Self::Lowpass(OnePoleLp::new(sample_rate)),
            OvertoneFilterType::Harmonic => Self::Harmonic(HighShelf::new(sample_rate)),
            OvertoneFilterType::Slew => Self::Slew(SlewBus::new()),
        }
    }

    pub(crate) fn process_stereo(
        &mut self,
        l: f32,
        r: f32,
        effective: f32,
    ) -> [f32; 2] {
        let e = effective.clamp(0.0, 1.0);
        match self {
            Self::Lowpass(p) => p.process(l, r, e),
            Self::Harmonic(p) => p.process(l, r, e),
            Self::Slew(p) => p.process(l, r, e),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OnePoleLp {
    sr: f32,
    y_l: f32,
    y_r: f32,
}

impl OnePoleLp {
    fn new(sr: f32) -> Self {
        Self {
            sr: sr.max(1.0),
            y_l: 0.0,
            y_r: 0.0,
        }
    }

    fn process(&mut self, l: f32, r: f32, effective: f32) -> [f32; 2] {
        let cutoff_max = 0.45 * self.sr;
        let cutoff = mix(cutoff_max, CUTOFF_MIN_HZ, effective).clamp(20.0, cutoff_max);
        let alpha = cutoff / (cutoff + self.sr * 0.55);
        self.y_l += alpha * (l - self.y_l);
        self.y_r += alpha * (r - self.y_r);
        [self.y_l, self.y_r]
    }
}

#[derive(Clone, Debug)]
pub(crate) struct HighShelf {
    sr: f32,
    lp_l: f32,
    lp_r: f32,
}

impl HighShelf {
    fn new(sr: f32) -> Self {
        Self {
            sr: sr.max(1.0),
            lp_l: 0.0,
            lp_r: 0.0,
        }
    }

    fn process(&mut self, l: f32, r: f32, effective: f32) -> [f32; 2] {
        // Split at ~0.25 * Nyquist via one-pole; attenuate high band.
        let split_hz = (0.25 * self.sr).clamp(20.0, 0.45 * self.sr);
        let alpha = split_hz / (split_hz + self.sr * 0.55);
        self.lp_l += alpha * (l - self.lp_l);
        self.lp_r += alpha * (r - self.lp_r);
        let g_db = mix(0.0, SHELF_MIN_DB, effective);
        let g = 10.0_f32.powf(g_db / 20.0);
        let high_l = l - self.lp_l;
        let high_r = r - self.lp_r;
        [self.lp_l + high_l * g, self.lp_r + high_r * g]
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SlewBus {
    prev_l: f32,
    prev_r: f32,
}

impl SlewBus {
    fn new() -> Self {
        Self {
            prev_l: 0.0,
            prev_r: 0.0,
        }
    }

    fn process(&mut self, l: f32, r: f32, effective: f32) -> [f32; 2] {
        let drive = 1.0 + effective * 2.5;
        let clipped_l = soft_clip_driven(l, drive);
        let clipped_r = soft_clip_driven(r, drive);
        let max_delta = mix(SLEW_OPEN, SLEW_TIGHT, effective);
        let out_l = slew_limit(&mut self.prev_l, clipped_l, max_delta);
        let out_r = slew_limit(&mut self.prev_r, clipped_r, max_delta);
        [out_l, out_r]
    }
}

fn soft_clip_driven(sample: f32, drive: f32) -> f32 {
    let d = drive.max(1.0);
    (sample * d).tanh() / d
}

fn slew_limit(prev: &mut f32, target: f32, max_delta: f32) -> f32 {
    let delta = (target - *prev).clamp(-max_delta, max_delta);
    *prev += delta;
    *prev
}

#[inline]
fn mix(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t.clamp(0.0, 1.0)
}
