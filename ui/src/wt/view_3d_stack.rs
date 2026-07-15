//! 2D stack overlay — all wave layers composited in one scope view.

use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Stroke, Ui, Vec2};
use reelsynth::osc::{layer_sign, sample_layer, StackMode, WtWarpMode};
use reelsynth::patch::WaveLayer;
use reelsynth::WavetableBank;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::audit_registry::{record_region, AuditId};
use crate::layout::RADIUS_SM;
use crate::oscillator_ui::WaveLayerUi;
use crate::region::region;
use crate::state::WtView3dMode;

use super::waveform::{nearest_waveform_distance, peak_point};

const HOVER_DISTANCE_PX: f32 = 14.0;
const WAVE_AMP: f32 = 0.42;
const WAVE_SAMPLES: usize = 256;

fn layer_palette(i: usize) -> Color32 {
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
        1.0 / 2048.0,
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

fn layer_waveform_points(
    layer: &WaveLayerUi,
    bank: &WavetableBank,
    rect: Rect,
    wt_pos_offset: f32,
    samples: usize,
) -> Vec<Pos2> {
    let patch = ui_layer_to_patch(layer);
    let sign = layer_sign(&patch);
    let level = if layer.enabled { layer.level.max(0.0) } else { 0.0 };
    let mut pts = Vec::with_capacity(samples + 1);
    for i in 0..=samples {
        let phase = i as f32 / samples as f32;
        let v = sign * sample_layer_at_phase(layer, bank, phase, wt_pos_offset) * level;
        let x = egui::lerp(rect.min.x..=rect.max.x, phase);
        let y = rect.center().y - v * rect.height() * WAVE_AMP;
        pts.push(Pos2::new(x, y));
    }
    pts
}

fn composite_waveform_points(
    layers: &[WaveLayerUi],
    bank: &WavetableBank,
    stack_mode: &str,
    rect: Rect,
    wt_pos_offset: f32,
    samples: usize,
) -> Vec<Pos2> {
    let mut pts = Vec::with_capacity(samples + 1);
    for i in 0..=samples {
        let phase = i as f32 / samples as f32;
        let v = composite_stack_sample(layers, bank, stack_mode, phase, wt_pos_offset);
        let x = egui::lerp(rect.min.x..=rect.max.x, phase);
        let y = rect.center().y - v * rect.height() * WAVE_AMP;
        pts.push(Pos2::new(x, y));
    }
    pts
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
    Composite,
}

pub struct WtView3dStackResponse {
    pub layer_selected: bool,
    pub wt_position_changed: bool,
    pub global_wt_scrub: bool,
}

pub struct WtView3dStack<'a> {
    pub layers: &'a mut [WaveLayerUi],
    pub stack_mode: &'a str,
    pub bank: Option<&'a WavetableBank>,
    pub wt_pos_offset: f32,
    pub wt_position: &'a mut f32,
    pub selected_layer: &'a mut Option<usize>,
    pub view_mode: Option<&'a mut WtView3dMode>,
    /// When false, hide Stack/Morph toggle (Design composite pane).
    pub show_mode_toggle: bool,
    pub time: f32,
}

