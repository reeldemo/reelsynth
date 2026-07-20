//! Quant-knob interpolation modes for wavetable reshape.
//!
//! Modes apply **per segment** between consecutive knobs (`0→1`, …, `(N-2)→(N-1)`).
//! The last knob has no outgoing segment. A curve-level default can fill all segments.

/// How quant knob amplitudes are written into the frame on one segment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WtQuantInterp {
    /// Flat band for the segment (step / rectangular).
    #[default]
    Hold,
    /// Straight line between the two knobs.
    Linear,
    /// Catmull-Rom cubic using neighboring knobs for tangents.
    Spline,
    /// Local cubic Lagrange through four neighboring knobs.
    Polynomial,
    /// Exponential ease between the two knob heights.
    Exponential,
    /// Linear on the segment, then a short centered box (moving-average) filter.
    MovingAverage,
}

impl WtQuantInterp {
    pub const LABELS: [&'static str; 6] = [
        "Hold",
        "Linear",
        "Spline",
        "Poly",
        "Expo",
        "MA",
    ];

    pub fn label(self) -> &'static str {
        Self::LABELS[self.index()]
    }

    pub fn index(self) -> usize {
        match self {
            Self::Hold => 0,
            Self::Linear => 1,
            Self::Spline => 2,
            Self::Polynomial => 3,
            Self::Exponential => 4,
            Self::MovingAverage => 5,
        }
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Linear,
            2 => Self::Spline,
            3 => Self::Polynomial,
            4 => Self::Exponential,
            5 => Self::MovingAverage,
            _ => Self::Hold,
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Hold => "Step — flat band on this segment (rectangular)",
            Self::Linear => "Linear — straight line between knobs",
            Self::Spline => "Spline — Catmull-Rom cubic through knobs",
            Self::Polynomial => "Poly — cubic Lagrange through four neighboring knobs",
            Self::Exponential => "Expo — exponential ease between adjacent knobs",
            Self::MovingAverage => {
                "MA — linear on segment, then short centered box filter"
            }
        }
    }

    pub fn to_patch_str(self) -> &'static str {
        match self {
            Self::Hold => "hold",
            Self::Linear => "linear",
            Self::Spline => "spline",
            Self::Polynomial => "poly",
            Self::Exponential => "expo",
            Self::MovingAverage => "ma",
        }
    }

    pub fn from_patch_str(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "linear" => Self::Linear,
            "spline" | "cubic" => Self::Spline,
            "poly" | "polynomial" => Self::Polynomial,
            "expo" | "exponential" => Self::Exponential,
            "ma" | "moving_average" | "moving-average" => Self::MovingAverage,
            // Legacy / unset — never surface raw `none` in UI labels.
            "" | "none" | "hold" => Self::Hold,
            _ => Self::Hold,
        }
    }
}

/// User-facing label for the curve-wide toolbar combo (`All·Hold`, …).
pub fn toolbar_curve_label(mode: WtQuantInterp) -> String {
    format!("All·{}", mode.label())
}

/// User-facing label for a per-segment toolbar combo (`1→2·Linear`, …).
pub fn toolbar_segment_label(slot: usize, mode: WtQuantInterp) -> String {
    format!("{}→{}·{}", slot + 1, slot + 2, mode.label())
}

/// Map a patch / preset string to a toolbar-safe label (defaults to Hold).
#[allow(dead_code)] // exercised in unit tests; for raw preset strings at UI boundaries
pub fn display_label_from_patch_str(s: &str) -> &'static str {
    WtQuantInterp::from_patch_str(s).label()
}

/// Number of interp segments for `slot_count` knobs (`max(0, N−1)`).
pub fn segment_count(slot_count: usize) -> usize {
    slot_count.saturating_sub(1)
}

/// Resize segment modes to match knob count, preserving existing entries and
/// filling new ones from `default`.
pub fn resize_segment_interps(
    segments: &mut Vec<WtQuantInterp>,
    slot_count: usize,
    default: WtQuantInterp,
) {
    let want = segment_count(slot_count);
    if segments.len() > want {
        segments.truncate(want);
    } else {
        while segments.len() < want {
            segments.push(default);
        }
    }
}

/// Replace all segment modes with `mode` (curve-wide default apply).
pub fn fill_segment_interps(
    segments: &mut Vec<WtQuantInterp>,
    slot_count: usize,
    mode: WtQuantInterp,
) {
    let want = segment_count(slot_count);
    segments.clear();
    segments.resize(want, mode);
}

/// Resolve mode for segment `seg_i` (outgoing from knob `seg_i`).
pub fn segment_mode(
    segments: &[WtQuantInterp],
    seg_i: usize,
    fallback: WtQuantInterp,
) -> WtQuantInterp {
    segments.get(seg_i).copied().unwrap_or(fallback)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn segment_count_is_knobs_minus_one() {
        assert_eq!(segment_count(0), 0);
        assert_eq!(segment_count(1), 0);
        assert_eq!(segment_count(8), 7);
        assert_eq!(segment_count(16), 15);
    }

    #[test]
    fn last_knob_has_no_segment() {
        let n = 8;
        assert_eq!(segment_count(n), n - 1);
        let segs = vec![WtQuantInterp::Linear; segment_count(n)];
        assert!(segs.get(n - 1).is_none());
        assert!(segs.get(n - 2).is_some());
    }

    #[test]
    fn curve_default_fills_all_segments() {
        let mut segs = vec![WtQuantInterp::Hold; 3];
        fill_segment_interps(&mut segs, 8, WtQuantInterp::Spline);
        assert_eq!(segs.len(), 7);
        assert!(segs.iter().all(|&m| m == WtQuantInterp::Spline));
    }

    #[test]
    fn resize_preserves_and_fills() {
        let mut segs = vec![WtQuantInterp::Linear, WtQuantInterp::Exponential];
        resize_segment_interps(&mut segs, 8, WtQuantInterp::Hold);
        assert_eq!(segs.len(), 7);
        assert_eq!(segs[0], WtQuantInterp::Linear);
        assert_eq!(segs[1], WtQuantInterp::Exponential);
        assert!(segs[2..].iter().all(|&m| m == WtQuantInterp::Hold));

        resize_segment_interps(&mut segs, 4, WtQuantInterp::Polynomial);
        assert_eq!(segs.len(), 3);
        assert_eq!(segs[0], WtQuantInterp::Linear);
    }

    #[test]
    fn patch_str_roundtrip() {
        for mode in [
            WtQuantInterp::Hold,
            WtQuantInterp::Linear,
            WtQuantInterp::Spline,
            WtQuantInterp::Polynomial,
            WtQuantInterp::Exponential,
            WtQuantInterp::MovingAverage,
        ] {
            assert_eq!(WtQuantInterp::from_patch_str(mode.to_patch_str()), mode);
        }
        assert_eq!(WtQuantInterp::from_patch_str("cubic"), WtQuantInterp::Spline);
        assert_eq!(WtQuantInterp::from_patch_str(""), WtQuantInterp::Hold);
        assert_eq!(WtQuantInterp::from_patch_str("none"), WtQuantInterp::Hold);
    }

    #[test]
    fn display_labels_never_show_none() {
        assert_eq!(display_label_from_patch_str("none"), "Hold");
        assert_eq!(display_label_from_patch_str(""), "Hold");
        assert_eq!(display_label_from_patch_str("linear"), "Linear");
        assert_eq!(toolbar_curve_label(WtQuantInterp::Hold), "All·Hold");
        assert_eq!(
            toolbar_segment_label(2, WtQuantInterp::Spline),
            "3→4·Spline"
        );
    }
}
