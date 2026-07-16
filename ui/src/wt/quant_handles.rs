//! Quant-snapped waveform knob handles for slot-based sample editing.
//!
//! Knobs sit on the drawn waveform at each quant X. Vertical drag edits
//! amplitude at that control point (wave height), not the slot→frame morph map
//! (that stays on the Curve tool).

use egui::{CursorIcon, Pos2, Rect, Sense, Ui};
use reelsynth::WavetableBank;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use super::slots::effective_quant_count;

const HANDLE_RADIUS: f32 = 6.0;
const WAVE_AMP: f32 = 0.42;

/// How quant knob amplitudes are written into the 2048-sample frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WtQuantInterp {
    /// Flat band per slot (step / rectangular — current legacy behavior).
    #[default]
    Hold,
    /// Straight segments between knob heights.
    Linear,
    /// Catmull-Rom spline through knob heights (smooth, same family as Shape upsample).
    Cubic,
}

impl WtQuantInterp {
    pub const LABELS: [&'static str; 3] = ["Hold", "Linear", "Spline"];

    pub fn label(self) -> &'static str {
        Self::LABELS[self.index()]
    }

    pub fn index(self) -> usize {
        match self {
            Self::Hold => 0,
            Self::Linear => 1,
            Self::Cubic => 2,
        }
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Linear,
            2 => Self::Cubic,
            _ => Self::Hold,
        }
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Hold => "Step — flat band per slot (rectangular)",
            Self::Linear => "Linear — straight lines between knobs",
            Self::Cubic => "Spline — smooth Catmull-Rom curve through knobs",
        }
    }
}

pub struct QuantHandleEditor<'a> {
    pub plot_rect: Rect,
    pub wave_quant: u8,
    pub bank: &'a mut WavetableBank,
    pub frame_idx: usize,
    pub interp: WtQuantInterp,
    /// Selected-layer display scale (level × sign). Knobs sit on that curve.
    pub display_scale: f32,
}

pub struct QuantHandleResponse {
    pub frame_edited: bool,
    pub hovered_slot: Option<usize>,
    pub dragged_slot: Option<usize>,
    pub status_label: Option<String>,
    /// True when pointer is near a knob on the selected curve (blocks other drags).
    pub over_handle: bool,
}

