//! Modulation matrix section (S4) — matches `.rs-mod-grid` in mockups.

use egui::{Color32, FontId, Rect, Ui};
use reelsynth::ModSlot;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::audit_registry::{record_region, record_used, AuditId};
use crate::layout::{UiScale, GRID_UNIT, RADIUS_SM, sidebar_panel_chrome_height};
use crate::region::region;
use crate::widgets::{
    button_ghost, button_toggle, card_stroke, collapsible_panel,
    sidebar_panel_audit,
};

const POLARITY_POSITIVE: Color32 = Color32::from_rgb(0x4a, 0xde, 0x80);
const POLARITY_NEGATIVE: Color32 = Color32::from_rgb(0xf8, 0x71, 0x71);

pub const MOD_ROW_HEIGHT: f32 = 22.0;
#[allow(dead_code)] // reserved for sidebar chrome height parity with FX
pub const MOD_SECTION_HEADER: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModPolarity {
    Positive,
    Negative,
    Bipolar,
}

#[derive(Debug, Clone)]
pub struct ModSlotUi {
    pub source: &'static str,
    pub target: &'static str,
    pub amount: i32,
    pub curve: &'static str,
    pub enabled: bool,
    pub polarity: ModPolarity,
}

impl Default for ModSlotUi {
    fn default() -> Self {
        Self {
            source: "LFO 1",
            target: "WT Pos",
            amount: 0,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Bipolar,
        }
    }
}

pub fn default_mod_slots() -> Vec<ModSlotUi> {
    vec![
        ModSlotUi {
            source: "LFO 1",
            target: "WT Pos",
            amount: 15,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Bipolar,
        },
        ModSlotUi {
            source: "Env 2",
            target: "Cutoff",
            amount: 25,
            curve: "Exp",
            enabled: true,
            polarity: ModPolarity::Positive,
        },
        ModSlotUi {
            source: "Velo",
            target: "Level",
            amount: 35,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Positive,
        },
    ]
}

pub struct ModMatrixState<'a> {
    pub open: &'a mut bool,
    pub routes: &'a mut [ModSlotUi],
    pub total_routes: usize,
}

pub struct ModMatrixResult {
    pub changed: bool,
}

pub fn draw_mod_matrix(
    ui: &mut Ui,
    rect: Rect,
    state: ModMatrixState<'_>,
    scale: UiScale,
) -> ModMatrixResult {
    draw_mod_matrix_inner(ui, rect, state, scale, ModChrome::Collapsible)
}

/// Left-column native panel (matches Filter / Envelope chrome).
pub fn draw_mod_matrix_sidebar(
    ui: &mut Ui,
    rect: Rect,
    state: ModMatrixState<'_>,
    scale: UiScale,
) -> ModMatrixResult {
    draw_mod_matrix_inner(ui, rect, state, scale, ModChrome::NativePanel)
}

#[derive(Clone, Copy)]
enum ModChrome {
    Collapsible,
    NativePanel,
}

