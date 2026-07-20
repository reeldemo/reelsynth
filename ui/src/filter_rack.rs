//! Musical voice filter chain UI — FxChain / Overtone-style add / reorder / remove.
//! Distinct from header Overtone (master-bus anti-crackle).

use egui::{FontId, Ui};
use reelsynth::{
    filter_type_label, legacy_filter_slots, normalize_filter_type, Filter, FilterSlot, FILTER_TYPES,
};
use reelsynth_ui_theme::Tokens;

use crate::layout::{RADIUS_SM, GRID_UNIT};
use crate::widgets::{
    button_icon, button_toggle, card_stroke, menu_selectable, reel_combo, select_value_text,
};

fn format_cutoff_hz(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1} kHz", hz / 1000.0)
    } else {
        format!("{:.0} Hz", hz)
    }
}

#[derive(Debug, Clone)]
pub struct FilterSlotUi {
    pub filter_type: String,
    pub cutoff: f32,
    pub resonance: f32,
    pub key_tracking: f32,
    pub drive: f32,
    pub bypassed: bool,
}

impl FilterSlotUi {
    pub fn from_slot(slot: &FilterSlot) -> Self {
        Self {
            filter_type: normalize_filter_type(&slot.filter_type).into(),
            cutoff: slot.cutoff.clamp(40.0, 12000.0),
            resonance: slot.resonance.clamp(0.0, 0.95),
            key_tracking: slot.key_tracking.clamp(0.0, 1.0),
            drive: slot.drive.clamp(0.0, 1.0),
            bypassed: slot.bypassed,
        }
    }

    pub fn to_slot(&self) -> FilterSlot {
        FilterSlot {
            filter_type: normalize_filter_type(&self.filter_type).into(),
            cutoff: self.cutoff.clamp(40.0, 12000.0),
            resonance: self.resonance.clamp(0.0, 0.95),
            key_tracking: self.key_tracking.clamp(0.0, 1.0),
            drive: self.drive.clamp(0.0, 1.0),
            bypassed: self.bypassed,
        }
    }

    pub fn from_filter(f: &Filter) -> Self {
        Self::from_slot(&FilterSlot::from_filter(f))
    }

    pub fn default_new() -> Self {
        Self::from_slot(&FilterSlot::lowpass())
    }

    pub fn is_active(&self) -> bool {
        !self.bypassed
    }
}

pub fn filter_slots_from_patch(filter: &Filter, filter2: &Filter, filters: &Option<Vec<FilterSlot>>) -> Vec<FilterSlotUi> {
    match filters {
        Some(slots) => slots.iter().map(FilterSlotUi::from_slot).collect(),
        None => legacy_filter_slots(filter, filter2)
            .iter()
            .map(FilterSlotUi::from_slot)
            .collect(),
    }
}

pub fn filter_slots_to_patch(slots: &[FilterSlotUi]) -> Option<Vec<FilterSlot>> {
    Some(
        slots
            .iter()
            .take(FilterSlot::MAX_SLOTS)
            .map(FilterSlotUi::to_slot)
            .collect(),
    )
}

pub struct FilterRackResult {
    pub changed: bool,
}

