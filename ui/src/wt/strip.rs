use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Ui, Vec2};
use reelsynth::WavetableBank;
use reelsynth_ui_theme::Tokens;

use crate::layout::WT_STRIP_HEIGHT;

pub struct WtStripResponse {
    pub response: Response,
    pub changed: bool,
}

pub struct WtStrip<'a> {
    pub position: &'a mut f32,
    pub bank: Option<&'a WavetableBank>,
    pub visible_frames: usize,
}

impl<'a> WtStrip<'a> {
    pub fn show(self, ui: &mut Ui) -> WtStripResponse {
        let tokens = Tokens::default();
        let accent_ui = Color32::from_rgb(0x2a, 0x6b, 0x8a);
        let num_frames = self
            .bank
            .map(|b| b.num_frames)
            .unwrap_or(256);
        let frame_count = self.visible_frames.min(num_frames).max(8);

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), WT_STRIP_HEIGHT), Sense::click_and_drag());

        let mut changed = false;
        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let t = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                let new_pos = t * (num_frames.saturating_sub(1)) as f32;
                if (*self.position - new_pos).abs() > 0.01 {
                    *self.position = new_pos;
                    changed = true;
                }
            }
        }

        if ui.is_rect_visible(rect) {
            let painter = ui.painter_at(rect);
            painter.rect_filled(rect, 8.0, tokens.surface2);
            painter.rect_stroke(
                rect,
                8.0,
                egui::Stroke::new(1.0_f32, tokens.border),
            );

            let pad = 4.0;
            let inner = rect.shrink(pad);
            let cell_w = inner.width() / frame_count as f32;
            let norm_pos = if num_frames > 1 {
                *self.position / (num_frames - 1) as f32
            } else {
                0.0
            };
            let active_idx = (norm_pos * (frame_count - 1) as f32).round() as usize;

            for i in 0..frame_count {
                let x = inner.min.x + i as f32 * cell_w;
                let cell = Rect::from_min_size(Pos2::new(x + 0.5, inner.min.y), Vec2::new(cell_w - 1.0, inner.height()));
                let is_active = i == active_idx;
                painter.rect_filled(cell, 4.0, tokens.bg);
                if is_active {
                    painter.rect_stroke(
                        cell,
                        4.0,
                        egui::Stroke::new(1.0_f32, accent_ui),
                    );
                } else {
                    painter.rect_stroke(
                        cell,
                        4.0,
                        egui::Stroke::new(1.0_f32, tokens.border),
                    );
                }

                if let Some(bank) = self.bank {
                    let fi = (i * num_frames / frame_count).min(num_frames - 1);
                    paint_waveform_thumbnail(&painter, cell, bank, fi, is_active, accent_ui, tokens.accent);
                } else {
                    paint_placeholder_wave(&painter, cell, is_active, accent_ui, tokens.accent);
                }
            }

            let playhead_x = inner.min.x + norm_pos * inner.width();
            painter.line_segment(
                [
                    Pos2::new(playhead_x, inner.min.y),
                    Pos2::new(playhead_x, inner.max.y),
                ],
                egui::Stroke::new(2.0, tokens.accent),
            );

            let frame_i = self.position.round() as u32;
            painter.text(
                Pos2::new(rect.min.x + 8.0, rect.min.y + 4.0),
                egui::Align2::LEFT_TOP,
                format!("Position · frame {frame_i} / {}", num_frames - 1),
                egui::FontId::proportional(10.0),
                tokens.text_muted,
            );
        }

        WtStripResponse { response, changed }
    }
}

fn paint_waveform_thumbnail(
    painter: &egui::Painter,
    rect: Rect,
    bank: &WavetableBank,
    frame_idx: usize,
    active: bool,
    accent_ui: Color32,
    accent: Color32,
) {
    let frame = bank.frame(frame_idx);
    let step = (frame.len() / 32).max(1);
    let mut points: Vec<Pos2> = Vec::new();
    let mid_y = rect.center().y;
    let half_h = rect.height() * 0.35;
    for (i, chunk) in frame.iter().step_by(step).take(32).enumerate() {
        let t = i as f32 / 31.0;
        let x = egui::lerp(rect.min.x..=rect.max.x, t);
        let y = mid_y - chunk * half_h;
        points.push(Pos2::new(x, y));
    }
    if points.len() >= 2 {
        let color = if active { accent } else { accent_ui };
        painter.add(Shape::line(points, egui::Stroke::new(if active { 2.0 } else { 1.5 }, color)));
    }
}

fn paint_placeholder_wave(
    painter: &egui::Painter,
    rect: Rect,
    active: bool,
    accent_ui: Color32,
    accent: Color32,
) {
    let mid_y = rect.center().y;
    let w = rect.width();
    let points: Vec<Pos2> = (0..=8)
        .map(|i| {
            let t = i as f32 / 8.0;
            let x = rect.min.x + t * w;
            let y = mid_y + (t * std::f32::consts::TAU * 2.0).sin() * rect.height() * 0.25;
            Pos2::new(x, y)
        })
        .collect();
    let color = if active { accent } else { accent_ui };
    painter.add(Shape::line(points, egui::Stroke::new(1.5, color)));
}
