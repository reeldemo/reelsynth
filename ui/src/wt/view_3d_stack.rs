//! 3D stack view — depth planes per wave layer + composite front plane.

use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Ui, Vec2};
use reelsynth::osc::{sample_layer, StackMode, WtWarpMode};
use reelsynth::patch::WaveLayer;
use reelsynth::WavetableBank;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::layout::RADIUS_SM;
use crate::region::region;
use crate::oscillator_ui::WaveLayerUi;
use crate::state::WtView3dMode;

const HOVER_DISTANCE_PX: f32 = 14.0;

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

struct StackLayout {
    inner: Rect,
    mesh_left: f32,
    mesh_width: f32,
    depth_pitch: f32,
    layer_count: usize,
}

impl StackLayout {
    fn new(inner: Rect, layer_count: usize) -> Self {
        let mesh_left = inner.min.x + inner.width() * 0.08;
        let mesh_width = inner.width() * 0.84;
        Self {
            inner,
            mesh_left,
            mesh_width,
            depth_pitch: inner.width() * 0.028,
            layer_count: layer_count.max(1),
        }
    }

    fn slice_geometry(&self, slice: usize, total: usize) -> Rect {
        let half = (total.saturating_sub(1)) as f32 * 0.5;
        let z_offset = (slice as f32 - half) * self.depth_pitch;
        let depth = (slice as f32 / total as f32 - 0.5).abs();
        let y_offset = depth * self.inner.height() * 0.22;
        Rect::from_min_max(
            Pos2::new(self.mesh_left + z_offset, self.inner.min.y + y_offset),
            Pos2::new(
                self.mesh_left + z_offset + self.mesh_width,
                self.inner.max.y - y_offset,
            ),
        )
    }
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

fn composite_stack_sample(
    layers: &[WaveLayerUi],
    bank: &WavetableBank,
    stack_mode: &str,
    phase: f32,
    wt_pos_offset: f32,
) -> f32 {
    let mode = StackMode::from_str(stack_mode);
    let mut sum = 0.0f32;
    let mut weight = 0.0f32;
    for layer in layers {
        if !layer.enabled || layer.level <= 0.0 {
            continue;
        }
        let s = sample_layer_at_phase(layer, bank, phase, wt_pos_offset);
        sum += s * layer.level;
        weight += layer.level;
    }
    if weight <= 0.0 {
        return 0.0;
    }
    match mode {
        StackMode::Add => sum,
        StackMode::Avg => sum / weight,
    }
}

fn layer_waveform_points(
    layer: &WaveLayerUi,
    bank: &WavetableBank,
    rect: Rect,
    wt_pos_offset: f32,
    samples: usize,
) -> Vec<Pos2> {
    let mut pts = Vec::with_capacity(samples + 1);
    for i in 0..=samples {
        let phase = i as f32 / samples as f32;
        let v = sample_layer_at_phase(layer, bank, phase, wt_pos_offset);
        let x = egui::lerp(rect.min.x..=rect.max.x, phase);
        let y = rect.center().y - v * rect.height() * 0.38;
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
        let y = rect.center().y - v * rect.height() * 0.38;
        pts.push(Pos2::new(x, y));
    }
    pts
}

fn distance_to_polyline(pos: Pos2, points: &[Pos2]) -> f32 {
    if points.len() < 2 {
        return f32::MAX;
    }
    points
        .windows(2)
        .map(|seg| {
            let ab = seg[1] - seg[0];
            let len_sq = ab.x * ab.x + ab.y * ab.y;
            if len_sq <= f32::EPSILON {
                return (pos - seg[0]).length();
            }
            let t = ((pos.x - seg[0].x) * ab.x + (pos.y - seg[0].y) * ab.y) / len_sq;
            let t = t.clamp(0.0, 1.0);
            let closest = Pos2::new(seg[0].x + ab.x * t, seg[0].y + ab.y * t);
            (pos - closest).length()
        })
        .fold(f32::MAX, f32::min)
}

pub struct WtView3dStackResponse {
    pub layer_selected: bool,
    pub wt_position_changed: bool,
}

pub struct WtView3dStack<'a> {
    pub layers: &'a mut [WaveLayerUi],
    pub stack_mode: &'a str,
    pub bank: Option<&'a WavetableBank>,
    pub wt_pos_offset: f32,
    pub selected_layer: &'a mut Option<usize>,
    pub view_mode: &'a mut WtView3dMode,
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

        if !ui.is_rect_visible(rect) {
            return WtView3dStackResponse {
                layer_selected,
                wt_position_changed,
            };
        }

