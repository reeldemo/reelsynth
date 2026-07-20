//! Column 3 — selected layer focus, toolbar, and quant reshape.

use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Ui, Vec2};
use reelsynth::WavetableBank;
use reelsynth_ui_theme::Tokens;

use crate::audit_registry::{record_region, AuditId};
use crate::layout::{RADIUS_SM, WT_TOOLBAR_HEIGHT};
use crate::oscillator_ui::WaveLayerUi;
use crate::quant_interp::WtQuantInterp;
use crate::region::region;

use super::curve_editor::CurveEditor;
use super::quant_handles::{
    knob_y_on_curve, nearest_quant_handle, paint_quant_knob, quant_control_points,
    quant_knob_visual, resample_frame_from_quant_points, slot_x, QuantHandleEditor,
};
use super::shape_editor::ShapeEditor;
use super::slots::effective_quant_count;
use super::toolbar::{WtEditTool, WtToolbar, WtToolbarResponse};
use super::view_2d::{apply_waveform_drag_inner, va_layer_waveform_points};
use super::residual::{layer_curve_label, layer_type_display};
use super::view_3d_stack::{
    layer_palette, layer_quant_display_scale, layer_waveform_points, HOVER_DISTANCE_PX,
    WAVE_SAMPLES,
};
use super::waveform::{
    frame_index, selected_curve_hovered, selected_pane_shows_quant_knobs, waveform_fill_shape,
};
use super::view_zoom::{consume_plot_scroll, WtCurveViewTransform};
use super::QuantSeamMode;

pub struct WtSelectedLayerResponse {
    pub frame_edited: bool,
    pub stack_changed: bool,
    pub analyze_requested: bool,
    /// Patch-level params changed (e.g. crackle) — sync to engine without WT rebuild.
    pub params_changed: bool,
    pub status_hint: Option<String>,
}

pub struct WtSelectedLayerView<'a> {
    pub wt_position: &'a mut f32,
    pub bank: Option<&'a mut WavetableBank>,
    pub tool: &'a mut WtEditTool,
    pub wave_quant: u8,
    pub quant_interp: &'a mut WtQuantInterp,
    pub selected_quant_slot: &'a mut Option<usize>,
    pub wave_slot: &'a mut u8,
    pub wave_slots: &'a mut Vec<reelsynth::patch::WaveSlot>,
    pub wave_layers: &'a mut Vec<WaveLayerUi>,
    pub selected_layer_idx: &'a mut Option<usize>,
    pub shape_control_points: usize,
    pub analyze_dialog_open: Option<&'a mut bool>,
    pub curve_view: &'a mut WtCurveViewTransform,
    pub quant_seam: &'a mut QuantSeamMode,
    /// Artistic crackle 0..1 synced to `patch.crackle`.
    pub patch_crackle: &'a mut f32,
}

