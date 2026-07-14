use egui::{Color32, Pos2, Rect, Sense, Shape, Ui, Vec2};
use reelsynth::WavetableBank;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::layout::{RADIUS_SM, WT_TOOLBAR_HEIGHT};
use crate::region::region;

use super::toolbar::{WtEditTool, WtToolbar};
use super::waveform::{frame_index, peak_point, waveform_points};

pub struct WtView2dResponse {
    pub frame_edited: bool,
}

pub struct WtView2d<'a> {
    pub position: f32,
    pub bank: Option<&'a mut WavetableBank>,
    pub bank_name: Option<&'a str>,
    pub tool: &'a mut WtEditTool,
    pub animate: bool,
    pub time: f32,
}

impl WtView2d<'_> {
    pub fn show(self, ui: &mut Ui) -> WtView2dResponse {
        let tokens = Tokens::default();
        let accent_ui = ACCENT_UI;
        let view_h = ui.available_height().max(48.0);
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), view_h),
            Sense::hover(),
        );

        if !ui.is_rect_visible(rect) {
            return WtView2dResponse {
                frame_edited: false,
            };
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS_SM, tokens.bg);
        painter.rect_stroke(
            rect,
            RADIUS_SM,
            egui::Stroke::new(1.0_f32, tokens.border),
        );

        let frame_idx = self
            .bank
            .as_ref()
            .map(|b| frame_index(self.position, b.num_frames))
            .unwrap_or(0);

        let mut frame_edited = false;
        let plot_top = rect.min.y + WT_TOOLBAR_HEIGHT;
        let plot_rect = Rect::from_min_max(
            egui::pos2(rect.min.x, plot_top),
            rect.max,
        );

        region(
            ui,
            Rect::from_min_max(rect.min, egui::pos2(rect.max.x, plot_top)),
            |ui| {
                WtToolbar::show(ui, self.tool);
            },
        );

        let inner = plot_rect.shrink2(egui::vec2(8.0, 12.0));
        let mid_y = inner.center().y;

        let wave = if let Some(bank) = self.bank.as_ref() {
            let frame = bank.frame(frame_idx);
            waveform_points(frame, inner, 256, 0.42)
        } else {
            placeholder_wave(inner, mid_y)
        };

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
                egui::Stroke::new(2.0_f32, accent_ui),
            ));

            if let Some(peak) = peak_point(&wave) {
                painter.circle_filled(peak, 4.0, tokens.accent);
                painter.circle_stroke(peak, 4.0, egui::Stroke::new(1.0_f32, tokens.accent_on));
            }
        }

        painter.line_segment(
            [Pos2::new(inner.min.x, mid_y), Pos2::new(inner.max.x, mid_y)],
            egui::Stroke::new(1.0_f32, tokens.border),
        );

        let label = if let Some(name) = self.bank_name {
            format!("2D Waveform · {name} · frame {frame_idx}")
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
            if let Some(bank) = self.bank {
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

        WtView2dResponse { frame_edited }
    }
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
