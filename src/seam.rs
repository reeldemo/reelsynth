//! Cycle seam / crackle character.
//!
//! `crackle` ∈ [0, 1]:
//! - **0** = eliminate (maximum wrap close — professional default)
//! - **1** = amplify (leave / emphasize wrap cliff — artistic)
//! - Mid = continuous blend (modulatable)
//!
//! Bake eliminate uses the multi-algo winner from [`crate::artifact_reduce`]
//! (`PeriodizeAlgo::BEST` / DualCosine).

use crate::artifact_reduce::{periodize_with_algo, PeriodizeAlgo};

/// Style hint for how fade length is chosen before crackle scaling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SeamStyle {
    /// Fixed strong fade (classic Soft).
    Soft,
    /// Fade scales with existing discontinuity.
    #[default]
    Adaptive,
    /// No automatic style — crackle alone decides (1 = fully open).
    Raw,
}

/// Periodize a single-cycle frame according to crackle amount.
///
/// `crackle = 0` → strongest close (`frame[last] = frame[0]`, long ease).  
/// `crackle = 1` → no modification (full cliff preserved for artistic use).
pub fn periodize_cycle(frame: &mut [f32], crackle: f32, style: SeamStyle) {
    periodize_with_algo(frame, crackle, style, PeriodizeAlgo::BEST);
}

/// Live edge / wrap character for artistic crackle (post-osc).
///
/// At `crackle = 0` this is identity (aside from state update).  
/// At `crackle > 0` it blends in a highpassed difference (emphasizes discontinuities).
#[derive(Debug, Clone, Copy, Default)]
pub struct CrackleVoice {
    prev: f32,
    hp: f32,
}

impl CrackleVoice {
    pub fn reset(&mut self) {
        self.prev = 0.0;
        self.hp = 0.0;
    }

    /// Process one sample. `crackle` 0 = clean passthrough, 1 = max edge emphasis.
    pub fn process(&mut self, x: f32, crackle: f32) -> f32 {
        let crackle = crackle.clamp(0.0, 1.0);
        let edge = x - self.prev;
        self.prev = x;
        // Simple one-pole HP on the delta → clicks / wrap grit.
        self.hp = self.hp * 0.85 + edge;
        if crackle < 1e-4 {
            return x;
        }
        // Soft asymmetric drive so amplify stays musical, not just louder noise.
        let grit = (self.hp * 2.5).tanh() * crackle;
        (x + grit * 0.55).clamp(-2.0, 2.0)
    }
}

/// Map legacy Seam UI modes to (crackle preset, style).
pub fn seam_mode_to_crackle(mode: &str) -> (f32, SeamStyle) {
    match mode.to_ascii_lowercase().as_str() {
        "off" | "seam·off" | "seam-off" => (1.0, SeamStyle::Raw),
        "soft" | "seam·soft" | "seam-soft" => (0.15, SeamStyle::Soft),
        "opt" | "denoise" | "denoise_opt" | "seam·opt" | "seam-opt" => (0.0, SeamStyle::Adaptive),
        _ => (0.0, SeamStyle::Adaptive), // Adaptive / clean default
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_ramp(n: usize) -> Vec<f32> {
        (0..n)
            .map(|i| -0.9 + 1.8 * (i as f32 / (n - 1) as f32))
            .collect()
    }

    fn wrap(frame: &[f32]) -> f32 {
        (frame[frame.len() - 1] - frame[0]).abs()
    }

    #[test]
    fn eliminate_closes_wrap() {
        let mut f = open_ramp(2048);
        periodize_cycle(&mut f, 0.0, SeamStyle::Adaptive);
        assert!(wrap(&f) < 1e-5, "wrap={}", wrap(&f));
    }

    #[test]
    fn amplify_preserves_cliff() {
        let mut f = open_ramp(2048);
        let before = wrap(&f);
        periodize_cycle(&mut f, 1.0, SeamStyle::Adaptive);
        assert!((wrap(&f) - before).abs() < 1e-5);
    }

    #[test]
    fn modulate_mid_between_eliminate_and_amplify() {
        let mut elim = open_ramp(2048);
        let mut mid = open_ramp(2048);
        let mut amp = open_ramp(2048);
        periodize_cycle(&mut elim, 0.0, SeamStyle::Soft);
        periodize_cycle(&mut mid, 0.5, SeamStyle::Soft);
        periodize_cycle(&mut amp, 1.0, SeamStyle::Soft);
        // Mid fade shorter than eliminate → larger residual approach step possible,
        // but wrap still pinned when crackle < 1.
        assert!(wrap(&elim) < 1e-5);
        // Mid blends clean strength — wrap reduced vs amplify, not necessarily pinned.
        assert!(wrap(&mid) < wrap(&amp) * 0.85, "mid wrap={}", wrap(&mid));
        assert!(wrap(&amp) > 1.0);
        let raw = open_ramp(2048);
        let dist = |a: &[f32]| {
            a.iter()
                .zip(raw.iter())
                .map(|(x, y)| (x - y).abs())
                .sum::<f32>()
        };
        assert!(
            dist(&mid) < dist(&elim) * 0.98,
            "mid should change less than eliminate"
        );
    }

    #[test]
    fn live_crackle_identity_at_zero() {
        let mut v = CrackleVoice::default();
        for &x in &[0.0f32, 0.5, -0.3, 0.9] {
            assert!((v.process(x, 0.0) - x).abs() < 1e-6);
        }
    }

    #[test]
    fn live_crackle_amplifies_edge() {
        let mut clean = CrackleVoice::default();
        let mut dirty = CrackleVoice::default();
        // Smooth then cliff
        let mut sig = vec![0.2f32; 32];
        sig.extend(std::iter::repeat_n(-0.8, 8));
        let mut e_clean = 0.0f32;
        let mut e_dirty = 0.0f32;
        for &x in &sig {
            let a = clean.process(x, 0.0);
            let b = dirty.process(x, 1.0);
            e_clean += a * a;
            e_dirty += b * b;
        }
        assert!(
            e_dirty > e_clean * 1.01,
            "amplify should add energy on edges ({e_dirty} vs {e_clean})"
        );
    }
}
