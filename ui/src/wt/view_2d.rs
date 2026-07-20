use egui::{Color32, CursorIcon, Pos2, Rect, Response, Sense, Shape, Ui, Vec2};
use reelsynth::patch::{Patch, WaveSlot};
use reelsynth::WavetableBank;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::audit_registry::{record_region, AuditId};
use crate::layout::{RADIUS_SM, WT_TOOLBAR_HEIGHT};
use crate::oscillator_ui::WaveLayerUi;
use crate::region::region;

use super::curve_editor::CurveEditor;
use super::mod_preview::{preview_mod_sources, preview_position_mod};
use crate::quant_interp::WtQuantInterp;
use super::quant_handles::{
    QuantHandleEditor, quant_control_points, resample_frame_from_quant_points_uniform, slot_x,
};
use super::shape_editor::ShapeEditor;
use super::slots::{apply_slot_selection, effective_quant_count};
use super::toolbar::{FrameShapeTemplate, WtEditTool, WtToolbar, WtToolbarResponse};
use super::view_3d_stack::{
    composite_waveform_points, layer_palette, layer_waveform_points, HOVER_DISTANCE_PX,
    WAVE_SAMPLES,
};
use super::waveform::{
    frame_index, hit_test_waveform, nearest_waveform_distance, peak_point, waveform_fill_shape,
    waveform_points,
};
use super::view_zoom::WtCurveViewTransform;

#[derive(Clone, Copy, PartialEq, Eq)]
enum SelectDragKind {
    Navigate,
    Waveform,
}

pub(crate) fn apply_waveform_drag_inner(
    bank: &mut WavetableBank,
    frame_idx: usize,
    inner: Rect,
    response: &Response,
) -> bool {
    if response.dragged() || response.drag_started() {
        if let Some(curr) = response.interact_pointer_pos() {
            let prev = curr - response.drag_delta();
            let (cx, cy) = view_coords(inner, curr);
            let (px, py) = view_coords(inner, prev);
            bank.apply_pencil_segment(frame_idx, px, py, cx, cy);
            return true;
        }
    }
    false
}

pub struct WtView2dResponse {
    pub frame_edited: bool,
    pub position_changed: bool,
    pub morph_changed: bool,
    pub slots_changed: bool,
    pub stack_changed: bool,
    pub analyze_requested: bool,
    /// Footer / status hint (e.g. drag affordance).
    pub status_hint: Option<String>,
}

impl WtView2dResponse {
    pub fn changed(&self) -> bool {
        self.position_changed || self.morph_changed || self.slots_changed || self.stack_changed
    }
}

pub struct WtView2d<'a> {
    pub position: &'a mut f32,
    pub bank: Option<&'a mut WavetableBank>,
    pub bank_name: Option<&'a str>,
    pub tool: &'a mut WtEditTool,
    pub morph_amount: Option<&'a mut f32>,
    pub patch: Option<&'a Patch>,
    pub macro_values: Option<&'a [f32; 4]>,
    pub wave_quant: u8,
    pub quant_interp: &'a mut WtQuantInterp,
    pub wave_slot: &'a mut u8,
    pub wave_slots: &'a mut Vec<WaveSlot>,
    pub wave_layers: Option<&'a mut Vec<WaveLayerUi>>,
    pub selected_layer_idx: &'a mut Option<usize>,
    pub stack_mode: Option<&'a mut String>,
    pub shape_control_points: usize,
    pub analyze_dialog_open: Option<&'a mut bool>,
    pub animate: bool,
    pub time: f32,
}

