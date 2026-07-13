//! Four-tap signal-chain scope strip (Osc → Filter → FX → Out).

use egui::{Color32, Pos2, Rect, Shape, Ui};
use reelsynth::{
    render_scope_previews, spectrum_magnitudes, Patch, ScopeLiveTaps, ScopePreviews,
    ScopeTap, SCOPE_DISPLAY_LEN, WavetableBank,
};
use reelsynth_ui_theme::Tokens;

use crate::layout::{GRID_UNIT, RADIUS_SM, SPACE_SM};
use crate::region::region;
use crate::wt::waveform_points;

pub const SCOPE_STRIP_HEIGHT: f32 = 72.0;
const PREVIEW_INTERVAL_SECS: f64 = 1.0 / 30.0;
const SPECTRUM_BARS: usize = 20;

const STAGE_LABELS: [&str; 4] = ["Osc", "Filter", "FX", "Out"];
const STAGE_COLORS: [Color32; 4] = [
    Color32::from_rgb(0x5b, 0xc0, 0xde),
    Color32::from_rgb(0x9b, 0x7e, 0xde),
    Color32::from_rgb(0xde, 0x9b, 0x7e),
    Color32::from_rgb(0x4a, 0xde, 0x80),
];

/// Cached analytical previews + throttle clock for idle mode.
#[derive(Clone, Debug, Default)]
pub struct ScopeStripState {
    cached: ScopePreviews,
    last_preview_secs: f64,
}

pub struct ScopeStripInput<'a> {
    pub patch: &'a Patch,
    pub banks: &'a [WavetableBank],
    pub bank_for_osc: &'a dyn Fn(usize) -> usize,
    pub live: Option<&'a ScopeLiveTaps>,
    pub is_playing: bool,
    pub now_secs: f64,
    pub state: &'a mut ScopeStripState,
}

pub fn draw_scope_strip(ui: &mut Ui, rect: Rect, input: ScopeStripInput<'_>) {
    let tokens = Tokens::default();
    let previews = resolve_previews(input);
    let inner = rect.shrink(SPACE_SM);

    region(ui, inner, |ui| {
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new("Signal chain")
                    .size(10.0)
                    .color(tokens.text_muted),
            );
            ui.add_space(4.0);

            let gap = GRID_UNIT;
            let cell_w = ((inner.width() - gap * 3.0) / 4.0).max(40.0);
            let cell_h = (inner.height() - 18.0).max(48.0);

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = gap;
                draw_wave_scope_cell(
                    ui,
                    &previews.osc,
                    STAGE_LABELS[0],
                    STAGE_COLORS[0],
                    cell_w,
                    cell_h,
                );
                draw_arrow(ui, cell_h);
                draw_wave_scope_cell(
                    ui,
                    &previews.filter,
                    STAGE_LABELS[1],
                    STAGE_COLORS[1],
                    cell_w,
                    cell_h,
                );
                draw_arrow(ui, cell_h);
                draw_wave_scope_cell(
                    ui,
                    &previews.fx,
                    STAGE_LABELS[2],
                    STAGE_COLORS[2],
                    cell_w,
                    cell_h,
                );
                draw_arrow(ui, cell_h);
                draw_spectrum_scope_cell(
                    ui,
                    &previews.out,
                    STAGE_LABELS[3],
                    STAGE_COLORS[3],
                    cell_w,
                    cell_h,
                );
            });
        });
    });
}

fn resolve_previews(input: ScopeStripInput<'_>) -> ScopePreviews {
    if input.is_playing {
        if let Some(live) = input.live {
            if live.playing {
                return live_to_previews(live);
            }
        }
    }

    if input.now_secs - input.state.last_preview_secs >= PREVIEW_INTERVAL_SECS
        || input.state.cached.osc.samples.is_empty()
    {
        input.state.cached = render_scope_previews(
            input.banks,
            input.bank_for_osc,
            input.patch,
            SCOPE_DISPLAY_LEN,
        );
        input.state.last_preview_secs = input.now_secs;
    }
    input.state.cached.clone()
}

fn live_to_previews(live: &ScopeLiveTaps) -> ScopePreviews {
    ScopePreviews {
        osc: ScopeTap {
            samples: live.osc.snapshot(SCOPE_DISPLAY_LEN),
        },
        filter: ScopeTap {
            samples: live.filter.snapshot(SCOPE_DISPLAY_LEN),
        },
        fx: ScopeTap {
            samples: live.fx.snapshot(SCOPE_DISPLAY_LEN),
        },
        out: ScopeTap {
            samples: live.out.snapshot(SCOPE_DISPLAY_LEN),
        },
    }
}

fn draw_wave_scope_cell(
    ui: &mut Ui,
    tap: &ScopeTap,
    label: &str,
    accent: Color32,
    width: f32,
    height: f32,
) {
    let tokens = Tokens::default();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, RADIUS_SM, tokens.surface2);
    painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0_f32, tokens.border));

    painter.text(
        egui::pos2(rect.min.x + 6.0, rect.min.y + 4.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::monospace(9.0),
        accent,
    );

    let wave_rect = rect.shrink2(egui::vec2(6.0, 16.0));
    let points = waveform_points(
        &tap.samples,
        wave_rect,
        SCOPE_DISPLAY_LEN.min(tap.samples.len().max(2)),
        0.42,
    );
    if points.len() >= 2 {
        painter.add(Shape::line(
            points,
            egui::Stroke::new(1.25_f32, accent.gamma_multiply(0.9)),
        ));
        let mid = wave_rect.center().y;
        painter.line_segment(
            [Pos2::new(wave_rect.min.x, mid), Pos2::new(wave_rect.max.x, mid)],
            egui::Stroke::new(0.5_f32, tokens.border),
        );
    }
}

fn draw_spectrum_scope_cell(
    ui: &mut Ui,
    tap: &ScopeTap,
    label: &str,
    accent: Color32,
    width: f32,
    height: f32,
) {
    let tokens = Tokens::default();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, RADIUS_SM, tokens.surface2);
    painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0_f32, tokens.border));

    painter.text(
        egui::pos2(rect.min.x + 6.0, rect.min.y + 4.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::monospace(9.0),
        accent,
    );

    let bars_rect = rect.shrink2(egui::vec2(6.0, 16.0));
    let magnitudes = spectrum_magnitudes(&tap.samples, SPECTRUM_BARS);
    if magnitudes.is_empty() {
        return;
    }

    let bar_gap = 1.5;
    let bar_w = ((bars_rect.width() - bar_gap * (SPECTRUM_BARS as f32 - 1.0)) / SPECTRUM_BARS as f32)
        .max(1.0);
    for (i, &mag) in magnitudes.iter().enumerate() {
        let x = bars_rect.min.x + i as f32 * (bar_w + bar_gap);
        let bar_h = bars_rect.height() * mag.clamp(0.04, 1.0);
        let bar = egui::Rect::from_min_max(
            egui::pos2(x, bars_rect.max.y - bar_h),
            egui::pos2(x + bar_w, bars_rect.max.y),
        );
        painter.rect_filled(bar, 1.0, accent.gamma_multiply(0.55 + mag * 0.45));
    }
}

fn draw_arrow(ui: &mut Ui, height: f32) {
    let tokens = Tokens::default();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(12.0, height), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let c = rect.center();
        ui.painter_at(rect).line_segment(
            [
                Pos2::new(rect.min.x + 2.0, c.y),
                Pos2::new(rect.max.x - 2.0, c.y),
            ],
            egui::Stroke::new(1.0_f32, tokens.text_muted),
        );
    }
}
