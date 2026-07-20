//! Quant-snapped waveform knob handles for slot-based sample editing.
//!
//! Knobs sit on the drawn waveform at each quant X. Vertical drag edits
//! amplitude at that control point (wave height). The Select tool owns
//! knobs; Shape owns control-point templates.

use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Ui};
use reelsynth::WavetableBank;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::quant_interp::segment_mode;

use super::slots::effective_quant_count;
use super::view_zoom::WtCurveViewTransform;

const HANDLE_RADIUS: f32 = 6.0;
const WAVE_AMP: f32 = 0.42;

/// Visual knobs for idle / hover / drag — shared by Selected, Result, and Layers.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuantKnobVisual {
    pub radius: f32,
    pub stroke_width: f32,
    /// Outer glow ring radius (None = no glow).
    pub glow_radius: Option<f32>,
    pub glow_alpha: f32,
    /// When true, use a brighter accent fill instead of surface.
    pub fill_brighter: bool,
    /// Vertical guide at the snapped slot X.
    pub show_slot_guide: bool,
}

/// Radius / stroke / glow for a quant knob given hover + drag state.
pub fn quant_knob_visual(hovered: bool, dragged: bool) -> QuantKnobVisual {
    if dragged {
        QuantKnobVisual {
            radius: HANDLE_RADIUS * 1.55,
            stroke_width: 2.6,
            glow_radius: Some(HANDLE_RADIUS * 2.55),
            glow_alpha: 0.28,
            fill_brighter: true,
            show_slot_guide: true,
        }
    } else if hovered {
        QuantKnobVisual {
            radius: HANDLE_RADIUS * 1.45,
            stroke_width: 2.35,
            glow_radius: Some(HANDLE_RADIUS * 2.35),
            glow_alpha: 0.22,
            fill_brighter: true,
            show_slot_guide: true,
        }
    } else {
        QuantKnobVisual {
            radius: HANDLE_RADIUS,
            stroke_width: 1.0,
            glow_radius: None,
            glow_alpha: 0.0,
            fill_brighter: false,
            show_slot_guide: false,
        }
    }
}

/// `(stroke_width, color_gamma_mul)` for the editable quantized curve.
pub fn quant_curve_stroke(active: bool) -> (f32, f32) {
    if active {
        (3.4, 1.0)
    } else {
        (2.0, 0.85)
    }
}

/// Status / tooltip while hovering (or dragging) a snapped slot.
pub fn quant_hover_status_label(slot: usize, sample: f32) -> String {
    format!("Slot {} · amp {:+.2}", slot + 1, sample)
}

/// Paint one quant knob with shared hover/drag emphasis.
pub fn paint_quant_knob(
    painter: &egui::Painter,
    center: Pos2,
    visual: QuantKnobVisual,
    fill: Color32,
    stroke: Color32,
    plot: Rect,
) {
    if let Some(glow_r) = visual.glow_radius {
        painter.circle_stroke(
            center,
            glow_r,
            egui::Stroke::new(2.0, stroke.gamma_multiply(visual.glow_alpha * 2.2)),
        );
        painter.circle_filled(center, glow_r, stroke.gamma_multiply(visual.glow_alpha));
    }
    if visual.show_slot_guide {
        painter.line_segment(
            [
                Pos2::new(center.x, plot.min.y),
                Pos2::new(center.x, plot.max.y),
            ],
            egui::Stroke::new(1.25, stroke.gamma_multiply(0.55)),
        );
    }
    painter.circle_filled(center, visual.radius, fill);
    painter.circle_stroke(
        center,
        visual.radius,
        egui::Stroke::new(visual.stroke_width, stroke),
    );
}

/// Re-export — modes live in [`crate::quant_interp`].
pub use crate::quant_interp::WtQuantInterp;

