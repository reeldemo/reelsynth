//! ADSR envelope graph — matches `.rs-adsr-graph` (80px tall).

use egui::{FontId, Pos2, Shape, Ui};
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

pub const ADSR_GRAPH_HEIGHT: f32 = 64.0;

/// Draw the amp envelope shape from normalized segment lengths.
pub fn adsr_graph(
    ui: &mut Ui,
    attack: f32,
    decay: f32,
    sustain: f32,
    _release: f32,
    scale: f32,
) -> egui::Response {
    let tokens = Tokens::default();
    let accent_ui = ACCENT_UI;
    let height = ADSR_GRAPH_HEIGHT * scale;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, tokens.surface2);
        painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0_f32, tokens.border));

        let inner = rect.shrink(8.0);
        let a = attack.max(0.001);
        let d = decay.max(0.001);
        let s = sustain.clamp(0.0, 1.0);
        let r = 0.25_f32;
        let total = a + d + r;
        let ax = inner.min.x + inner.width() * (a / total);
        let dx = inner.min.x + inner.width() * ((a + d) / total);
        let rx = inner.max.x - inner.width() * (r / total);
        let top = inner.min.y + 8.0;
        let bottom = inner.max.y - 4.0;
        let sustain_y = bottom - (bottom - top) * s;

        let points = vec![
            Pos2::new(inner.min.x, bottom),
            Pos2::new(ax, top),
            Pos2::new(dx, sustain_y),
            Pos2::new(rx, sustain_y),
            Pos2::new(inner.max.x, bottom),
        ];
        painter.add(Shape::line(
            points,
            egui::Stroke::new(2.0_f32, accent_ui),
        ));
        for p in [Pos2::new(ax, top), Pos2::new(dx, sustain_y)] {
            painter.circle_filled(p, 3.0, tokens.accent);
            painter.circle_stroke(p, 3.0, egui::Stroke::new(1.0_f32, tokens.accent_on));
        }
    }

    response
}

pub fn format_env_time(seconds: f32) -> String {
    let ms = seconds * 1000.0;
    if ms < 1000.0 {
        format!("{:.0} ms", ms.max(1.0))
    } else {
        format!("{:.2} s", seconds)
    }
}

pub fn format_sustain(level: f32) -> String {
    format!("{:.0}%", level.clamp(0.0, 1.0) * 100.0)
}

pub fn format_lfo_rate(hz: f32) -> String {
    format!("{:.1} Hz", hz.max(0.0))
}

pub fn format_depth(depth: f32) -> String {
    format!("{:.0}%", depth.clamp(0.0, 1.0) * 100.0)
}

pub fn format_pan(pan: f32) -> String {
    if pan.abs() < 0.05 {
        "C".into()
    } else if pan < 0.0 {
        format!("L{:.0}", (-pan * 100.0).round())
    } else {
        format!("R{:.0}", (pan * 100.0).round())
    }
}

pub fn format_coarse(cents: f32) -> String {
    format!("{:.0} st", cents / 100.0)
}

pub fn format_unison(count: u32) -> String {
    if count <= 1 {
        "1 voice".into()
    } else {
        format!("{count} voices")
    }
}

pub fn knob_value_label(ui: &mut Ui, text: &str) {
    let tokens = Tokens::default();
    ui.label(
        egui::RichText::new(text)
            .font(FontId::monospace(11.0))
            .color(tokens.text),
    );
}