impl WtView2d<'_> {
    pub fn show(mut self, ui: &mut Ui) -> WtView2dResponse {
        let tokens = Tokens::default();
        let accent_ui = ACCENT_UI;
        let view_h = ui.available_height().max(48.0);
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), view_h),
            Sense::hover(),
        );

        let mut frame_edited = false;
        let mut position_changed = false;
        let mut morph_changed = false;
        let mut slots_changed = false;
        let mut stack_changed = false;
        let mut analyze_requested = false;
        let mut status_hint: Option<String> = None;

        if !ui.is_rect_visible(rect) {
            return WtView2dResponse {
                frame_edited,
                position_changed,
                morph_changed,
                slots_changed,
                stack_changed,
                analyze_requested,
                status_hint,
            };
        }

        let plot_top = rect.min.y + WT_TOOLBAR_HEIGHT;
        let plot_rect = Rect::from_min_max(
            egui::pos2(rect.min.x, plot_top),
            rect.max,
        );
        let inner = plot_rect.shrink2(egui::vec2(8.0, 12.0));
        let mid_y = inner.center().y;

        let num_frames = self
            .bank
            .as_ref()
            .map(|b| b.num_frames)
            .unwrap_or(256)
            .max(1);
        let max_pos = (num_frames - 1) as f32;

        let layer_idx = self
            .selected_layer_idx
            .unwrap_or(0);
        let active_layer_va = self
            .wave_layers
            .as_ref()
            .and_then(|layers| layers.get(layer_idx))
            .map(|l| l.is_va())
            .unwrap_or(true);
        let active_layer_wt = self
            .wave_layers
            .as_ref()
            .and_then(|layers| layers.get(layer_idx))
            .map(|l| l.is_wavetable())
            .unwrap_or(false);
        let quant_active = active_layer_wt && self.wave_quant > 0;
        let stack_overlay = self
            .wave_layers
            .as_ref()
            .map(|l| !l.is_empty())
            .unwrap_or(false);

        let toolbar_rect = Rect::from_min_max(rect.min, egui::pos2(rect.max.x, plot_top));
        let toolbar_resp = region(
            ui,
            toolbar_rect,
            |ui| {
                WtToolbar::show_with_analyze(
                    ui,
                    self.tool,
                    if quant_active { self.wave_quant } else { 0 },
                    self.quant_interp,
                    None,
                    None,
                    None,
                    None,
                )
            },
        );
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
            if let Some(layers) = self.wave_layers.as_mut() {
                if let Some(layer) = layers.get_mut(layer_idx) {
                    layer.source_type = shape_template_source_type(kind).into();
                    stack_changed = true;
                    status_hint = Some(format!(
                        "Layer {} → {}",
                        layer_idx + 1,
                        shape_template_source_type(kind)
                    ));
                }
            }
        }

        let layer_wt_position = self
            .wave_layers
            .as_ref()
            .and_then(|layers| layers.get(layer_idx))
            .map(|l| l.wt_position)
            .unwrap_or(*self.position);

        let frame_idx = self
            .bank
            .as_ref()
            .map(|b| frame_index(layer_wt_position, b.num_frames))
            .unwrap_or(0);

        if interp_changed && quant_active {
            if let Some(bank) = self.bank.as_mut() {
                if self.wave_quant > 0 && quant_active {
                    let slot_count = effective_quant_count(self.wave_quant);
                    let frame = bank.frame_mut(frame_idx);
                    let points = quant_control_points(frame, slot_count);
                    resample_frame_from_quant_points_uniform(frame, &points, *self.quant_interp);
                    frame_edited = true;
                    status_hint = Some(format!(
                        "Interp → {} (frame rebuilt)",
                        self.quant_interp.label()
                    ));
                }
            }
        }

        let wave = if active_layer_va {
            self.wave_layers
                .as_ref()
                .and_then(|layers| layers.get(layer_idx))
                .map(|layer| va_layer_waveform_points(layer, inner, 256))
                .unwrap_or_else(|| placeholder_wave(inner, mid_y))
        } else if let Some(bank) = self.bank.as_ref() {
            let frame = bank.frame(frame_idx);
            waveform_points(frame, inner, 256, 0.42)
        } else {
            placeholder_wave(inner, mid_y)
        };

        // Layer drag on Result pane when not reshaping quant knobs.
        if *self.tool == WtEditTool::Select && stack_overlay && !quant_active {
            let sense = Sense::click_and_drag();
            let response = ui.allocate_rect(inner, sense);
            let drag_kind_id = ui.id().with("left_result_layer_drag");
            let bank_ref = self.bank.as_ref().map(|b| &**b);
            let empty = WavetableBank::factory_saw_morph();
            let bank = bank_ref.unwrap_or(&empty);

            let layer_pts: Vec<(usize, Vec<Pos2>)> = self
                .wave_layers
                .as_ref()
                .map(|layers| {
                    layers
                        .iter()
                        .enumerate()
                        .filter(|(_, l)| l.enabled && l.level > 0.0)
                        .map(|(i, l)| {
                            (
                                i,
                                layer_waveform_points(l, bank, inner, 0.0, WAVE_SAMPLES),
                            )
                        })
                        .collect()
                })
                .unwrap_or_default();

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
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(drag_kind_id, Some(idx)));
                    } else {
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(drag_kind_id, None::<usize>));
                    }
                }
            }

            if response.dragged() {
                let sel = ui
                    .ctx()
                    .data(|d| d.get_temp::<Option<usize>>(drag_kind_id))
                    .flatten()
                    .or(*self.selected_layer_idx);
                if let Some(idx) = sel {
                    if let Some(layers) = self.wave_layers.as_mut() {
                        if let Some(layer) = layers.get_mut(idx) {
                            let delta = response.drag_delta();
                            if delta.y.abs() > 0.0 {
                                let next =
                                    (layer.level - delta.y / inner.height()).clamp(0.0, 1.0);
                                if (next - layer.level).abs() > f32::EPSILON {
                                    layer.level = next;
                                    stack_changed = true;
                                }
                            }
                            if delta.x.abs() > 0.0 {
                                let max_pos =
                                    (bank.num_frames.saturating_sub(1)).max(1) as f32;
                                let px_per_frame = inner.width() / max_pos.max(1.0);
                                if layer.is_wavetable() {
                                    layer.wt_position = (layer.wt_position
                                        + delta.x / px_per_frame)
                                        .clamp(0.0, max_pos);
                                    stack_changed = true;
                                } else {
                                    layer.phase +=
                                        delta.x / inner.width() * std::f32::consts::TAU;
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
                status_hint = Some("Result · drag a layer (Y=level, X=phase/WT)".into());
            }
        }

        if *self.tool == WtEditTool::Select && active_layer_wt && !stack_overlay {
            let sense = Sense::click_and_drag();
            let response = ui.allocate_rect(inner, sense);
            let drag_kind_id = ui.id().with("wt_select_drag_kind");
            let wave_tolerance = 10.0;
            let quant_mode = quant_active;

            if response.drag_started() {
                let kind = response
                    .interact_pointer_pos()
                    .map(|pos| {
                        if quant_mode {
                            SelectDragKind::Waveform
                        } else if wave.len() >= 2 && hit_test_waveform(&wave, pos, wave_tolerance) {
                            SelectDragKind::Waveform
                        } else {
                            SelectDragKind::Navigate
                        }
                    })
                    .unwrap_or(SelectDragKind::Navigate);
                ui.ctx()
                    .data_mut(|d| d.insert_temp(drag_kind_id, kind));
            }

            if response.dragged() {
                let kind = ui
                    .ctx()
                    .data(|d| d.get_temp(drag_kind_id))
                    .unwrap_or(SelectDragKind::Navigate);
                match kind {
                    SelectDragKind::Waveform => {
                        if !quant_mode {
                            if let Some(bank) = self.bank.as_mut() {
                                if apply_waveform_drag_inner(bank, frame_idx, inner, &response) {
                                    frame_edited = true;
                                }
                            }
                        }
                    }
                    SelectDragKind::Navigate => {
                        let delta = response.drag_delta();
                        if delta.x.abs() > 0.0 {
                            let px_per_frame = inner.width() / max_pos.max(1.0);
                            let next =
                                (*self.position + delta.x / px_per_frame).clamp(0.0, max_pos);
                            if (next - *self.position).abs() > 0.01 {
                                *self.position = next;
                                position_changed = true;
                            }
                        }
                        if delta.y.abs() > 0.0 {
                            if let Some(morph) = self.morph_amount {
                                let delta_amount = -delta.y / inner.height();
                                let next = (*morph + delta_amount).clamp(0.0, 1.0);
                                if (next - *morph).abs() > f32::EPSILON {
                                    *morph = next;
                                    morph_changed = true;
                                }
                            }
                        }
                    }
                }
            } else if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    if inner.contains(pos) {
                        if wave.len() >= 2 && hit_test_waveform(&wave, pos, wave_tolerance) {
                            if let Some(bank) = self.bank.as_mut() {
                                let (x, y) = view_coords(inner, pos);
                                bank.apply_pencil_segment(frame_idx, x, y, x, y);
                                frame_edited = true;
                            }
                        } else if self.wave_quant > 0 {
                            let slot_t =
                                ((pos.x - inner.min.x) / inner.width()).clamp(0.0, 1.0);
                            let slot = (slot_t * (self.wave_quant as f32 - 1.0).max(0.0))
                                .round() as u8;
                            let prev = *self.wave_slot;
                            apply_slot_selection_from_parts(
                                self.wave_quant,
                                self.wave_slot,
                                self.wave_slots,
                                self.position,
                                slot,
                                num_frames,
                            );
                            if *self.wave_slot != prev {
                                slots_changed = true;
                                position_changed = true;
                            }
                        } else {
                            let next = position_from_plot_x(inner, pos.x, num_frames);
                            if (next - *self.position).abs() > 0.01 {
                                *self.position = next;
                                position_changed = true;
                            }
                        }
                    }
                }
            }

            if response.hovered() {
                let cursor = response
                    .interact_pointer_pos()
                    .map(|pos| {
                        if wave.len() >= 2 && hit_test_waveform(&wave, pos, wave_tolerance) {
                            CursorIcon::Grab
                        } else {
                            CursorIcon::ResizeHorizontal
                        }
                    })
                    .unwrap_or(CursorIcon::ResizeHorizontal);
                ui.ctx().set_cursor_icon(cursor);
            }
        }

        let pos_mod = if let (Some(patch), Some(macros)) = (self.patch, self.macro_values) {
            let sources = preview_mod_sources(patch, self.time, macros);
            preview_position_mod(patch, &sources, macros)
        } else {
            0.0
        };
        let modulated_pos = (*self.position + pos_mod).clamp(0.0, max_pos);
        let modulated_frame_idx = self
            .bank
            .as_ref()
            .map(|b| frame_index(modulated_pos, b.num_frames))
            .unwrap_or(0);

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS_SM, tokens.bg);
        painter.rect_stroke(
            rect,
            RADIUS_SM,
            egui::Stroke::new(1.0_f32, tokens.border),
        );

        paint_grid(&painter, inner, tokens.border);

        if self.wave_quant > 0 && quant_active {
            let quant = effective_quant_count(self.wave_quant);
            for i in 0..quant {
                let x = slot_x(i, quant, inner);
                painter.line_segment(
                    [Pos2::new(x, inner.min.y), Pos2::new(x, inner.max.y)],
                    egui::Stroke::new(0.5, tokens.border.gamma_multiply(0.5)),
                );
            }
            let slot_t = if quant > 1 {
                *self.wave_slot as f32 / (quant as f32 - 1.0)
            } else {
                0.0
            };
            let band_x = egui::lerp(inner.min.x..=inner.max.x, slot_t);
            let band_w = inner.width() / quant as f32;
            let band = Rect::from_min_max(
                Pos2::new(band_x - band_w * 0.5, inner.min.y),
                Pos2::new(band_x + band_w * 0.5, inner.max.y),
            );
            painter.rect_filled(band, 0.0, tokens.accent.gamma_multiply(0.08));
            painter.rect_stroke(band, 0.0, egui::Stroke::new(1.0, accent_ui.gamma_multiply(0.35)));
        }

        if stack_overlay && *self.tool != WtEditTool::Curve {
            let empty = WavetableBank::factory_saw_morph();
            let bank = self.bank.as_ref().map(|b| &**b).unwrap_or(&empty);
            let mode = self
                .stack_mode
                .as_ref()
                .map(|s| s.as_str())
                .unwrap_or("avg");
            let layers = self.wave_layers.as_ref().map(|l| l.as_slice()).unwrap_or(&[]);
            let selected = *self.selected_layer_idx;

            for (i, layer) in layers.iter().enumerate() {
                if !layer.enabled || layer.level <= 0.0 {
                    continue;
                }
                let pts = layer_waveform_points(layer, bank, inner, 0.0, WAVE_SAMPLES);
                if pts.len() < 2 {
                    continue;
                }
                let color = layer_palette(i);
                let is_sel = selected == Some(i);
                let alpha = if is_sel { 0.85 } else { 0.40 };
                let stroke_w = if is_sel { 1.8 } else { 1.2 };
                painter.add(Shape::line(
                    pts,
                    egui::Stroke::new(stroke_w, color.gamma_multiply(alpha)),
                ));
            }

            let result_pts =
                composite_waveform_points(layers, bank, mode, inner, 0.0, WAVE_SAMPLES);
            if result_pts.len() >= 2 {
                if let Some(fill) =
                    waveform_fill_shape(&result_pts, mid_y, tokens.accent.gamma_multiply(0.28))
                {
                    painter.add(fill);
                }
                painter.add(Shape::line(
                    result_pts.clone(),
                    egui::Stroke::new(2.6, accent_ui),
                ));
                if let Some(peak) = peak_point(&result_pts) {
                    painter.circle_filled(peak, 4.0, tokens.accent);
                    painter.circle_stroke(
                        peak,
                        4.0,
                        egui::Stroke::new(1.0, tokens.accent_on),
                    );
                }
                record_region(ui.ctx(), AuditId::CenterWt2dResult, inner, inner);
            }
        } else if wave.len() >= 2 && *self.tool != WtEditTool::Curve {
            if let Some(fill) =
                waveform_fill_shape(&wave, mid_y, tokens.accent.gamma_multiply(0.35))
            {
                painter.add(fill);
            }
            painter.add(Shape::line(
                wave.clone(),
                egui::Stroke::new(2.0_f32, accent_ui),
            ));

            if let Some(peak) = peak_point(&wave) {
                painter.circle_filled(peak, 4.0, tokens.accent);
                painter.circle_stroke(peak, 4.0, egui::Stroke::new(1.0_f32, tokens.accent_on));
            }
        }

        if pos_mod.abs() > 0.01 {
            if let Some(bank) = self.bank.as_ref() {
                let ghost_frame = bank.frame(modulated_frame_idx);
                let ghost = waveform_points(ghost_frame, inner, 256, 0.42);
                if ghost.len() >= 2 {
                    let ghost_stroke = accent_ui.gamma_multiply(0.45);
                    painter.add(Shape::line(
                        ghost,
                        egui::Stroke::new(1.5_f32, ghost_stroke),
                    ));
                }
            }

            let marker_x = egui::lerp(
                inner.min.x..=inner.max.x,
                (modulated_pos / max_pos.max(1.0)).clamp(0.0, 1.0),
            );
            painter.line_segment(
                [
                    Pos2::new(marker_x, inner.min.y),
                    Pos2::new(marker_x, inner.max.y),
                ],
                egui::Stroke::new(1.0_f32, accent_ui.gamma_multiply(0.55)),
            );
        }

        painter.line_segment(
            [Pos2::new(inner.min.x, mid_y), Pos2::new(inner.max.x, mid_y)],
            egui::Stroke::new(1.0_f32, tokens.border.gamma_multiply(0.75)),
        );

        let mut status_override: Option<String> = None;

        if *self.tool == WtEditTool::Pencil && active_layer_wt {
            if let Some(bank) = self.bank.as_mut() {
                let sense = Sense::click_and_drag();
                let response = ui.allocate_rect(inner, sense);
                if apply_waveform_drag_inner(bank, frame_idx, inner, &response) {
                    frame_edited = true;
                }
                if response.hovered() {
                    ui.ctx().set_cursor_icon(CursorIcon::Grab);
                }
            }
        }

        if *self.tool == WtEditTool::Curve && quant_active {
            if !self.wave_slots.is_empty() {
                let curve_before = inner;
                let curve = CurveEditor {
                    plot_rect: inner,
                    wave_quant: self.wave_quant,
                    wave_slots: self.wave_slots.as_mut_slice(),
                };
                if curve.show(ui).changed {
                    slots_changed = true;
                }
                record_region(ui.ctx(), AuditId::CenterWt2dCurveEditor, curve_before, inner);
            }
        }

        if *self.tool == WtEditTool::Shape && active_layer_wt {
            if let Some(bank) = self.bank.as_mut() {
                let shape_before = inner;
                let shape = ShapeEditor {
                    plot_rect: inner,
                    bank,
                    frame_idx,
                    control_points: self.shape_control_points,
                };
                if shape.show(ui).frame_edited {
                    frame_edited = true;
                }
                record_region(ui.ctx(), AuditId::CenterWt2dShapeEditor, shape_before, inner);
            }
        }

        if *self.tool == WtEditTool::Select && quant_active {
            if let Some(bank) = self.bank.as_mut() {
                let display_scale = self
                    .wave_layers
                    .as_ref()
                    .and_then(|layers| {
                        let idx = self.selected_layer_idx.unwrap_or(0);
                        layers.get(idx)
                    })
                    .map(|l| {
                        let sign = if l.invert { -1.0 } else { 1.0 };
                        let level = if l.enabled { l.level.max(0.0) } else { 0.0 };
                        // Match stacked layer amplitude so dots sit on the selected curve.
                        sign * level.max(0.05)
                    })
                    .unwrap_or(1.0);
                let mut selected_slot: Option<usize> = None;
                let segs = vec![
                    *self.quant_interp;
                    effective_quant_count(self.wave_quant).saturating_sub(1)
                ];
                let editor = QuantHandleEditor {
                    plot_rect: inner,
                    wave_quant: self.wave_quant,
                    bank,
                    frame_idx,
                    segment_interps: &segs,
                    curve_default: *self.quant_interp,
                    selected_slot: &mut selected_slot,
                    display_scale,
                    view: WtCurveViewTransform::default(),
                };
                let qh = editor.show(ui);
                if qh.frame_edited {
                    frame_edited = true;
                }
                if let Some(label) = qh.status_label {
                    status_hint = Some(label.clone());
                    status_override = Some(label);
                }
            }
        }

        let layer_type = self
            .wave_layers
            .as_ref()
            .and_then(|layers| layers.get(layer_idx))
            .map(|l| l.source_type.as_str())
            .unwrap_or("saw");
        let label = status_override.unwrap_or_else(|| {
            if stack_overlay {
                let n = self
                    .wave_layers
                    .as_ref()
                    .map(|l| l.iter().filter(|x| x.enabled).count())
                    .unwrap_or(0);
                format!("Result · {n} layers · Select+Quant: drag dots on selected curve")
            } else if active_layer_va {
                format!("Edit · Layer {} · {layer_type}", layer_idx + 1)
            } else if pos_mod.abs() > 0.01 {
                format!("Edit · Layer {} · WT · frame {frame_idx} → {:.0}", layer_idx + 1, modulated_pos)
            } else {
                format!("Edit · Layer {} · WT · frame {frame_idx}", layer_idx + 1)
            }
        });
        painter.text(
            Pos2::new(plot_rect.min.x + 8.0, plot_rect.min.y + 4.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(10.0),
            tokens.text_secondary,
        );

        if analyze_requested {
            stack_changed = true;
        }

        record_region(ui.ctx(), AuditId::CenterWt2d, rect, rect);
        record_region(ui.ctx(), AuditId::CenterWt2dPlot, plot_rect, plot_rect);

        WtView2dResponse {
            frame_edited,
            position_changed,
            morph_changed,
            slots_changed,
            stack_changed,
            analyze_requested,
            status_hint,
        }
    }
}

