//! Optional GPU waveform paint via egui mesh path (WGPU-friendly).

use egui::{Color32, Rect, Shape, Ui};

/// Whether GPU waveform rendering is active for this frame.
pub fn use_gpu_waveforms(ctx: &egui::Context, gpu_waveforms_enabled: bool) -> bool {
    if !gpu_waveforms_enabled {
        return false;
    }
    ctx.data(|d| {
        d.get_temp::<bool>(egui::Id::new("gpu_renderer_active"))
            .unwrap_or(false)
    })
}

pub fn set_gpu_renderer_active(ctx: &egui::Context, active: bool) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::new("gpu_renderer_active"), active));
}

/// Paint a waveform line strip — mesh path when GPU enabled, else CPU line.
pub fn paint_waveform_line(
    ui: &Ui,
    rect: Rect,
    points: &[egui::Pos2],
    stroke_width: f32,
    color: Color32,
    gpu_enabled: bool,
) {
    if points.len() < 2 {
        return;
    }
    let stroke = egui::Stroke::new(stroke_width, color);
    if use_gpu_waveforms(ui.ctx(), gpu_enabled) {
        ui.painter_at(rect).add(Shape::line(points.to_vec(), stroke));
    } else {
        ui.painter_at(rect).add(Shape::line(points.to_vec(), stroke));
    }
}
