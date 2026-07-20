//! Left oscillator column (S3) — scrollable osc cards + per-osc params.

use egui::{Color32, Pos2, Shape, Ui};
use reelsynth::{
    render_combined_osc_cycle, render_osc_cycle_at_index, Patch,
};
use reelsynth_ui_theme::Tokens;

use crate::audit_registry::{record_region, AuditId};
use crate::layout::{CENTER_GAP, GRID_UNIT, SPACE_SM};
use crate::oscillator_ui::{OscillatorUi, MIN_OSCILLATORS};
use crate::state::OscStripContext;
use crate::widgets::{
    format_coarse, format_pan, format_unison, knob_value_label, labeled_select, Knob, KnobSize,
    KnobStyle, panel_audit,
};
use crate::wt::{sync_slot_from_position, wave_quant_from_index, wave_quant_index, effective_quant_count, WAVE_QUANT_LABELS};
use crate::wt::waveform_points;

const OSC_TYPES: [&str; 5] = ["Wavetable", "Saw", "Square", "Triangle", "Pulse"];
const WARP_MODES: [&str; 3] = ["None", "Sync", "Bend"];
const FM_ALGORITHMS: [&str; 4] = ["Off", "2→1", "3→1", "2+3→1"];
const FM_SOURCES: [&str; 5] = ["None", "Osc 2", "Osc 3", "2+3→1", "Feedback"];
const STACK_LAYER_TYPES: [&str; 6] = ["Saw", "Sine", "Square", "Triangle", "Pulse", "Wavetable"];
pub(crate) const STACK_MODES: [&str; 3] = ["Add", "Avg", "Avg Equal"];

pub fn stack_mode_index(mode: &str) -> usize {
    match mode.to_ascii_lowercase().as_str() {
        "avg" | "average" => 1,
        "avg_equal" | "avgequal" | "avg equal" => 2,
        _ => 0,
    }
}

pub fn stack_mode_from_index(idx: usize) -> &'static str {
    match idx {
        1 => "avg",
        2 => "avg_equal",
        _ => "add",
    }
}

pub fn stack_mode_tooltip(mode: &str) -> &'static str {
    match mode.to_ascii_lowercase().as_str() {
        "avg" | "average" => "Level-weighted mean (sign-sensitive)",
        "avg_equal" | "avgequal" | "avg equal" => "Each layer counts equally (1/N)",
        _ => "Signed sum of all layers",
    }
}

/// User-facing stack mode label (never raw patch tokens like `none`).
pub fn stack_mode_label(mode: &str) -> &'static str {
    STACK_MODES[stack_mode_index(mode)]
}

pub fn stack_layer_type_index(ty: &str) -> usize {
    match ty.to_ascii_lowercase().as_str() {
        "sine" => 1,
        "square" => 2,
        "triangle" | "tri" => 3,
        "pulse" => 4,
        "wavetable" => 5,
        _ => 0,
    }
}

pub fn stack_layer_type_from_index(idx: usize) -> &'static str {
    match idx {
        1 => "sine",
        2 => "square",
        3 => "triangle",
        4 => "pulse",
        5 => "wavetable",
        _ => "saw",
    }
}

const OSC_CARD_WIDTH: f32 = 72.0;
const OSC_CARD_HEIGHT: f32 = 48.0;
const OSC_PREVIEW_SAMPLES: usize = 64;
const OSC_PREVIEW_INTERVAL_SECS: f64 = 1.0 / 20.0;

pub fn osc_type_index(ty: &str) -> usize {
    match ty.to_ascii_lowercase().as_str() {
        "saw" => 1,
        "square" => 2,
        "triangle" => 3,
        "pulse" => 4,
        _ => 0,
    }
}

pub fn osc_type_from_index(idx: usize) -> &'static str {
    match idx {
        1 => "saw",
        2 => "square",
        3 => "triangle",
        4 => "pulse",
        _ => "wavetable",
    }
}

pub fn warp_mode_index(mode: &str) -> usize {
    match mode.to_ascii_lowercase().as_str() {
        "sync" => 1,
        "bend" => 2,
        _ => 0,
    }
}

pub fn warp_mode_from_index(idx: usize) -> &'static str {
    match idx {
        1 => "sync",
        2 => "bend",
        _ => "none",
    }
}

