//! Slot→frame curve editor (morph scan map).

use egui::{CursorIcon, Pos2, Rect, Sense, Shape, Ui, Vec2};
use reelsynth::patch::WaveSlot;
use reelsynth_ui_theme::Tokens;

use super::slots::effective_quant_count;

pub struct CurveEditorResponse {
    pub changed: bool,
}

pub struct CurveEditor<'a> {
    pub plot_rect: Rect,
    pub wave_quant: u8,
    pub wave_slots: &'a mut [WaveSlot],
}

impl CurveEditor<'_> {
    pub fn show(self, ui: &mut Ui) -> CurveEditorResponse {
        let tokens = Tokens::default();
        let mut changed = false;

        if self.wave_quant == 0 || self.wave_slots.is_empty() {
            return CurveEditorResponse { changed };
        }

        let quant = effective_quant_count(self.wave_quant);
        let rect = self.plot_rect;
        let max_frame = 255.0_f32;

        ui.allocate_rect(rect, Sense::hover());

        let painter = ui.painter_at(rect);
        let grid_stroke = egui::Stroke::new(0.5_f32, tokens.border.gamma_multiply(0.4));
        for i in 0..=4 {
            let y = egui::lerp(rect.min.y..=rect.max.y, i as f32 / 4.0);
            painter.line_segment(
                [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
                grid_stroke,
            );
        }

        let mut curve_pts = Vec::with_capacity(quant);
        for (i, slot) in self.wave_slots.iter().enumerate().take(quant) {
            let x_t = if quant > 1 {
                i as f32 / (quant - 1) as f32
            } else {
                0.0
            };
            let y_t = 1.0 - slot.frame / max_frame;
            let x = egui::lerp(rect.min.x..=rect.max.x, x_t);
            let y = egui::lerp(rect.min.y..=rect.max.y, y_t);
            curve_pts.push(Pos2::new(x, y));
        }

        if curve_pts.len() >= 2 {
            painter.add(Shape::line(
                curve_pts.clone(),
                egui::Stroke::new(1.5_f32, tokens.accent.gamma_multiply(0.6)),
            ));
        }

        let handle_radius = if quant > 64 { 3.0 } else { 5.0 };
        let hit = handle_radius + 5.0;
        let visible = ui.clip_rect().intersect(rect);

        for (i, pt) in curve_pts.iter().enumerate().rev() {
            if !visible
                .expand2(egui::vec2(handle_radius, handle_radius))
                .contains(*pt)
            {
                continue;
            }

            let handle_rect = Rect::from_center_size(*pt, Vec2::splat(hit * 2.0));
            let id = ui.id().with(("curve_slot", i));
            let response = ui.interact(handle_rect, id, Sense::drag());

            if response.dragged() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let frame_t =
                        1.0 - ((pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);
                    let frame = (frame_t * max_frame).clamp(0.0, max_frame);
                    if i < self.wave_slots.len() {
                        if (self.wave_slots[i].frame - frame).abs() > 0.5 {
                            self.wave_slots[i].frame = frame;
                            changed = true;
                        }
                    }
                }
            }

            if response.hovered() || response.dragged() {
                ui.ctx().set_cursor_icon(CursorIcon::Grab);
            }

            painter.circle_filled(*pt, handle_radius, tokens.accent);
            painter.circle_stroke(*pt, handle_radius, egui::Stroke::new(1.0_f32, tokens.accent_on));
            if quant <= 32 {
                painter.text(
                    *pt + egui::vec2(0.0, -8.0),
                    egui::Align2::CENTER_BOTTOM,
                    format!("{}", self.wave_slots[i].frame.round() as i32),
                    egui::FontId::monospace(8.0),
                    tokens.text_muted,
                );
            }
        }

        painter.text(
            Pos2::new(rect.min.x + 4.0, rect.min.y + 2.0),
            egui::Align2::LEFT_TOP,
            "Curve · slot → frame",
            egui::FontId::proportional(9.0),
            tokens.text_muted,
        );

        CurveEditorResponse { changed }
    }
}
