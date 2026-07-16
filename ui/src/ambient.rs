//! Ambient panel helpers (animations removed — kept as static stubs for call sites).

use egui::{Color32, Painter, Pos2, Rect};

/// No-op — animated backgrounds disabled.
pub fn paint_ambient_waves(_painter: &Painter, _rect: Rect, _time: f64) {}

/// Static waveform placeholder (no time-based motion).
pub fn animated_wave_points(
    inner: Rect,
    mid_y: f32,
    _time: f32,
    position: f32,
    steps: usize,
) -> Vec<Pos2> {
    let morph = position / 255.0;
    (0..=steps)
        .map(|i| {
            let u = i as f32 / steps as f32;
            let x = egui::lerp(inner.min.x..=inner.max.x, u);
            let base = (u * std::f32::consts::TAU * 2.0).sin();
            let harm = (u * std::f32::consts::TAU * 5.0 + morph * 3.0).sin() * 0.35;
            let y = mid_y + (base + harm) * inner.height() * 0.32;
            Pos2::new(x, y)
        })
        .collect()
}

/// Static accent at waveform peak.
pub fn peak_glow_color(accent: Color32, _time: f32) -> Color32 {
    accent
}