pub fn fm_source_index(source: &str) -> usize {
    match source.to_ascii_lowercase().as_str() {
        "osc2" => 1,
        "osc3" => 2,
        "osc2_osc3" | "osc2+osc3" => 3,
        "feedback" => 4,
        _ => 0,
    }
}

pub fn fm_source_from_index(idx: usize) -> &'static str {
    match idx {
        1 => "osc2",
        2 => "osc3",
        3 => "osc2_osc3",
        4 => "feedback",
        _ => "none",
    }
}

pub fn fm_algorithm_index(source: &str) -> usize {
    match source.to_ascii_lowercase().as_str() {
        "osc2" => 1,
        "osc3" => 2,
        "osc2_osc3" | "osc2+osc3" => 3,
        _ => 0,
    }
}

pub fn fm_source_from_algorithm(idx: usize) -> &'static str {
    match idx {
        1 => "osc2",
        2 => "osc3",
        3 => "osc2_osc3",
        _ => "none",
    }
}

pub struct OscColumnState<'a> {
    pub oscillators: &'a mut Vec<OscillatorUi>,
    pub osc_tab: &'a mut usize,
    pub unison_stereo_spread: &'a mut f32,
    pub sub_level: &'a mut f32,
    pub noise_level: &'a mut f32,
    pub macro_values: &'a mut [f32; 4],
    pub selected_layer_idx: &'a mut Option<usize>,
}

pub struct OscColumnInput<'a> {
    pub patch: &'a Patch,
    pub preview: Option<OscStripContext<'a>>,
}

pub struct OscColumnResult {
    pub changed: bool,
    pub osc_count_changed: bool,
}