impl QuantHandleEditor<'_> {
    pub fn show(self, ui: &mut Ui) -> QuantHandleResponse {
        let tokens = Tokens::default();
        let accent_ui = ACCENT_UI;
        let slot_count = effective_quant_count(self.wave_quant).max(1);
        let show_all_knobs = self.wave_quant <= 64;
        let scale = if self.display_scale.abs() < 1e-4 {
            1.0
        } else {
            self.display_scale
        };
        const HIT_PX: f32 = 14.0;

        if self.wave_quant == 0 || self.frame_idx >= self.bank.num_frames {
            return QuantHandleResponse {
                frame_edited: false,
                hovered_slot: None,
                dragged_slot: None,
                status_label: None,
                over_handle: false,
            };
        }

        let drag_slot_id = ui.id().with("quant_drag_slot");
        let locked_slot: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_slot_id));

        let sense = Sense::click_and_drag();
        let response = ui.allocate_rect(self.plot_rect, sense);

        let mut frame_edited = false;
        let mut hovered_slot = None;
        let mut dragged_slot = locked_slot;
        let mut status_label = None;
        let mut over_handle = false;

        let pointer = response
            .interact_pointer_pos()
            .filter(|p| self.plot_rect.contains(*p));

        // Sample at the same phases as slot_x (i / (n−1)) so knobs sit on the wave.
        let points = quant_control_points(self.bank.frame(self.frame_idx), slot_count);

        if let Some(pos) = pointer {
            if nearest_quant_handle(pos, self.plot_rect, &points, scale, HIT_PX).is_some() {
                over_handle = true;
            }
        }

        if response.drag_started() {
            if let Some(pos) = pointer {
                // Snap to the nearest knob on the selected curve — ignore bare X columns.
                if let Some(slot) =
                    nearest_quant_handle(pos, self.plot_rect, &points, scale, HIT_PX)
                {
                    ui.ctx().data_mut(|d| d.insert_temp(drag_slot_id, slot));
                    dragged_slot = Some(slot);
                    over_handle = true;
                }
            }
        }

        if !response.dragged() && response.drag_stopped() {
            ui.ctx().data_mut(|d| d.remove::<usize>(drag_slot_id));
            dragged_slot = None;
        }

        if let Some(pos) = pointer {
            if locked_slot.is_none() && !response.dragged() {
                hovered_slot =
                    nearest_quant_handle(pos, self.plot_rect, &points, scale, HIT_PX);
                if hovered_slot.is_some() {
                    over_handle = true;
                }
            }
        }

        if let Some(slot) = dragged_slot {
            over_handle = true;
            if response.dragged() {
                if let Some(pos) = pointer {
                    let sample = sample_from_knob_y(pos.y, scale, self.plot_rect);
                    let prev = points.get(slot).copied().unwrap_or(0.0);
                    if (prev - sample).abs() > 1e-4 {
                        apply_quant_slot_amplitude(
                            self.bank.frame_mut(self.frame_idx),
                            slot,
                            slot_count,
                            sample,
                            self.interp,
                        );
                        frame_edited = true;
                        status_label = Some(format!(
                            "Slot {} → amp {:+.2} · {}",
                            slot + 1,
                            sample,
                            self.interp.label()
                        ));
                    }
                }
            }
        }

        let points = if frame_edited {
            quant_control_points(self.bank.frame(self.frame_idx), slot_count)
        } else {
            points
        };

        let painter = ui.painter_at(self.plot_rect);

        // Editable quantized curve through knobs (distinct from Result / other layers).
        let poly = quantized_curve_polyline(&points, self.plot_rect, scale);
        if poly.len() >= 2 {
            match self.interp {
                WtQuantInterp::Hold => {
                    // Step silhouette: horizontal then vertical between knobs.
                    let mut step_pts = Vec::with_capacity(poly.len() * 2);
                    for w in poly.windows(2) {
                        step_pts.push(w[0]);
                        step_pts.push(Pos2::new(w[1].x, w[0].y));
                    }
                    if let Some(last) = poly.last() {
                        step_pts.push(*last);
                    }
                    painter.add(egui::Shape::line(
                        step_pts,
                        egui::Stroke::new(2.0, accent_ui.gamma_multiply(0.85)),
                    ));
                }
                WtQuantInterp::Linear | WtQuantInterp::Cubic => {
                    // Dense resample so spline looks continuous under the knobs.
                    let dense = 64.max(slot_count * 4);
                    let mut dense_pts = Vec::with_capacity(dense + 1);
                    for i in 0..=dense {
                        let phase = i as f32 / dense as f32;
                        let s = sample_interp_at_phase(&points, phase, self.interp);
                        let x = egui::lerp(self.plot_rect.min.x..=self.plot_rect.max.x, phase);
                        let y = knob_y_on_curve(s, scale, self.plot_rect);
                        dense_pts.push(Pos2::new(x, y));
                    }
                    painter.add(egui::Shape::line(
                        dense_pts,
                        egui::Stroke::new(2.2, accent_ui.gamma_multiply(0.9)),
                    ));
                }
            }
        }

        for i in 0..slot_count {
            let show = show_all_knobs
                || hovered_slot == Some(i)
                || dragged_slot == Some(i)
                || i == 0
                || i + 1 == slot_count;
            if !show {
                continue;
            }
            let x = slot_x(i, slot_count, self.plot_rect);
            let sample = points.get(i).copied().unwrap_or(0.0);
            let y = knob_y_on_curve(sample, scale, self.plot_rect);
            let center = Pos2::new(x, y);
            let active = dragged_slot == Some(i) || hovered_slot == Some(i);
            let radius = if active {
                HANDLE_RADIUS * 1.25
            } else {
                HANDLE_RADIUS
            };
            let fill = if active {
                accent_ui.gamma_multiply(0.35)
            } else {
                tokens.surface2
            };
            painter.circle_filled(center, radius, fill);
            painter.circle_stroke(
                center,
                radius,
                egui::Stroke::new(if active { 2.0 } else { 1.0 }, accent_ui),
            );

            if active {
                let band_w = self.plot_rect.width() / slot_count as f32;
                let band = Rect::from_center_size(
                    Pos2::new(x, self.plot_rect.center().y),
                    egui::vec2(band_w, self.plot_rect.height()),
                );
                painter.rect_filled(band, 0.0, tokens.accent.gamma_multiply(0.12));
            }
        }

        if over_handle || response.hovered() || response.dragged() {
            if over_handle || response.dragged() {
                let cursor = if response.dragged() && dragged_slot.is_some() {
                    CursorIcon::Grabbing
                } else if over_handle {
                    CursorIcon::Grab
                } else {
                    CursorIcon::Default
                };
                ui.ctx().set_cursor_icon(cursor);
            }
            if over_handle {
                response
                    .clone()
                    .on_hover_text("Drag dots on the selected curve to reshape");
                if status_label.is_none() {
                    status_label =
                        Some("Drag dots on the selected curve to reshape".into());
                }
            }
        }

        QuantHandleResponse {
            frame_edited,
            hovered_slot,
            dragged_slot,
            status_label,
            over_handle,
        }
    }
}