/// Map shape template to layer `source_type` string.
pub fn shape_template_source_type(kind: FrameShapeTemplate) -> &'static str {
    match kind {
        FrameShapeTemplate::Saw => "saw",
        FrameShapeTemplate::Square => "square",
        FrameShapeTemplate::Sine => "sine",
        FrameShapeTemplate::Tri => "triangle",
    }
}

/// Write a basic cycle template into a wavetable frame (click-to-assign palette).
pub fn apply_frame_shape_template(frame: &mut [f32], kind: FrameShapeTemplate) {
    let n = frame.len().max(1) as f32;
    for (i, sample) in frame.iter_mut().enumerate() {
        let p = i as f32 / n;
        *sample = match kind {
            FrameShapeTemplate::Saw => 2.0 * p - 1.0,
            FrameShapeTemplate::Square => {
                if p < 0.5 {
                    1.0
                } else {
                    -1.0
                }
            }
            FrameShapeTemplate::Sine => (p * std::f32::consts::TAU).sin(),
            FrameShapeTemplate::Tri => 1.0 - 4.0 * (p - 0.5).abs(),
        };
    }
}

/// Map VA `source_type` to a bakeable shape template.
pub fn va_source_to_shape_template(source_type: &str) -> Option<FrameShapeTemplate> {
    match source_type.to_ascii_lowercase().as_str() {
        "saw" => Some(FrameShapeTemplate::Saw),
        "square" | "pulse" => Some(FrameShapeTemplate::Square),
        "sine" => Some(FrameShapeTemplate::Sine),
        "triangle" | "tri" => Some(FrameShapeTemplate::Tri),
        _ => None,
    }
}