        let inner = rect.shrink2(egui::vec2(8.0, 20.0));
        let bank = match self.bank {
            Some(b) => b,
            None => {
                let painter = ui.painter_at(rect);
                painter.rect_filled(rect, RADIUS_SM, tokens.bg);
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    "3D Stack · no bank",
                    egui::FontId::proportional(11.0),
                    tokens.text_muted,
                );
                return WtView3dStackResponse {
                    layer_selected,
                    wt_position_changed,
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

        let plane_count = active_indices.len() + 1;
        let layout = StackLayout::new(inner, plane_count);

        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let mut best_idx = None;
                let mut best_dist = HOVER_DISTANCE_PX;
                for (pi, &orig_idx) in active_indices.iter().enumerate() {
                    let layer = &self.layers[orig_idx];
                    let slice_rect = layout.slice_geometry(pi, plane_count);
                    let pts = layer_waveform_points(layer, bank, slice_rect, self.wt_pos_offset, 64);
                    let dist = distance_to_polyline(pos, &pts);
                    if dist < best_dist {
                        best_dist = dist;
                        best_idx = Some(orig_idx);
                    }
                }
                let composite_rect = layout.slice_geometry(plane_count - 1, plane_count);
                let comp_pts = composite_waveform_points(
                    self.layers,
                    bank,
                    self.stack_mode,
                    composite_rect,
                    self.wt_pos_offset,
                    64,
                );
                if distance_to_polyline(pos, &comp_pts) < best_dist {
                    best_idx = None;
                }
                if let Some(idx) = best_idx {
                    *self.selected_layer = Some(idx);
                    layer_selected = true;
                }
            }
        }

        if response.dragged() {
            if let Some(sel) = *self.selected_layer {
                if let Some(layer) = self.layers.get_mut(sel) {
                    if layer.source_type.eq_ignore_ascii_case("wavetable") {
                        let delta = response.drag_delta();
                        if delta.x.abs() > 0.0 {
                            let max_pos = (bank.num_frames.saturating_sub(1)).max(1) as f32;
                            let px_per_frame = inner.width() / max_pos.max(1.0);
                            layer.wt_position =
                                (layer.wt_position + delta.x / px_per_frame).clamp(0.0, max_pos);
                            wt_position_changed = true;
                        }
                    }
                }
            }
        }

        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
        }

        let selected_idx = *self.selected_layer;

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS_SM, tokens.bg);
        painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0, tokens.border));

        let label = format!(
            "3D Stack · {} layers · {} mode",
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
        region(
            ui,
            Rect::from_min_max(
                egui::pos2(rect.max.x - 120.0, rect.min.y + 4.0),
                egui::pos2(rect.max.x - 4.0, rect.min.y + 22.0),
            ),
            |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(self.view_mode, WtView3dMode::Stack, "Stack");
                    ui.selectable_value(self.view_mode, WtView3dMode::Morph, "Morph");
                });
            },
        );

        for (pi, &orig_idx) in active_indices.iter().enumerate() {
            let layer = &self.layers[orig_idx];
            let slice_rect = layout.slice_geometry(pi, plane_count);
            let color = layer_palette(orig_idx);
            let selected = selected_idx == Some(orig_idx);
            let pts = layer_waveform_points(layer, bank, slice_rect, self.wt_pos_offset, 64);
            if pts.len() >= 2 {
                let stroke_w = if selected { 2.5 } else { 1.2 };
                painter.add(Shape::line(
                    pts,
                    egui::Stroke::new(stroke_w, color.gamma_multiply(0.85)),
                ));
            }
            let type_label = &layer.source_type;
            painter.text(
                Pos2::new(slice_rect.min.x + 4.0, slice_rect.min.y + 2.0),
                egui::Align2::LEFT_TOP,
                type_label,
                egui::FontId::monospace(9.0),
                color,
            );
        }

        let composite_rect = layout.slice_geometry(plane_count - 1, plane_count);
        let comp_pts = composite_waveform_points(
            self.layers,
            bank,
            self.stack_mode,
            composite_rect,
            self.wt_pos_offset,
            128,
        );
        if comp_pts.len() >= 2 {
            painter.add(Shape::line(
                comp_pts,
                egui::Stroke::new(2.5, accent_ui),
            ));
            painter.text(
                Pos2::new(composite_rect.min.x + 4.0, composite_rect.min.y + 2.0),
                egui::Align2::LEFT_TOP,
                "Sum",
                egui::FontId::monospace(9.0),
                accent_ui,
            );
        }

        let phase_anim = (self.time * 0.5).fract();
        let play_x = egui::lerp(composite_rect.min.x..=composite_rect.max.x, phase_anim);
        painter.line_segment(
            [
                Pos2::new(play_x, composite_rect.min.y),
                Pos2::new(play_x, composite_rect.max.y),
            ],
            egui::Stroke::new(1.0, tokens.text_muted.gamma_multiply(0.5)),
        );

        ui.ctx().request_repaint();

        WtView3dStackResponse {
            layer_selected,
            wt_position_changed,
        }
    }
}
