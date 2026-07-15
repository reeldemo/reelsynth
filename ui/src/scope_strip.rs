//! Four-tap signal-chain scope strip (Osc → Filter → FX → Out).

use egui::{Color32, Pos2, Rect, ScrollArea, Shape, Ui};
use reelsynth::{
    render_osc_cycle_at_index, render_scope_previews, spectrum_magnitudes, Patch, ScopeLiveTaps,
    ScopePreviews, ScopeTap, SCOPE_DISPLAY_LEN, WavetableBank,
};
use reelsynth_ui_theme::Tokens;

use crate::audit_registry::{record_region, record_used, AuditId};
use crate::layout::{GRID_UNIT, RADIUS_SM, SPACE_SM};
use crate::region::region;
use crate::wt::waveform_points;

pub const SCOPE_STRIP_HEIGHT: f32 = 56.0;
const PREVIEW_INTERVAL_SECS: f64 = 1.0 / 30.0;
const SPECTRUM_BARS: usize = 20;
const TRACE_LUMINANCE_FLOOR: f32 = 0.42;
const MIN_CELL_W: f32 = 72.0;
const ARROW_W: f32 = 10.0;

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
    per_osc: Vec<Vec<f32>>,
    last_preview_secs: f64,
    stack_clipping: bool,
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

pub fn draw_scope_strip(ui: &mut Ui, rect: Rect, mut input: ScopeStripInput<'_>) {
    let tokens = Tokens::default();
    let stack_clipping = input.state.stack_clipping;
    let previews = resolve_previews(&mut input);
    let osc_count = input.patch.oscillators.len().max(1);
    let per_osc = input.state.per_osc.clone();
    let inner = rect.shrink(SPACE_SM);
    let cell_h = (inner.height() - 14.0).max(36.0);

    region(ui, inner, |ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Signal chain")
                        .size(10.0)
                        .color(tokens.text_muted),
                );
                if stack_clipping {
                    ui.label(
                        egui::RichText::new("Stack clipping")
                            .size(10.0)
                            .color(Color32::from_rgb(0xde, 0xa0, 0x4a)),
                    );
                }
            });
            ui.add_space(2.0);

            ScrollArea::horizontal()
                .id_salt("scope_strip_scroll")
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = GRID_UNIT * 0.5;
                        if osc_count >= 3 {
                            for oi in 0..osc_count {
                                let tap = ScopeTap {
                                    samples: per_osc
                                        .get(oi)
                                        .cloned()
                                        .unwrap_or_else(|| previews.osc.samples.clone()),
                                };
                                let cell = draw_wave_scope_cell(
                                    ui,
                                    &tap,
                                    &format!("Osc {}", oi + 1),
                                    STAGE_COLORS[0],
                                    MIN_CELL_W,
                                    cell_h,
                                    false,
                                );
                                record_used(ui.ctx(), AuditId::CenterScopeCellOsc, cell);
                                if oi + 1 < osc_count {
                                    draw_arrow(ui, cell_h);
                                }
                            }
                            draw_arrow(ui, cell_h);
                        } else {
                            let osc_cell = draw_wave_scope_cell(
                                ui,
                                &previews.osc,
                                STAGE_LABELS[0],
                                STAGE_COLORS[0],
                                MIN_CELL_W,
                                cell_h,
                                false,
                            );
                            record_used(ui.ctx(), AuditId::CenterScopeCellOsc, osc_cell);
                            draw_arrow(ui, cell_h);
                        }

                        let filt_cell = draw_wave_scope_cell(
                            ui,
                            &previews.filter,
                            STAGE_LABELS[1],
                            STAGE_COLORS[1],
                            MIN_CELL_W,
                            cell_h,
                            false,
                        );
                        record_used(ui.ctx(), AuditId::CenterScopeCellFilter, filt_cell);
                        draw_arrow(ui, cell_h);

                        let fx_cell = draw_wave_scope_cell(
                            ui,
                            &previews.fx,
                            STAGE_LABELS[2],
                            STAGE_COLORS[2],
                            MIN_CELL_W,
                            cell_h,
                            false,
                        );
                        record_used(ui.ctx(), AuditId::CenterScopeCellFx, fx_cell);
                        draw_arrow(ui, cell_h);

                        let out_cell = draw_spectrum_scope_cell(
                            ui,
                            &previews.out,
                            STAGE_LABELS[3],
                            STAGE_COLORS[3],
                            MIN_CELL_W,
                            cell_h,
                            input.state.stack_clipping,
                        );
                        record_used(ui.ctx(), AuditId::CenterScopeCellOut, out_cell);
                    });
                });
        });
        record_region(ui.ctx(), AuditId::CenterScope, inner, ui.min_rect());
    });
}

fn resolve_previews(input: &mut ScopeStripInput<'_>) -> ScopePreviews {
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
            &|oi| (input.bank_for_osc)(oi),
            input.patch,
            SCOPE_DISPLAY_LEN,
        );
        input.state.per_osc = (0..input.patch.oscillators.len())
            .map(|oi| {
                render_osc_cycle_at_index(
                    input.banks,
                    |idx| (input.bank_for_osc)(idx),
                    input.patch,
                    oi,
                    SCOPE_DISPLAY_LEN,
                )
            })
            .collect();
        input.state.stack_clipping = detect_stack_clipping(input.patch);
        input.state.last_preview_secs = input.now_secs;
    }
    input.state.cached.clone()
}