pub struct QuantHandleEditor<'a> {
    pub plot_rect: Rect,
    pub wave_quant: u8,
    pub bank: &'a mut WavetableBank,
    pub frame_idx: usize,
    /// Per-segment modes (`len = slot_count - 1`).
    pub segment_interps: &'a [WtQuantInterp],
    pub curve_default: WtQuantInterp,
    pub selected_slot: &'a mut Option<usize>,
    /// Selected-layer display scale (level x sign). Knobs sit on that curve.
    pub display_scale: f32,
    /// Shared Design curve zoom / pan (identity = no zoom).
    pub view: WtCurveViewTransform,
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
        let hit_r = self.view.hit_radius(HIT_PX);
        let view = self.view;

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
            let plot_pos = view.unmap_pos(pos, self.plot_rect);
            if nearest_quant_handle(plot_pos, self.plot_rect, &points, scale, hit_r).is_some() {
                over_handle = true;
            }
        }

        if response.drag_started() {
            if let Some(pos) = pointer {
                let plot_pos = view.unmap_pos(pos, self.plot_rect);
                if let Some(slot) =
                    nearest_quant_handle(plot_pos, self.plot_rect, &points, scale, hit_r)
                {
                    ui.ctx().data_mut(|d| d.insert_temp(drag_slot_id, slot));
                    dragged_slot = Some(slot);
                    *self.selected_slot = Some(slot);
                    over_handle = true;
                }
            }
        }
        if response.clicked() {
            if let Some(pos) = pointer {
                let plot_pos = view.unmap_pos(pos, self.plot_rect);
                if let Some(slot) =
                    nearest_quant_handle(plot_pos, self.plot_rect, &points, scale, hit_r)
                {
                    *self.selected_slot = Some(slot);
                }
            }
        }

        if !response.dragged() && response.drag_stopped() {
            ui.ctx().data_mut(|d| d.remove::<usize>(drag_slot_id));
            dragged_slot = None;
        }

        if let Some(pos) = pointer {
            if locked_slot.is_none() && !response.dragged() {
                let plot_pos = view.unmap_pos(pos, self.plot_rect);
                hovered_slot =
                    nearest_quant_handle(plot_pos, self.plot_rect, &points, scale, hit_r);
                if hovered_slot.is_some() {
                    over_handle = true;
                }
            }
        }

        if let Some(slot) = dragged_slot {
            over_handle = true;
            if response.dragged() {
                if let Some(pos) = pointer {
                    let plot_pos = view.unmap_pos(pos, self.plot_rect);
                    let sample = sample_from_knob_y(plot_pos.y, scale, self.plot_rect);
                    let prev = points.get(slot).copied().unwrap_or(0.0);
                    if (prev - sample).abs() > 1e-4 {
                        apply_quant_slot_amplitude(
                            self.bank.frame_mut(self.frame_idx),
                            slot,
                            slot_count,
                            sample,
                            self.segment_interps,
                            self.curve_default,
                        );
                        frame_edited = true;
                        let seg_label = if slot + 1 < slot_count {
                            segment_mode(self.segment_interps, slot, self.curve_default).label()
                        } else {
                            "end"
                        };
                        status_label = Some(format!(
                            "Slot {} → amp {:+.2} · {}",
                            slot + 1,
                            sample,
                            seg_label
                        ));
                        *self.selected_slot = Some(slot);
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
        let curve_active = hovered_slot.is_some() || dragged_slot.is_some();
        let (curve_w, curve_mul) = quant_curve_stroke(curve_active);

        // Editable quantized curve through knobs (distinct from Result / other layers).
        let poly = quantized_curve_polyline(&points, self.plot_rect, scale);
        {
            let dense = 64.max(slot_count * 4);
            let mut dense_pts = Vec::with_capacity(dense + 1);
            for i in 0..=dense {
                let phase = i as f32 / dense as f32;
                let s = sample_interp_at_phase(
                    &points,
                    phase,
                    self.segment_interps,
                    self.curve_default,
                );
                let x = egui::lerp(self.plot_rect.min.x..=self.plot_rect.max.x, phase);
                let y = knob_y_on_curve(s, scale, self.plot_rect);
                dense_pts.push(view.map_pos(Pos2::new(x, y), self.plot_rect));
            }
            if dense_pts.len() >= 2 {
                let focus = self.selected_slot.filter(|s| *s + 1 < slot_count);
                // Tint outgoing segment under selected/hovered knob when possible.
                let _ = (focus, hovered_slot, poly);
                painter.add(egui::Shape::line(
                    dense_pts,
                    egui::Stroke::new(
                        curve_w + 0.2,
                        accent_ui.gamma_multiply(curve_mul.max(0.9)),
                    ),
                ));
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
            let center = view.map_pos(Pos2::new(x, y), self.plot_rect);
            let hovered = hovered_slot == Some(i);
            let dragged = dragged_slot == Some(i);
            let visual = quant_knob_visual(hovered, dragged);
            let fill = if visual.fill_brighter {
                accent_ui.gamma_multiply(if dragged { 0.55 } else { 0.42 })
            } else {
                tokens.surface2
            };
            paint_quant_knob(
                &painter,
                center,
                visual,
                fill,
                accent_ui,
                self.plot_rect,
            );

            if visual.show_slot_guide {
                let band_w = self.plot_rect.width() / slot_count as f32;
                let band = Rect::from_center_size(
                    Pos2::new(x, self.plot_rect.center().y),
                    egui::vec2(band_w, self.plot_rect.height()),
                );
                painter.rect_filled(band, 0.0, tokens.accent.gamma_multiply(0.14));
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
                let focus_slot = dragged_slot.or(hovered_slot);
                let hover_text = if let Some(slot) = focus_slot {
                    let amp = points.get(slot).copied().unwrap_or(0.0);
                    quant_hover_status_label(slot, amp)
                } else {
                    "Drag dots on the selected curve to reshape".into()
                };
                response.clone().on_hover_text(&hover_text);
                if status_label.is_none() {
                    status_label = Some(hover_text);
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

/// How aggressively to close the wavetable wrap seam after Quant rebuilds.
///
/// Periodic cycles need `frame[0] ≈ frame[last]`. A hard overwrite of the last
/// sample made the last Quant knob appear stuck; adaptive modes fade only as
/// much as the discontinuity requires.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuantSeamMode {
    /// No wrap fade — may crackle on discontinuous ends.
    Off,
    /// Fixed-width ease toward `frame[0]` (legacy Soft).
    Soft,
    /// Fade length scales with seam size; skips work when already closed.
    #[default]
    Adaptive,
    /// Unsupervised DenoiseOpt (fitted denoise+shape loss) — inference only.
    Opt,
}

impl QuantSeamMode {
    pub const LABELS: [&'static str; 4] =
        ["Seam·Off", "Seam·Soft", "Seam·Adapt", "Seam·Opt"];

    pub fn label(self) -> &'static str {
        Self::LABELS[self.index()]
    }

    pub fn tooltip(self) -> &'static str {
        match self {
            Self::Off => "No wrap fade — max edit freedom, may click at cycle wrap",
            Self::Soft => "Fixed fade into frame[0] (stronger crackle reduction)",
            Self::Adaptive => "Fade only as much as the wrap discontinuity needs",
            Self::Opt => {
                "AI DenoiseOpt — fitted once on denoise+shape loss; mid-cycle shape conserved"
            }
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Off => 0,
            Self::Soft => 1,
            Self::Adaptive => 2,
            Self::Opt => 3,
        }
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            0 => Self::Off,
            1 => Self::Soft,
            3 => Self::Opt,
            _ => Self::Adaptive,
        }
    }
}

thread_local! {
    static QUANT_SEAM_MODE: std::cell::Cell<QuantSeamMode> =
        std::cell::Cell::new(QuantSeamMode::Adaptive);
    static CRACKLE_AMOUNT: std::cell::Cell<f32> = std::cell::Cell::new(0.0);
}

/// Set seam mode for Quant rebuilds on this UI thread (call once per Design frame).
pub fn set_quant_seam_mode(mode: QuantSeamMode) {
    QUANT_SEAM_MODE.with(|c| c.set(mode));
}

fn current_quant_seam_mode() -> QuantSeamMode {
    QUANT_SEAM_MODE.with(|c| c.get())
}

/// Artistic crackle 0..1 (0 = eliminate / professional clean). Synced from patch.
pub fn set_crackle_amount(amount: f32) {
    CRACKLE_AMOUNT.with(|c| c.set(amount.clamp(0.0, 1.0)));
}

pub fn current_crackle_amount() -> f32 {
    CRACKLE_AMOUNT.with(|c| c.get())
}

/// Update one quant knob, then rebuild using per-segment interpolation.
///
/// First and last knobs are **linked** (periodic wrap): editing either writes
/// both control points so the last knob stays draggable and the cycle closes.
pub fn apply_quant_slot_amplitude(
    frame: &mut [f32],
    slot: usize,
    slot_count: usize,
    sample: f32,
    segments: &[WtQuantInterp],
    curve_default: WtQuantInterp,
) {
    if frame.is_empty() || slot_count == 0 {
        return;
    }
    let mut points = quant_control_points(frame, slot_count);
    let sample = sample.clamp(-1.0, 1.0);
    if slot < points.len() {
        points[slot] = sample;
    }
    // Periodic cycle: phase 0 and phase 1 are the same wrap point (unless Seam·Off).
    if current_quant_seam_mode() != QuantSeamMode::Off
        && points.len() >= 2
        && (slot == 0 || slot + 1 == points.len())
    {
        points[0] = sample;
        let last = points.len() - 1;
        points[last] = sample;
    }
    resample_frame_from_quant_points(frame, &points, segments, curve_default);
}

/// Convenience: apply one mode to every segment.
#[allow(dead_code)]
pub fn apply_quant_slot_amplitude_uniform(
    frame: &mut [f32],
    slot: usize,
    slot_count: usize,
    sample: f32,
    mode: WtQuantInterp,
) {
    let segs = vec![mode; slot_count.saturating_sub(1)];
    apply_quant_slot_amplitude(frame, slot, slot_count, sample, &segs, mode);
}

/// Fill `frame` from control-point amplitudes using per-segment modes.
pub fn resample_frame_from_quant_points(
    frame: &mut [f32],
    points: &[f32],
    segments: &[WtQuantInterp],
    curve_default: WtQuantInterp,
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
        *sample = sample_interp_at_phase(points, phase, segments, curve_default);
    }
    apply_moving_average_segments(frame, points, segments, curve_default);
    if len > 1 && points.len() > 1 {
        for (slot, &amp) in points.iter().enumerate() {
            let phase = slot as f32 / (points.len() - 1) as f32;
            let idx = (phase * (len - 1) as f32).round() as usize;
            frame[idx.min(len - 1)] = amp;
        }
    }
    // Quant rebuilds can leave a raw wrap cliff (slot 0 ≠ last). Close the seam
    // so WT BLEP + voice slew are not fighting a near-vertical edge every cycle.
    periodize_quant_frame(frame);
}

/// Soft-close wrap after Quant resampling. Mode is [`current_quant_seam_mode`].
pub fn periodize_quant_frame(frame: &mut [f32]) {
    periodize_quant_frame_with_mode(frame, current_quant_seam_mode());
}

/// Apply wrap-seam reduction with an explicit mode (tests / CLI).
///
/// Seam·Off forces crackle=1; Soft/Adaptive/Opt use [`current_crackle_amount`] (default 0).
/// Seam·Opt runs the frozen unsupervised DenoiseOpt stack (inference only).
pub fn periodize_quant_frame_with_mode(frame: &mut [f32], mode: QuantSeamMode) {
    use reelsynth::artifact_reduce::{periodize_with_algo, PeriodizeAlgo};
    use reelsynth::{periodize_cycle, SeamStyle};
    match mode {
        QuantSeamMode::Off => periodize_cycle(frame, 1.0, SeamStyle::Raw),
        QuantSeamMode::Soft => {
            periodize_cycle(frame, current_crackle_amount(), SeamStyle::Soft)
        }
        QuantSeamMode::Adaptive => {
            periodize_cycle(frame, current_crackle_amount(), SeamStyle::Adaptive)
        }
        QuantSeamMode::Opt => periodize_with_algo(
            frame,
            current_crackle_amount(),
            SeamStyle::Adaptive,
            PeriodizeAlgo::DenoiseOpt,
        ),
    }
}

/// Uniform-mode resample (all segments share `mode`).
pub fn resample_frame_from_quant_points_uniform(
    frame: &mut [f32],
    points: &[f32],
    mode: WtQuantInterp,
) {
    let segs = vec![mode; points.len().saturating_sub(1)];
    resample_frame_from_quant_points(frame, points, &segs, mode);
}

fn apply_moving_average_segments(
    frame: &mut [f32],
    points: &[f32],
    segments: &[WtQuantInterp],
    curve_default: WtQuantInterp,
) {
    let n = points.len();
    let len = frame.len();
    if n < 2 || len < 3 {
        return;
    }
    let mut scratch = frame.to_vec();
    for seg in 0..n - 1 {
        if segment_mode(segments, seg, curve_default) != WtQuantInterp::MovingAverage {
            continue;
        }
        let i0 = ((seg as f32 / (n - 1) as f32) * (len - 1) as f32).round() as usize;
        let i1 = (((seg + 1) as f32 / (n - 1) as f32) * (len - 1) as f32).round() as usize;
        let i1 = i1.min(len - 1).max(i0);
        let win = ((i1 - i0) / 4).max(3) | 1;
        let half = win / 2;
        for i in i0..=i1 {
            let mut sum = 0.0_f32;
            let mut count = 0_u32;
            for j in i.saturating_sub(half)..=(i + half).min(len - 1) {
                if j < i0 || j > i1 {
                    continue;
                }
                sum += frame[j];
                count += 1;
            }
            if count > 0 {
                scratch[i] = sum / count as f32;
            }
        }
        frame[i0..=i1].copy_from_slice(&scratch[i0..=i1]);
    }
}

fn sample_interp_at_phase(
    points: &[f32],
    phase: f32,
    segments: &[WtQuantInterp],
    curve_default: WtQuantInterp,
) -> f32 {
    let n = points.len();
    debug_assert!(n >= 1);
    if n == 1 {
        return points[0];
    }
    let phase = phase.clamp(0.0, 1.0);
    let t = phase * (n - 1) as f32;
    let i = (t.floor() as usize).min(n - 2);
    let frac = (t - i as f32).clamp(0.0, 1.0);
    let mode = segment_mode(segments, i, curve_default);
    let mode = if mode == WtQuantInterp::MovingAverage {
        WtQuantInterp::Linear
    } else {
        mode
    };
    sample_segment(points, i, frac, mode)
}

fn sample_segment(points: &[f32], i: usize, frac: f32, mode: WtQuantInterp) -> f32 {
    let n = points.len();
    let y1 = points[i];
    let y2 = points[(i + 1).min(n - 1)];
    match mode {
        WtQuantInterp::Hold => y1,
        WtQuantInterp::Linear | WtQuantInterp::MovingAverage => egui::lerp(y1..=y2, frac),
        WtQuantInterp::Spline => {
            let y0 = points[i.saturating_sub(1)];
            let y3 = points[(i + 2).min(n - 1)];
            cubic_catmull(y0, y1, y2, y3, frac)
        }
        WtQuantInterp::Polynomial => {
            let y0 = points[i.saturating_sub(1)];
            let y3 = points[(i + 2).min(n - 1)];
            lagrange_cubic(y0, y1, y2, y3, frac)
        }
        WtQuantInterp::Exponential => {
            const K: f32 = 3.0;
            let eased = ((K * frac).exp() - 1.0) / (K.exp() - 1.0);
            egui::lerp(y1..=y2, eased)
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

fn lagrange_cubic(y0: f32, y1: f32, y2: f32, y3: f32, t: f32) -> f32 {
    let x = 1.0 + t;
    let l0 = ((x - 1.0) * (x - 2.0) * (x - 3.0)) / ((0.0 - 1.0) * (0.0 - 2.0) * (0.0 - 3.0));
    let l1 = ((x - 0.0) * (x - 2.0) * (x - 3.0)) / ((1.0 - 0.0) * (1.0 - 2.0) * (1.0 - 3.0));
    let l2 = ((x - 0.0) * (x - 1.0) * (x - 3.0)) / ((2.0 - 0.0) * (2.0 - 1.0) * (2.0 - 3.0));
    let l3 = ((x - 0.0) * (x - 1.0) * (x - 2.0)) / ((3.0 - 0.0) * (3.0 - 1.0) * (3.0 - 2.0));
    y0 * l0 + y1 * l1 + y2 * l2 + y3 * l3
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
    // Inset so first/last knobs are not flush with the clip edge (easier to grab).
    let inset = 3.0_f32.min(plot.width() * 0.02);
    let left = plot.min.x + inset;
    let right = plot.max.x - inset;
    egui::lerp(left..=right, t)
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
        resample_frame_from_quant_points_uniform(&mut frame, &points, WtQuantInterp::Linear);
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
        apply_quant_slot_amplitude_uniform(bank.frame_mut(0), 3, slot_count, 0.85, WtQuantInterp::Hold);
        let after = sample_at_quant_phase(bank.frame(0), 3, slot_count);
        assert!((after - before).abs() > 0.2);
        assert!((after - 0.85).abs() < 1e-3);
    }

    #[test]
    fn linear_interp_smooths_between_slots() {
        let mut frame = vec![0.0_f32; 256];
        let points = vec![-1.0, 1.0, -1.0, 1.0];
        resample_frame_from_quant_points_uniform(&mut frame, &points, WtQuantInterp::Linear);
        let mid = frame[frame.len() / 4];
        assert!(mid.abs() < 0.95 && mid.abs() > 0.05, "mid should blend, got {mid}");
    }

    #[test]
    fn hold_flat_per_slot_band() {
        set_quant_seam_mode(QuantSeamMode::Off);
        set_crackle_amount(1.0);
        let mut frame = vec![0.0_f32; 256];
        let points = vec![0.0, 1.0, 0.0, 1.0];
        resample_frame_from_quant_points_uniform(&mut frame, &points, WtQuantInterp::Hold);
        let q1 = frame[32];
        let q2 = frame[96];
        assert!((q1 - 0.0).abs() < 1e-3, "got q1={q1}");
        assert!((q2 - 1.0).abs() < 1e-3, "got q2={q2}");
    }

    #[test]
    fn frame_y_roundtrip() {
        let plot = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 100.0));
        let frame = 128.0;
        let y = frame_to_y(frame, plot);
        let back = y_to_frame(y, plot);
        assert!((back - frame).abs() < 2.0, "roundtrip {frame} -> {back}");
    }

    /// Idle knobs stay compact; hover/drag grow + glow so the snapped slot is obvious.
    #[test]
    fn quant_knob_visual_enlarges_and_glows_on_hover() {
        let idle = quant_knob_visual(false, false);
        let hover = quant_knob_visual(true, false);
        let drag = quant_knob_visual(true, true);
        assert!(hover.radius > idle.radius * 1.3, "hover must enlarge clearly");
        assert!(drag.radius >= hover.radius, "drag at least as large as hover");
        assert!(hover.glow_radius.is_some(), "hover needs outer glow ring");
        assert!(drag.glow_radius.is_some(), "drag needs outer glow ring");
        assert!(
            hover.stroke_width > idle.stroke_width,
            "hover stroke thicker"
        );
        assert!(hover.fill_brighter, "hover fill brighter than idle");
        assert!(hover.show_slot_guide, "hover shows vertical slot guide");
        assert!(!idle.show_slot_guide);
    }

    /// Curve stroke widens while a knob on that curve is hovered/dragged.
    #[test]
    fn quant_curve_stroke_thickens_when_active() {
        let (idle_w, _) = quant_curve_stroke(false);
        let (active_w, _) = quant_curve_stroke(true);
        assert!(active_w > idle_w + 0.5, "active curve must read thicker");
    }

    #[test]
    fn quant_hover_status_names_slot_and_amp() {
        let label = quant_hover_status_label(3, -0.42);
        assert!(
            label.contains("Slot 4") && label.contains("-0.42"),
            "got {label}"
        );
    }
    #[test]
    fn last_quant_knob_stays_editable_with_adaptive_seam() {
        set_quant_seam_mode(QuantSeamMode::Adaptive);
        let mut frame = vec![0.0_f32; 256];
        let n = 8usize;
        let init: Vec<f32> = (0..n).map(|i| -0.5 + i as f32 / (n - 1) as f32).collect();
        resample_frame_from_quant_points_uniform(&mut frame, &init, WtQuantInterp::Linear);
        apply_quant_slot_amplitude_uniform(
            &mut frame,
            n - 1,
            n,
            0.75,
            WtQuantInterp::Linear,
        );
        let points = quant_control_points(&frame, n);
        assert!(
            (points[n - 1] - 0.75).abs() < 0.08,
            "last knob must retain edited value, got {}",
            points[n - 1]
        );
        assert!(
            (points[0] - 0.75).abs() < 0.08,
            "first knob linked to last under Adaptive, got {}",
            points[0]
        );
    }

    #[test]
    fn adaptive_seam_uses_short_fade_when_already_closed() {
        let mut frame = vec![0.5_f32; 256];
        frame[200] = 0.4;
        periodize_quant_frame_with_mode(&mut frame, QuantSeamMode::Adaptive);
        // Ends already match — Adaptive should not demolish the body.
        assert!((frame[200] - 0.4).abs() < 0.15, "body preserved");
        assert!((frame[frame.len() - 1] - frame[0]).abs() < 1e-3);
    }

    #[test]
    fn linear_hits_endpoints() {
        set_quant_seam_mode(QuantSeamMode::Off);
        set_crackle_amount(1.0);
        let mut frame = vec![0.0_f32; 128];
        let points = vec![-0.5_f32, 0.8];
        resample_frame_from_quant_points_uniform(&mut frame, &points, WtQuantInterp::Linear);
        assert!(
            (frame[0] - (-0.5)).abs() < 1e-3,
            "first sample got {}",
            frame[0]
        );
        assert!(
            (frame[frame.len() - 1] - 0.8).abs() < 1e-3,
            "last sample got {}",
            frame[frame.len() - 1]
        );
        set_crackle_amount(0.0);
        periodize_quant_frame_with_mode(&mut frame, QuantSeamMode::Soft);
        assert!(
            (frame[frame.len() - 1] - frame[0]).abs() < 1e-3,
            "soft periodize must close wrap seam"
        );
    }

    #[test]
    fn quant_hold_wrap_seam_is_periodized() {
        set_quant_seam_mode(QuantSeamMode::Soft);
        set_crackle_amount(0.0);
        let mut frame = vec![0.0_f32; 512];
        // Opposite endpoints — classic Hold wrap cliff without periodize.
        let points = vec![1.0_f32, 0.0, 0.0, -1.0];
        resample_frame_from_quant_points_uniform(&mut frame, &points, WtQuantInterp::Hold);
        let seam = (frame[frame.len() - 1] - frame[0]).abs();
        assert!(
            seam < 1e-3,
            "Hold quant wrap must be periodized (seam={seam})"
        );
        // Adjacent samples near wrap must not be a near-vertical cliff.
        let jump = (frame[0] - frame[frame.len() - 2]).abs();
        assert!(
            jump < 0.35,
            "periodized wrap adjacent jump too steep: {jump}"
        );
    }

    #[test]
    fn expo_monotonic_same_sign_endpoints() {
        set_quant_seam_mode(QuantSeamMode::Off);
        set_crackle_amount(1.0);
        let mut frame = vec![0.0_f32; 128];
        let points = vec![0.2_f32, 0.9];
        resample_frame_from_quant_points_uniform(&mut frame, &points, WtQuantInterp::Exponential);
        // Off/Raw leaves wrap open — only assert the rising body, not the seam sample.
        let body_end = frame.len().saturating_sub(2).max(2);
        for w in frame[..body_end].windows(2) {
            assert!(w[1] + 1e-4 >= w[0], "expo body must be non-decreasing");
        }
        assert!(frame[body_end - 1] > frame[0] + 0.3, "expo must climb");
    }

    #[test]
    fn ma_reduces_high_freq_vs_hold() {
        let points = vec![1.0_f32, -1.0, 1.0, -1.0, 1.0];
        let mut hold = vec![0.0_f32; 256];
        let mut ma = vec![0.0_f32; 256];
        resample_frame_from_quant_points_uniform(&mut hold, &points, WtQuantInterp::Hold);
        resample_frame_from_quant_points_uniform(&mut ma, &points, WtQuantInterp::MovingAverage);
        // Hold jumps by ~2.0 at the first segment boundary; MA should ease across it.
        let b = ((hold.len() - 1) as f32 * 0.25).round() as usize;
        let hold_jump = (hold[b] - hold[b.saturating_sub(1)]).abs();
        let ma_jump = (ma[b] - ma[b.saturating_sub(1)]).abs();
        assert!(
            hold_jump > 1.5,
            "Hold should cliff at band edge, jump={hold_jump}"
        );
        assert!(
            ma_jump < hold_jump * 0.5,
            "MA should ease Hold cliff (ma_jump={ma_jump} hold_jump={hold_jump})"
        );
    }

    #[test]
    fn per_segment_modes_differ_from_uniform() {
        let points = vec![0.0_f32, 1.0, 0.0, 1.0];
        let mut uniform = vec![0.0_f32; 128];
        let mut mixed = vec![0.0_f32; 128];
        resample_frame_from_quant_points_uniform(&mut uniform, &points, WtQuantInterp::Hold);
        let segs = [
            WtQuantInterp::Hold,
            WtQuantInterp::Linear,
            WtQuantInterp::Spline,
        ];
        resample_frame_from_quant_points(&mut mixed, &points, &segs, WtQuantInterp::Hold);
        let diff: f32 = uniform.iter().zip(mixed.iter()).map(|(a, b)| (a - b).abs()).sum();
        assert!(diff > 1.0);
    }

    #[test]
    fn edge_knobs_always_in_control_points() {
        for n in [2usize, 8, 16, 65, 256] {
            let frame = vec![0.25_f32; 512];
            let pts = quant_control_points(&frame, n);
            assert_eq!(pts.len(), n);
            assert!((pts[0] - 0.25).abs() < 1e-3);
            assert!((pts[n - 1] - 0.25).abs() < 1e-3);
        }
    }

}