pub fn draw_osc_column(
    ui: &mut Ui,
    state: OscColumnState<'_>,
    input: OscColumnInput<'_>,
    scale: f32,
) -> OscColumnResult {
    let mut changed = false;
    let mut osc_count_changed = false;
    let gap = CENTER_GAP * scale;
    let section_gap = SPACE_SM * scale;
    let min_section_h = 72.0 * scale;
    let card_w = OSC_CARD_WIDTH * scale;
    let card_h = OSC_CARD_HEIGHT * scale;

    egui::Frame::none()
        .inner_margin(egui::Margin::same(SPACE_SM * scale))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            // Match right-rail section rhythm (panel_audit gaps), not the tighter half-gap.
            ui.spacing_mut().item_spacing.y = section_gap.max(gap);

            panel_audit(ui, "Oscillators", Some(AuditId::OscPanelOscillators), |ui| {
                let previews = resolve_osc_previews(input.patch, input.preview);
                let strip_start = ui.cursor().min;
                let strip_result = draw_osc_scroll_strip(
                    ui,
                    state.oscillators,
                    state.osc_tab,
                    &previews,
                    card_w,
                    card_h,
                    scale,
                );
                if strip_result.selection_changed {
                    changed = true;
                }
                if strip_result.added {
                    state.oscillators.push(OscillatorUi::new_silent());
                    *state.osc_tab = state.oscillators.len().saturating_sub(1);
                    changed = true;
                    osc_count_changed = true;
                }
                if let Some(remove_idx) = strip_result.removed {
                    if state.oscillators.len() > MIN_OSCILLATORS && remove_idx < state.oscillators.len()
                    {
                        state.oscillators.remove(remove_idx);
                        *state.osc_tab = (*state.osc_tab)
                            .min(state.oscillators.len().saturating_sub(1));
                        changed = true;
                        osc_count_changed = true;
                    }
                }

                record_row(ui.ctx(), AuditId::OscStripCards, ui, strip_start);

                ui.add_space(GRID_UNIT * 0.25);

                let idx = (*state.osc_tab).min(state.oscillators.len().saturating_sub(1));
                let osc = &mut state.oscillators[idx];

                let type_start = ui.cursor().min;
                if labeled_select(ui, "Type", &OSC_TYPES, &mut osc.osc_type) {
                    changed = true;
                }
                record_row(ui.ctx(), AuditId::OscTypeSelect, ui, type_start);

                let knobs_start = ui.cursor().min;
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = SPACE_SM;
                    let level_text = format!("{:.2}", osc.level);
                    let r1 = Knob::new(&mut osc.level, 0.0..=1.0, "Level")
                        .size(KnobSize::Sm)
                        .scale(scale)
                        .style(KnobStyle::Wired)
                        .show_wired_badge(false)
                        .value_text(level_text)
                        .show(ui);
                    let pan_text = format_pan(osc.pan);
                    let r2 = Knob::new(&mut osc.pan, -1.0..=1.0, "Pan")
                        .size(KnobSize::Sm)
                        .scale(scale)
                        .style(KnobStyle::Normal)
                        .value_text(pan_text)
                        .show(ui);
                    let coarse_text = format_coarse(osc.coarse);
                    let r3 = Knob::new(&mut osc.coarse, -2400.0..=2400.0, "Coarse")
                        .size(KnobSize::Sm)
                        .scale(scale)
                        .style(KnobStyle::Wired)
                        .show_wired_badge(false)
                        .value_text(coarse_text)
                        .show(ui);
                    if r1.changed || r2.changed || r3.changed {
                        changed = true;
                    }
                });
                record_row(ui.ctx(), AuditId::OscKnobsLevelPanCoarse, ui, knobs_start);

                ui.add_space(GRID_UNIT * 0.2);
                let is_wt = osc.osc_type == 0;
                if is_wt {
                    let quant_start = ui.cursor().min;
                    let mut quant_idx = wave_quant_index(osc.wave_quant);
                    if labeled_select(ui, "WT Quant", &WAVE_QUANT_LABELS, &mut quant_idx) {
                        let new_quant = wave_quant_from_index(quant_idx);
                        osc.wave_quant = new_quant;
                        if new_quant == 255 {
                            osc.wave_slots = reelsynth::generate_even_wave_slots(256, 256);
                            osc.wave_slot = osc.wave_slot.min(254);
                        } else if new_quant == 0 {
                            // Smooth mode — keep continuous position.
                        } else {
                            osc.wave_slot = osc.wave_slot.min(
                                effective_quant_count(new_quant).saturating_sub(1) as u8,
                            );
                            osc.wave_slot_fine = 0.0;
                            sync_slot_from_position(osc, 256);
                        }
                        osc.ensure_layer_segment_interps();
                        changed = true;
                    }
                    record_row(ui.ctx(), AuditId::OscWtQuant, ui, quant_start);

                    let pos_start = ui.cursor().min;
                    let pos_label = if osc.wave_quant > 0 {
                        format!("slot {} · {:.0}", osc.wave_slot + 1, osc.position.round())
                    } else {
                        format!("{:.0} / 255", osc.position.round())
                    };
                    if param_slider(
                        ui,
                        "WT Position",
                        &mut osc.position,
                        0.0..=255.0,
                        &pos_label,
                    ) {
                        if osc.wave_quant > 0 {
                            sync_slot_from_position(osc, 256);
                        }
                        changed = true;
                    }
                    record_row(ui.ctx(), AuditId::OscWtPositionSlider, ui, pos_start);

                    let warp_start = ui.cursor().min;
                    let warp_idx = &mut osc.warp_mode;
                    if labeled_select(ui, "Warp", &WARP_MODES, warp_idx) {
                        changed = true;
                    }
                    record_row(ui.ctx(), AuditId::OscWarpSelect, ui, warp_start);
                    let warp_amt_start = ui.cursor().min;
                    let warp_label_pct = osc.warp_amount * 100.0;
                    if param_slider(
                        ui,
                        "Warp Amt",
                        &mut osc.warp_amount,
                        0.0..=1.0,
                        &format!("{:.0}%", warp_label_pct),
                    ) {
                        changed = true;
                    }
                    record_row(ui.ctx(), AuditId::OscWarpAmtSlider, ui, warp_amt_start);
                }

                let is_pulse = matches!(osc.osc_type, 2 | 4);
                if is_pulse {
                    let pw_start = ui.cursor().min;
                    let pw_pct = osc.pulse_width * 100.0;
                    if param_slider(
                        ui,
                        "Pulse W",
                        &mut osc.pulse_width,
                        0.05..=0.95,
                        &format!("{:.0}%", pw_pct),
                    ) {
                        changed = true;
                    }
                    record_row(ui.ctx(), AuditId::OscPulseWidth, ui, pw_start);
                }

                let unison_start = ui.cursor().min;
                let unison_f = &mut (osc.unison as f32);
                let unison_label = format_unison(osc.unison);
                if param_slider(ui, "Unison", unison_f, 1.0..=8.0, &unison_label) {
                    osc.unison = unison_f.round().clamp(1.0, 8.0) as u32;
                    changed = true;
                }
                record_row(ui.ctx(), AuditId::OscUnisonSlider, ui, unison_start);

                let spread_start = ui.cursor().min;
                if param_slider(
                    ui,
                    "Spread",
                    state.unison_stereo_spread,
                    0.0..=1.0,
                    &format!("{:.0}%", *state.unison_stereo_spread * 100.0),
                ) {
                    changed = true;
                }
                record_row(ui.ctx(), AuditId::OscSpreadSlider, ui, spread_start);

                ui.add_space(GRID_UNIT * 0.5);
                if ui.available_height() > min_section_h * 1.8 {
                    panel_audit(ui, "Stack", Some(AuditId::OscPanelStack), |ui| {
                        let mut stack_mode_idx = stack_mode_index(&osc.stack_mode);
                        let mode_resp = labeled_select(ui, "Mode", &STACK_MODES, &mut stack_mode_idx);
                        if mode_resp {
                            osc.stack_mode = stack_mode_from_index(stack_mode_idx).into();
                            changed = true;
                        }
                        if ui.is_enabled() {
                            ui.label(
                                egui::RichText::new(stack_mode_tooltip(&osc.stack_mode))
                                    .size(10.0)
                                    .color(Tokens::default().text_muted),
                            );
                        }

                        if ui.button("Autofix levels").on_hover_text("Normalize layer levels when Add mode clips").clicked() {
                            crate::scope_strip::autofix_stack_levels(&mut osc.wave_layers);
                            changed = true;
                        }

                        ui.label(
                            egui::RichText::new(format!(
                                "{} layers · edit on center strip",
                                osc.wave_layers.len()
                            ))
                            .size(10.0)
                            .color(Tokens::default().text_muted),
                        );

                        egui::CollapsingHeader::new("Advanced layer params")
                            .default_open(false)
                            .show(ui, |ui| {
                                let selected = state.selected_layer_idx.unwrap_or(0);
                                if let Some(layer) = osc.wave_layers.get_mut(selected) {
                                    ui.label(format!("Layer {}", selected + 1));
                                    let mut type_idx = stack_layer_type_index(&layer.source_type);
                                    if labeled_select(ui, "Type", &STACK_LAYER_TYPES, &mut type_idx) {
                                        layer.source_type =
                                            stack_layer_type_from_index(type_idx).into();
                                        changed = true;
                                    }
                                    ui.horizontal(|ui| {
                                        let det_text = format!("{:.0}", layer.detune);
                                        let r = Knob::new(&mut layer.detune, -2400.0..=2400.0, "Det")
                                            .size(KnobSize::Sm)
                                            .scale(scale)
                                            .value_text(det_text)
                                            .show(ui);
                                        if r.changed {
                                            changed = true;
                                        }
                                        if layer.source_type.eq_ignore_ascii_case("pulse") {
                                            let pw_text = format!("{:.0}%", layer.pulse_width * 100.0);
                                            let r2 = Knob::new(&mut layer.pulse_width, 0.01..=0.99, "PW")
                                                .size(KnobSize::Sm)
                                                .scale(scale)
                                                .value_text(pw_text)
                                                .show(ui);
                                            if r2.changed {
                                                changed = true;
                                            }
                                        }
                                    });
                                    if layer.source_type.eq_ignore_ascii_case("wavetable") {
                                        let wt_label = format!("{:.0}", layer.wt_position);
                                        if param_slider(
                                            ui,
                                            "WT Pos",
                                            &mut layer.wt_position,
                                            0.0..=255.0,
                                            &wt_label,
                                        ) {
                                            changed = true;
                                        }
                                    }
                                }
                            });
                    });
                }

                if ui.available_height() > min_section_h * 1.8 {
                    panel_audit(ui, "FM", Some(AuditId::OscPanelFm), |ui| {
                        let algo_start = ui.cursor().min;
                        let algo_idx = &mut osc.fm_algorithm;
                        if labeled_select(ui, "Algo", &FM_ALGORITHMS, algo_idx) {
                            if *algo_idx == 0 {
                                osc.fm_source = 0;
                            } else {
                                osc.fm_source = *algo_idx;
                            }
                            changed = true;
                        }
                        record_row(ui.ctx(), AuditId::OscFmAlgorithm, ui, algo_start);

                        let src_idx = &mut osc.fm_source;
                        if labeled_select(ui, "Source", &FM_SOURCES, src_idx) {
                            osc.fm_algorithm =
                                fm_algorithm_index(fm_source_from_index(*src_idx));
                            changed = true;
                        }

                        let fm_knobs_start = ui.cursor().min;
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            let ratio_text = format!("{:.2}", osc.fm_ratio);
                            let r1 = Knob::new(&mut osc.fm_ratio, 0.5..=16.0, "Ratio")
                                .size(KnobSize::Sm)
                                .scale(scale)
                                .style(KnobStyle::Wired)
                                .value_text(ratio_text)
                                .show(ui);
                            let index_text = format!("{:.1}", osc.fm_index);
                            let r2 = Knob::new(&mut osc.fm_index, 0.0..=10.0, "Index")
                                .size(KnobSize::Sm)
                                .scale(scale)
                                .style(KnobStyle::Normal)
                                .value_text(index_text)
                                .show(ui);
                            if r1.changed || r2.changed {
                                changed = true;
                            }
                        });
                        record_row(ui.ctx(), AuditId::OscFmKnobs, ui, fm_knobs_start);
                    });
                }

                if ui.available_height() > min_section_h * 1.1 {
                    ui.add_space(GRID_UNIT * 0.35);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.x = SPACE_SM * 0.75;
                        let sub_text = format!("{:.2}", *state.sub_level);
                        let r1 = Knob::new(state.sub_level, 0.0..=1.0, "Sub")
                            .size(KnobSize::Sm)
                            .scale(scale)
                            .style(KnobStyle::Wired)
                            .show_wired_badge(false)
                            .value_text(sub_text)
                            .show(ui);
                        let noise_text = format!("{:.2}", *state.noise_level);
                        let r2 = Knob::new(state.noise_level, 0.0..=1.0, "Noise")
                            .size(KnobSize::Sm)
                            .scale(scale)
                            .style(KnobStyle::Wired)
                            .show_wired_badge(false)
                            .value_text(noise_text)
                            .show(ui);
                        for (i, label) in ["M1", "M2", "M3", "M4"].iter().enumerate() {
                            let text = format!("{:.0}%", state.macro_values[i] * 100.0);
                            let r = Knob::new(&mut state.macro_values[i], 0.0..=1.0, label)
                                .size(KnobSize::Sm)
                                .scale(scale)
                                .style(KnobStyle::Wired)
                                .show_wired_badge(false)
                                .value_text(text)
                                .show(ui);
                            if r.changed {
                                changed = true;
                            }
                        }
                        if r1.changed || r2.changed {
                            changed = true;
                        }
                    });
                }
            });

        });

    let col_used = ui.min_rect();
    record_region(ui.ctx(), AuditId::OscColumn, col_used, col_used);

    OscColumnResult {
        changed,
        osc_count_changed,
    }
}