/// Update one quant knob, then rebuild the frame using the chosen interpolation.
pub fn apply_quant_slot_amplitude(
    frame: &mut [f32],
    slot: usize,
    slot_count: usize,
    sample: f32,
    mode: WtQuantInterp,
) {
    if frame.is_empty() || slot_count == 0 {
        return;
    }
    let mut points = quant_control_points(frame, slot_count);
    if slot < points.len() {
        points[slot] = sample.clamp(-1.0, 1.0);
    }
    resample_frame_from_quant_points(frame, &points, mode);
}

/// Fill `frame` from evenly spaced control-point amplitudes.
pub fn resample_frame_from_quant_points(
    frame: &mut [f32],
    points: &[f32],
    mode: WtQuantInterp,
) {
    let n = points.len();
    if n == 0 || frame.is_empty() {
        return;
    }
    if n == 1 {
        frame.fill(points[0]);
        return;
    }
    let len = frame.len();
    for (i, sample) in frame.iter_mut().enumerate() {
        let phase = if len <= 1 {
            0.0
        } else {
            i as f32 / (len - 1) as f32
        };
        *sample = sample_interp_at_phase(points, phase, mode);
    }
    // Pin exact knot samples so knobs read back the values you set (Hold edges).
    if len > 1 && points.len() > 1 {
        for (slot, &amp) in points.iter().enumerate() {
            let phase = slot as f32 / (points.len() - 1) as f32;
            let idx = (phase * (len - 1) as f32).round() as usize;
            frame[idx.min(len - 1)] = amp;
        }
    }
}

fn sample_interp_at_phase(points: &[f32], phase: f32, mode: WtQuantInterp) -> f32 {
    let n = points.len();
    debug_assert!(n >= 1);
    if n == 1 {
        return points[0];
    }
    let phase = phase.clamp(0.0, 1.0);
    let t = phase * (n - 1) as f32;
    match mode {
        WtQuantInterp::Hold => {
            let slot = t.floor() as usize;
            points[slot.min(n - 1)]
        }
        WtQuantInterp::Linear => {
            let i = t.floor() as usize;
            if i >= n - 1 {
                return points[n - 1];
            }
            let frac = t - i as f32;
            egui::lerp(points[i]..=points[i + 1], frac)
        }
        WtQuantInterp::Cubic => {
            let i = (t.floor() as usize).min(n - 2);
            let frac = (t - i as f32).clamp(0.0, 1.0);
            let y0 = points[i.saturating_sub(1)];
            let y1 = points[i];
            let y2 = points[(i + 1).min(n - 1)];
            let y3 = points[(i + 2).min(n - 1)];
            cubic_catmull(y0, y1, y2, y3, frac)
        }
    }
}