impl WtSelectedLayerView<'_> {
    pub fn show(mut self, ui: &mut Ui) -> WtSelectedLayerResponse {
        let tokens = Tokens::default();
        let view_h = ui.available_height().max(48.0);
        let (rect, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), view_h), Sense::hover());

        let mut frame_edited = false;
        let mut stack_changed = false;
        let mut analyze_requested = false;
        let mut params_changed = false;
        let mut status_hint: Option<String> = None;

        if !ui.is_rect_visible(rect) {
            return WtSelectedLayerResponse {
                frame_edited,
                stack_changed,
                analyze_requested,
                params_changed,
                status_hint,
            };
        }

        let plot_top = rect.min.y + WT_TOOLBAR_HEIGHT;
        let plot_rect = Rect::from_min_max(egui::pos2(rect.min.x, plot_top), rect.max);
        let inner = plot_rect.shrink2(egui::vec2(8.0, 12.0));
        let mid_y = inner.center().y;
        let _ = consume_plot_scroll(ui, inner, self.curve_view);
        let curve_view = *self.curve_view;
        let hit_r = curve_view.hit_radius(HOVER_DISTANCE_PX);

        let layer_idx = self.selected_layer_idx.unwrap_or(0);
        let slot_count = effective_quant_count(self.wave_quant);

        // Promote VA → WT before gate so Selected pane shows knobs on L1/L2/….
        if self.wave_quant > 0 {
            if let Some(bank) = self.bank.as_deref_mut() {
                let occupied: Vec<usize> = self
                    .wave_layers
                    .iter()
                    .enumerate()
                    .filter(|(i, l)| *i != layer_idx && l.is_wavetable())
                    .map(|(_, l)| frame_index(l.wt_position, bank.num_frames))
                    .collect();
                if let Some(layer) = self.wave_layers.get_mut(layer_idx) {
                    if super::waveform::layer_quant_editable(layer) && layer.is_va() {
                        if crate::wt::promote_va_layer_for_quant(layer, bank, &occupied) {
                            frame_edited = true;
                        }
                    }
                }
            }
        }

        if let Some(layer) = self.wave_layers.get_mut(layer_idx) {
            layer.ensure_segment_interps(slot_count);
            *self.quant_interp = layer.quant_interp;
        }

        let layer_va = self
            .wave_layers
            .get(layer_idx)
            .map(|l| l.is_va())
            .unwrap_or(true);
        let layer_wt = self
            .wave_layers
            .get(layer_idx)
            .map(|l| l.is_wavetable())
            .unwrap_or(false);
        // Quant knobs whenever the selected layer is editable (tool-independent visibility).
        let quant_active = selected_pane_shows_quant_knobs(
            *self.selected_layer_idx,
            self.wave_layers,
            self.wave_quant,
        );

        let selected_slot = *self.selected_quant_slot;
        let show_seg = quant_active && selected_slot.is_some_and(|s| s + 1 < slot_count.max(1));

        let rebuild = |bank: &mut WavetableBank,
                       layers: &[WaveLayerUi],
                       layer_idx: usize,
                       wt_position: f32,
                       wave_quant: u8| {
            let layer_wt_position = layers
                .get(layer_idx)
                .map(|l| l.wt_position)
                .unwrap_or(wt_position);
            let frame_idx = frame_index(layer_wt_position, bank.num_frames);
            let segs = layers
                .get(layer_idx)
                .map(|l| l.quant_segment_interps.as_slice())
                .unwrap_or(&[]);
            let curve_default = layers
                .get(layer_idx)
                .map(|l| l.quant_interp)
                .unwrap_or_default();
            let slots = effective_quant_count(wave_quant).max(1);
            let frame = bank.frame_mut(frame_idx);
            let points = quant_control_points(frame, slots);
            resample_frame_from_quant_points(frame, &points, segs, curve_default);
        };

        // Interp / toolbar mutations apply after paint so the plot fill cannot cover chrome.
        let mut seg_scratch = WtQuantInterp::Hold;
        let mut seg_scratch_for_toolbar = false;

        let layer_wt_position = self
            .wave_layers
            .get(layer_idx)
            .map(|l| l.wt_position)
            .unwrap_or(*self.wt_position);
        let frame_idx = self
            .bank
            .as_ref()
            .map(|b| frame_index(layer_wt_position, b.num_frames))
            .unwrap_or(0);

        let empty = WavetableBank::factory_saw_morph();
        let bank_ro = self.bank.as_ref().map(|b| &**b).unwrap_or(&empty);

        let layer_pts: Option<Vec<Pos2>> =
            self.wave_layers.get(layer_idx).and_then(|layer| {
                if layer.enabled && layer.level > 0.0 {
                    let pts = if layer_va {
                        va_layer_waveform_points(layer, inner, WAVE_SAMPLES)
                    } else {
                        layer_waveform_points(layer, bank_ro, inner, 0.0, WAVE_SAMPLES)
                    };
                    if pts.len() >= 2 {
                        Some(curve_view.map_points(&pts, inner))
                    } else {
                        None
                    }
                } else {
                    None
                }
            });

        let pointer_in_plot = ui
            .ctx()
            .pointer_latest_pos()
            .filter(|p| inner.contains(*p));
        let hover_slot_pre = if quant_active {
            pointer_in_plot.and_then(|pos| {
                self.bank.as_ref().and_then(|bank| {
                    self.wave_layers.get(layer_idx).map(|layer| {
                        let slot_count = effective_quant_count(self.wave_quant);
                        let scale = layer_quant_display_scale(layer);
                        let points = quant_control_points(bank.frame(frame_idx), slot_count);
                        nearest_quant_handle(
                            curve_view.unmap_pos(pos, inner),
                            inner,
                            &points,
                            scale,
                            hit_r,
                        )
                    })
                })
            })
            .flatten()
        } else {
            None
        };
        let over_quant_knob = hover_slot_pre.is_some();
        let curve_hovered = pointer_in_plot.is_some_and(|pos| {
            layer_pts
                .as_ref()
                .is_some_and(|pts| selected_curve_hovered(pts, pos, over_quant_knob, HOVER_DISTANCE_PX))
        });

        // Paint plot first (bg → wave → knobs) so knobs cannot be covered by a late fill.
        let mut painter = ui.painter_at(rect);
        painter.set_clip_rect(inner.expand(1.0));
        painter.rect_filled(rect, RADIUS_SM, tokens.bg);
        painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0, tokens.border));
        paint_grid(&painter, inner, tokens.border);

        // Zero line / fill baseline follow zoom-pan (curve points are already mapped).
        let baseline_y = curve_view.map_pos(Pos2::new(inner.center().x, mid_y), inner).y;

        if let Some(pts) = layer_pts.as_ref() {
            let color = layer_palette(layer_idx);
            if let Some(fill) = waveform_fill_shape(
                pts,
                baseline_y,
                color.gamma_multiply(if curve_hovered { 0.32 } else { 0.25 }),
            ) {
                painter.add(fill);
            }
            let (stroke_w, stroke_alpha) = if curve_hovered {
                (3.4, 1.0)
            } else {
                (2.8, 0.98)
            };
            if curve_hovered {
                painter.add(Shape::line(
                    pts.clone(),
                    egui::Stroke::new(stroke_w + 2.0, color.gamma_multiply(0.35)),
                ));
            }
            painter.add(Shape::line(
                pts.clone(),
                egui::Stroke::new(stroke_w, color.gamma_multiply(stroke_alpha)),
            ));
        }

        painter.line_segment(
            [
                Pos2::new(inner.min.x, baseline_y),
                Pos2::new(inner.max.x, baseline_y),
            ],
            egui::Stroke::new(1.0, tokens.border.gamma_multiply(0.75)),
        );

        // Always paint Quant knobs on the same painter as the wave when editable.
        // QuantHandleEditor also paints when interactive; double-draw is identical.
        if quant_active {
            if let (Some(bank), Some(layer)) = (self.bank.as_ref(), self.wave_layers.get(layer_idx))
            {
                let slot_count = effective_quant_count(self.wave_quant);
                let scale = layer_quant_display_scale(layer);
                let points = quant_control_points(bank.frame(frame_idx), slot_count);
                let color = layer_palette(layer_idx);
                let pointer = pointer_in_plot;
                let hover_slot = hover_slot_pre.or_else(|| {
                    pointer.and_then(|pos| {
                        nearest_quant_handle(
                            curve_view.unmap_pos(pos, inner),
                            inner,
                            &points,
                            scale,
                            hit_r,
                        )
                    })
                });
                for i in 0..slot_count {
                    let show = self.wave_quant <= 64
                        || hover_slot == Some(i)
                        || i == 0
                        || i + 1 == slot_count;
                    if !show {
                        continue;
                    }
                    let x = slot_x(i, slot_count, inner);
                    let sample = points.get(i).copied().unwrap_or(0.0);
                    let y = knob_y_on_curve(sample, scale, inner);
                    let center = curve_view.map_pos(Pos2::new(x, y), inner);
                    let visual = quant_knob_visual(hover_slot == Some(i), false);
                    let fill = if visual.fill_brighter {
                        color.gamma_multiply(0.5)
                    } else {
                        color.gamma_multiply(0.22)
                    };
                    paint_quant_knob(&painter, center, visual, fill, color, inner);
                }
            }
        }

        let layer_type = self
            .wave_layers
            .get(layer_idx)
            .map(|l| {
                if l.residual {
                    "residual".to_string()
                } else {
                    layer_type_display(&l.source_type)
                }
            })
            .unwrap_or_else(|| "saw".to_string());
        let label = format!("Edit · Layer {} · {layer_type}", layer_idx + 1);
        painter.text(
            Pos2::new(rect.min.x + 8.0, rect.min.y + 4.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(10.0),
            tokens.text_secondary,
        );

        // Interactive editors (after paint so allocate_rect hit-tests win).
        if *self.tool == WtEditTool::Pencil && layer_wt {
            if let Some(bank) = self.bank.as_mut() {
                let response = ui.allocate_rect(inner, Sense::click_and_drag());
                if apply_waveform_drag_inner(bank, frame_idx, inner, &response) {
                    frame_edited = true;
                }
            }
        }

        if *self.tool == WtEditTool::Curve && quant_active && !self.wave_slots.is_empty() {
            let curve = CurveEditor {
                plot_rect: inner,
                wave_quant: self.wave_quant,
                wave_slots: self.wave_slots.as_mut_slice(),
            };
            if curve.show(ui).changed {
                stack_changed = true;
            }
        }

        if *self.tool == WtEditTool::Shape && layer_wt {
            if let Some(bank) = self.bank.as_mut() {
                let shape = ShapeEditor {
                    plot_rect: inner,
                    bank,
                    frame_idx,
                    control_points: self.shape_control_points,
                };
                if shape.show(ui).frame_edited {
                    frame_edited = true;
                }
            }
        }

        // Drag reshape: Select/Pencil always; also when idle tools so knobs stay editable.
        let quant_interactive = quant_active
            && matches!(
                *self.tool,
                WtEditTool::Select | WtEditTool::Pencil | WtEditTool::Line | WtEditTool::Smooth
            );
        let mut quant_hint_active = false;
        if quant_interactive {
            if let Some(bank) = self.bank.as_mut() {
                if let Some(layer) = self.wave_layers.get(layer_idx) {
                    let display_scale = layer_quant_display_scale(layer);
                    let curve_default = layer.quant_interp;
                    let segs = layer.quant_segment_interps.clone();
                    let editor = QuantHandleEditor {
                        plot_rect: inner,
                        wave_quant: self.wave_quant,
                        bank,
                        frame_idx,
                        segment_interps: &segs,
                        curve_default,
                        selected_slot: self.selected_quant_slot,
                        display_scale,
                        view: curve_view,
                    };
                    let qh = editor.show(ui);
                    if qh.frame_edited {
                        frame_edited = true;
                    }
                    if let Some(label) = qh.status_label {
                        status_hint = Some(label);
                        quant_hint_active = true;
                    }
                }
            }
        } else if self.wave_quant > 0 && layer_va {
            status_hint.get_or_insert_with(|| {
                "Quant knobs: select a wavetable / residual layer".into()
            });
        }

        // Toolbar last — drawn above the plot fill / knobs.
        let toolbar_rect = Rect::from_min_max(rect.min, egui::pos2(rect.max.x, plot_top));
        if show_seg {
            if let Some(layer) = self.wave_layers.get(layer_idx) {
                let s = selected_slot.unwrap();
                seg_scratch = layer
                    .quant_segment_interps
                    .get(s)
                    .copied()
                    .unwrap_or(layer.quant_interp);
                seg_scratch_for_toolbar = true;
            }
        }
        let toolbar_resp = region(ui, toolbar_rect, |ui| {
            WtToolbar::show_with_analyze(
                ui,
                self.tool,
                if quant_active { self.wave_quant } else { 0 },
                self.quant_interp,
                selected_slot.filter(|_| quant_active),
                if seg_scratch_for_toolbar {
                    Some(&mut seg_scratch)
                } else {
                    None
                },
                Some(self.quant_seam),
                Some(self.patch_crackle),
            )
        });
        record_region(
            ui.ctx(),
            AuditId::CenterWt2dToolbar,
            toolbar_rect,
            toolbar_rect,
        );
        let WtToolbarResponse {
            analyze_requested: req,
            assign_shape,
            interp_changed,
            segment_interp_changed,
            seam_changed: _,
            crackle_changed,
            ..
        } = toolbar_resp;
        if crackle_changed {
            params_changed = true;
            crate::wt::set_crackle_amount(*self.patch_crackle);
        }

        if req {
            if let Some(open) = self.analyze_dialog_open {
                *open = true;
            }
            analyze_requested = true;
        }
        if let Some(kind) = assign_shape {
            if let Some(layer) = self.wave_layers.get_mut(layer_idx) {
                layer.source_type = super::view_2d::shape_template_source_type(kind).into();
                stack_changed = true;
                status_hint = Some(format!(
                    "Layer {} → {}",
                    layer_idx + 1,
                    super::view_2d::shape_template_source_type(kind)
                ));
            }
        }
        if seg_scratch_for_toolbar && segment_interp_changed {
            if let Some(layer) = self.wave_layers.get_mut(layer_idx) {
                let s = selected_slot.unwrap();
                if s < layer.quant_segment_interps.len() {
                    layer.quant_segment_interps[s] = seg_scratch;
                }
            }
        }
        if interp_changed && quant_active {
            if let Some(layer) = self.wave_layers.get_mut(layer_idx) {
                layer.apply_curve_interp_to_segments(slot_count, *self.quant_interp);
            }
            if let Some(bank) = self.bank.as_mut() {
                rebuild(
                    bank,
                    self.wave_layers,
                    layer_idx,
                    *self.wt_position,
                    self.wave_quant,
                );
                frame_edited = true;
                status_hint = Some(format!(
                    "Interp → {} (all segments)",
                    self.quant_interp.label()
                ));
            }
        } else if segment_interp_changed && quant_active {
            if let Some(bank) = self.bank.as_mut() {
                rebuild(
                    bank,
                    self.wave_layers,
                    layer_idx,
                    *self.wt_position,
                    self.wave_quant,
                );
                frame_edited = true;
            }
            if seg_scratch_for_toolbar {
                let s = selected_slot.unwrap();
                status_hint = Some(format!(
                    "Segment {}→{} · {}",
                    s + 1,
                    s + 2,
                    seg_scratch.label()
                ));
            }
        }

        if curve_hovered && !over_quant_knob && !quant_hint_active {
            if let Some(layer) = self.wave_layers.get(layer_idx) {
                status_hint = Some(format!("Hover · {}", layer_curve_label(layer_idx, layer)));
            }
            if pointer_in_plot.is_some() {
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }
        }

        record_region(ui.ctx(), AuditId::CenterWtSelected, rect, rect);
        record_region(ui.ctx(), AuditId::CenterWt2dPlot, plot_rect, plot_rect);

        WtSelectedLayerResponse {
            frame_edited,
            stack_changed,
            analyze_requested,
            params_changed,
            status_hint,
        }
    }
}