fn draw_mod_matrix_inner(
    ui: &mut Ui,
    rect: Rect,
    state: ModMatrixState<'_>,
    scale: UiScale,
    chrome: ModChrome,
) -> ModMatrixResult {
    let mut changed = false;
    let s = scale.ui();
    let row_h = MOD_ROW_HEIGHT * s;
    let chrome_h = sidebar_panel_chrome_height(s, true);
    let body_h = (rect.height() - chrome_h).max(0.0);
    let row_gap = 2.0 * s;
    let max_rows = ((body_h - GRID_UNIT * s) / (row_h + row_gap))
        .floor()
        .max(1.0) as usize;

    let ModMatrixState {
        open,
        routes,
        total_routes,
    } = state;

    region(ui, rect, |ui| {
        ui.set_clip_rect(rect);
        ui.set_min_height(rect.height());
        ui.set_max_height(rect.height());
        let active = routes.iter().filter(|r| r.enabled).count();
        let meta = format!("{active} / {total_routes} routes");
        let body = |ui: &mut Ui| {
            ui.set_max_height(body_h);
            egui::ScrollArea::vertical()
                .id_salt("mod_matrix_sidebar_scroll")
                .max_height(body_h)
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = row_gap;
                    for (row_idx, route) in routes.iter_mut().take(max_rows).enumerate() {
                        if draw_mod_row(ui, route, row_h, row_idx).changed {
                            changed = true;
                        }
                    }
                });
        };

        match chrome {
            ModChrome::Collapsible => {
                collapsible_panel(ui, "Modulation Matrix", &meta, open, |ui| {
                    ui.spacing_mut().item_spacing.y = row_gap;
                    for (row_idx, route) in routes.iter_mut().take(max_rows).enumerate() {
                        if draw_mod_row(ui, route, row_h, row_idx).changed {
                            changed = true;
                        }
                    }
                });
            }
            ModChrome::NativePanel => {
                sidebar_panel_audit(
                    ui,
                    "Modulation Matrix",
                    &meta,
                    Some(AuditId::OscModPanel),
                    body,
                );
            }
        }
        record_region(ui.ctx(), AuditId::OscModPanel, rect, ui.min_rect().intersect(rect));
    });

    ModMatrixResult { changed }
}

/// Map UI rows to engine [`ModSlot`] entries (S6).
pub fn mod_slots_to_patch(routes: &[ModSlotUi]) -> Vec<ModSlot> {
    routes
        .iter()
        .filter_map(|route| {
            let source = ui_source_to_engine(route.source)?;
            let target = ui_target_to_engine(route.target)?;
            let amount = route.amount as f32 / 100.0;
            Some(ModSlot {
                source,
                target,
                amount,
                enabled: route.enabled,
            })
        })
        .collect()
}

/// Hydrate UI rows from patch mod matrix; pads with defaults when sparse.
pub fn mod_slots_from_patch(slots: &[ModSlot]) -> Vec<ModSlotUi> {
    if slots.is_empty() {
        return default_mod_slots();
    }
    slots
        .iter()
        .map(|slot| ModSlotUi {
            source: engine_source_to_ui(&slot.source),
            target: engine_target_to_ui(&slot.target),
            amount: (slot.amount * 100.0).round() as i32,
            curve: "Lin",
            enabled: slot.enabled,
            polarity: polarity_from_amount(slot.amount),
        })
        .collect()
}

fn polarity_from_amount(amount: f32) -> ModPolarity {
    if amount < 0.0 {
        ModPolarity::Negative
    } else if amount > 0.0 {
        ModPolarity::Positive
    } else {
        ModPolarity::Bipolar
    }
}

fn ui_source_to_engine(label: &str) -> Option<String> {
    Some(
        match label {
            "LFO 1" => "lfo1",
            "LFO 2" => "lfo2",
            "Env 2" => "filt_env",
            "Env 1" => "env",
            "Velo" => "velocity",
            "ModWh" => "modwheel",
            "After" => "aftertouch",
            "Step" => "step",
            "Rand" => "rand",
            "M1" => "macro1",
            "M2" => "macro2",
            "M3" => "macro3",
            "M4" => "macro4",
            other => other,
        }
        .into(),
    )
}

fn engine_source_to_ui(source: &str) -> &'static str {
    match source {
        "lfo1" | "lfo" => "LFO 1",
        "lfo2" => "LFO 2",
        "filt_env" | "env2" => "Env 2",
        "env1" | "env" => "Env 1",
        "velocity" | "vel" => "Velo",
        "modwheel" => "ModWh",
        "aftertouch" => "After",
        "step" => "Step",
        "rand" => "Rand",
        "macro1" | "m1" => "M1",
        "macro2" | "m2" => "M2",
        "macro3" | "m3" => "M3",
        "macro4" | "m4" => "M4",
        _ => "LFO 1",
    }
}

pub const AUTOMATION_TARGET_LABELS: &[&str] = &["Cutoff", "WT Pos"];

