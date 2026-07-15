//! Waveform shape editor — control points upsampled to 2048-sample frame.

use egui::{CursorIcon, Pos2, Rect, Sense, Ui};
use reelsynth::WavetableBank;
use reelsynth_ui_theme::Tokens;

pub struct ShapeEditorResponse {
    pub frame_edited: bool,
}

pub struct ShapeEditor<'a> {
    pub plot_rect: Rect,
    pub bank: &'a mut WavetableBank,
    pub frame_idx: usize,
    pub control_points: usize,
}

impl ShapeEditor<'_> {
    pub fn show(self, ui: &mut Ui) -> ShapeEditorResponse {
        let tokens = Tokens::default();
        let mut frame_edited = false;

        let rect = self.plot_rect;
        let n = self.control_points.clamp(8, 256);
        let frame = self.bank.frame(self.frame_idx).to_vec();
        let mut points = WavetableBank::downsample_frame_control_points(&frame, n);

        let sense = Sense::click_and_drag();
        let response = ui.allocate_rect(rect, sense);

        if response.dragged() || response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                if rect.contains(pos) {
                    let x_t = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                    let y_t = 1.0 - ((pos.y - rect.min.y) / rect.height()).clamp(0.0, 1.0);
                    let idx = (x_t * (n - 1) as f32).round() as usize;
                    let value = (y_t * 2.0 - 1.0).clamp(-1.0, 1.0);
                    if idx < points.len() {
                        if (points[idx] - value).abs() > 1e-4 {
                            points[idx] = value;
                            let out = self.bank.frame_mut(self.frame_idx);
                            WavetableBank::upsample_control_points_to_frame(&points, out);
                            frame_edited = true;
                        }
                    }
                }
            }
        }

        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::Grab);
        }

        let painter = ui.painter_at(rect);
        let mid_y = rect.center().y;
        painter.line_segment(
            [Pos2::new(rect.min.x, mid_y), Pos2::new(rect.max.x, mid_y)],
            egui::Stroke::new(0.5, tokens.border),
        );

        let handle_radius = if n > 64 { 3.0 } else { 4.5 };
        let visible = ui.clip_rect().intersect(rect);
        for (i, &val) in points.iter().enumerate() {
            let x_t = i as f32 / (n - 1).max(1) as f32;
            let x = egui::lerp(rect.min.x..=rect.max.x, x_t);
            let y = mid_y - val * rect.height() * 0.42;
            let pt = Pos2::new(x, y);
            if !visible
                .expand2(egui::vec2(handle_radius, handle_radius))
                .contains(pt)
            {
                continue;
            }
            painter.circle_filled(pt, handle_radius, tokens.accent);
            painter.circle_stroke(pt, handle_radius, egui::Stroke::new(1.0, tokens.accent_on));
        }

        painter.text(
            Pos2::new(rect.min.x + 4.0, rect.min.y + 2.0),
            egui::Align2::LEFT_TOP,
            format!("Shape · {n} points"),
            egui::FontId::proportional(9.0),
            tokens.text_muted,
        );

        ShapeEditorResponse { frame_edited }
    }
}
