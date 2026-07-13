use egui::{Color32, Pos2, Rect, Sense, Shape, Ui, Vec2};
use reelsynth::WavetableBank;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::layout::{RADIUS_SM, WT_VIEW_MIN_HEIGHT};

use super::waveform::{frame_index, waveform_points};

pub struct WtView3d<'a> {
    pub position: f32,
    pub bank: Option<&'a WavetableBank>,
    pub time: f32,
}

impl WtView3d<'_> {
    pub fn show(self, ui: &mut Ui) -> Rect {
        let tokens = Tokens::default();
        let accent_ui = ACCENT_UI;
        let view_h = ui.available_height().max(WT_VIEW_MIN_HEIGHT * 0.5);
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), view_h),
            Sense::hover(),
        );

        if !ui.is_rect_visible(rect) {
            return rect;
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS_SM, tokens.bg);
        painter.rect_stroke(
            rect,
            RADIUS_SM,
            egui::Stroke::new(1.0_f32, tokens.border),
        );

        painter.text(
            Pos2::new(rect.min.x + 8.0, rect.min.y + 6.0),
            egui::Align2::LEFT_TOP,
            "3D Mesh",
            egui::FontId::proportional(10.0),
            tokens.text_muted,
        );

        let inner = rect.shrink2(egui::vec2(8.0, 20.0));
        paint_grid(&painter, inner, tokens.border);

        if let Some(bank) = self.bank {
            paint_mesh_from_bank(
                &painter,
                inner,
                bank,
                self.position,
                self.time,
                accent_ui,
                tokens.accent,
            );
        } else {
            paint_placeholder_mesh(&painter, inner, self.time, accent_ui);
        }

        rect
    }
}

fn paint_grid(painter: &egui::Painter, rect: Rect, border: Color32) {
    let step = 24.0;
    let mut x = rect.min.x;
    while x <= rect.max.x {
        painter.line_segment(
            [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
            egui::Stroke::new(0.5_f32, border.gamma_multiply(0.5)),
        );
        x += step;
    }
    let mut y = rect.min.y;
    while y <= rect.max.y {
        painter.line_segment(
            [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
            egui::Stroke::new(0.5_f32, border.gamma_multiply(0.5)),
        );
        y += step;
    }
}

fn paint_mesh_from_bank(
    painter: &egui::Painter,
    rect: Rect,
    bank: &WavetableBank,
    position: f32,
    time: f32,
    accent_ui: Color32,
    accent: Color32,
) {
    let num_slices = 16usize;
    let center_frame = frame_index(position, bank.num_frames);
    let half = num_slices / 2;
    let drift = (time * 0.15).sin() * 2.0;
    let center_frame = ((center_frame as f32 + drift).round() as i32)
        .clamp(0, bank.num_frames.saturating_sub(1) as i32) as usize;
    let mesh_left = rect.min.x + rect.width() * 0.08;
    let mesh_width = rect.width() * 0.84;
    let depth_pitch = rect.width() * 0.028;

    let mut slice_polylines: Vec<Vec<Pos2>> = Vec::with_capacity(num_slices);

    for s in 0..num_slices {
        let fi = (center_frame as i32 + s as i32 - half as i32)
            .clamp(0, bank.num_frames.saturating_sub(1) as i32) as usize;
        let depth = (s as f32 / num_slices as f32 - 0.5).abs();
        let z_offset = (s as f32 - half as f32) * depth_pitch;
        let y_offset = depth * rect.height() * 0.22;

        let slice_rect = Rect::from_min_max(
            Pos2::new(mesh_left + z_offset, rect.min.y + y_offset),
            Pos2::new(mesh_left + z_offset + mesh_width, rect.max.y - y_offset),
        );

        let frame = bank.frame(fi);
        let points = waveform_points(frame, slice_rect, 64, 0.30);
        slice_polylines.push(points);
    }

    // Vertical mesh ribs between adjacent slices.
    let rib_count = 12usize;
    for rib in 0..=rib_count {
        let t = rib as f32 / rib_count as f32;
        for window in slice_polylines.windows(2) {
            if let [a, b] = window {
                if a.is_empty() || b.is_empty() {
                    continue;
                }
                let ia = ((a.len() - 1) as f32 * t).round() as usize;
                let ib = ((b.len() - 1) as f32 * t).round() as usize;
                let pa = a[ia.min(a.len() - 1)];
                let pb = b[ib.min(b.len() - 1)];
                painter.line_segment(
                    [pa, pb],
                    egui::Stroke::new(0.75_f32, accent_ui.gamma_multiply(0.25)),
                );
            }
        }
    }

    for (s, points) in slice_polylines.iter().enumerate() {
        if points.len() < 2 {
            continue;
        }
        let depth = (s as f32 / num_slices as f32 - 0.5).abs();
        let alpha = (1.0 - depth * 1.5).clamp(0.2, 1.0);
        let is_active = s == half;
        let color = if is_active {
            accent
        } else {
            accent_ui.gamma_multiply(alpha)
        };
        let width_stroke = if is_active { 2.0_f32 } else { 1.0_f32 };
        painter.add(Shape::line(
            points.clone(),
            egui::Stroke::new(width_stroke, color),
        ));
    }
}

fn paint_placeholder_mesh(painter: &egui::Painter, rect: Rect, time: f32, accent_ui: Color32) {
    for i in 0..10 {
        let t = i as f32 / 9.0;
        let y_off = t * rect.height() * 0.32;
        let x_off = (t - 0.5) * rect.width() * 0.22 + (time * 0.2 + t).sin() * 4.0;
        let points: Vec<Pos2> = (0..=40)
            .map(|j| {
                let u = j as f32 / 40.0;
                let x = rect.min.x + x_off + u * rect.width() * 0.78;
                let y = rect.center().y + y_off
                    + (u * std::f32::consts::TAU * 2.0 + t * 2.0).sin() * rect.height() * 0.18;
                Pos2::new(x, y)
            })
            .collect();
        painter.add(Shape::line(
            points,
            egui::Stroke::new(1.0_f32, accent_ui.gamma_multiply(0.35 + t * 0.45)),
        ));
    }
}