/// Map UI automation target label to engine mod-matrix id.
pub fn automation_target_to_engine(label: &str) -> String {
    ui_target_to_engine(label).unwrap_or_else(|| "filter_cutoff".into())
}

fn ui_target_to_engine(label: &str) -> Option<String> {
    Some(
        match label {
            "WT Pos" => "osc1_position",
            "WT Fine" => "osc1_position",
            "WT Slot" => "osc1_wave_slot",
            "Cutoff" => "filter_cutoff",
            "Res" => "filter_resonance",
            "FM Idx" => "osc1_fm_index",
            "Pitch" | "Detune" => "osc1_detune",
            "Level" => "osc1_level",
            "Pan" => "osc1_pan",
            other => other,
        }
        .into(),
    )
}

fn engine_target_to_ui(target: &str) -> &'static str {
    match target {
        t if t.ends_with("_wave_slot") => "WT Slot",
        t if t.ends_with("_position") => "WT Pos",
        "filter_cutoff" => "Cutoff",
        "filter_resonance" => "Res",
        t if t.ends_with("_fm_index") => "FM Idx",
        t if t.ends_with("_detune") => "Detune",
        t if t.ends_with("_level") => "Level",
        t if t.ends_with("_pan") => "Pan",
        _ => "WT Pos",
    }
}

#[cfg(test)]
mod bridge_tests {
    use super::*;

    #[test]
    fn wt_fine_target_maps() {
        let routes = vec![ModSlotUi {
            source: "LFO 2",
            target: "WT Fine",
            amount: 8,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Bipolar,
        }];
        let slots = mod_slots_to_patch(&routes);
        assert_eq!(slots[0].target, "osc1_position");
    }

    #[test]
    fn wt_slot_target_maps() {
        let routes = vec![ModSlotUi {
            source: "LFO 1",
            target: "WT Slot",
            amount: 10,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Bipolar,
        }];
        let slots = mod_slots_to_patch(&routes);
        assert_eq!(slots[0].target, "osc1_wave_slot");
        let restored = mod_slots_from_patch(&slots);
        assert_eq!(restored[0].target, "WT Slot");
    }

    #[test]
    fn env2_maps_to_filt_env() {
        let routes = vec![ModSlotUi {
            source: "Env 2",
            target: "Cutoff",
            amount: 25,
            curve: "Exp",
            enabled: true,
            polarity: ModPolarity::Positive,
        }];
        let slots = mod_slots_to_patch(&routes);
        assert_eq!(slots.len(), 1);
        assert_eq!(slots[0].source, "filt_env");
        assert_eq!(slots[0].target, "filter_cutoff");
    }

    #[test]
    fn round_trip_mod_route() {
        let routes = default_mod_slots();
        let slots = mod_slots_to_patch(&routes);
        assert!(!slots.is_empty());
        let restored = mod_slots_from_patch(&slots);
        assert_eq!(restored.len(), routes.len());
        assert_eq!(restored[0].source, routes[0].source);
        assert_eq!(restored[0].target, routes[0].target);
    }

    #[test]
    fn disabled_routes_persist() {
        let mut routes = default_mod_slots();
        routes[0].enabled = false;
        let slots = mod_slots_to_patch(&routes);
        assert!(!slots[0].enabled);
    }
}

struct ModRowResult {
    changed: bool,
}

fn draw_mod_row(ui: &mut Ui, route: &mut ModSlotUi, row_h: f32, row_idx: usize) -> ModRowResult {
    ui.push_id(("mod_row", row_idx), |ui| draw_mod_row_inner(ui, route, row_h, row_idx))
        .inner
}