fn cubic_catmull(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let a = -0.5 * y0 + 1.5 * y1 - 1.5 * y2 + 0.5 * y3;
    let b = y0 - 2.5 * y1 + 2.0 * y2 - 0.5 * y3;
    let c = -0.5 * y0 + 0.5 * y2;
    let d = y1;
    ((a * t + b) * t + c) * t + d
}

/// Control-point amplitudes at each quant slot phase (aligned with [`slot_x`]).
pub fn quant_control_points(frame: &[f32], slot_count: usize) -> Vec<f32> {
    let n = slot_count.max(1);
    (0..n)
        .map(|i| sample_at_quant_phase(frame, i, n))
        .collect()
}

pub fn sample_at_quant_phase(frame: &[f32], slot: usize, slot_count: usize) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let phase = if slot_count <= 1 {
        0.0
    } else {
        slot as f32 / (slot_count - 1) as f32
    };
    // Nearest knot sample (pinned by resample) so Hold edges stay exact.
    let pos = phase * (frame.len().saturating_sub(1)) as f32;
    let idx = pos.round() as usize;
    frame[idx.min(frame.len() - 1)]
}

pub fn snap_x_to_slot(x: f32, plot: Rect, slot_count: usize) -> usize {
    nearest_slot(x, plot, slot_count)
}

pub fn nearest_slot(x: f32, plot: Rect, slot_count: usize) -> usize {
    if slot_count <= 1 {
        return 0;
    }
    let t = ((x - plot.min.x) / plot.width()).clamp(0.0, 1.0);
    (t * (slot_count - 1) as f32).round() as usize
}

/// Nearest quant knob whose center lies on the (optionally scaled) curve.
/// Returns `None` when the pointer is farther than `max_dist` from every knob —
/// so grabs snap to the selected curve, not just the slot column.
pub fn nearest_quant_handle(
    pos: Pos2,
    plot: Rect,
    points: &[f32],
    display_scale: f32,
    max_dist: f32,
) -> Option<usize> {
    let n = points.len();
    if n == 0 {
        return None;
    }
    let mut best = None;
    let mut best_d = max_dist;
    for (i, &sample) in points.iter().enumerate() {
        let center = Pos2::new(slot_x(i, n, plot), knob_y_on_curve(sample, display_scale, plot));
        let d = pos.distance(center);
        if d <= best_d {
            best_d = d;
            best = Some(i);
        }
    }
    best
}

pub fn slot_x(slot: usize, slot_count: usize, plot: Rect) -> f32 {
    if slot_count <= 1 {
        return plot.center().x;
    }
    let t = slot as f32 / (slot_count - 1) as f32;
    egui::lerp(plot.min.x..=plot.max.x, t)
}

/// Map sample amplitude (−1..1) to plot Y (matches [`super::waveform::waveform_points`]).
pub fn sample_to_y(sample: f32, plot: Rect) -> f32 {
    plot.center().y - sample.clamp(-1.0, 1.0) * plot.height() * WAVE_AMP
}

pub fn y_to_sample(y: f32, plot: Rect) -> f32 {
    ((plot.center().y - y) / (plot.height() * WAVE_AMP)).clamp(-1.0, 1.0)
}

/// Knob Y on the selected layer curve (`display_scale` = level × sign).
pub fn knob_y_on_curve(sample: f32, display_scale: f32, plot: Rect) -> f32 {
    sample_to_y(sample * display_scale, plot)
}

/// Inverse of [`knob_y_on_curve`] for vertical reshaping.
pub fn sample_from_knob_y(y: f32, display_scale: f32, plot: Rect) -> f32 {
    let scale = if display_scale.abs() < 1e-4 {
        1.0
    } else {
        display_scale
    };
    (y_to_sample(y, plot) / scale).clamp(-1.0, 1.0)
}

/// Polyline through quant knobs — the editable quantized curve.
pub fn quantized_curve_polyline(points: &[f32], plot: Rect, display_scale: f32) -> Vec<Pos2> {
    let n = points.len();
    points
        .iter()
        .enumerate()
        .map(|(i, &s)| Pos2::new(slot_x(i, n, plot), knob_y_on_curve(s, display_scale, plot)))
        .collect()
}

