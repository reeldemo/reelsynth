//! Subtle animated backgrounds for center panels.

use egui::{Color32, Painter, Pos2, Rect, Shape};
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

/// Slow drifting wave lines behind mod/FX panels.
pub fn paint_ambient_waves(painter: &Painter, rect: Rect, time: f64) {
    if !painter.clip_rect().intersects(rect) {
        return;
    }
    let tokens = Tokens::default();
    let t = time as f32;

    // Soft vignette
    painter.rect_filled(rect, 0.0, tokens.bg.gamma_multiply(0.0));

    let mid_y = rect.center().y;
    for layer in 0..3 {
        let phase = t * (0.35 + layer as f32 * 0.12) + layer as f32 * 1.7;
        let amp = rect.height() * (0.08 + layer as f32 * 0.04);
        let alpha = 0.12 - layer as f32 * 0.03;
        let color = ACCENT_UI.gamma_multiply(alpha);
        let steps = 64;
        let points: Vec<Pos2> = (0..=steps)
            .map(|i| {
                let u = i as f32 / steps as f32;
                let x = egui::lerp(rect.min.x..=rect.max.x, u);
                let y = mid_y
                    + (u * std::f32::consts::TAU * 2.5 + phase).sin() * amp
                    + (u * std::f32::consts::TAU * 5.0 + phase * 1.3).sin() * amp * 0.35;
                Pos2::new(x, y)
            })
            .collect();
        if points.len() >= 2 {
            painter.add(Shape::line(
                points,
                egui::Stroke::new(1.0_f32 + layer as f32 * 0.5, color),
            ));
        }
    }

    // Shimmer scan line
    let scan_x = rect.min.x + (t * 40.0).sin().abs() * rect.width();
    painter.line_segment(
        [
            Pos2::new(scan_x, rect.min.y),
            Pos2::new(scan_x + rect.width() * 0.08, rect.max.y),
        ],
        egui::Stroke::new(1.0_f32, tokens.accent.gamma_multiply(0.06)),
    );
}

/// Animated fill under a waveform plot.
pub fn animated_wave_points(
    inner: Rect,
    mid_y: f32,
    time: f32,
    position: f32,
    steps: usize,
) -> Vec<Pos2> {
    let morph = position / 255.0;
    (0..=steps)
        .map(|i| {
            let u = i as f32 / steps as f32;
            let x = egui::lerp(inner.min.x..=inner.max.x, u);
            let base = (u * std::f32::consts::TAU * 2.0 + time * 1.2).sin();
            let harm = (u * std::f32::consts::TAU * 5.0 + time * 0.7 + morph * 3.0).sin() * 0.35;
            let y = mid_y + (base + harm) * inner.height() * 0.32;
            Pos2::new(x, y)
        })
        .collect()
}

/// Static accent at waveform peak (no idle pulse).
pub fn peak_glow_color(accent: Color32, _time: f32) -> Color32 {
    accent
}