fn draw_mod_row_inner(ui: &mut Ui, route: &mut ModSlotUi, row_h: f32, row_idx: usize) -> ModRowResult {
    let tokens = Tokens::default();
    let mut changed = false;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let active = route.enabled;
        let stroke = card_stroke(active, response.hovered(), &tokens);
        let fill = if active {
            tokens.bg
        } else {
            tokens.surface2.gamma_multiply(0.68)
        };
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS_SM, fill);
        painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0_f32, stroke));
        if response.hovered() && active {
            painter.rect_filled(rect, RADIUS_SM, tokens.accent.gamma_multiply(0.12));
        }

        let label_color = if active {
            tokens.text
        } else {
            tokens.text_secondary
        };
        let target_color = if active {
            tokens.text_secondary
        } else {
            tokens.text_muted
        };

        region(ui, rect.shrink2(egui::vec2(GRID_UNIT, 2.0)), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                let source = ui.label(
                    egui::RichText::new(route.source)
                        .font(FontId::monospace(10.0))
                        .color(label_color),
                );
                record_used(
                    ui.ctx(),
                    AuditId::OscModSourceSelect(row_idx),
                    source.rect,
                );
                ui.label(
                    egui::RichText::new("→")
                        .size(10.0)
                        .color(tokens.text_secondary),
                );
                let target = ui.label(
                    egui::RichText::new(route.target)
                        .size(11.0)
                        .color(target_color),
                );
                record_used(
                    ui.ctx(),
                    AuditId::OscModTargetSelect(row_idx),
                    target.rect,
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let on_label = if route.enabled { "On" } else { "Off" };
                    if button_toggle(ui, on_label, route.enabled).clicked() {
                        route.enabled = !route.enabled;
                        changed = true;
                    }
                    if button_ghost(ui, route.curve).clicked() {
                        route.curve = match route.curve {
                            "Lin" => "Exp",
                            "Exp" => "Step",
                            _ => "Lin",
                        };
                        changed = true;
                    }
                    let (amount_text, amount_fill, amount_stroke) =
                        polarity_amount_style(route.polarity, &tokens);
                    let amount_frame = egui::Frame {
                        fill: amount_fill,
                        stroke: egui::Stroke::new(1.0_f32, amount_stroke),
                        rounding: egui::Rounding::same(4.0),
                        inner_margin: egui::Margin::symmetric(4.0, 2.0),
                        ..Default::default()
                    }
                    .show(ui, |ui| {
                        let mut drag = egui::DragValue::new(&mut route.amount)
                            .speed(0.5)
                            .range(-100..=100);
                        match route.polarity {
                            ModPolarity::Bipolar => {}
                            ModPolarity::Positive => {
                                drag = drag.prefix("+");
                            }
                            ModPolarity::Negative => {
                                drag = drag.prefix("−");
                            }
                        }
                        let changed_amount = {
                            let visuals = ui.visuals_mut();
                            visuals.override_text_color = Some(amount_text);
                            ui.add(drag).changed()
                        };
                        if changed_amount {
                            if route.polarity != ModPolarity::Bipolar {
                                route.polarity = if route.amount < 0 {
                                    ModPolarity::Negative
                                } else {
                                    ModPolarity::Positive
                                };
                            }
                            changed = true;
                        }
                    });
                    let amount_rect = amount_frame.response.rect;
                    if amount_rect.is_positive() {
                        record_region(
                            ui.ctx(),
                            AuditId::OscModAmountDrag(row_idx),
                            amount_rect,
                            amount_rect,
                        );
                    }
                });
            });
        });
    }

    record_region(ui.ctx(), AuditId::OscModRow(row_idx), rect, rect);

    ModRowResult { changed }
}

fn polarity_amount_style(polarity: ModPolarity, tokens: &Tokens) -> (Color32, Color32, Color32) {
    match polarity {
        ModPolarity::Positive => (
            POLARITY_POSITIVE,
            tokens.surface2,
            POLARITY_POSITIVE.gamma_multiply(0.4),
        ),
        ModPolarity::Negative => (
            POLARITY_NEGATIVE,
            tokens.surface2,
            POLARITY_NEGATIVE.gamma_multiply(0.4),
        ),
        ModPolarity::Bipolar => (tokens.accent_on, tokens.accent_muted, ACCENT_UI),
    }
}
