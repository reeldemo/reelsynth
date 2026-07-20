//! Column 1 — Result composite + residual quant editing.

use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Ui, Vec2};
use reelsynth::WavetableBank;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::audit_registry::{record_region, AuditId};
use crate::layout::RADIUS_SM;
use crate::oscillator_ui::WaveLayerUi;

use super::quant_handles::{
    apply_quant_slot_amplitude, knob_y_on_curve, nearest_quant_handle, paint_quant_knob,
    quant_control_points, quant_curve_stroke, quant_hover_status_label, quant_knob_visual,
    resample_frame_from_quant_points, sample_from_knob_y, slot_x,
};
use super::residual::{
    composite_quant_points, ensure_residual_layer, layer_curve_label, residual_frame_from_desired,
};
use super::slots::effective_quant_count;
use super::view_3d_stack::{
    composite_waveform_points, layer_palette, layer_quant_display_scale, layer_waveform_points,
    HOVER_DISTANCE_PX, WAVE_SAMPLES,
};
use super::waveform::{
    hovered_layer_from_pointer, peak_point, quant_knobs_for_selection, selection_from_curve_click,
    waveform_fill_shape,
};
use super::view_zoom::{consume_plot_scroll, WtCurveViewTransform};

pub struct WtViewResultResponse {
    pub frame_edited: bool,
    pub stack_changed: bool,
    pub status_hint: Option<String>,
}

pub struct WtViewResult<'a> {
    pub wt_position: &'a f32,
    pub bank: Option<&'a mut WavetableBank>,
    pub wave_layers: &'a mut Vec<WaveLayerUi>,
    pub selected_layer_idx: &'a mut Option<usize>,
    pub stack_mode: &'a mut String,
    pub wave_quant: u8,
    pub wavetable_id: Option<String>,
    #[allow(dead_code)]
    pub active_osc: usize,
    pub curve_view: &'a mut WtCurveViewTransform,
}

