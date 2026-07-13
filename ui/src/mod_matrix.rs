//! Modulation matrix section (S4) — matches `.rs-mod-grid` in mockups.

use egui::{FontId, Rect, Ui};
use reelsynth::ModSlot;
use reelsynth_ui_theme::{heading_font, Tokens};

use crate::layout::{UiScale, GRID_UNIT, SPACE_SM};
use crate::region::region;

pub const MOD_ROW_HEIGHT: f32 = 22.0;
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
            amount: 32,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Bipolar,
        },
        ModSlotUi {
            source: "Env 2",
            target: "Cutoff",
            amount: 68,
            curve: "Exp",
            enabled: true,
            polarity: ModPolarity::Positive,
        },
        ModSlotUi {
            source: "Velo",
            target: "Level",
            amount: 45,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Positive,
        },
        ModSlotUi {
            source: "ModWh",
            target: "Res",
            amount: -18,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Negative,
        },
        ModSlotUi {
            source: "After",
            target: "Pitch",
            amount: 12,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Bipolar,
        },
        ModSlotUi {
            source: "LFO 2",
            target: "Pan",
            amount: 40,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Bipolar,
        },
        ModSlotUi {
            source: "Step",
            target: "WT Pos",
            amount: 100,
            curve: "Step",
            enabled: true,
            polarity: ModPolarity::Positive,
        },
        ModSlotUi {
            source: "Rand",
            target: "Detune",
            amount: 8,
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
    let tokens = Tokens::default();
    let mut changed = false;
    let s = scale.ui();
    let header_h = MOD_SECTION_HEADER * s;
    let row_h = MOD_ROW_HEIGHT * s;
    let body_h = (rect.height() - header_h).max(0.0);
    let row_gap = 2.0 * s;
    let max_rows = ((body_h - GRID_UNIT * s) / (row_h + row_gap))
        .floor()
        .max(1.0) as usize;

    region(ui, rect, |ui| {
        egui::Frame::none()
            .fill(tokens.bg_muted)
            .stroke(egui::Stroke::new(1.0_f32, tokens.border))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let active = state.routes.iter().filter(|r| r.enabled).count();
                let meta = format!("{active} / {} routes", state.total_routes);
                let header = section_header(ui, "Modulation Matrix", &meta, *state.open, header_h);
                if header.clicked() {
                    *state.open = !*state.open;
                }

                if *state.open {
                    egui::Frame::none()
                        .inner_margin(egui::Margin::symmetric(SPACE_SM * s, GRID_UNIT * s))
                        .show(ui, |ui| {
                            ui.spacing_mut().item_spacing.y = row_gap;
                            for route in state.routes.iter_mut().take(max_rows) {
                                if draw_mod_row(ui, route, row_h).changed {
                                    changed = true;
                                }
                            }
                        });
                }
            });
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
            "Env 2" | "Env 1" => "env",
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
        "env1" | "env" => "Env 2",
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

fn ui_target_to_engine(label: &str) -> Option<String> {
    Some(
        match label {
            "WT Pos" => "osc1_position",
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

fn section_header(ui: &mut Ui, title: &str, meta: &str, open: bool, height: f32) -> egui::Response {
    let tokens = Tokens::default();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), height), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, tokens.surface2);
        painter.line_segment(
            [rect.left_bottom(), rect.right_bottom()],
            egui::Stroke::new(1.0_f32, tokens.border),
        );
        let chevron = if open { "▼" } else { "▶" };
        painter.text(
            egui::pos2(rect.min.x + SPACE_SM, rect.center().y),
            egui::Align2::LEFT_CENTER,
            chevron,
            FontId::proportional(10.0),
            tokens.text_muted,
        );
        painter.text(
            egui::pos2(rect.min.x + SPACE_SM + 16.0, rect.center().y),
            egui::Align2::LEFT_CENTER,
            title.to_uppercase(),
            heading_font(11.0),
            tokens.text,
        );
        painter.text(
            egui::pos2(rect.max.x - SPACE_SM, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            meta,
            FontId::monospace(10.0),
            tokens.text_muted,
        );
    }
    response
}

struct ModRowResult {
    changed: bool,
}

fn draw_mod_row(ui: &mut Ui, route: &mut ModSlotUi, row_h: f32) -> ModRowResult {
    let tokens = Tokens::default();
    let mut changed = false;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        if response.hovered() {
            ui.painter_at(rect).rect_filled(
                rect,
                4.0,
                tokens.accent.gamma_multiply(0.08),
            );
        }

        ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(4.0, 1.0)), |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 6.0;
                ui.label(
                    egui::RichText::new(route.source)
                        .font(FontId::monospace(10.0))
                        .color(tokens.text),
                );
                ui.label(
                    egui::RichText::new("→")
                        .size(10.0)
                        .color(tokens.text_muted),
                );
                ui.label(
                    egui::RichText::new(route.target)
                        .size(11.0)
                        .color(tokens.text),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 6.0;
                    let on_label = if route.enabled { "On" } else { "Off" };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(on_label).font(FontId::monospace(10.0)),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        route.enabled = !route.enabled;
                        changed = true;
                    }
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(route.curve).font(FontId::monospace(10.0)),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        route.curve = match route.curve {
                            "Lin" => "Exp",
                            "Exp" => "Step",
                            _ => "Lin",
                        };
                        changed = true;
                    }
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
                    if ui.add(drag).changed() {
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
            });
        });
    }

    ModRowResult { changed }
}
