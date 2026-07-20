//! 2D stack overlay — all wave layers composited in one scope view.

use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Ui, Vec2};
use reelsynth::osc::{layer_sign, sample_layer, StackMode, WtWarpMode};
use reelsynth::patch::WaveLayer;
use reelsynth::WavetableBank;
use reelsynth_ui_theme::Tokens;

use crate::audit_registry::{record_region, AuditId};
use crate::layout::RADIUS_SM;
use crate::oscillator_ui::WaveLayerUi;
use crate::region::region;
use crate::state::WtView3dMode;

use super::quant_handles::{
    apply_quant_slot_amplitude, knob_y_on_curve, nearest_quant_handle, paint_quant_knob,
    quant_control_points, quant_hover_status_label, quant_knob_visual, sample_from_knob_y,
    slot_x,
};
use super::residual::layer_curve_label;
use super::slots::effective_quant_count;
use super::waveform::{
    frame_index, hovered_layer_from_pointer, layers_pointer_prefers_curve_select, peak_point,
    quant_knobs_for_selection, selection_from_curve_click,
};
use super::view_zoom::{consume_plot_scroll, WtCurveViewTransform};

pub(crate) const HOVER_DISTANCE_PX: f32 = 14.0;
pub(crate) const WAVE_AMP: f32 = 0.42;
pub(crate) const WAVE_SAMPLES: usize = 256;

pub(crate) fn layer_palette(i: usize) -> Color32 {
    const COLORS: [Color32; 6] = [
        Color32::from_rgb(0x5b, 0xc0, 0xde),
        Color32::from_rgb(0x4a, 0xde, 0x80),
        Color32::from_rgb(0xde, 0x8a, 0x4a),
        Color32::from_rgb(0xc0, 0x5b, 0xde),
        Color32::from_rgb(0xde, 0x5b, 0x7a),
        Color32::from_rgb(0xde, 0xde, 0x4a),
    ];
    COLORS[i % COLORS.len()]
}

fn ui_layer_to_patch(layer: &WaveLayerUi) -> WaveLayer {
    layer.to_patch()
}

fn sample_layer_at_phase(
    layer: &WaveLayerUi,
    bank: &WavetableBank,
    phase: f32,
    wt_pos_offset: f32,
) -> f32 {
    if !layer.enabled || layer.level <= 0.0 {
        return 0.0;
    }
    let patch_layer = ui_layer_to_patch(layer);
    sample_layer(
        &patch_layer,
        bank,
        phase,
        // Display one cycle over WAVE_SAMPLES; blep_dt widens the wrap cliff.
        1.0 / WAVE_SAMPLES as f32,
        wt_pos_offset,
        WtWarpMode::None,
        0.0,
        0.0,
        0.0,
        1.0,
    )
}

pub fn composite_stack_sample(
    layers: &[WaveLayerUi],
    bank: &WavetableBank,
    stack_mode: &str,
    phase: f32,
    wt_pos_offset: f32,
) -> f32 {
    let mode = StackMode::from_str(stack_mode);
    let mut sum = 0.0f32;
    let mut weight = 0.0f32;
    let mut count = 0u32;
    for layer in layers {
        if !layer.enabled || layer.level <= 0.0 {
            continue;
        }
        let patch = ui_layer_to_patch(layer);
        let sign = layer_sign(&patch);
        let s = sample_layer_at_phase(layer, bank, phase, wt_pos_offset);
        let signed = sign * s * layer.level;
        match mode {
            StackMode::Add => sum += signed,
            StackMode::Avg => {
                sum += signed;
                weight += layer.level.abs();
            }
            StackMode::AvgEqual => {
                sum += sign * s;
                count += 1;
            }
        }
    }
    match mode {
        StackMode::Add => sum,
        StackMode::Avg => {
            if weight <= 0.0 {
                0.0
            } else {
                sum / weight
            }
        }
        StackMode::AvgEqual => {
            if count == 0 {
                0.0
            } else {
                sum / count as f32
            }
        }
    }
}

