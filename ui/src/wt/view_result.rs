//! Column 1 — Result composite + residual quant editing.

use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Ui, Vec2};
use reelsynth::WavetableBank;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::audit_registry::{record_region, AuditId};
use crate::layout::RADIUS_SM;
use crate::oscillator_ui::WaveLayerUi;

use super::quant_handles::{
    knob_y_on_curve, nearest_quant_handle, resample_frame_from_quant_points,
    sample_from_knob_y, slot_x, WtQuantInterp,
};
use super::residual::{
    composite_quant_points, ensure_residual_layer, residual_frame_from_desired,
};
use super::slots::effective_quant_count;
use super::view_3d_stack::{
    composite_waveform_points, layer_palette, layer_waveform_points, HOVER_DISTANCE_PX,
    WAVE_SAMPLES,
};
use super::waveform::{nearest_waveform_distance, peak_point};

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
    pub quant_interp: WtQuantInterp,
    pub wavetable_id: Option<String>,
    #[allow(dead_code)]
    pub active_osc: usize,
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
                        && nearest_quant_handle(pos, inner, &points, 1.0, 14.0).is_some()
                    {
                        quant_grab = true;
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
                        layer_waveform_points(l, bank_ro, inner, 0.0, WAVE_SAMPLES),
                    )
                })
                .collect();

            if response.clicked() || response.drag_started() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let mut best_idx = None;
                    let mut best_dist = HOVER_DISTANCE_PX;
                    for &(idx, ref pts) in &layer_pts {
                        let dist = nearest_waveform_distance(pts, pos);
                        if dist < best_dist {
                            best_dist = dist;
                            best_idx = Some(idx);
                        }
                    }
                    if let Some(idx) = best_idx {
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
                } else {
                    CursorIcon::Grab
                });
            }
        }

        let painter = ui.painter_at(rect);
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
            for (i, layer) in self.wave_layers.iter().enumerate() {
                if !layer.enabled || layer.level <= 0.0 {
                    continue;
                }
                let pts = layer_waveform_points(layer, bank_ro, inner, 0.0, WAVE_SAMPLES);
                if pts.len() < 2 {
                    continue;
                }
                let color = layer_palette(i);
                let is_sel = selected == Some(i);
                let alpha = if is_sel { 0.55 } else { 0.28 };
                painter.add(Shape::line(
                    pts,
                    egui::Stroke::new(if is_sel { 1.6 } else { 1.0 }, color.gamma_multiply(alpha)),
                ));
            }

            let result_pts =
                composite_waveform_points(self.wave_layers, bank_ro, &stack_mode, inner, 0.0, WAVE_SAMPLES);
            if result_pts.len() >= 2 {
                let mut fill = result_pts.clone();
                fill.push(Pos2::new(inner.max.x, mid_y));
                fill.push(Pos2::new(inner.min.x, mid_y));
                painter.add(Shape::convex_polygon(
                    fill,
                    tokens.accent.gamma_multiply(0.28),
                    egui::Stroke::NONE,
                ));
                painter.add(Shape::line(
                    result_pts.clone(),
                    egui::Stroke::new(2.6, accent_ui),
                ));
                if let Some(peak) = peak_point(&result_pts) {
                    painter.circle_filled(peak, 4.0, tokens.accent);
                }
                record_region(ui.ctx(), AuditId::CenterWt2dResult, inner, inner);
            }
        }

        painter.line_segment(
            [Pos2::new(inner.min.x, mid_y), Pos2::new(inner.max.x, mid_y)],
            egui::Stroke::new(1.0, tokens.border.gamma_multiply(0.75)),
        );

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
            let sense = Sense::click_and_drag();
            let response = ui.allocate_rect(inner, sense);
            let pointer = response
                .interact_pointer_pos()
                .filter(|p| inner.contains(*p));

            let locked: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_slot_id));

            if response.drag_started() {
                if let Some(pos) = pointer {
                    if let Some(slot) = nearest_quant_handle(pos, inner, &desired, 1.0, 14.0) {
                        ui.ctx().data_mut(|d| d.insert_temp(drag_slot_id, slot));
                    }
                }
            }
            if !response.dragged() && response.drag_stopped() {
                ui.ctx().data_mut(|d| d.remove::<usize>(drag_slot_id));
            }

            let active_slot = locked.or_else(|| {
                pointer.and_then(|pos| nearest_quant_handle(pos, inner, &desired, 1.0, 14.0))
            });

            if let Some(slot) = active_slot {
                if response.dragged() {
                    if let Some(pos) = pointer {
                        let sample = sample_from_knob_y(pos.y, 1.0, inner);
                        desired[slot] = sample;

                        let (residual_idx, mode_changed) = ensure_residual_layer(
                            self.wave_layers,
                            self.stack_mode,
                            self.wavetable_id.clone(),
                        );
                        *self.selected_layer_idx = Some(residual_idx);
                        stack_changed = true;
                        if mode_changed {
                            status_hint.get_or_insert_with(|| "Stack → add (Result edit)".into());
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
                            resample_frame_from_quant_points(frame, &raw, self.quant_interp);
                        }
                        frame_edited = true;
                        status_hint = Some(format!(
                            "Result slot {} → {:+.2}",
                            slot + 1,
                            sample
                        ));
                    }
                } else if response.hovered() || pointer.is_some() {
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
                let show = self.wave_quant <= 64
                    || locked == Some(i)
                    || i == 0
                    || i + 1 == slot_count;
                if !show {
                    continue;
                }
                let x = slot_x(i, slot_count, inner);
                let sample = desired.get(i).copied().unwrap_or(0.0);
                let y = knob_y_on_curve(sample, 1.0, inner);
                let center = Pos2::new(x, y);
                let active = locked == Some(i);
                painter.circle_filled(
                    center,
                    if active { 7.5 } else { 6.0 },
                    if active {
                        accent_ui.gamma_multiply(0.35)
                    } else {
                        tokens.surface2
                    },
                );
                painter.circle_stroke(
                    center,
                    if active { 7.5 } else { 6.0 },
                    egui::Stroke::new(if active { 2.0 } else { 1.0 }, accent_ui),
                );
            }
        }

        let n = self
            .wave_layers
            .iter()
            .filter(|l| l.enabled && l.level > 0.0)
            .count();
        let label = format!(
            "Result · {} · {}",
            n,
            self.stack_mode
        );
        painter.text(
            Pos2::new(plot_rect.min.x + 8.0, plot_rect.min.y + 6.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(10.0),
            tokens.text_secondary,
        );

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
