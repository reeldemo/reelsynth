//! Overtone / anti-crackle chain UI — FxChain-style add / reorder / remove.

use egui::{FontId, Ui};
use reelsynth::{OvertoneFilterSlot, OvertoneFilterType};
use reelsynth_ui_theme::Tokens;

use crate::layout::{RADIUS_SM, GRID_UNIT};
use crate::widgets::{
    button_icon, button_toggle, card_stroke, menu_selectable, reel_combo, select_value_text,
};

#[derive(Debug, Clone)]
pub struct OvertoneFilterSlotUi {
    pub filter_type: OvertoneFilterType,
    pub strength: f32,
    pub bypassed: bool,
}

impl OvertoneFilterSlotUi {
    pub fn from_slot(slot: &OvertoneFilterSlot) -> Self {
        Self {
            filter_type: slot.filter_type.clone(),
            strength: slot.strength.clamp(0.0, 1.0),
            bypassed: slot.bypassed,
        }
    }

    pub fn to_slot(&self) -> OvertoneFilterSlot {
        OvertoneFilterSlot {
            filter_type: self.filter_type.clone(),
            strength: self.strength.clamp(0.0, 1.0),
            bypassed: self.bypassed,
        }
    }

    pub fn default_new() -> Self {
        Self::from_slot(&OvertoneFilterSlot::lowpass())
    }

    pub fn is_active(&self) -> bool {
        !self.bypassed && self.strength > 0.001
    }
}

pub fn overtone_slots_to_engine(slots: &[OvertoneFilterSlotUi]) -> Vec<OvertoneFilterSlot> {
    slots.iter().map(OvertoneFilterSlotUi::to_slot).collect()
}

pub struct OvertoneRackResult {
    pub changed: bool,
}

/// Compact chain UI for header popup / menu (FxChain mechanics).
pub fn draw_overtone_chain_menu(ui: &mut Ui, slots: &mut Vec<OvertoneFilterSlotUi>) -> OvertoneRackResult {
    let tokens = Tokens::default();
    let mut changed = false;
    ui.set_min_width(240.0);
    ui.label(
        egui::RichText::new("Master anti-crackle (before FX)")
            .size(10.0)
            .color(tokens.text_muted),
    );
    ui.add_space(4.0);

    if slots.is_empty() {
        ui.label(
            egui::RichText::new("Empty = Off (identity)")
                .size(11.0)
                .color(tokens.text_secondary),
        );
        ui.add_space(4.0);
    }

    let mut remove_idx: Option<usize> = None;
    let mut swap: Option<(usize, usize)> = None;

    for idx in 0..slots.len() {
        ui.push_id(("overtone_slot", idx), |ui| {
            let active = slots[idx].is_active();
            let bypassed = slots[idx].bypassed;
            egui::Frame {
                fill: if active {
                    tokens.accent_muted.gamma_multiply(0.55)
                } else if bypassed {
                    tokens.bg
                } else {
                    tokens.surface2.gamma_multiply(0.85)
                },
                stroke: egui::Stroke::new(1.0_f32, card_stroke(active, false, &tokens)),
                rounding: egui::Rounding::same(RADIUS_SM),
                inner_margin: egui::Margin::symmetric(GRID_UNIT * 0.5, 4.0),
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{}", idx + 1))
                            .size(10.0)
                            .color(tokens.text_muted),
                    );
                    let on_label = if slots[idx].bypassed { "Off" } else { "On" };
                    if button_toggle(ui, on_label, !slots[idx].bypassed).clicked() {
                        slots[idx].bypassed = !slots[idx].bypassed;
                        changed = true;
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if button_icon(ui, "✕").clicked() {
                            remove_idx = Some(idx);
                        }
                        if idx + 1 < slots.len() && button_icon(ui, "▶").clicked() {
                            swap = Some((idx, idx + 1));
                        }
                        if idx > 0 && button_icon(ui, "◀").clicked() {
                            swap = Some((idx, idx - 1));
                        }
                    });
                });

                ui.horizontal(|ui| {
                    reel_combo(
                        ui,
                        format!("overtone_type_{idx}"),
                        select_value_text(slots[idx].filter_type.label()),
                        110.0,
                        |ui| {
                            for ty in OvertoneFilterType::ALL {
                                if menu_selectable(
                                    ui,
                                    slots[idx].filter_type == ty,
                                    ty.label(),
                                )
                                .clicked()
                                {
                                    let strength = slots[idx].strength;
                                    let bypassed = slots[idx].bypassed;
                                    slots[idx] = OvertoneFilterSlotUi::from_slot(
                                        &OvertoneFilterSlot::for_type(ty.clone()),
                                    );
                                    slots[idx].strength = strength;
                                    slots[idx].bypassed = bypassed;
                                    changed = true;
                                }
                            }
                        },
                    );
                    ui.label(
                        egui::RichText::new("Str")
                            .size(10.0)
                            .color(tokens.text_muted),
                    );
                    let mut pct = (slots[idx].strength * 100.0).clamp(0.0, 100.0);
                    if ui
                        .add(
                            egui::DragValue::new(&mut pct)
                                .speed(0.5)
                                .range(0.0..=100.0)
                                .suffix("%"),
                        )
                        .changed()
                    {
                        slots[idx].strength = (pct / 100.0).clamp(0.0, 1.0);
                        changed = true;
                    }
                });
            });
        });
        ui.add_space(4.0);
    }

    if let Some(i) = remove_idx {
        slots.remove(i);
        changed = true;
    } else if let Some((a, b)) = swap {
        slots.swap(a, b);
        changed = true;
    }

    let add = draw_add_filter_row(ui);
    if add.clicked() {
        slots.push(OvertoneFilterSlotUi::default_new());
        changed = true;
    }

    OvertoneRackResult { changed }
}

fn draw_add_filter_row(ui: &mut Ui) -> egui::Response {
    let tokens = Tokens::default();
    let h = 28.0;
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let stroke = card_stroke(false, response.hovered(), &tokens);
        let fill = if response.hovered() {
            tokens.surface2
        } else {
            tokens.bg
        };
        painter.rect_filled(rect, RADIUS_SM, fill);
        painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0_f32, stroke));
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "+ Add filter",
            FontId::proportional(11.0),
            if response.hovered() {
                tokens.text
            } else {
                tokens.text_secondary
            },
        );
    }
    response
}