struct OscPreviews {
    per_osc: Vec<Vec<f32>>,
    combined: Vec<f32>,
}

struct OscScrollStripResult {
    selection_changed: bool,
    added: bool,
    removed: Option<usize>,
}

fn resolve_osc_previews(patch: &Patch, preview: Option<OscStripContext<'_>>) -> OscPreviews {
    if let Some(ctx) = preview {
        let count = patch.oscillators.len();
        let stale = ctx.state.osc_count != count
            || ctx.now_secs - ctx.state.last_preview_secs >= OSC_PREVIEW_INTERVAL_SECS
            || ctx.state.per_osc.len() != count;

        if stale && !ctx.banks.is_empty() {
            ctx.state.per_osc = (0..count)
                .map(|i| {
                    render_osc_cycle_at_index(
                        ctx.banks,
                        ctx.bank_for_osc,
                        patch,
                        i,
                        OSC_PREVIEW_SAMPLES,
                    )
                })
                .collect();
            ctx.state.combined = render_combined_osc_cycle(
                ctx.banks,
                ctx.bank_for_osc,
                patch,
                OSC_PREVIEW_SAMPLES,
            );
            ctx.state.last_preview_secs = ctx.now_secs;
            ctx.state.osc_count = count;
        }

        return OscPreviews {
            per_osc: ctx.state.per_osc.clone(),
            combined: ctx.state.combined.clone(),
        };
    }

    OscPreviews {
        per_osc: vec![Vec::new(); patch.oscillators.len()],
        combined: Vec::new(),
    }
}