pub(crate) fn layer_waveform_points(
    layer: &WaveLayerUi,
    bank: &WavetableBank,
    rect: Rect,
    wt_pos_offset: f32,
    samples: usize,
) -> Vec<Pos2> {
    let patch = ui_layer_to_patch(layer);
    let sign = layer_sign(&patch);
    let level = if layer.enabled { layer.level.max(0.0) } else { 0.0 };
    let samples = samples.max(2);
    let mut pts = Vec::with_capacity(samples);
    // Sample phase in [0, 1) only. Including phase 1.0 wraps to 0 via fract() and
    // duplicates the start sample at the right edge (visible end jump).
    for i in 0..samples {
        let phase = i as f32 / samples as f32;
        let t = i as f32 / (samples - 1) as f32;
        let v = sign * sample_layer_at_phase(layer, bank, phase, wt_pos_offset) * level;
        let x = egui::lerp(rect.min.x..=rect.max.x, t);
        let y = rect.center().y - v * rect.height() * WAVE_AMP;
        pts.push(Pos2::new(x, y));
    }
    pts
}

pub(crate) fn composite_waveform_points(
    layers: &[WaveLayerUi],
    bank: &WavetableBank,
    stack_mode: &str,
    rect: Rect,
    wt_pos_offset: f32,
    samples: usize,
) -> Vec<Pos2> {
    let samples = samples.max(2);
    let mut pts = Vec::with_capacity(samples);
    // Same [0, 1) phase rule as `layer_waveform_points` — avoid phase 1.0 wrap jump.
    for i in 0..samples {
        let phase = i as f32 / samples as f32;
        let t = i as f32 / (samples - 1) as f32;
        let v = composite_stack_sample(layers, bank, stack_mode, phase, wt_pos_offset);
        let x = egui::lerp(rect.min.x..=rect.max.x, t);
        let y = rect.center().y - v * rect.height() * WAVE_AMP;
        pts.push(Pos2::new(x, y));
    }
    pts
}

/// Caption for the Selected pane — tracks the selected layer type.
#[allow(dead_code)]
pub(crate) fn selected_layer_edit_label(
    selected: Option<usize>,
    layers: &[WaveLayerUi],
) -> String {
    match selected.and_then(|i| layers.get(i).map(|l| (i, l))) {
        Some((i, layer)) => format!(
            "Edit · Layer {} · {}",
            i + 1,
            layer.source_type
        ),
        None => "Layers · pick a layer".into(),
    }
}

/// Primary edit curve for the selected layer (right pane focus).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn primary_layer_waveform_points(
    selected: Option<usize>,
    layers: &[WaveLayerUi],
    bank: &WavetableBank,
    rect: Rect,
    wt_pos_offset: f32,
    samples: usize,
) -> Vec<Pos2> {
    let idx = selected.unwrap_or(0);
    layers
        .get(idx)
        .map(|layer| layer_waveform_points(layer, bank, rect, wt_pos_offset, samples))
        .unwrap_or_default()
}

/// Quant knob display scale — matches selected layer level × invert sign.
pub(crate) fn layer_quant_display_scale(layer: &WaveLayerUi) -> f32 {
    let sign = if layer.invert { -1.0 } else { 1.0 };
    let level = if layer.enabled { layer.level.max(0.0) } else { 0.0 };
    sign * level.max(0.05)
}