fn detect_stack_clipping(patch: &Patch) -> bool {
    use crate::oscillator_ui::WaveLayerUi;
    use crate::wt::composite_stack_sample;
    use reelsynth::WavetableBank;

    let bank = WavetableBank::factory_saw_morph();
    patch.oscillators.iter().any(|osc| {
        if !osc.stack_mode.eq_ignore_ascii_case("add") || osc.wave_layers.is_empty() {
            return false;
        }
        let layers: Vec<WaveLayerUi> = osc.wave_layers.iter().map(WaveLayerUi::from_patch).collect();
        let mut peak = 0.0f32;
        for i in 0..64 {
            let phase = i as f32 / 64.0;
            let v = composite_stack_sample(&layers, &bank, "add", phase, 0.0).abs();
            peak = peak.max(v);
        }
        peak > 1.0
    })
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
    clip_warn: bool,
) -> Rect {
    let tokens = Tokens::default();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return rect;
    }

    let border = if clip_warn {
        Color32::from_rgb(0xde, 0xa0, 0x4a)
    } else {
        tokens.border
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, RADIUS_SM, tokens.surface2);
    painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0_f32, border));

    painter.text(
        egui::pos2(rect.min.x + 6.0, rect.min.y + 3.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::monospace(9.0),
        accent,
    );

    let wave_rect = rect.shrink2(egui::vec2(6.0, 14.0));
    let points = waveform_points(
        &tap.samples,
        wave_rect,
        SCOPE_DISPLAY_LEN.min(tap.samples.len().max(2)),
        0.42,
    );
    if points.len() >= 2 {
        painter.add(Shape::line(
            points,
            egui::Stroke::new(1.25_f32, trace_color(accent)),
        ));
        let mid = wave_rect.center().y;
        painter.line_segment(
            [Pos2::new(wave_rect.min.x, mid), Pos2::new(wave_rect.max.x, mid)],
            egui::Stroke::new(0.5_f32, tokens.border),
        );
    }
    rect
}

fn draw_spectrum_scope_cell(
    ui: &mut Ui,
    tap: &ScopeTap,
    label: &str,
    accent: Color32,
    width: f32,
    height: f32,
    clip_warn: bool,
) -> Rect {
    let tokens = Tokens::default();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return rect;
    }

    let border = if clip_warn {
        Color32::from_rgb(0xde, 0xa0, 0x4a)
    } else {
        tokens.border
    };
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, RADIUS_SM, tokens.surface2);
    painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0_f32, border));

    painter.text(
        egui::pos2(rect.min.x + 6.0, rect.min.y + 3.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::monospace(9.0),
        accent,
    );

    let bars_rect = rect.shrink2(egui::vec2(6.0, 14.0));
    let magnitudes = spectrum_magnitudes(&tap.samples, SPECTRUM_BARS);
    if magnitudes.is_empty() {
        return rect;
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
        painter.rect_filled(bar, 1.0, trace_color(accent.gamma_multiply(0.55 + mag * 0.45)));
    }
    rect
}

fn trace_color(color: Color32) -> Color32 {
    let [r, g, b, a] = color.to_array();
    let lum =
        (0.2126 * r as f32 + 0.7152 * g as f32 + 0.0722 * b as f32) / 255.0 * (a as f32 / 255.0);
    if lum >= TRACE_LUMINANCE_FLOOR {
        return color;
    }
    let scale = TRACE_LUMINANCE_FLOOR / lum.max(0.01);
    Color32::from_rgba_unmultiplied(
        (r as f32 * scale).min(255.0) as u8,
        (g as f32 * scale).min(255.0) as u8,
        (b as f32 * scale).min(255.0) as u8,
        a,
    )
}

fn draw_arrow(ui: &mut Ui, height: f32) {
    let tokens = Tokens::default();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ARROW_W, height), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let c = rect.center();
        ui.painter_at(rect).line_segment(
            [
                Pos2::new(rect.min.x + 1.0, c.y),
                Pos2::new(rect.max.x - 1.0, c.y),
            ],
            egui::Stroke::new(1.0_f32, tokens.text_muted),
        );
    }
}

/// Scale layer levels proportionally so signed Add peak ≤ 1.0.
pub fn autofix_stack_levels(layers: &mut [crate::oscillator_ui::WaveLayerUi]) {
    let mut peak = 0.0f32;
    for i in 0..64 {
        let phase = i as f32 / 64.0;
        let _ = phase;
        let mut sum = 0.0f32;
        for layer in layers.iter() {
            if !layer.enabled || layer.level <= 0.0 {
                continue;
            }
            let sign = if layer.invert { -1.0 } else { 1.0 };
            sum += sign * layer.level;
        }
        peak = peak.max(sum.abs());
    }
    if peak <= 1.0 || peak <= f32::EPSILON {
        return;
    }
    let scale = 1.0 / peak;
    for layer in layers.iter_mut() {
        if layer.enabled {
            layer.level *= scale;
        }
    }
}