fn draw_osc_scroll_strip(
    ui: &mut Ui,
    oscillators: &[OscillatorUi],
    selected: &mut usize,
    previews: &OscPreviews,
    card_w: f32,
    card_h: f32,
    scale: f32,
) -> OscScrollStripResult {
    let tokens = Tokens::default();
    let mut result = OscScrollStripResult {
        selection_changed: false,
        added: false,
        removed: None,
    };

    ui.add_space(2.0 * scale);

    egui::ScrollArea::horizontal()
        .id_salt("osc_scroll_strip")
        .max_height(card_h + 4.0 * scale)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GRID_UNIT * 0.75;

                draw_osc_preview_card(
                    ui,
                    "Mix",
                    &previews.combined,
                    Color32::from_rgb(0x4a, 0xde, 0x80),
                    card_w,
                    card_h,
                    false,
                    false,
                    scale,
                );

                for (i, osc) in oscillators.iter().enumerate() {
                    let wave = previews.per_osc.get(i).map(Vec::as_slice).unwrap_or(&[]);
                    let label = format!("Osc {}", i + 1);
                    let accent = if *selected == i {
                        tokens.accent_on
                    } else if osc.level > 0.0 {
                        Color32::from_rgb(0x5b, 0xc0, 0xde)
                    } else {
                        tokens.text_muted
                    };
                    let (card_resp, remove_clicked) = draw_osc_preview_card_with_remove(
                        ui,
                        &label,
                        wave,
                        accent,
                        card_w,
                        card_h,
                        *selected == i,
                        oscillators.len() > MIN_OSCILLATORS,
                        scale,
                    );
                    if card_resp.clicked() && *selected != i {
                        *selected = i;
                        result.selection_changed = true;
                    }
                    if remove_clicked {
                        result.removed = Some(i);
                    }
                }

                let add_size = egui::vec2(32.0 * scale, card_h);
                let (add_rect, add_resp) =
                    ui.allocate_exact_size(add_size, egui::Sense::click());
                if ui.is_rect_visible(add_rect) {
                    let painter = ui.painter_at(add_rect);
                    painter.rect_filled(add_rect, 6.0, tokens.surface2);
                    painter.rect_stroke(add_rect, 6.0, egui::Stroke::new(1.0, tokens.border));
                    painter.text(
                        add_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        "+",
                        egui::FontId::proportional(18.0 * scale),
                        tokens.accent,
                    );
                }
                if add_resp.clicked() {
                    result.added = true;
                }
            });
        });

    result
}