/// Pick a bank frame not already used by other wavetable layers (prefer high indices).
pub fn allocate_unused_wt_frame(
    num_frames: usize,
    occupied: &[usize],
) -> usize {
    if num_frames == 0 {
        return 0;
    }
    (0..num_frames)
        .rev()
        .find(|i| !occupied.contains(i))
        .unwrap_or(num_frames.saturating_sub(1))
}

/// Bake a VA layer into an unused bank frame and convert it to wavetable so Quant
/// knobs can edit it. Returns `true` when a conversion happened.
pub fn promote_va_layer_for_quant(
    layer: &mut WaveLayerUi,
    bank: &mut reelsynth::WavetableBank,
    occupied_frames: &[usize],
) -> bool {
    if layer.is_wavetable() || !layer.enabled || layer.level <= 0.0 {
        return false;
    }
    let frame_idx = allocate_unused_wt_frame(bank.num_frames, occupied_frames);
    let kind = va_source_to_shape_template(&layer.source_type)
        .unwrap_or(FrameShapeTemplate::Sine);
                apply_frame_shape_template(bank.frame_mut(frame_idx), kind);
    // Soft-close wrap so default ends are not a raw cliff (saw −1…+1).
    crate::wt::periodize_quant_frame(bank.frame_mut(frame_idx));
    layer.source_type = "wavetable".into();
    layer.wt_position = frame_idx as f32;
    true
}

