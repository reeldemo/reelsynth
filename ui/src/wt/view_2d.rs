use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Ui, Vec2};
use reelsynth::patch::{Patch, WaveSlot};
use reelsynth::WavetableBank;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::layout::{RADIUS_SM, WT_TOOLBAR_HEIGHT};
use crate::oscillator_ui::WaveLayerUi;
use crate::region::region;

use super::curve_editor::CurveEditor;
use super::mod_preview::{has_position_mod_routes, preview_mod_sources, preview_position_mod};
use super::shape_editor::ShapeEditor;
use super::slots::apply_slot_selection;
use super::toolbar::{WtEditTool, WtToolbar, WtToolbarResponse};
use super::waveform::{frame_index, peak_point, waveform_points};

pub struct WtView2dResponse {
    pub frame_edited: bool,
    pub position_changed: bool,
    pub morph_changed: bool,
    pub slots_changed: bool,
    pub stack_changed: bool,
    pub analyze_requested: bool,
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
    pub wave_slot: &'a mut u8,
    pub wave_slots: &'a mut Vec<WaveSlot>,
    pub wave_layers: Option<&'a mut Vec<WaveLayerUi>>,
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

        if !ui.is_rect_visible(rect) {
            return WtView2dResponse {
                frame_edited,
                position_changed,
                morph_changed,
                slots_changed,
                stack_changed,
                analyze_requested,
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

        let toolbar_resp = region(
            ui,
            Rect::from_min_max(rect.min, egui::pos2(rect.max.x, plot_top)),
            |ui| WtToolbar::show_with_analyze(ui, self.tool),
        );
        if let WtToolbarResponse {
            analyze_requested: req,
            ..
        } = toolbar_resp
        {
            if req {
                if let Some(open) = self.analyze_dialog_open {
                    *open = true;
                }
                analyze_requested = true;
            }
        }

        if *self.tool == WtEditTool::Select {
            let sense = Sense::click_and_drag();
            let response = ui.allocate_rect(inner, sense);
            if response.dragged() {
                let delta = response.drag_delta();
                if delta.x.abs() > 0.0 {
                    let px_per_frame = inner.width() / max_pos.max(1.0);
                    let next = (*self.position + delta.x / px_per_frame).clamp(0.0, max_pos);
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
            } else if response.clicked() {
                if let Some(pos) = response.interact_pointer_pos() {
                    if inner.contains(pos) {
                        if self.wave_quant > 0 {
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
                ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
            }
        }

        let frame_idx = self
            .bank
            .as_ref()
            .map(|b| frame_index(*self.position, b.num_frames))
            .unwrap_or(0);

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

        if self.wave_quant > 0 {
            let quant = self.wave_quant as f32;
            let slot_t = if quant > 1.0 {
                *self.wave_slot as f32 / (quant - 1.0)
            } else {
                0.0
            };
            let band_x = egui::lerp(inner.min.x..=inner.max.x, slot_t);
            let band_w = inner.width() / quant.max(1.0);
            let band = Rect::from_min_max(
                Pos2::new(band_x - band_w * 0.5, inner.min.y),
                Pos2::new(band_x + band_w * 0.5, inner.max.y),
            );
            painter.rect_filled(band, 0.0, tokens.accent.gamma_multiply(0.08));
            painter.rect_stroke(band, 0.0, egui::Stroke::new(1.0, accent_ui.gamma_multiply(0.35)));
        }

        let wave = if let Some(bank) = self.bank.as_ref() {
            let frame = bank.frame(frame_idx);
            waveform_points(frame, inner, 256, 0.42)
        } else {
            placeholder_wave(inner, mid_y)
        };

        if wave.len() >= 2 && *self.tool != WtEditTool::Curve {
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

        let label = if let Some(name) = self.bank_name {
            if pos_mod.abs() > 0.01 {
                format!(
                    "2D Waveform · {name} · frame {frame_idx} → {:.0}",
                    modulated_pos
                )
            } else {
                format!("2D Waveform · {name} · frame {frame_idx}")
            }
        } else if pos_mod.abs() > 0.01 {
            format!("2D Waveform · frame {frame_idx} → {:.0}", modulated_pos)
        } else {
            format!("2D Waveform · frame {frame_idx}")
        };
        painter.text(
            Pos2::new(plot_rect.min.x + 8.0, plot_rect.min.y + 4.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(10.0),
            tokens.text_secondary,
        );

        if *self.tool == WtEditTool::Pencil {
            if let Some(bank) = self.bank.as_mut() {
                let sense = Sense::click_and_drag();
                let response = ui.allocate_rect(inner, sense);
                if response.dragged() || response.drag_started() {
                    if let Some(curr) = response.interact_pointer_pos() {
                        let prev = curr - response.drag_delta();
                        let (cx, cy) = view_coords(inner, curr);
                        let (px, py) = view_coords(inner, prev);
                        bank.apply_pencil_segment(frame_idx, px, py, cx, cy);
                        frame_edited = true;
                    }
                }
            }
        }

        if *self.tool == WtEditTool::Curve && self.wave_quant > 0 {
            if !self.wave_slots.is_empty() {
                let curve = CurveEditor {
                    plot_rect: inner,
                    wave_quant: self.wave_quant,
                    wave_slots: self.wave_slots.as_mut_slice(),
                };
                if curve.show(ui).changed {
                    slots_changed = true;
                }
            }
        }

        if *self.tool == WtEditTool::Shape {
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

        if analyze_requested {
            stack_changed = true;
        }

        if self.animate {
            if let Some(patch) = self.patch {
                if has_position_mod_routes(patch) {
                    ui.ctx().request_repaint();
                }
            }
        }

        WtView2dResponse {
            frame_edited,
            position_changed,
            morph_changed,
            slots_changed,
            stack_changed,
            analyze_requested,
        }
    }
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
}