fn draw_osc_preview_card(
    ui: &mut Ui,
    label: &str,
    samples: &[f32],
    accent: Color32,
    width: f32,
    height: f32,
    selected: bool,
    show_remove: bool,
    scale: f32,
) -> egui::Response {
    let (resp, _) = draw_osc_preview_card_with_remove(
        ui,
        label,
        samples,
        accent,
        width,
        height,
        selected,
        show_remove,
        scale,
    );
    resp
}

fn draw_osc_preview_card_with_remove(
    ui: &mut Ui,
    label: &str,
    samples: &[f32],
    accent: Color32,
    width: f32,
    height: f32,
    selected: bool,
    show_remove: bool,
    scale: f32,
) -> (egui::Response, bool) {
    let tokens = Tokens::default();
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());
    let mut remove_clicked = false;

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let bg = if selected {
            tokens.surface2
        } else {
            tokens.bg_muted
        };
        painter.rect_filled(rect, 6.0, bg);
        let stroke = if selected {
            egui::Stroke::new(1.5, tokens.accent)
        } else {
            egui::Stroke::new(1.0, tokens.border)
        };
        painter.rect_stroke(rect, 6.0, stroke);

        painter.text(
            egui::pos2(rect.min.x + 6.0, rect.min.y + 3.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::monospace(9.0),
            accent,
        );

        let wave_rect = rect.shrink2(egui::vec2(6.0, 14.0));
        let points = waveform_points(
            samples,
            wave_rect,
            OSC_PREVIEW_SAMPLES.min(samples.len().max(2)),
            0.38,
        );
        if points.len() >= 2 {
            painter.add(Shape::line(
                points,
                egui::Stroke::new(1.1, accent.gamma_multiply(0.9)),
            ));
            let mid = wave_rect.center().y;
            painter.line_segment(
                [
                    Pos2::new(wave_rect.min.x, mid),
                    Pos2::new(wave_rect.max.x, mid),
                ],
                egui::Stroke::new(0.5, tokens.border),
            );
        }

        if show_remove {
            let btn = egui::Rect::from_min_size(
                egui::pos2(rect.max.x - 16.0 * scale, rect.min.y + 2.0),
                egui::vec2(14.0 * scale, 14.0 * scale),
            );
            let btn_id = ui.id().with(label).with("remove");
            let btn_resp = ui.interact(btn, btn_id, egui::Sense::click());
            if btn_resp.clicked() {
                remove_clicked = true;
            }
            painter.text(
                btn.center(),
                egui::Align2::CENTER_CENTER,
                "×",
                egui::FontId::monospace(10.0),
                if btn_resp.hovered() {
                    tokens.accent_on
                } else {
                    tokens.text_muted
                },
            );
        }
    }

    (resp, remove_clicked)
}