impl WtViewResult<'_> {
    pub fn show(mut self, ui: &mut Ui) -> WtViewResultResponse {
        let tokens = Tokens::default();
        let accent_ui = ACCENT_UI;
        let view_h = ui.available_height().max(48.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), view_h), Sense::hover());

        let mut frame_edited = false;
        let mut stack_changed = false;
        let mut status_hint: Option<String> = None;

        if !ui.is_rect_visible(rect) {
            return WtViewResultResponse {
                frame_edited,
                stack_changed,
                status_hint,
            };
        }

        let plot_rect = rect;
        let inner = plot_rect.shrink2(egui::vec2(8.0, 20.0));
        let mid_y = inner.center().y;
        let _ = consume_plot_scroll(ui, inner, self.curve_view);
        let curve_view = *self.curve_view;
        let hit_r = curve_view.hit_radius(HOVER_DISTANCE_PX);
        let stack_mode = self.stack_mode.clone();
        let layers_empty = self.wave_layers.is_empty();
        let quant_active = self.wave_quant > 0 && !layers_empty;

        let empty = WavetableBank::factory_saw_morph();

        let frame_idx = self
            .bank
            .as_ref()
            .map(|b| super::waveform::frame_index(*self.wt_position, b.num_frames))
            .unwrap_or(0);

        // Layer pick + level/phase drag (no quant grab).
        // Selected-layer Quant knobs only when that layer is WT/residual.
        let selected_wt = quant_knobs_for_selection(
            *self.selected_layer_idx,
            self.wave_layers,
            self.wave_quant,
        );
        let selected_layer_quant = selected_wt.is_some();
        let mut quant_grab = false;
        if quant_active {
            if let Some(bank) = self.bank.as_ref() {
                let bank_ro = &**bank;
                let slot_count = effective_quant_count(self.wave_quant);
                let points = composite_quant_points(
                    self.wave_layers,
                    bank_ro,
                    &stack_mode,
                    slot_count,
                );
                if let Some(pos) = ui.ctx().pointer_latest_pos() {
                    if inner.contains(pos)
                        && nearest_quant_handle(
                            curve_view.unmap_pos(pos, inner),
                            inner,
                            &points,
                            1.0,
                            hit_r,
                        )
                        .is_some()
                    {
                        quant_grab = true;
                    }
                }
            }
        }
        if selected_layer_quant && !quant_grab {
            if let (Some(bank), Some(layer_i)) = (self.bank.as_ref(), selected_wt) {
                if let Some(layer) = self.wave_layers.get(layer_i) {
                    let slot_count = effective_quant_count(self.wave_quant);
                    let scale = layer_quant_display_scale(layer);
                    let frame_i = super::waveform::frame_index(layer.wt_position, bank.num_frames);
                    let points = quant_control_points(bank.frame(frame_i), slot_count);
                    if let Some(pos) = ui.ctx().pointer_latest_pos() {
                        if inner.contains(pos)
                            && nearest_quant_handle(
                                curve_view.unmap_pos(pos, inner),
                                inner,
                                &points,
                                scale,
                                hit_r,
                            )
                            .is_some()
                        {
                            quant_grab = true;
                        }
                    }
                }
            }
        }

        if !quant_grab {
            let bank_ro = self.bank.as_ref().map(|b| &**b).unwrap_or(&empty);
            let sense = Sense::click_and_drag();
            let response = ui.allocate_rect(inner, sense);
            let layer_pts: Vec<(usize, Vec<Pos2>)> = self
                .wave_layers
                .iter()
                .enumerate()
                .filter(|(_, l)| l.enabled && l.level > 0.0)
                .map(|(i, l)| {
                    (
                        i,
                        curve_view.map_points(
                            &layer_waveform_points(l, bank_ro, inner, 0.0, WAVE_SAMPLES),
                            inner,
                        ),
                    )
                })
                .collect();

            let hovered_curve = response.hover_pos().and_then(|pos| {
                hovered_layer_from_pointer(
                    layer_pts.iter().map(|(i, pts)| (*i, pts.as_slice())),
                    pos,
                    HOVER_DISTANCE_PX,
                )
            });

            if response.clicked() || response.drag_started() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let hovered = hovered_layer_from_pointer(
                        layer_pts.iter().map(|(i, pts)| (*i, pts.as_slice())),
                        pos,
                        HOVER_DISTANCE_PX,
                    );
                    if let Some(idx) = selection_from_curve_click(hovered, false) {
                        *self.selected_layer_idx = Some(idx);
                        stack_changed = true;
                    }
                }
            }

            if response.dragged() {
                if let Some(idx) = *self.selected_layer_idx {
                    if let Some(layer) = self.wave_layers.get_mut(idx) {
                        let delta = response.drag_delta();
                        if delta.y.abs() > 0.0 {
                            let next = (layer.level - delta.y / inner.height()).clamp(0.0, 1.0);
                            if (next - layer.level).abs() > f32::EPSILON {
                                layer.level = next;
                                stack_changed = true;
                            }
                        }
                        if delta.x.abs() > 0.0 {
                            if let Some(bank) = self.bank.as_ref() {
                                let max_pos = (bank.num_frames.saturating_sub(1)).max(1) as f32;
                                let px_per_frame = inner.width() / max_pos.max(1.0);
                                if layer.is_wavetable() {
                                    layer.wt_position =
                                        (layer.wt_position + delta.x / px_per_frame).clamp(0.0, max_pos);
                                    stack_changed = true;
                                } else {
                                    layer.phase += delta.x / inner.width() * std::f32::consts::TAU;
                                    stack_changed = true;
                                }
                            }
                        }
                    }
                }
            }

            if response.hovered() {
                ui.ctx().set_cursor_icon(if response.dragged() {
                    CursorIcon::Grabbing
                } else if hovered_curve.is_some() {
                    CursorIcon::PointingHand
                } else {
                    CursorIcon::Grab
                });
            }

            if let Some(idx) = hovered_curve {
                if let Some(layer) = self.wave_layers.get(idx) {
                    status_hint = Some(format!("Hover · {}", layer_curve_label(idx, layer)));
                }
            }
        }

        let mut painter = ui.painter_at(rect);
        painter.set_clip_rect(inner.expand(1.0));
        painter.rect_filled(rect, RADIUS_SM, tokens.bg);
        painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0, tokens.border));
        paint_grid(&painter, inner, tokens.border);

        if quant_active {
            let quant = effective_quant_count(self.wave_quant);
            for i in 0..quant {
                let x = slot_x(i, quant, inner);
                painter.line_segment(
                    [Pos2::new(x, inner.min.y), Pos2::new(x, inner.max.y)],
                    egui::Stroke::new(0.5, tokens.border.gamma_multiply(0.5)),
                );
            }
        }

        let selected = *self.selected_layer_idx;
        if !layers_empty {
            let bank_ro = self.bank.as_ref().map(|b| &**b).unwrap_or(&empty);
            let layer_pts: Vec<(usize, Vec<Pos2>)> = self
                .wave_layers
                .iter()
                .enumerate()
                .filter(|(_, l)| l.enabled && l.level > 0.0)
                .map(|(i, l)| {
                    (
                        i,
                        curve_view.map_points(
                            &layer_waveform_points(l, bank_ro, inner, 0.0, WAVE_SAMPLES),
                            inner,
                        ),
                    )
                })
                .collect();
            // When over Result quant knobs, skip curve-hover preview (knob wins).
            let hovered_curve = if quant_grab {
                None
            } else {
                ui.ctx().pointer_latest_pos().and_then(|pos| {
                    if !inner.contains(pos) {
                        return None;
                    }
                    hovered_layer_from_pointer(
                        layer_pts.iter().map(|(i, pts)| (*i, pts.as_slice())),
                        pos,
                        HOVER_DISTANCE_PX,
                    )
                })
            };
            let curve_hover_active = hovered_curve.is_some();

            let mut paint_order: Vec<usize> = (0..layer_pts.len()).collect();
            paint_order.sort_by_key(|&i| {
                let orig = layer_pts[i].0;
                if hovered_curve == Some(orig) {
                    2u8
                } else if selected == Some(orig) {
                    1u8
                } else {
                    0u8
                }
            });

            for &paint_i in &paint_order {
                let (i, pts) = &layer_pts[paint_i];
                let i = *i;
                if pts.len() < 2 {
                    continue;
                }
                let color = layer_palette(i);
                let is_sel = selected == Some(i);
                let is_hover = hovered_curve == Some(i);
                let (alpha, stroke_w) = if is_hover {
                    (0.72, 2.2)
                } else if is_sel {
                    if curve_hover_active {
                        (0.42, 1.4)
                    } else {
                        (0.55, 1.6)
                    }
                } else if curve_hover_active {
                    (0.16, 0.9)
                } else {
                    (0.28, 1.0)
                };
                if is_hover {
                    painter.add(Shape::line(
                        pts.clone(),
                        egui::Stroke::new(stroke_w + 1.6, color.gamma_multiply(0.28)),
                    ));
                }
                painter.add(Shape::line(
                    pts.clone(),
                    egui::Stroke::new(stroke_w, color.gamma_multiply(alpha)),
                ));
            }

            let result_pts = curve_view.map_points(
                &composite_waveform_points(
                    self.wave_layers,
                    bank_ro,
                    &stack_mode,
                    inner,
                    0.0,
                    WAVE_SAMPLES,
                ),
                inner,
            );
            let baseline_y = curve_view.map_pos(Pos2::new(inner.center().x, mid_y), inner).y;
            if result_pts.len() >= 2 {
                if let Some(fill) =
                    waveform_fill_shape(&result_pts, baseline_y, tokens.accent.gamma_multiply(0.28))
                {
                    painter.add(fill);
                }
                let result_knob_hot = quant_active
                    && ui.ctx().pointer_latest_pos().is_some_and(|pos| {
                        if !inner.contains(pos) {
                            return false;
                        }
                        let pts = composite_quant_points(
                            self.wave_layers,
                            bank_ro,
                            &stack_mode,
                            effective_quant_count(self.wave_quant),
                        );
                        nearest_quant_handle(
                            curve_view.unmap_pos(pos, inner),
                            inner,
                            &pts,
                            1.0,
                            hit_r,
                        )
                        .is_some()
                    });
                let (stroke_w, stroke_mul) = quant_curve_stroke(result_knob_hot);
                painter.add(Shape::line(
                    result_pts.clone(),
                    egui::Stroke::new(
                        if result_knob_hot {
                            stroke_w + 0.4
                        } else {
                            2.6
                        },
                        accent_ui.gamma_multiply(if result_knob_hot {
                            stroke_mul
                        } else {
                            1.0
                        }),
                    ),
                ));
                if let Some(peak) = peak_point(&result_pts) {
                    painter.circle_filled(peak, 4.0, tokens.accent);
                }
                record_region(ui.ctx(), AuditId::CenterWt2dResult, inner, inner);
            }

            painter.line_segment(
                [
                    Pos2::new(inner.min.x, baseline_y),
                    Pos2::new(inner.max.x, baseline_y),
                ],
                egui::Stroke::new(1.0, tokens.border.gamma_multiply(0.75)),
            );
        } else {
            painter.line_segment(
                [Pos2::new(inner.min.x, mid_y), Pos2::new(inner.max.x, mid_y)],
                egui::Stroke::new(1.0, tokens.border.gamma_multiply(0.75)),
            );
        }

        if quant_active {
            let slot_count = effective_quant_count(self.wave_quant);
            let stack_mode_snap = self.stack_mode.clone();
            let mut desired = {
                let bank_ro = self.bank.as_ref().map(|b| &**b).unwrap_or(&empty);
                composite_quant_points(
                    self.wave_layers,
                    bank_ro,
                    &stack_mode_snap,
                    slot_count,
                )
            };

            let drag_slot_id = ui.id().with("result_quant_drag");
            let drag_kind_id = ui.id().with("result_quant_kind"); // 0 = result, 1 = selected layer
            let sense = Sense::click_and_drag();
            let response = ui.allocate_rect(inner, sense);
            let pointer = response
                .interact_pointer_pos()
                .filter(|p| inner.contains(*p));

            let locked: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_slot_id));
            let locked_kind: Option<u8> = ui.ctx().data(|d| d.get_temp(drag_kind_id));

            // Nearest selected-layer knob (when selected is WT).
            let selected_knob: Option<(usize, usize, f32)> =
                if let (Some(layer_i), Some(bank)) = (selected_wt, self.bank.as_ref()) {
                    self.wave_layers.get(layer_i).and_then(|layer| {
                        let scale = layer_quant_display_scale(layer);
                        let frame_i =
                            super::waveform::frame_index(layer.wt_position, bank.num_frames);
                        let points = quant_control_points(bank.frame(frame_i), slot_count);
                        pointer.and_then(|pos| {
                            let plot_pos = curve_view.unmap_pos(pos, inner);
                            nearest_quant_handle(plot_pos, inner, &points, scale, hit_r).map(
                                |slot| {
                                    let x = slot_x(slot, slot_count, inner);
                                    let y = knob_y_on_curve(points[slot], scale, inner);
                                    let center = curve_view.map_pos(Pos2::new(x, y), inner);
                                    (layer_i, slot, pos.distance(center))
                                },
                            )
                        })
                    })
                } else {
                    None
                };
            let result_knob: Option<(usize, f32)> = pointer.and_then(|pos| {
                let plot_pos = curve_view.unmap_pos(pos, inner);
                nearest_quant_handle(plot_pos, inner, &desired, 1.0, hit_r).map(|slot| {
                    let x = slot_x(slot, slot_count, inner);
                    let y = knob_y_on_curve(desired[slot], 1.0, inner);
                    let center = curve_view.map_pos(Pos2::new(x, y), inner);
                    (slot, pos.distance(center))
                })
            });

            // Prefer whichever knob is closer; ties go to Result composite.
            let pick = match (result_knob, selected_knob) {
                (Some((_rs, rd)), Some((li, ls, sd))) if sd + 0.01 < rd => {
                    Some((1u8, li, ls))
                }
                (Some((rs, _)), _) => Some((0u8, 0, rs)),
                (None, Some((li, ls, _))) => Some((1u8, li, ls)),
                (None, None) => None,
            };

            if response.drag_started() {
                if let Some((kind, layer_i, slot)) = pick {
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(drag_slot_id, slot);
                        d.insert_temp(drag_kind_id, kind);
                        if kind == 1 {
                            d.insert_temp(ui.id().with("result_sel_layer"), layer_i);
                        }
                    });
                } else if let Some(pos) = pointer {
                    // Click/drag on a curve (not a knob) → select that layer.
                    let bank_ro = self.bank.as_ref().map(|b| &**b).unwrap_or(&empty);
                    let layer_pts: Vec<(usize, Vec<Pos2>)> = self
                        .wave_layers
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| l.enabled && l.level > 0.0)
                        .map(|(i, l)| {
                            (
                                i,
                                curve_view.map_points(
                                    &layer_waveform_points(l, bank_ro, inner, 0.0, WAVE_SAMPLES),
                                    inner,
                                ),
                            )
                        })
                        .collect();
                    let hovered = hovered_layer_from_pointer(
                        layer_pts.iter().map(|(i, pts)| (*i, pts.as_slice())),
                        pos,
                        HOVER_DISTANCE_PX,
                    );
                    if let Some(idx) = selection_from_curve_click(hovered, false) {
                        *self.selected_layer_idx = Some(idx);
                        stack_changed = true;
                    }
                }
            }
            if response.clicked() && pick.is_none() {
                if let Some(pos) = pointer {
                    let bank_ro = self.bank.as_ref().map(|b| &**b).unwrap_or(&empty);
                    let layer_pts: Vec<(usize, Vec<Pos2>)> = self
                        .wave_layers
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| l.enabled && l.level > 0.0)
                        .map(|(i, l)| {
                            (
                                i,
                                curve_view.map_points(
                                    &layer_waveform_points(l, bank_ro, inner, 0.0, WAVE_SAMPLES),
                                    inner,
                                ),
                            )
                        })
                        .collect();
                    let hovered = hovered_layer_from_pointer(
                        layer_pts.iter().map(|(i, pts)| (*i, pts.as_slice())),
                        pos,
                        HOVER_DISTANCE_PX,
                    );
                    if let Some(idx) = selection_from_curve_click(hovered, false) {
                        *self.selected_layer_idx = Some(idx);
                        stack_changed = true;
                    }
                }
            }
            if !response.dragged() && response.drag_stopped() {
                ui.ctx().data_mut(|d| {
                    d.remove::<usize>(drag_slot_id);
                    d.remove::<u8>(drag_kind_id);
                    d.remove::<usize>(ui.id().with("result_sel_layer"));
                });
            }

            let active_kind = locked_kind.or(pick.map(|(k, _, _)| k));
            let active_slot = locked.or(pick.map(|(_, _, s)| s));

            if let (Some(kind), Some(slot)) = (active_kind, active_slot) {
                if response.dragged() {
                    if let Some(pos) = pointer {
                        if kind == 0 {
                            let plot_pos = curve_view.unmap_pos(pos, inner);
                            let sample = sample_from_knob_y(plot_pos.y, 1.0, inner);
                            desired[slot] = sample;

                            let (residual_idx, mode_changed) = ensure_residual_layer(
                                self.wave_layers,
                                self.stack_mode,
                                self.wavetable_id.clone(),
                            );
                            *self.selected_layer_idx = Some(residual_idx);
                            stack_changed = true;
                            if mode_changed {
                                status_hint
                                    .get_or_insert_with(|| "Stack → add (Result edit)".into());
                            }

                            let stack_mode_now = self.stack_mode.clone();
                            let raw = {
                                let b = self.bank.as_ref().map(|x| &**x).unwrap_or(&empty);
                                residual_frame_from_desired(
                                    &desired,
                                    self.wave_layers,
                                    b,
                                    &stack_mode_now,
                                    residual_idx,
                                )
                            };
                            if let Some(bank) = self.bank.as_mut() {
                                let frame = bank.frame_mut(frame_idx);
                                let layer = &mut self.wave_layers[residual_idx];
                                layer.ensure_segment_interps(slot_count);
                                let segs = layer.quant_segment_interps.clone();
                                let curve_default = layer.quant_interp;
                                resample_frame_from_quant_points(
                                    frame,
                                    &raw,
                                    &segs,
                                    curve_default,
                                );
                            }
                            frame_edited = true;
                            let interp_label = self.wave_layers[residual_idx]
                                .quant_segment_interps
                                .get(slot)
                                .copied()
                                .unwrap_or(self.wave_layers[residual_idx].quant_interp)
                                .label();
                            status_hint = Some(format!(
                                "Result {} · {}",
                                quant_hover_status_label(slot, sample),
                                interp_label
                            ));
                        } else if let Some(layer_i) = selected_wt.or_else(|| {
                            ui.ctx()
                                .data(|d| d.get_temp(ui.id().with("result_sel_layer")))
                        }) {
                            if let Some(layer) = self.wave_layers.get_mut(layer_i) {
                                layer.ensure_segment_interps(slot_count);
                                let segs = layer.quant_segment_interps.clone();
                                let curve_default = layer.quant_interp;
                                let scale = layer_quant_display_scale(layer);
                                let plot_pos = curve_view.unmap_pos(pos, inner);
                                let sample = sample_from_knob_y(plot_pos.y, scale, inner);
                                let wt_pos = layer.wt_position;
                                if let Some(bank) = self.bank.as_mut() {
                                    let frame_i =
                                        super::waveform::frame_index(wt_pos, bank.num_frames);
                                    apply_quant_slot_amplitude(
                                        bank.frame_mut(frame_i),
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
                        }
                    }
                } else if pointer.is_some() {
                    if kind == 0 {
                        let amp = desired.get(slot).copied().unwrap_or(0.0);
                        status_hint = Some(format!(
                            "Result · {}",
                            quant_hover_status_label(slot, amp)
                        ));
                    } else if let Some(layer_i) = selected_wt {
                        status_hint = Some(format!(
                            "L{} · {}",
                            layer_i + 1,
                            quant_hover_status_label(slot, 0.0)
                        ));
                        if let Some(bank) = self.bank.as_ref() {
                            if let Some(layer) = self.wave_layers.get(layer_i) {
                                let frame_i = super::waveform::frame_index(
                                    layer.wt_position,
                                    bank.num_frames,
                                );
                                let points =
                                    quant_control_points(bank.frame(frame_i), slot_count);
                                let amp = points.get(slot).copied().unwrap_or(0.0);
                                status_hint = Some(format!(
                                    "L{} · {}",
                                    layer_i + 1,
                                    quant_hover_status_label(slot, amp)
                                ));
                            }
                        }
                    }
                    ui.ctx().set_cursor_icon(if response.dragged() {
                        CursorIcon::Grabbing
                    } else {
                        CursorIcon::Grab
                    });
                }
            }

            desired = {
                let bank_ro = self.bank.as_ref().map(|b| &**b).unwrap_or(&empty);
                composite_quant_points(
                    self.wave_layers,
                    bank_ro,
                    self.stack_mode,
                    slot_count,
                )
            };
            for i in 0..slot_count {
                let hovered = locked.is_none() && active_kind == Some(0) && active_slot == Some(i);
                let dragged = locked_kind == Some(0) && locked == Some(i);
                let show = self.wave_quant <= 64
                    || hovered
                    || dragged
                    || i == 0
                    || i + 1 == slot_count;
                if !show {
                    continue;
                }
                let x = slot_x(i, slot_count, inner);
                let sample = desired.get(i).copied().unwrap_or(0.0);
                let y = knob_y_on_curve(sample, 1.0, inner);
                let center = curve_view.map_pos(Pos2::new(x, y), inner);
                let visual = quant_knob_visual(hovered, dragged);
                let fill = if visual.fill_brighter {
                    accent_ui.gamma_multiply(if dragged { 0.55 } else { 0.42 })
                } else {
                    tokens.surface2
                };
                paint_quant_knob(&painter, center, visual, fill, accent_ui, inner);
            }

            // Selected WT/residual curve knobs (siblings stay stroke-only).
            if let (Some(layer_i), Some(bank)) = (selected_wt, self.bank.as_ref()) {
                if let Some(layer) = self.wave_layers.get(layer_i) {
                    let scale = layer_quant_display_scale(layer);
                    let frame_i =
                        super::waveform::frame_index(layer.wt_position, bank.num_frames);
                    let points = quant_control_points(bank.frame(frame_i), slot_count);
                    let color = layer_palette(layer_i);
                    let pointer = ui
                        .ctx()
                        .pointer_latest_pos()
                        .filter(|p| inner.contains(*p));
                    let hover_slot = if locked_kind != Some(1) {
                        pointer.and_then(|pos| {
                            nearest_quant_handle(
                                curve_view.unmap_pos(pos, inner),
                                inner,
                                &points,
                                scale,
                                hit_r,
                            )
                        })
                    } else {
                        None
                    };
                    for i in 0..slot_count {
                        let dragged =
                            locked_kind == Some(1) && locked == Some(i);
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

        let n = self
            .wave_layers
            .iter()
            .filter(|l| l.enabled && l.level > 0.0)
            .count();

        // Overlay method (stack_mode) — visible + selectable in Result header.
        let mode_anchor = Pos2::new(plot_rect.min.x + 8.0, plot_rect.min.y + 4.0);
        let mode_id = ui.id().with("result_stack_mode");
        let mut mode_idx = crate::osc_column::stack_mode_index(self.stack_mode);
        let mode_rect =
            Rect::from_min_size(mode_anchor, egui::vec2((plot_rect.width() - 16.0).max(80.0), 18.0));
        crate::region::region(ui, mode_rect, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(
                    egui::RichText::new(format!("Result · {n} ·"))
                        .size(10.0)
                        .color(tokens.text_secondary),
                );
                let combo = egui::ComboBox::from_id_salt(mode_id)
                    .selected_text(crate::osc_column::stack_mode_label(self.stack_mode))
                    .width(72.0)
                    .show_ui(ui, |ui| {
                        for (i, label) in crate::osc_column::STACK_MODES.iter().enumerate() {
                            if ui.selectable_label(mode_idx == i, *label).clicked() {
                                mode_idx = i;
                            }
                        }
                    });
                combo
                    .response
                    .on_hover_text(crate::osc_column::stack_mode_tooltip(self.stack_mode));
            });
        });
        let new_mode = crate::osc_column::stack_mode_from_index(mode_idx);
        if self.stack_mode != new_mode {
            *self.stack_mode = new_mode.into();
            stack_changed = true;
            status_hint = Some(format!(
                "Stack → {}",
                crate::osc_column::stack_mode_label(new_mode)
            ));
        }

        record_region(ui.ctx(), AuditId::CenterWtResult, rect, rect);

        WtViewResultResponse {
            frame_edited,
            stack_changed,
            status_hint,
        }
    }
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