fn paint_grid(painter: &egui::Painter, inner: Rect, border: Color32) {
    let mid_y = inner.center().y;
    for i in 1..4 {
        let t = i as f32 / 4.0;
        let x = egui::lerp(inner.min.x..=inner.max.x, t);
        painter.line_segment(
            [Pos2::new(x, inner.min.y), Pos2::new(x, inner.max.y)],
            egui::Stroke::new(1.0, border.gamma_multiply(0.35)),
        );
        let y = egui::lerp(inner.min.y..=inner.max.y, t);
        if (y - mid_y).abs() > 2.0 {
            painter.line_segment(
                [Pos2::new(inner.min.x, y), Pos2::new(inner.max.x, y)],
                egui::Stroke::new(1.0, border.gamma_multiply(0.25)),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oscillator_ui::WaveLayerUi;

    #[test]
    fn selected_pane_shows_knobs_for_any_audible_layer() {
        let layers = vec![
            WaveLayerUi {
                source_type: "saw".into(),
                level: 1.0,
                enabled: true,
                ..WaveLayerUi::default()
            },
            WaveLayerUi {
                source_type: "wavetable".into(),
                level: 1.0,
                enabled: true,
                ..WaveLayerUi::default()
            },
            WaveLayerUi {
                source_type: "wavetable".into(),
                level: 1.0,
                enabled: true,
                residual: true,
                ..WaveLayerUi::default()
            },
        ];
        assert!(selected_pane_shows_quant_knobs(Some(0), &layers, 16));
        assert!(selected_pane_shows_quant_knobs(Some(1), &layers, 16));
        assert!(selected_pane_shows_quant_knobs(Some(2), &layers, 16));
        assert!(!selected_pane_shows_quant_knobs(Some(1), &layers, 0));
    }
}