fn paint_grid(painter: &egui::Painter, rect: Rect, border: Color32) {
    let step = 24.0;
    let stroke = egui::Stroke::new(0.5, border.gamma_multiply(0.75));
    let mut x = rect.min.x;
    while x <= rect.max.x {
        painter.line_segment(
            [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
            stroke,
        );
        x += step;
    }
    let mut y = rect.min.y;
    while y <= rect.max.y {
        painter.line_segment(
            [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
            stroke,
        );
        y += step;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum StackDragTarget {
    None,
    Layer(usize),
}

pub struct WtView3dStackResponse {
    pub layer_selected: bool,
    pub wt_position_changed: bool,
    pub global_wt_scrub: bool,
    pub frame_edited: bool,
    pub status_hint: Option<String>,
}

pub struct WtView3dStack<'a> {
    pub layers: &'a mut [WaveLayerUi],
    pub stack_mode: &'a str,
    pub bank: Option<&'a mut WavetableBank>,
    pub wt_pos_offset: f32,
    pub wt_position: &'a mut f32,
    pub selected_layer: &'a mut Option<usize>,
    pub view_mode: Option<&'a mut WtView3dMode>,
    /// When false, hide Stack/Morph toggle (Design composite pane).
    pub show_mode_toggle: bool,
    /// Osc index for pane caption (`Layers · Osc N`).
    pub active_osc: usize,
    pub time: f32,
    /// Osc quant count — enables snap knobs on the **selected** wavetable layer.
    pub wave_quant: u8,
    /// Shared Design curve zoom / pan (mouse wheel).
    pub curve_view: &'a mut WtCurveViewTransform,
}

impl WtView3dStack<'_> {
    pub fn show(self, ui: &mut Ui) -> WtView3dStackResponse {
        let tokens = Tokens::default();
        let view_h = ui.available_height().max(48.0);

        let mut layer_selected = false;
        let mut wt_position_changed = false;
        let global_wt_scrub = false;
        let mut frame_edited = false;
        let mut status_hint: Option<String> = None;

        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), view_h),
            Sense::hover(),
        );

        if !ui.is_rect_visible(rect) {
            return WtView3dStackResponse {
                layer_selected,
                wt_position_changed,
                global_wt_scrub,
                frame_edited,
                status_hint,
            };
        }

        let inner = rect.shrink2(egui::vec2(8.0, 20.0));
        let mid_y = inner.center().y;

        let _ = consume_plot_scroll(ui, inner, self.curve_view);
        let curve_view = *self.curve_view;
        let hit_r = curve_view.hit_radius(HOVER_DISTANCE_PX);

        record_region(ui.ctx(), AuditId::CenterWt3dStack, rect, rect);

        let bank = match self.bank {
            Some(b) => b,
            None => {
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, RADIUS_SM, tokens.bg);
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "No bank",
                    egui::FontId::proportional(12.0),
                    tokens.text_muted,
                );
                return WtView3dStackResponse {
                    layer_selected,
                    wt_position_changed,
                    global_wt_scrub,
                    frame_edited,
                    status_hint,
                };
            }
        };

        // VA → WT bake so Quant knobs work on every selected audible curve.
        if self.wave_quant > 0 {
            if let Some(idx) = *self.selected_layer {
                let occupied: Vec<usize> = self
                    .layers
                    .iter()
                    .enumerate()
                    .filter(|(i, l)| *i != idx && l.is_wavetable())
                    .map(|(_, l)| frame_index(l.wt_position, bank.num_frames))
                    .collect();
                if let Some(layer) = self.layers.get_mut(idx) {
                    if super::waveform::layer_quant_editable(layer) && layer.is_va() {
                        if crate::wt::promote_va_layer_for_quant(layer, bank, &occupied) {
                            frame_edited = true;
                        }
                    }
                }
            }
        }

        let active_indices: Vec<usize> = self
            .layers
            .iter()
            .enumerate()
            .filter(|(_, l)| l.enabled && l.level > 0.0)
            .map(|(i, _)| i)
            .collect();

        let layer_points: Vec<(usize, Vec<Pos2>, bool)> = active_indices
            .iter()
            .map(|&idx| {
                (
                    idx,
                    layer_waveform_points(
                        &self.layers[idx],
                        bank,
                        inner,
                        self.wt_pos_offset,
                        WAVE_SAMPLES,
                    ),
                    self.layers[idx].invert,
                )
            })
            .collect();
        // Screen-space polylines after zoom/pan (hover + paint share these).
        let screen_points: Vec<(usize, Vec<Pos2>, bool)> = layer_points
            .iter()
            .map(|(idx, pts, inv)| (*idx, curve_view.map_points(pts, inner), *inv))
            .collect();

        let selected_idx = *self.selected_layer;
        // Quant knobs only on the selected WT/residual curve — siblings stay stroke-only.
        let quant_layer_idx =
            quant_knobs_for_selection(selected_idx, self.layers, self.wave_quant);
        let quant_active = quant_layer_idx.is_some();
        let edit_frame_idx = quant_layer_idx
            .and_then(|i| self.layers.get(i).map(|l| l.wt_position))
            .map(|p| frame_index(p, bank.num_frames))
            .unwrap_or_else(|| frame_index(*self.wt_position, bank.num_frames));

        let drag_slot_id = ui.id().with("layers_multi_quant_drag");
        let drag_layer_id = ui.id().with("layers_multi_quant_layer");
        let drag_target_id = ui.id().with("stack_drag_target");

        let pointer_in_plot = ui
            .ctx()
            .pointer_latest_pos()
            .filter(|p| inner.contains(*p));
        let pointer_plot = pointer_in_plot.map(|pos| curve_view.unmap_pos(pos, inner));
        let hovered_curve = pointer_in_plot.and_then(|pos| {
            hovered_layer_from_pointer(
                screen_points
                    .iter()
                    .map(|(idx, pts, _)| (*idx, pts.as_slice())),
                pos,
                HOVER_DISTANCE_PX,
            )
        });
        // Knob proximity wins for interaction; curve preview only when not on a knob.
        let over_quant_knob = quant_active
            && pointer_plot.is_some_and(|pos| {
                quant_layer_idx.is_some_and(|layer_i| {
                    self.layers.get(layer_i).is_some_and(|layer| {
                        let slot_count = effective_quant_count(self.wave_quant);
                        let scale = layer_quant_display_scale(layer);
                        let points = quant_control_points(bank.frame(edit_frame_idx), slot_count);
                        nearest_quant_handle(pos, inner, &points, scale, hit_r).is_some()
                    })
                })
            });
        let curve_preview = if over_quant_knob
            && !layers_pointer_prefers_curve_select(hovered_curve, quant_layer_idx, true)
        {
            None
        } else {
            hovered_curve
        };

        let quant_locked = ui.ctx().data(|d| d.get_temp::<usize>(drag_slot_id).is_some());

        // When Quant knobs are shown, the quant response owns the plot hit-test
        // (so knob proximity can win). Otherwise the stack interact owns clicks.
        let response = if quant_active {
            None
        } else {
            Some(ui.interact(
                rect,
                ui.id().with("stack_interact"),
                Sense::click_and_drag(),
            ))
        };

        if let Some(response) = response.as_ref() {
            if response.clicked() || response.drag_started() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let hovered = hovered_layer_from_pointer(
                        screen_points
                            .iter()
                            .map(|(i, pts, _)| (*i, pts.as_slice())),
                        pos,
                        HOVER_DISTANCE_PX,
                    );
                    if let Some(idx) = selection_from_curve_click(hovered, false) {
                        *self.selected_layer = Some(idx);
                        layer_selected = true;
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(drag_target_id, StackDragTarget::Layer(idx))
                        });
                    } else if response.drag_started() {
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(drag_target_id, StackDragTarget::None));
                    }
                }
            }

            if response.dragged() {
                let target = ui
                    .ctx()
                    .data(|d| d.get_temp(drag_target_id))
                    .unwrap_or(StackDragTarget::None);
                let delta = response.drag_delta();
                match target {
                    StackDragTarget::Layer(sel) => {
                        if let Some(layer) = self.layers.get_mut(sel) {
                            if delta.x.abs() > 0.0 {
                                let max_pos = (bank.num_frames.saturating_sub(1)).max(1) as f32;
                                let px_per_frame = inner.width() / max_pos.max(1.0);
                                if layer.is_wavetable() {
                                    layer.wt_position = (layer.wt_position + delta.x / px_per_frame)
                                        .clamp(0.0, max_pos);
                                    wt_position_changed = true;
                                } else {
                                    layer.phase += delta.x / inner.width() * std::f32::consts::TAU;
                                    wt_position_changed = true;
                                }
                            }
                            if delta.y.abs() > 0.0 {
                                let next = (layer.level - delta.y / inner.height()).clamp(0.0, 1.0);
                                if (next - layer.level).abs() > f32::EPSILON {
                                    layer.level = next;
                                    wt_position_changed = true;
                                }
                            }
                        }
                    }
                    StackDragTarget::None => {}
                }
            }

            if response.hovered() {
                ui.ctx().set_cursor_icon(if response.dragged() {
                    CursorIcon::Grabbing
                } else if curve_preview.is_some() {
                    CursorIcon::PointingHand
                } else {
                    CursorIcon::Grab
                });
            }
        }

        if quant_active {
            let slot_count = effective_quant_count(self.wave_quant);
            let locked_slot: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_slot_id));
            let locked_layer: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_layer_id));
            let layer_i = quant_layer_idx.expect("quant_active implies selected WT layer");

            let sense = Sense::click_and_drag();
            let q_response = ui.allocate_rect(inner, sense);
            let pointer = q_response
                .interact_pointer_pos()
                .filter(|p| inner.contains(*p));

            let nearest_slot = pointer.and_then(|pos| {
                let plot_pos = curve_view.unmap_pos(pos, inner);
                self.layers.get(layer_i).and_then(|layer| {
                    let scale = layer_quant_display_scale(layer);
                    let points = quant_control_points(bank.frame(edit_frame_idx), slot_count);
                    nearest_quant_handle(plot_pos, inner, &points, scale, hit_r)
                })
            });
            let over_knob = nearest_slot.is_some() || quant_locked;

            if q_response.clicked() || q_response.drag_started() {
                let hovered = pointer.and_then(|pos| {
                    hovered_layer_from_pointer(
                        screen_points
                            .iter()
                            .map(|(i, pts, _)| (*i, pts.as_slice())),
                        pos,
                        HOVER_DISTANCE_PX,
                    )
                });
                // Sibling curves win over knobs so L1/L2 stay selectable when L3 has knobs.
                if layers_pointer_prefers_curve_select(hovered, quant_layer_idx, nearest_slot.is_some())
                {
                    if let Some(idx) = selection_from_curve_click(hovered, false) {
                        *self.selected_layer = Some(idx);
                        layer_selected = true;
                        if q_response.drag_started() {
                            ui.ctx().data_mut(|d| {
                                d.insert_temp(drag_target_id, StackDragTarget::Layer(idx))
                            });
                        }
                    }
                } else if let Some(slot) = nearest_slot {
                    if q_response.drag_started() {
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(drag_slot_id, slot);
                            d.insert_temp(drag_layer_id, layer_i);
                        });
                    }
                } else if let Some(idx) = selection_from_curve_click(hovered, false) {
                    *self.selected_layer = Some(idx);
                    layer_selected = true;
                    if q_response.drag_started() {
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(drag_target_id, StackDragTarget::Layer(idx))
                        });
                    }
                }
            }
            if !q_response.dragged() && q_response.drag_stopped() {
                ui.ctx().data_mut(|d| {
                    d.remove::<usize>(drag_slot_id);
                    d.remove::<usize>(drag_layer_id);
                });
            }

            // Level / phase drag when not grabbing a knob.
            if q_response.dragged() && !over_knob && !quant_locked {
                let target = ui
                    .ctx()
                    .data(|d| d.get_temp(drag_target_id))
                    .unwrap_or_else(|| {
                        selected_idx
                            .map(StackDragTarget::Layer)
                            .unwrap_or(StackDragTarget::None)
                    });
                let delta = q_response.drag_delta();
                if let StackDragTarget::Layer(sel) = target {
                    if let Some(layer) = self.layers.get_mut(sel) {
                        if delta.x.abs() > 0.0 {
                            let max_pos = (bank.num_frames.saturating_sub(1)).max(1) as f32;
                            let px_per_frame = inner.width() / max_pos.max(1.0);
                            if layer.is_wavetable() {
                                layer.wt_position = (layer.wt_position + delta.x / px_per_frame)
                                    .clamp(0.0, max_pos);
                                wt_position_changed = true;
                            } else {
                                layer.phase += delta.x / inner.width() * std::f32::consts::TAU;
                                wt_position_changed = true;
                            }
                        }
                        if delta.y.abs() > 0.0 {
                            let next = (layer.level - delta.y / inner.height()).clamp(0.0, 1.0);
                            if (next - layer.level).abs() > f32::EPSILON {
                                layer.level = next;
                                wt_position_changed = true;
                            }
                        }
                    }
                }
            }

            let active_slot = locked_slot.or(nearest_slot);
            let editing_this = locked_layer == Some(layer_i) || locked_layer.is_none();

            if let Some(slot) = active_slot {
                if editing_this && q_response.dragged() && (quant_locked || nearest_slot.is_some()) {
                    if let Some(pos) = pointer {
                        if let Some(layer) = self.layers.get_mut(layer_i) {
                            layer.ensure_segment_interps(slot_count);
                            let segs = layer.quant_segment_interps.clone();
                            let curve_default = layer.quant_interp;
                            let scale = layer_quant_display_scale(layer);
                            let plot_pos = curve_view.unmap_pos(pos, inner);
                            let sample = sample_from_knob_y(plot_pos.y, scale, inner);
                            apply_quant_slot_amplitude(
                                bank.frame_mut(edit_frame_idx),
                                slot,
                                slot_count,
                                sample,
                                &segs,
                                curve_default,
                            );
                            frame_edited = true;
                            status_hint = Some(format!(
                                "L{} · {}",
                                layer_i + 1,
                                quant_hover_status_label(slot, sample)
                            ));
                        }
                    }
                } else if pointer.is_some() && editing_this && over_knob {
                    let points = quant_control_points(bank.frame(edit_frame_idx), slot_count);
                    let amp = points.get(slot).copied().unwrap_or(0.0);
                    status_hint = Some(format!(
                        "L{} · {}",
                        layer_i + 1,
                        quant_hover_status_label(slot, amp)
                    ));
                    ui.ctx().set_cursor_icon(if q_response.dragged() {
                        CursorIcon::Grabbing
                    } else {
                        CursorIcon::Grab
                    });
                }
            } else if let Some(idx) = curve_preview {
                // Not on a quant knob — preview which curve click would select.
                if let Some(layer) = self.layers.get(idx) {
                    status_hint = Some(format!("Hover · {}", layer_curve_label(idx, layer)));
                }
                if q_response.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
                }
            }
        } else if let Some(idx) = curve_preview {
            if let Some(layer) = self.layers.get(idx) {
                status_hint = Some(format!("Hover · {}", layer_curve_label(idx, layer)));
            }
        }

        let mut painter = ui.painter_at(rect);
        painter.set_clip_rect(inner.expand(1.0));
        painter.rect_filled(rect, RADIUS_SM, tokens.bg);
        painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0, tokens.border));

        paint_grid(&painter, inner, tokens.border);
        painter.line_segment(
            [Pos2::new(inner.min.x, mid_y), Pos2::new(inner.max.x, mid_y)],
            egui::Stroke::new(1.0, tokens.border.gamma_multiply(0.75)),
        );

        if quant_active {
            let quant = effective_quant_count(self.wave_quant);
            for i in 0..quant {
                let x = curve_view
                    .map_pos(Pos2::new(slot_x(i, quant, inner), mid_y), inner)
                    .x;
                painter.line_segment(
                    [Pos2::new(x, inner.min.y), Pos2::new(x, inner.max.y)],
                    egui::Stroke::new(0.5, tokens.border.gamma_multiply(0.5)),
                );
            }
        }

        let label = if quant_active {
            if curve_view.zoom > 1.01 {
                format!(
                    "Layers · Osc {} · Quant · zoom {:.1}× · wheel zoom · Shift+wheel pan",
                    self.active_osc + 1,
                    curve_view.zoom
                )
            } else {
                format!(
                    "Layers · Osc {} · Quant · drag WT dots · wheel zoom",
                    self.active_osc + 1
                )
            }
        } else {
            format!(
                "Layers · Osc {} · {}/{} · drag level / phase",
                self.active_osc + 1,
                active_indices.len(),
                self.layers.len()
            )
        };
        painter.text(
            Pos2::new(rect.min.x + 8.0, rect.min.y + 6.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(10.0),
            tokens.text_secondary,
        );

        if self.show_mode_toggle {
            if let Some(mode) = self.view_mode {
                region(
                    ui,
                    Rect::from_min_max(
                        egui::pos2(rect.max.x - 120.0, rect.min.y + 4.0),
                        egui::pos2(rect.max.x - 4.0, rect.min.y + 22.0),
                    ),
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.selectable_value(mode, WtView3dMode::Stack, "Stack");
                            ui.selectable_value(mode, WtView3dMode::Morph, "Morph");
                        });
                        let toggle_rect = ui.min_rect();
                        record_region(
                            ui.ctx(),
                            AuditId::CenterWt3dModeToggle,
                            toggle_rect,
                            toggle_rect,
                        );
                    },
                );
            }
        }

        // Paint order: dim siblings → selected (if not hovered) → hovered / quant-hot on top.
        let mut paint_order: Vec<usize> = (0..layer_points.len()).collect();
        paint_order.sort_by_key(|&i| {
            let orig = layer_points[i].0;
            let quant_hot = quant_active && {
                let locked_layer: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_layer_id));
                locked_layer == Some(orig) || (over_quant_knob && quant_layer_idx == Some(orig))
            };
            let is_hover = curve_preview == Some(orig);
            let is_sel = selected_idx == Some(orig);
            if quant_hot || is_hover {
                2u8
            } else if is_sel {
                1u8
            } else {
                0u8
            }
        });

        let curve_hover_active = curve_preview.is_some();

        for &paint_i in &paint_order {
            let (orig_idx, pts, inverted) = &screen_points[paint_i];
            let orig_idx = *orig_idx;
            let inverted = *inverted;
            if pts.len() < 2 {
                continue;
            }
            let color = layer_palette(orig_idx);
            let is_sel = selected_idx == Some(orig_idx);
            let is_hover = curve_preview == Some(orig_idx);
            let quant_hot = quant_active
                && quant_layer_idx == Some(orig_idx)
                && (over_quant_knob
                    || ui
                        .ctx()
                        .data(|d| d.get_temp::<usize>(drag_layer_id) == Some(orig_idx)));
            let (alpha, stroke_w) = if quant_hot {
                (0.95, 3.2)
            } else if is_hover {
                (0.92, 2.9)
            } else if is_sel {
                if curve_hover_active {
                    (0.62, 1.9)
                } else {
                    (0.75, 2.0)
                }
            } else if curve_hover_active {
                (0.28, 1.15)
            } else {
                (0.45, 1.4)
            };
            if inverted {
                let dash_stroke = egui::Stroke::new(stroke_w, color.gamma_multiply(alpha));
                for chunk in pts.windows(2).step_by(2) {
                    if chunk.len() == 2 {
                        painter.line_segment([chunk[0], chunk[1]], dash_stroke);
                    }
                }
            } else {
                if is_hover && !quant_hot {
                    // Subtle outline so the preview target reads before click.
                    painter.add(Shape::line(
                        pts.clone(),
                        egui::Stroke::new(stroke_w + 2.0, color.gamma_multiply(0.35)),
                    ));
                }
                painter.add(Shape::line(
                    pts.clone(),
                    egui::Stroke::new(stroke_w, color.gamma_multiply(alpha)),
                ));
            }
            if let Some(peak) = peak_point(pts) {
                let lbl = layer_curve_label(orig_idx, &self.layers[orig_idx]);
                let label_alpha = if is_hover || quant_hot {
                    1.0
                } else if is_sel {
                    0.9
                } else if curve_hover_active {
                    0.55
                } else {
                    0.9
                };
                painter.text(
                    Pos2::new(peak.x + 4.0, peak.y - 10.0),
                    egui::Align2::LEFT_BOTTOM,
                    lbl,
                    egui::FontId::proportional(if is_hover { 10.0 } else { 9.0 }),
                    color.gamma_multiply(label_alpha),
                );
            }
        }

        if quant_active {
            if let Some(layer_i) = quant_layer_idx {
                if let Some(layer) = self.layers.get(layer_i) {
                    let slot_count = effective_quant_count(self.wave_quant);
                    let locked_slot: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_slot_id));
                    let locked_layer: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_layer_id));
                    let pointer = ui
                        .ctx()
                        .pointer_latest_pos()
                        .filter(|p| inner.contains(*p));
                    let scale = layer_quant_display_scale(layer);
                    let points = quant_control_points(bank.frame(edit_frame_idx), slot_count);
                    let hover_slot = if locked_layer.is_none() {
                        pointer.and_then(|pos| {
                            let plot_pos = curve_view.unmap_pos(pos, inner);
                            nearest_quant_handle(plot_pos, inner, &points, scale, hit_r)
                        })
                    } else {
                        None
                    };
                    let color = layer_palette(layer_i);
                    for i in 0..slot_count {
                        let dragged =
                            locked_layer == Some(layer_i) && locked_slot == Some(i);
                        let hovered = hover_slot == Some(i);
                        let show = self.wave_quant <= 64
                            || hovered
                            || dragged
                            || i == 0
                            || i + 1 == slot_count;
                        if !show {
                            continue;
                        }
                        let x = slot_x(i, slot_count, inner);
                        let sample = points.get(i).copied().unwrap_or(0.0);
                        let y = knob_y_on_curve(sample, scale, inner);
                        let center = curve_view.map_pos(Pos2::new(x, y), inner);
                        let visual = quant_knob_visual(hovered, dragged);
                        let fill = if visual.fill_brighter {
                            color.gamma_multiply(if dragged { 0.65 } else { 0.5 })
                        } else {
                            color.gamma_multiply(0.22)
                        };
                        paint_quant_knob(&painter, center, visual, fill, color, inner);
                    }
                }
            }
        }

        WtView3dStackResponse {
            layer_selected,
            wt_position_changed,
            global_wt_scrub,
            frame_edited,
            status_hint,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;

    #[test]
    fn composite_stack_sample_respects_levels() {
        let bank = WavetableBank::factory_saw_morph();
        let layers = vec![
            WaveLayerUi {
                source_type: "sine".into(),
                level: 1.0,
                enabled: true,
                ..WaveLayerUi::default()
            },
            WaveLayerUi {
                source_type: "sine".into(),
                level: 0.0,
                enabled: true,
                ..WaveLayerUi::default()
            },
        ];
        let s = composite_stack_sample(&layers, &bank, "add", 0.25, 0.0);
        assert!(s.abs() > 0.0);
    }

    #[test]
    fn invert_flips_composite_sign() {
        let bank = WavetableBank::factory_saw_morph();
        let positive = vec![WaveLayerUi {
            source_type: "sine".into(),
            level: 1.0,
            enabled: true,
            invert: false,
            ..WaveLayerUi::default()
        }];
        let negative = vec![WaveLayerUi {
            source_type: "sine".into(),
            level: 1.0,
            enabled: true,
            invert: true,
            ..WaveLayerUi::default()
        }];
        let p = composite_stack_sample(&positive, &bank, "add", 0.25, 0.0);
        let n = composite_stack_sample(&negative, &bank, "add", 0.25, 0.0);
        assert!((p + n).abs() < 1e-4, "invert should flip sign: p={p} n={n}");
    }

    #[test]
    fn avg_equal_differs_from_weighted_avg() {
        let bank = WavetableBank::factory_saw_morph();
        let layers = vec![
            WaveLayerUi {
                source_type: "saw".into(),
                level: 1.0,
                enabled: true,
                ..WaveLayerUi::default()
            },
            WaveLayerUi {
                source_type: "sine".into(),
                level: 0.25,
                enabled: true,
                ..WaveLayerUi::default()
            },
        ];
        let avg = composite_stack_sample(&layers, &bank, "avg", 0.25, 0.0);
        let eq = composite_stack_sample(&layers, &bank, "avg_equal", 0.25, 0.0);
        assert!(
            (avg - eq).abs() > 1e-4,
            "weighted avg ({avg}) should differ from equal avg ({eq})"
        );
    }

    /// Right-pane caption / primary curve must follow the selected layer type.
    #[test]
    fn selected_layer_edit_label_tracks_selection() {
        let layers = vec![
            WaveLayerUi {
                source_type: "saw".into(),
                level: 0.55,
                enabled: true,
                ..WaveLayerUi::default()
            },
            WaveLayerUi {
                source_type: "sine".into(),
                level: 0.30,
                enabled: true,
                ..WaveLayerUi::default()
            },
        ];
        let a = selected_layer_edit_label(Some(0), &layers);
        let b = selected_layer_edit_label(Some(1), &layers);
        assert!(a.contains("saw"), "{a}");
        assert!(b.contains("sine"), "{b}");
        assert_ne!(a, b);
    }

    /// Selecting a different VA layer must change the primary edit waveform.
    #[test]
    fn primary_edit_curve_changes_with_selected_layer() {
        let bank = WavetableBank::factory_saw_morph();
        let plot = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(200.0, 100.0));
        let layers = vec![
            WaveLayerUi {
                source_type: "saw".into(),
                level: 1.0,
                enabled: true,
                ..WaveLayerUi::default()
            },
            WaveLayerUi {
                source_type: "sine".into(),
                level: 1.0,
                enabled: true,
                ..WaveLayerUi::default()
            },
        ];
        let saw = primary_layer_waveform_points(Some(0), &layers, &bank, plot, 0.0, 64);
        let sine = primary_layer_waveform_points(Some(1), &layers, &bank, plot, 0.0, 64);
        assert_eq!(saw.len(), sine.len());
        let mut diff = 0.0f32;
        for (a, b) in saw.iter().zip(sine.iter()) {
            diff += (a.y - b.y).abs();
        }
        assert!(
            diff > 20.0,
            "saw vs sine primary curves should differ (diff={diff})"
        );
    }

    /// Quant reshape on the right pane must use the selected layer's display scale.
    #[test]
    fn selected_layer_quant_display_scale_matches_level() {
        let layer = WaveLayerUi {
            source_type: "wavetable".into(),
            level: 0.4,
            enabled: true,
            invert: true,
            ..WaveLayerUi::default()
        };
        let scale = layer_quant_display_scale(&layer);
        assert!((scale - (-0.4)).abs() < 1e-4, "scale={scale}");
    }

    #[test]
    fn layer_waveform_points_span_plot_width() {
        let bank = WavetableBank::factory_saw_morph();
        let layer = WaveLayerUi {
            source_type: "saw".into(),
            level: 1.0,
            enabled: true,
            ..WaveLayerUi::default()
        };
        let rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 80.0));
        let pts = layer_waveform_points(&layer, &bank, rect, 0.0, 32);
        assert_eq!(pts.len(), 32);
        assert!((pts.first().unwrap().x - rect.min.x).abs() < 1e-4);
        assert!((pts.last().unwrap().x - rect.max.x).abs() < 1e-4);
        // Must not duplicate start sample at the right edge (phase 1.0 wrap).
        let jump = (pts[pts.len() - 1].y - pts[pts.len() - 2].y).abs();
        let full = (pts[0].y - pts[pts.len() / 4].y).abs().max(1.0);
        assert!(
            jump < full * 0.75,
            "end segment jump too large (wrap artifact?): jump={jump} full~{full}"
        );
    }
}
