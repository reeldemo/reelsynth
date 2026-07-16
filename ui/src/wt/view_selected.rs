//! Column 3 — selected layer focus, toolbar, and quant reshape.

use egui::{Color32, Pos2, Rect, Sense, Shape, Ui, Vec2};
use reelsynth::WavetableBank;
use reelsynth_ui_theme::Tokens;

use crate::audit_registry::{record_region, AuditId};
use crate::layout::{RADIUS_SM, WT_TOOLBAR_HEIGHT};
use crate::oscillator_ui::WaveLayerUi;
use crate::region::region;

use super::curve_editor::CurveEditor;
use super::quant_handles::{QuantHandleEditor, WtQuantInterp};
use super::shape_editor::ShapeEditor;
use super::slots::effective_quant_count;
use super::toolbar::{WtEditTool, WtToolbar, WtToolbarResponse};
use super::view_3d_stack::{layer_palette, layer_quant_display_scale, layer_waveform_points, WAVE_SAMPLES};
use super::view_2d::{apply_waveform_drag_inner, va_layer_waveform_points};
use super::waveform::frame_index;

pub struct WtSelectedLayerResponse {
    pub frame_edited: bool,
    pub stack_changed: bool,
    pub analyze_requested: bool,
    pub status_hint: Option<String>,
}

pub struct WtSelectedLayerView<'a> {
    pub wt_position: &'a mut f32,
    pub bank: Option<&'a mut WavetableBank>,
    pub tool: &'a mut WtEditTool,
    pub wave_quant: u8,
    pub quant_interp: &'a mut WtQuantInterp,
    pub wave_slot: &'a mut u8,
    pub wave_slots: &'a mut Vec<reelsynth::patch::WaveSlot>,
    pub wave_layers: &'a mut Vec<WaveLayerUi>,
    pub selected_layer_idx: &'a mut Option<usize>,
    pub shape_control_points: usize,
    pub analyze_dialog_open: Option<&'a mut bool>,
}

impl WtSelectedLayerView<'_> {
    pub fn show(mut self, ui: &mut Ui) -> WtSelectedLayerResponse {
        let tokens = Tokens::default();
        let view_h = ui.available_height().max(48.0);
        let (rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), view_h), Sense::hover());

        let mut frame_edited = false;
        let mut stack_changed = false;
        let mut analyze_requested = false;
        let mut status_hint: Option<String> = None;

        if !ui.is_rect_visible(rect) {
            return WtSelectedLayerResponse {
                frame_edited,
                stack_changed,
                analyze_requested,
                status_hint,
            };
        }

        let plot_top = rect.min.y + WT_TOOLBAR_HEIGHT;
        let plot_rect = Rect::from_min_max(egui::pos2(rect.min.x, plot_top), rect.max);
        let inner = plot_rect.shrink2(egui::vec2(8.0, 12.0));
        let mid_y = inner.center().y;

        let layer_idx = self.selected_layer_idx.unwrap_or(0);
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
        let quant_active = layer_wt && self.wave_quant > 0;

        let toolbar_rect = Rect::from_min_max(rect.min, egui::pos2(rect.max.x, plot_top));
        let toolbar_resp = region(ui, toolbar_rect, |ui| {
            WtToolbar::show_with_analyze(
                ui,
                self.tool,
                if quant_active { self.wave_quant } else { 0 },
                self.quant_interp,
            )
        });
        record_region(ui.ctx(), AuditId::CenterWt2dToolbar, toolbar_rect, toolbar_rect);
        let WtToolbarResponse {
            analyze_requested: req,
            assign_shape,
            interp_changed,
            ..
        } = toolbar_resp;
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

        if interp_changed && quant_active {
            if let Some(bank) = self.bank.as_mut() {
                use super::quant_handles::{quant_control_points, resample_frame_from_quant_points};
                let slot_count = effective_quant_count(self.wave_quant);
                let frame = bank.frame_mut(frame_idx);
                let points = quant_control_points(frame, slot_count);
                resample_frame_from_quant_points(frame, &points, *self.quant_interp);
                frame_edited = true;
            }
        }

        let empty = WavetableBank::factory_saw_morph();
        let bank_ro = self.bank.as_ref().map(|b| &**b).unwrap_or(&empty);

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS_SM, tokens.bg);
        painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0, tokens.border));
        paint_grid(&painter, inner, tokens.border);

        if let Some(layer) = self.wave_layers.get(layer_idx) {
            if layer.enabled && layer.level > 0.0 {
                let pts = if layer_va {
                    va_layer_waveform_points(layer, inner, WAVE_SAMPLES)
                } else {
                    layer_waveform_points(layer, bank_ro, inner, 0.0, WAVE_SAMPLES)
                };
                if pts.len() >= 2 {
                    let color = layer_palette(layer_idx);
                    let mut fill = pts.clone();
                    fill.push(Pos2::new(inner.max.x, mid_y));
                    fill.push(Pos2::new(inner.min.x, mid_y));
                    painter.add(Shape::convex_polygon(
                        fill,
                        color.gamma_multiply(0.25),
                        egui::Stroke::NONE,
                    ));
                    painter.add(Shape::line(
                        pts,
                        egui::Stroke::new(2.8, color.gamma_multiply(0.98)),
                    ));
                }
            }
        }

        painter.line_segment(
            [Pos2::new(inner.min.x, mid_y), Pos2::new(inner.max.x, mid_y)],
            egui::Stroke::new(1.0, tokens.border.gamma_multiply(0.75)),
        );

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

        if *self.tool == WtEditTool::Select && quant_active {
            if let Some(bank) = self.bank.as_mut() {
                if let Some(layer) = self.wave_layers.get(layer_idx) {
                    let display_scale = layer_quant_display_scale(layer);
                    let editor = QuantHandleEditor {
                        plot_rect: inner,
                        wave_quant: self.wave_quant,
                        bank,
                        frame_idx,
                        interp: *self.quant_interp,
                        display_scale,
                    };
                    let qh = editor.show(ui);
                    if qh.frame_edited {
                        frame_edited = true;
                    }
                    if let Some(label) = qh.status_label {
                        status_hint = Some(label);
                    }
                }
            }
        }

        let layer_type = self
            .wave_layers
            .get(layer_idx)
            .map(|l| {
                if l.residual {
                    "residual"
                } else {
                    l.source_type.as_str()
                }
            })
            .unwrap_or("saw");
        let label = format!("Edit · Layer {} · {layer_type}", layer_idx + 1);
        painter.text(
            Pos2::new(rect.min.x + 8.0, rect.min.y + 4.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(10.0),
            tokens.text_secondary,
        );

        record_region(ui.ctx(), AuditId::CenterWtSelected, rect, rect);
        record_region(ui.ctx(), AuditId::CenterWt2dPlot, plot_rect, plot_rect);

        WtSelectedLayerResponse {
            frame_edited,
            stack_changed,
            analyze_requested,
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
