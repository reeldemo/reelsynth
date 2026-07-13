//! Modulation matrix section (S4) — matches `.rs-mod-grid` in mockups.

use egui::{Color32, FontId, Rect, Ui};
use reelsynth::ModSlot;
use reelsynth_ui_theme::{heading_font, Tokens};

use crate::layout::{GRID_UNIT, SPACE_SM};

pub const MOD_ROW_HEIGHT: f32 = 28.0;
pub const MOD_SECTION_HEADER: f32 = 28.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModPolarity {
    Positive,
    Negative,
    Bipolar,
}

#[derive(Debug, Clone)]
pub struct ModRouteUi {
    pub source: &'static str,
    pub target: &'static str,
    pub amount: i32,
    pub curve: &'static str,
    pub enabled: bool,
    pub polarity: ModPolarity,
}

impl Default for ModRouteUi {
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

pub fn default_mod_routes() -> Vec<ModRouteUi> {
    vec![
        ModRouteUi {
            source: "LFO 1",
            target: "WT Pos",
            amount: 32,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Bipolar,
        },
        ModRouteUi {
            source: "Env 2",
            target: "Cutoff",
            amount: 68,
            curve: "Exp",
            enabled: true,
            polarity: ModPolarity::Positive,
        },
        ModRouteUi {
            source: "Velo",
            target: "Level",
            amount: 45,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Positive,
        },
        ModRouteUi {
            source: "ModWh",
            target: "Res",
            amount: -18,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Negative,
        },
        ModRouteUi {
            source: "After",
            target: "Pitch",
            amount: 12,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Bipolar,
        },
        ModRouteUi {
            source: "LFO 2",
            target: "Pan",
            amount: 40,
            curve: "Lin",
            enabled: true,
            polarity: ModPolarity::Bipolar,
        },
        ModRouteUi {
            source: "Step",
            target: "WT Pos",
            amount: 100,
            curve: "Step",
            enabled: true,
            polarity: ModPolarity::Positive,
        },
        ModRouteUi {
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
    pub routes: &'a mut [ModRouteUi],
    pub total_routes: usize,
}

pub struct ModMatrixResult {
    pub changed: bool,
}

pub fn draw_mod_matrix(ui: &mut Ui, rect: Rect, state: ModMatrixState<'_>) -> ModMatrixResult {
    let tokens = Tokens::default();
    let mut changed = false;

    ui.allocate_ui_at_rect(rect, |ui| {
        egui::Frame::none()
            .fill(tokens.bg_muted)
            .stroke(egui::Stroke::new(1.0_f32, tokens.border))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let active = state.routes.iter().filter(|r| r.enabled).count();
                let meta = format!("{active} / {} routes", state.total_routes);
                let header = section_header(ui, "Modulation Matrix", &meta, *state.open);
                if header.clicked() {
                    *state.open = !*state.open;
                }

                if *state.open {
                    ui.add_space(GRID_UNIT);
                    egui::Frame::none()
                        .inner_margin(egui::Margin::symmetric(SPACE_SM, GRID_UNIT))
                        .show(ui, |ui| {
                            egui::ScrollArea::vertical()
                                .max_height(rect.height() - MOD_SECTION_HEADER - GRID_UNIT * 2.0)
                                .show(ui, |ui| {
                                    ui.spacing_mut().item_spacing.y = 2.0;
                                    for route in state.routes.iter_mut() {
                                        if draw_mod_row(ui, route).changed {
            changed = true;
        }
                                    }
                                });
                        });
                }
            });
    });

    ModMatrixResult { changed }
}

/// Map UI rows to engine [`ModSlot`] entries (S6).
pub fn mod_routes_to_slots(routes: &[ModRouteUi]) -> Vec<ModSlot> {
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
pub fn mod_routes_from_slots(slots: &[ModSlot]) -> Vec<ModRouteUi> {
    if slots.is_empty() {
        return default_mod_routes();
    }
    slots
        .iter()
        .map(|slot| ModRouteUi {
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
        let routes = default_mod_routes();
        let slots = mod_routes_to_slots(&routes);
        assert!(!slots.is_empty());
        let restored = mod_routes_from_slots(&slots);
        assert_eq!(restored.len(), routes.len());
        assert_eq!(restored[0].source, routes[0].source);
        assert_eq!(restored[0].target, routes[0].target);
    }

    #[test]
    fn disabled_routes_persist() {
        let mut routes = default_mod_routes();
        routes[0].enabled = false;
        let slots = mod_routes_to_slots(&routes);
        assert!(!slots[0].enabled);
    }
}

fn section_header(ui: &mut Ui, title: &str, meta: &str, open: bool) -> egui::Response {
    let tokens = Tokens::default();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), MOD_SECTION_HEADER), egui::Sense::click());
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

fn draw_mod_row(ui: &mut Ui, route: &mut ModRouteUi) -> ModRowResult {
    let tokens = Tokens::default();
    let mut changed = false;
    let row_h = MOD_ROW_HEIGHT;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), row_h), egui::Sense::hover());

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let fill = if response.hovered() {
            tokens.accent.gamma_multiply(0.14)
        } else {
            tokens.bg_muted
        };
        painter.rect_filled(rect, 6.0, fill);
        painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0_f32, tokens.border));

        let src_w = 100.0;
        let cell_w = 72.0;
        let on_w = 48.0;
        let gap = GRID_UNIT;
        let inner = rect.shrink2(egui::vec2(8.0, 4.0));

        painter.text(
            egui::pos2(inner.min.x, inner.center().y),
            egui::Align2::LEFT_CENTER,
            route.source,
            FontId::monospace(10.0),
            tokens.text,
        );

        let arrow_x = inner.min.x + src_w;
        painter.text(
            egui::pos2(arrow_x, inner.center().y),
            egui::Align2::LEFT_CENTER,
            format!("→ {}", route.target),
            FontId::proportional(11.0),
            tokens.text_muted,
        );

        let amount_x = inner.max.x - on_w - gap - cell_w - gap - cell_w;
        let amount_rect = Rect::from_min_size(
            egui::pos2(amount_x, inner.min.y),
            egui::vec2(cell_w, inner.height()),
        );
        if paint_mod_cell(ui, amount_rect, route) {
            changed = true;
        }

        let curve_rect = Rect::from_min_size(
            egui::pos2(amount_x + cell_w + gap, inner.min.y),
            egui::vec2(cell_w, inner.height()),
        );
        if ui.allocate_rect(curve_rect, egui::Sense::click()).clicked() {
            route.curve = match route.curve {
                "Lin" => "Exp",
                "Exp" => "Step",
                _ => "Lin",
            };
            changed = true;
        }
        if ui.is_rect_visible(curve_rect) {
            let painter = ui.painter_at(curve_rect);
            painter.rect_filled(curve_rect, 4.0, tokens.surface2);
            painter.rect_stroke(curve_rect, 4.0, egui::Stroke::new(1.0_f32, tokens.border));
            painter.text(
                curve_rect.center(),
                egui::Align2::CENTER_CENTER,
                route.curve,
                FontId::monospace(11.0),
                tokens.text,
            );
        }

        let on_rect = Rect::from_min_size(
            egui::pos2(inner.max.x - on_w, inner.min.y),
            egui::vec2(on_w, inner.height()),
        );
        if ui.allocate_rect(on_rect, egui::Sense::click()).clicked() {
            route.enabled = !route.enabled;
            changed = true;
        }
        if ui.is_rect_visible(on_rect) {
            let on_label = if route.enabled { "On" } else { "Off" };
            ui.painter_at(on_rect).text(
                on_rect.center(),
                egui::Align2::CENTER_CENTER,
                on_label,
                FontId::monospace(10.0),
                tokens.text_muted,
            );
        }
    }

    ModRowResult { changed }
}