impl WtView3dStack<'_> {
    pub fn show(self, ui: &mut Ui) -> WtView3dStackResponse {
        let tokens = Tokens::default();
        let accent_ui = ACCENT_UI;
        let view_h = ui.available_height().max(48.0);
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), view_h),
            Sense::click_and_drag(),
        );

        let mut layer_selected = false;
        let mut wt_position_changed = false;
        let mut global_wt_scrub = false;

        if !ui.is_rect_visible(rect) {
            return WtView3dStackResponse {
                layer_selected,
                wt_position_changed,
                global_wt_scrub,
            };
        }

        let inner = rect.shrink2(egui::vec2(8.0, 20.0));
        let mid_y = inner.center().y;

        record_region(ui.ctx(), AuditId::CenterWt3dStack, rect, rect);

        let bank = match self.bank {
            Some(b) => b,
            None => {
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, RADIUS_SM, tokens.bg);
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "Stack · no bank",
                    egui::FontId::proportional(11.0),
                    tokens.text_muted,
                );
                return WtView3dStackResponse {
                    layer_selected,
                    wt_position_changed,
                    global_wt_scrub,
                };
            }
        };

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

        let composite_pts = composite_waveform_points(
            self.layers,
            bank,
            self.stack_mode,
            inner,
            self.wt_pos_offset,
            WAVE_SAMPLES,
        );

        let drag_target_id = ui.id().with("stack_drag_target");

        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let mut best_idx = None;
                let mut best_dist = HOVER_DISTANCE_PX;
                for &(orig_idx, ref pts, _) in &layer_points {
                    let dist = nearest_waveform_distance(pts, pos);
                    if dist < best_dist {
                        best_dist = dist;
                        best_idx = Some(orig_idx);
                    }
                }
                let composite_dist = nearest_waveform_distance(&composite_pts, pos);
                let hit_composite = composite_dist < best_dist;
                if hit_composite {
                    *self.selected_layer = None;
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(drag_target_id, StackDragTarget::Composite));
                } else if let Some(idx) = best_idx {
                    *self.selected_layer = Some(idx);
                    layer_selected = true;
                    ui.ctx()
                        .data_mut(|d| d.insert_temp(drag_target_id, StackDragTarget::Layer(idx)));
                }
            }
        }

        if response.drag_started() {
            if let Some(pos) = response.interact_pointer_pos() {
                let mut best_idx = None;
                let mut best_dist = HOVER_DISTANCE_PX;
                for &(orig_idx, ref pts, _) in &layer_points {
                    let dist = nearest_waveform_distance(pts, pos);
                    if dist < best_dist {
                        best_dist = dist;
                        best_idx = Some(orig_idx);
                    }
                }
                let composite_dist = nearest_waveform_distance(&composite_pts, pos);
                let target = if composite_dist < best_dist {
                    StackDragTarget::Composite
                } else if let Some(idx) = best_idx {
                    StackDragTarget::Layer(idx)
                } else {
                    StackDragTarget::None
                };
                ui.ctx()
                    .data_mut(|d| d.insert_temp(drag_target_id, target));
            }
        }

        if response.dragged() {
            let target = ui
                .ctx()
                .data(|d| d.get_temp(drag_target_id))
                .unwrap_or(StackDragTarget::None);
            let delta = response.drag_delta();
            if delta.x.abs() > 0.0 {
                let max_pos = (bank.num_frames.saturating_sub(1)).max(1) as f32;
                let px_per_frame = inner.width() / max_pos.max(1.0);
                match target {
                    StackDragTarget::Composite => {
                        *self.wt_position =
                            (*self.wt_position + delta.x / px_per_frame).clamp(0.0, max_pos);
                        global_wt_scrub = true;
                        wt_position_changed = true;
                    }
                    StackDragTarget::Layer(sel) => {
                        if let Some(layer) = self.layers.get_mut(sel) {
                            if layer.source_type.eq_ignore_ascii_case("wavetable") {
                                layer.wt_position =
                                    (layer.wt_position + delta.x / px_per_frame)
                                        .clamp(0.0, max_pos);
                                wt_position_changed = true;
                            } else if layer.source_type.eq_ignore_ascii_case("sine") {
                                layer.phase += delta.x / inner.width() * std::f32::consts::TAU;
                                wt_position_changed = true;
                            }
                        }
                    }
                    StackDragTarget::None => {}
                }
            }
        }

        if response.hovered() {
            let cursor = if response.dragged() {
                CursorIcon::Grabbing
            } else {
                CursorIcon::Grab
            };
            ui.ctx().set_cursor_icon(cursor);
        }

        let selected_idx = *self.selected_layer;

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS_SM, tokens.bg);
        painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0, tokens.border));

        paint_grid(&painter, inner, tokens.border);
        painter.line_segment(
            [Pos2::new(inner.min.x, mid_y), Pos2::new(inner.max.x, mid_y)],
            egui::Stroke::new(1.0, tokens.border.gamma_multiply(0.75)),
        );

        let label = format!(
            "Composite · {} layers · {} mode",
            active_indices.len(),
            self.stack_mode
        );
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

        for &(orig_idx, ref pts, inverted) in &layer_points {
            if pts.len() < 2 {
                continue;
            }
            let color = layer_palette(orig_idx);
            let selected = selected_idx == Some(orig_idx);
            let alpha = if selected { 0.92 } else { 0.45 };
            let stroke_w = if selected { 2.2 } else { 1.4 };
            if inverted {
                let dash_stroke = egui::Stroke::new(stroke_w, color.gamma_multiply(alpha));
                for chunk in pts.windows(2).step_by(2) {
                    if chunk.len() == 2 {
                        painter.line_segment([chunk[0], chunk[1]], dash_stroke);
                    }
                }
                painter.text(
                    pts.first().copied().unwrap_or(inner.left_top()),
                    egui::Align2::LEFT_TOP,
                    "↓",
                    egui::FontId::proportional(9.0),
                    color.gamma_multiply(0.85),
                );
            } else {
                painter.add(Shape::line(
                    pts.clone(),
                    egui::Stroke::new(stroke_w, color.gamma_multiply(alpha)),
                ));
            }
        }

        if composite_pts.len() >= 2 {
            let mut fill = composite_pts.clone();
            fill.push(Pos2::new(inner.max.x, mid_y));
            fill.push(Pos2::new(inner.min.x, mid_y));
            painter.add(Shape::convex_polygon(
                fill,
                tokens.accent.gamma_multiply(0.22),
                egui::Stroke::NONE,
            ));
            painter.add(Shape::line(
                composite_pts.clone(),
                egui::Stroke::new(2.0, accent_ui),
            ));
            if let Some(peak) = peak_point(&composite_pts) {
                painter.circle_filled(peak, 3.5, tokens.accent);
                painter.circle_stroke(peak, 3.5, egui::Stroke::new(1.0, tokens.accent_on));
            }
        }

        let phase_anim = (self.time * 0.5).fract();
        let play_x = egui::lerp(inner.min.x..=inner.max.x, phase_anim);
        painter.line_segment(
            [
                Pos2::new(play_x, inner.min.y),
                Pos2::new(play_x, inner.max.y),
            ],
            egui::Stroke::new(1.0, tokens.text_muted.gamma_multiply(0.5)),
        );

        ui.ctx().request_repaint();

        WtView3dStackResponse {
            layer_selected,
            wt_position_changed,
            global_wt_scrub,
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
        assert_eq!(pts.len(), 33);
        assert!((pts.first().unwrap().x - rect.min.x).abs() < 1e-4);
        assert!((pts.last().unwrap().x - rect.max.x).abs() < 1e-4);
    }
}