/// Right-rail musical filter chain (serial SVF). Empty = bypass.
pub fn draw_filter_chain(ui: &mut Ui, slots: &mut Vec<FilterSlotUi>, scale: f32) -> FilterRackResult {
    let tokens = Tokens::default();
    let mut changed = false;

    ui.label(
        egui::RichText::new("Voice filter chain (serial)")
            .size(10.0 * scale)
            .color(tokens.text_muted),
    );
    ui.add_space(2.0 * scale);

    if slots.is_empty() {
        ui.label(
            egui::RichText::new("Empty = bypass")
                .size(11.0 * scale)
                .color(tokens.text_secondary),
        );
        ui.add_space(4.0 * scale);
    }

    let mut remove_idx: Option<usize> = None;
    let mut swap: Option<(usize, usize)> = None;

    for idx in 0..slots.len() {
        ui.push_id(("filter_slot", idx), |ui| {
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
                inner_margin: egui::Margin::symmetric(GRID_UNIT * 0.5 * scale, 4.0 * scale),
                ..Default::default()
            }
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{}", idx + 1))
                            .size(10.0 * scale)
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

                reel_combo(
                    ui,
                    format!("filter_type_{idx}"),
                    select_value_text(filter_type_label(&slots[idx].filter_type)),
                    ui.available_width().max(100.0),
                    |ui| {
                        for ty in FILTER_TYPES {
                            if menu_selectable(
                                ui,
                                normalize_filter_type(&slots[idx].filter_type) == ty,
                                filter_type_label(ty),
                            )
                            .clicked()
                            {
                                let keep = (
                                    slots[idx].cutoff,
                                    slots[idx].resonance,
                                    slots[idx].key_tracking,
                                    slots[idx].drive,
                                    slots[idx].bypassed,
                                );
                                slots[idx] = FilterSlotUi::from_slot(&FilterSlot::for_type(ty));
                                slots[idx].cutoff = keep.0;
                                slots[idx].resonance = keep.1;
                                slots[idx].key_tracking = keep.2;
                                slots[idx].drive = keep.3;
                                slots[idx].bypassed = keep.4;
                                changed = true;
                            }
                        }
                    },
                );

                ui.add_space(2.0 * scale);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Cut")
                            .size(10.0 * scale)
                            .color(tokens.text_muted),
                    );
                    let mut cut = slots[idx].cutoff;
                    let cut_text = format_cutoff_hz(cut);
                    if ui
                        .add(
                            egui::DragValue::new(&mut cut)
                                .speed(8.0)
                                .range(40.0..=12000.0)
                                .custom_formatter(|_, _| cut_text.clone())
                                .custom_parser(|s| s.trim_end_matches(" Hz").trim_end_matches(" kHz").parse().ok()),
                        )
                        .changed()
                    {
                        slots[idx].cutoff = cut.clamp(40.0, 12000.0);
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Res")
                            .size(10.0 * scale)
                            .color(tokens.text_muted),
                    );
                    let mut res = slots[idx].resonance;
                    if ui
                        .add(egui::DragValue::new(&mut res).speed(0.005).range(0.0..=0.95))
                        .changed()
                    {
                        slots[idx].resonance = res.clamp(0.0, 0.95);
                        changed = true;
                    }
                    ui.label(
                        egui::RichText::new("Drv")
                            .size(10.0 * scale)
                            .color(tokens.text_muted),
                    );
                    let mut drv_pct = (slots[idx].drive * 100.0).clamp(0.0, 100.0);
                    if ui
                        .add(
                            egui::DragValue::new(&mut drv_pct)
                                .speed(0.5)
                                .range(0.0..=100.0)
                                .suffix("%"),
                        )
                        .changed()
                    {
                        slots[idx].drive = (drv_pct / 100.0).clamp(0.0, 1.0);
                        changed = true;
                    }
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Key")
                            .size(10.0 * scale)
                            .color(tokens.text_muted),
                    );
                    let mut key_pct = (slots[idx].key_tracking * 100.0).clamp(0.0, 100.0);
                    if ui
                        .add(
                            egui::DragValue::new(&mut key_pct)
                                .speed(0.5)
                                .range(0.0..=100.0)
                                .suffix("%"),
                        )
                        .changed()
                    {
                        slots[idx].key_tracking = (key_pct / 100.0).clamp(0.0, 1.0);
                        changed = true;
                    }
                });
            });
        });
        ui.add_space(4.0 * scale);
    }

    if let Some(i) = remove_idx {
        slots.remove(i);
        changed = true;
    } else if let Some((a, b)) = swap {
        slots.swap(a, b);
        changed = true;
    }

    if slots.len() < FilterSlot::MAX_SLOTS {
        let add = draw_add_filter_row(ui, scale);
        if add.clicked() {
            slots.push(FilterSlotUi::default_new());
            changed = true;
        }
    }

    FilterRackResult { changed }
}

fn draw_add_filter_row(ui: &mut Ui, scale: f32) -> egui::Response {
    let tokens = Tokens::default();
    let h = 28.0 * scale;
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
            FontId::proportional(11.0 * scale),
            if response.hovered() {
                tokens.text
            } else {
                tokens.text_secondary
            },
        );
    }
    response
}