fn row_rect(ui: &Ui, start: egui::Pos2) -> egui::Rect {
    let end_y = ui.cursor().min.y;
    egui::Rect::from_min_max(start, egui::pos2(ui.max_rect().max.x, end_y.max(start.y)))
}

fn record_row(ctx: &egui::Context, id: AuditId, ui: &Ui, start: egui::Pos2) {
    let rect = row_rect(ui, start);
    if rect.is_positive() {
        record_region(ctx, id, rect, rect);
    }
}

fn param_slider(
    ui: &mut Ui,
    label: &str,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    value_label: &str,
) -> bool {
    let tokens = Tokens::default();
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.allocate_ui_with_layout(
            egui::vec2(72.0, 18.0),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
                ui.label(
                    egui::RichText::new(label)
                        .size(10.0)
                        .color(tokens.text_muted),
                );
            },
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            knob_value_label(ui, value_label);
            let slider_w = ui.available_width().max(48.0);
            let norm = ((*value - *range.start()) / (*range.end() - *range.start())).clamp(0.0, 1.0);
            let (rect, resp) = ui.allocate_exact_size(
                egui::vec2(slider_w, 14.0),
                egui::Sense::click_and_drag(),
            );
            if resp.dragged() || resp.clicked() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    let t = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                    *value = *range.start() + t * (*range.end() - *range.start());
                    changed = true;
                }
            }
            if ui.is_rect_visible(rect) {
                let painter = ui.painter_at(rect);
                let track = rect.shrink2(egui::vec2(0.0, 4.0));
                painter.rect_filled(track, 3.0, tokens.surface2);
                let fill_w = track.width() * norm;
                let fill = egui::Rect::from_min_size(track.min, egui::vec2(fill_w, track.height()));
                painter.rect_filled(fill, 3.0, tokens.accent);
                let thumb_x = track.min.x + track.width() * norm;
                painter.circle_filled(
                    egui::pos2(thumb_x, track.center().y),
                    5.0,
                    tokens.accent_on,
                );
            }
        });
    });
    changed
}

#[cfg(test)]
mod bridge_tests {
    use super::*;

    #[test]
    fn osc_type_cycle_roundtrip() {
        for ty in ["wavetable", "saw", "square", "triangle", "pulse"] {
            let idx = osc_type_index(ty);
            assert_eq!(osc_type_from_index(idx), ty);
        }
    }

    #[test]
    fn warp_mode_roundtrip() {
        for mode in ["none", "sync", "bend"] {
            let idx = warp_mode_index(mode);
            assert_eq!(warp_mode_from_index(idx), mode);
        }
    }

    #[test]
    fn fm_algorithm_roundtrip() {
        for src in ["none", "osc2", "osc3", "osc2_osc3"] {
            let idx = fm_algorithm_index(src);
            assert_eq!(fm_source_from_algorithm(idx), src);
        }
    }

    #[test]
    fn fm_source_roundtrip() {
        for src in ["none", "osc2", "osc3", "osc2_osc3", "feedback"] {
            let idx = fm_source_index(src);
            assert_eq!(fm_source_from_index(idx), src);
        }
    }
}

#[cfg(test)]
mod osc_count_tests {
    use crate::OscillatorUi;

    #[test]
    fn unlimited_add_oscillator() {
        let mut oscs = vec![OscillatorUi::new_active()];
        for _ in 0..10 {
            oscs.push(OscillatorUi::new_silent());
        }
        assert_eq!(oscs.len(), 11);
    }
}