fn apply_slot_selection_from_parts(
    wave_quant: u8,
    wave_slot: &mut u8,
    wave_slots: &[WaveSlot],
    position: &mut f32,
    slot: u8,
    num_frames: usize,
) {
    let mut osc_ui = crate::oscillator_ui::OscillatorUi {
        wave_quant,
        wave_slot: *wave_slot,
        wave_slots: wave_slots.to_vec(),
        position: *position,
        ..crate::oscillator_ui::OscillatorUi::new_silent()
    };
    apply_slot_selection(&mut osc_ui, slot, num_frames);
    *wave_slot = osc_ui.wave_slot;
    *position = osc_ui.position;
}

fn paint_grid(painter: &egui::Painter, rect: Rect, border: Color32) {
    let step = 24.0;
    let stroke = egui::Stroke::new(0.5_f32, border.gamma_multiply(0.75));
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

fn position_from_plot_x(inner: Rect, x: f32, num_frames: usize) -> f32 {
    let max_pos = (num_frames.saturating_sub(1)) as f32;
    let t = ((x - inner.min.x) / inner.width()).clamp(0.0, 1.0);
    t * max_pos
}

fn view_coords(inner: Rect, pos: Pos2) -> (f32, f32) {
    let x = ((pos.x - inner.min.x) / inner.width()).clamp(0.0, 1.0);
    let y = ((pos.y - inner.min.y) / inner.height()).clamp(0.0, 1.0);
    (x, y)
}

pub(crate) fn va_layer_waveform_points(layer: &WaveLayerUi, inner: Rect, samples: usize) -> Vec<Pos2> {
    use reelsynth::osc::{layer_sign, sample_layer, WtWarpMode};

    let bank = WavetableBank::factory_saw_morph();
    let patch = layer.to_patch();
    let sign = layer_sign(&patch);
    let level = if layer.enabled { layer.level.max(0.0) } else { 0.0 };
    let mid_y = inner.center().y;
    let samples = samples.max(2);
    (0..samples)
        .map(|i| {
            let phase = i as f32 / samples as f32;
            let t = i as f32 / (samples - 1) as f32;
            let v = sign
                * sample_layer(
                    &patch,
                    &bank,
                    phase,
                    1.0 / samples.max(1) as f32,
                    0.0,
                    WtWarpMode::None,
                    0.0,
                    0.0,
                    0.0,
                    1.0,
                )
                * level;
            let x = egui::lerp(inner.min.x..=inner.max.x, t);
            let y = mid_y - v * inner.height() * 0.42;
            Pos2::new(x, y)
        })
        .collect()
}

fn placeholder_wave(inner: Rect, mid_y: f32) -> Vec<Pos2> {
    (0..=128)
        .map(|i| {
            let t = i as f32 / 128.0;
            let x = egui::lerp(inner.min.x..=inner.max.x, t);
            let y = mid_y + (t * std::f32::consts::TAU * 2.0).sin() * inner.height() * 0.35;
            Pos2::new(x, y)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;

    #[test]
    fn position_from_plot_x_endpoints() {
        let inner = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 100.0));
        assert!((position_from_plot_x(inner, inner.min.x, 256) - 0.0).abs() < 1e-4);
        assert!((position_from_plot_x(inner, inner.max.x, 256) - 255.0).abs() < 1e-4);
        assert!((position_from_plot_x(inner, inner.center().x, 256) - 127.5).abs() < 1.0);
    }

    #[test]
    fn allocate_unused_wt_frame_prefers_free_high_index() {
        assert_eq!(allocate_unused_wt_frame(8, &[7, 6]), 5);
        assert_eq!(allocate_unused_wt_frame(4, &[0, 1, 2, 3]), 3);
        assert_eq!(allocate_unused_wt_frame(0, &[]), 0);
    }

    #[test]
    fn promote_va_layer_for_quant_bakes_and_converts() {
        let mut bank = WavetableBank::new(8, 64);
        let mut layer = WaveLayerUi {
            source_type: "saw".into(),
            level: 0.5,
            enabled: true,
            ..WaveLayerUi::default()
        };
        assert!(promote_va_layer_for_quant(&mut layer, &mut bank, &[7]));
        assert!(layer.is_wavetable());
        assert!((layer.wt_position - 6.0).abs() < f32::EPSILON);
        let frame = bank.frame(6);
        assert!(frame.iter().any(|s| s.abs() > 0.1));
        // Already WT — second call is a no-op.
        assert!(!promote_va_layer_for_quant(&mut layer, &mut bank, &[]));
    }
}
