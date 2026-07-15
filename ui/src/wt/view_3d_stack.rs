//! Stack view — composite stacked waveform (matches audio output).

use egui::{CursorIcon, Pos2, Rect, Sense, Shape, Ui, Vec2};
use reelsynth::osc::{sample_layer, StackMode, WtWarpMode};
use reelsynth::patch::WaveLayer;
use reelsynth::WavetableBank;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::audit_registry::{record_region, AuditId};
use crate::layout::RADIUS_SM;
use crate::oscillator_ui::WaveLayerUi;
use crate::region::region;
use crate::state::WtView3dMode;

use super::waveform::peak_point;

const WAVE_AMP: f32 = 0.42;
const WAVE_SAMPLES: usize = 256;

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

        let layer_selected = false;
        let mut wt_position_changed = false;

        if !ui.is_rect_visible(rect) {
            return WtView3dStackResponse {
                layer_selected,
                wt_position_changed,
            };
        }

        let inner = rect.shrink2(egui::vec2(8.0, 20.0));
        let mid_y = inner.center().y;

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
                };
            }
        };

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

        if response.hovered() && self.selected_layer.is_some() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS_SM, tokens.bg);
        painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0, tokens.border));

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
                let toggle_rect = ui.min_rect();
                record_region(
                    ui.ctx(),
                    AuditId::CenterWt3dModeToggle,
                    toggle_rect,
                    toggle_rect,
                );
            },
        );

        let wave = composite_waveform_points(
            self.layers,
            bank,
            self.stack_mode,
            inner,
            self.wt_pos_offset,
            WAVE_SAMPLES,
        );

        if wave.len() >= 2 {
            let mut fill = wave.clone();
            fill.push(Pos2::new(inner.max.x, mid_y));
            fill.push(Pos2::new(inner.min.x, mid_y));
            painter.add(Shape::convex_polygon(
                fill,
                tokens.accent.gamma_multiply(0.35),
                egui::Stroke::NONE,
            ));
            painter.add(Shape::line(
                wave.clone(),
                egui::Stroke::new(2.0, accent_ui),
            ));
            if let Some(peak) = peak_point(&wave) {
                painter.circle_filled(peak, 4.0, tokens.accent);
                painter.circle_stroke(peak, 4.0, egui::Stroke::new(1.0, tokens.accent_on));
            }
        }

        painter.line_segment(
            [Pos2::new(inner.min.x, mid_y), Pos2::new(inner.max.x, mid_y)],
            egui::Stroke::new(1.0, tokens.border.gamma_multiply(0.75)),
        );

        let _ = self.time;

        record_region(ui.ctx(), AuditId::CenterWt3dStack, rect, rect);

        WtView3dStackResponse {
            layer_selected,
            wt_position_changed,
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