fn paint_mod_cell(ui: &mut Ui, rect: Rect, route: &mut ModRouteUi) -> bool {
    let tokens = Tokens::default();
    let (stroke, text_color, fill) = match route.polarity {
        ModPolarity::Positive => (
            Color32::from_rgb(0x4a, 0xde, 0x80).gamma_multiply(0.4),
            Color32::from_rgb(0x4a, 0xde, 0x80),
            tokens.surface2,
        ),
        ModPolarity::Negative => (
            Color32::from_rgb(0xf8, 0x71, 0x71).gamma_multiply(0.4),
            Color32::from_rgb(0xf8, 0x71, 0x71),
            tokens.surface2,
        ),
        ModPolarity::Bipolar => (
            Color32::from_rgb(0x2a, 0x6b, 0x8a),
            tokens.accent_on,
            tokens.accent.gamma_multiply(0.35),
        ),
    };

    let amount_label = match route.polarity {
        ModPolarity::Bipolar => format!("±{}", route.amount.abs()),
        ModPolarity::Negative => format!("−{}", route.amount.abs()),
        ModPolarity::Positive => format!("+{}", route.amount),
    };

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, fill);
        painter.rect_stroke(rect, 4.0, egui::Stroke::new(1.0_f32, stroke));
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            amount_label,
            FontId::monospace(11.0),
            text_color,
        );
    }

    let mut amount = route.amount as f32;
    let resp = ui.allocate_rect(rect, egui::Sense::click_and_drag());
    if resp.dragged() {
        amount += resp.drag_delta().x * 0.5;
        amount = amount.clamp(-100.0, 100.0);
        route.amount = amount.round() as i32;
        if route.polarity != ModPolarity::Bipolar {
            route.polarity = if route.amount < 0 {
                ModPolarity::Negative
            } else {
                ModPolarity::Positive
            };
        }
        return true;
    }
    false
}