/// Legacy alias used by curve morph overlays (slot frame index → Y).
pub fn frame_to_y(frame: f32, plot: Rect) -> f32 {
    let norm = (frame / 255.0).clamp(-1.0, 1.0);
    plot.center().y - norm * plot.height() * WAVE_AMP
}

pub fn y_to_frame(y: f32, plot: Rect) -> f32 {
    let norm = (plot.center().y - y) / (plot.height() * WAVE_AMP);
    (norm * 255.0).clamp(-255.0, 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;

    #[test]
    fn snap_x_to_slot_boundaries() {
        let plot = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(160.0, 80.0));
        assert_eq!(snap_x_to_slot(plot.min.x, plot, 16), 0);
        assert_eq!(snap_x_to_slot(plot.max.x, plot, 16), 15);
        assert_eq!(snap_x_to_slot(plot.center().x, plot, 16), 8);
    }

    /// Knobs must only engage when the pointer is near the dot on the curve —
    /// not merely matching X (which made far-away vertical grabs feel broken).
    #[test]
    fn nearest_quant_handle_requires_proximity_to_curve_dot() {
        let plot = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 100.0));
        let points = vec![0.0_f32, 0.8, -0.5, 0.2, 0.0];
        let slot_count = points.len();
        let scale = 1.0;
        let i = 1usize;
        let on_curve = Pos2::new(
            slot_x(i, slot_count, plot),
            sample_to_y(points[i] * scale, plot),
        );
        assert_eq!(
            nearest_quant_handle(on_curve, plot, &points, scale, 12.0),
            Some(i)
        );
        let same_x_far_y = Pos2::new(on_curve.x, plot.min.y + 2.0);
        assert_eq!(
            nearest_quant_handle(same_x_far_y, plot, &points, scale, 12.0),
            None,
            "far from curve Y must not grab the slot"
        );
    }

    /// When the selected layer is quieter than ±1, knobs must sit on that
    /// displayed curve (scaled), not at full-scale sample Y.
    #[test]
    fn knob_y_respects_display_scale_of_selected_curve() {
        let plot = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 100.0));
        let sample = 0.8_f32;
        let scale = 0.55_f32;
        let y_full = sample_to_y(sample, plot);
        let y_scaled = knob_y_on_curve(sample, scale, plot);
        assert!(
            (y_scaled - sample_to_y(sample * scale, plot)).abs() < 1e-4,
            "scaled knob must match displayed curve"
        );
        assert!(
            (y_full - y_scaled).abs() > 2.0,
            "scaled Y must differ from full-scale Y"
        );
        let recovered = sample_from_knob_y(y_scaled, scale, plot);
        assert!((recovered - sample).abs() < 1e-3, "drag Y → sample via scale");
    }

    /// After shaping a knob, re-sampled control points must stay on the
    /// quantized curve (no drift off the editable polyline).
    #[test]
    fn quant_knobs_stay_on_resampled_curve() {
        let mut frame = vec![0.0_f32; 512];
        let mut points = vec![0.0, 0.25, -0.4, 0.7, 0.1, -0.2, 0.5, 0.0];
        points[3] = 0.9;
        resample_frame_from_quant_points(&mut frame, &points, WtQuantInterp::Linear);
        let back = quant_control_points(&frame, points.len());
        for (i, (&want, &got)) in points.iter().zip(back.iter()).enumerate() {
            assert!(
                (want - got).abs() < 0.05,
                "slot {i}: knob drifted off curve want={want} got={got}"
            );
        }
    }

    /// Quantized edit polyline must pass through every knob (intuitive shape).
    #[test]
    fn quantized_curve_polyline_passes_through_knobs() {
        let plot = Rect::from_min_max(Pos2::new(10.0, 20.0), Pos2::new(210.0, 120.0));
        let points = vec![-0.6_f32, 0.2, 0.9, -0.3, 0.0];
        let scale = 0.7;
        let poly = quantized_curve_polyline(&points, plot, scale);
        assert_eq!(poly.len(), points.len());
        for (i, p) in poly.iter().enumerate() {
            let expect = Pos2::new(
                slot_x(i, points.len(), plot),
                knob_y_on_curve(points[i], scale, plot),
            );
            assert!(
                (p.x - expect.x).abs() < 1e-3 && (p.y - expect.y).abs() < 1e-3,
                "vertex {i} off knob"
            );
        }
    }

    #[test]
    fn sample_y_roundtrip() {
        let plot = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 100.0));
        for sample in [-1.0_f32, -0.5, 0.0, 0.42, 1.0] {
            let y = sample_to_y(sample, plot);
            let back = y_to_sample(y, plot);
            assert!(
                (back - sample).abs() < 1e-4,
                "roundtrip {sample} -> {back}"
            );
        }
    }

    #[test]
    fn knob_y_matches_waveform_intersection() {
        let plot = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 100.0));
        let frame: Vec<f32> = (0..2048)
            .map(|i| (i as f32 / 2048.0 * std::f32::consts::TAU).sin())
            .collect();
        let slot_count = 16;
        let points = quant_control_points(&frame, slot_count);
        let wave = crate::wt::waveform::waveform_points(&frame, plot, 256, WAVE_AMP);
        assert!(wave.len() >= 2);

        for i in 0..slot_count {
            let x = slot_x(i, slot_count, plot);
            let knob_y = sample_to_y(points[i], plot);
            let nearest = wave
                .windows(2)
                .filter_map(|seg| {
                    let (a, b) = (seg[0], seg[1]);
                    if (a.x - x).abs() < 1e-3 {
                        return Some(a.y);
                    }
                    if (a.x - x) * (b.x - x) <= 0.0 && (b.x - a.x).abs() > 1e-6 {
                        let t = (x - a.x) / (b.x - a.x);
                        return Some(a.y + (b.y - a.y) * t);
                    }
                    None
                })
                .next()
                .unwrap_or(knob_y);
            assert!(
                (nearest - knob_y).abs() < plot.height() * 0.08,
                "slot {i}: knob_y={knob_y} wave_y={nearest}"
            );
        }
    }

    #[test]
    fn vertical_edit_changes_frame_amplitude() {
        let mut bank = WavetableBank::factory_sine();
        let slot_count = 8;
        let before = sample_at_quant_phase(bank.frame(0), 3, slot_count);
        apply_quant_slot_amplitude(bank.frame_mut(0), 3, slot_count, 0.85, WtQuantInterp::Hold);
        let after = sample_at_quant_phase(bank.frame(0), 3, slot_count);
        assert!((after - before).abs() > 0.2);
        assert!((after - 0.85).abs() < 1e-3);
    }

    #[test]
    fn linear_interp_smooths_between_slots() {
        let mut frame = vec![0.0_f32; 256];
        let points = vec![-1.0, 1.0, -1.0, 1.0];
        resample_frame_from_quant_points(&mut frame, &points, WtQuantInterp::Linear);
        let mid = frame[frame.len() / 4];
        assert!(mid.abs() < 0.95 && mid.abs() > 0.05, "mid should blend, got {mid}");
    }

    #[test]
    fn hold_flat_per_slot_band() {
        let mut frame = vec![0.0_f32; 256];
        let points = vec![0.0, 1.0, 0.0, 1.0];
        resample_frame_from_quant_points(&mut frame, &points, WtQuantInterp::Hold);
        let q1 = frame[32];
        let q2 = frame[96];
        assert!((q1 - 0.0).abs() < 1e-3);
        assert!((q2 - 1.0).abs() < 1e-3);
    }

    #[test]
    fn frame_y_roundtrip() {
        let plot = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 100.0));
        let frame = 128.0;
        let y = frame_to_y(frame, plot);
        let back = y_to_frame(y, plot);
        assert!((back - frame).abs() < 2.0, "roundtrip {frame} -> {back}");
    }
}
