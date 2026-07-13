//! FX rack section (S5/S6) — reorderable slot cards per COMPONENT_SPEC.

use egui::{Color32, FontId, Rect, Ui};
use reelsynth::{EffectSlot, EffectType};
use reelsynth_ui_theme::{heading_font, Tokens};

use crate::layout::{GRID_UNIT, RADIUS_SM, SPACE_SM};
use crate::widgets::button_icon;

pub const FX_SLOT_WIDTH: f32 = 160.0;
pub const FX_SECTION_HEADER: f32 = 28.0;
const CPU_WARN_ACTIVE_SLOTS: usize = 4;

#[derive(Debug, Clone)]
pub struct EffectSlotUi {
    pub effect_type: EffectType,
    pub bypassed: bool,
    pub mix: f32,
    pub rate: f32,
    pub depth: f32,
    pub time_ms: f32,
    pub feedback: f32,
    pub size: f32,
    pub damping: f32,
    pub drive: f32,
    pub tone: f32,
    pub threshold: f32,
    pub ratio: f32,
    pub attack: f32,
    pub release: f32,
}

impl EffectSlotUi {
    pub fn from_slot(slot: &EffectSlot) -> Self {
        Self {
            effect_type: slot.effect_type.clone(),
            bypassed: slot.bypassed,
            mix: slot.mix,
            rate: slot.rate,
            depth: slot.depth,
            time_ms: slot.time_ms,
            feedback: slot.feedback,
            size: slot.size,
            damping: slot.damping,
            drive: slot.drive,
            tone: slot.tone,
            threshold: slot.threshold,
            ratio: slot.ratio,
            attack: slot.attack,
            release: slot.release,
        }
    }

    pub fn to_slot(&self) -> EffectSlot {
        let mut slot = EffectSlot::for_type(self.effect_type.clone());
        slot.bypassed = self.bypassed;
        slot.mix = self.mix;
        slot.rate = self.rate;
        slot.depth = self.depth;
        slot.time_ms = self.time_ms;
        slot.feedback = self.feedback;
        slot.size = self.size;
        slot.damping = self.damping;
        slot.drive = self.drive;
        slot.tone = self.tone;
        slot.threshold = self.threshold;
        slot.ratio = self.ratio;
        slot.attack = self.attack;
        slot.release = self.release;
        slot
    }

    pub fn detail(&self) -> String {
        if self.bypassed {
            return "Bypassed".into();
        }
        match self.effect_type {
            EffectType::Chorus => {
                format!("Mix {:.0}% · {:.1} Hz", self.mix * 100.0, self.rate)
            }
            EffectType::Delay => {
                format!("{:.0} ms · FB {:.0}%", self.time_ms, self.feedback * 100.0)
            }
            EffectType::Reverb => {
                format!("Size {:.0}% · Mix {:.0}%", self.size * 100.0, self.mix * 100.0)
            }
            EffectType::Distortion => {
                format!("Drive {:.0}% · Mix {:.0}%", self.drive * 100.0, self.mix * 100.0)
            }
            EffectType::Compressor => {
                format!("{:.0} dB · {:.1}:1", self.threshold, self.ratio)
            }
        }
    }

    pub fn is_active(&self) -> bool {
        !self.bypassed && self.mix > 0.001
    }
}

pub fn default_effect_slots() -> Vec<EffectSlotUi> {
    effect_slots_from_patch(&reelsynth::default_effects())
}

pub fn effect_slots_from_patch(effects: &[EffectSlot]) -> Vec<EffectSlotUi> {
    if effects.is_empty() {
        return default_effect_slots();
    }
    effects.iter().map(EffectSlotUi::from_slot).collect()
}

pub fn effect_slots_to_patch(slots: &[EffectSlotUi]) -> Vec<EffectSlot> {
    slots.iter().map(EffectSlotUi::to_slot).collect()
}

/// Legacy bridge for old bypass-only API.
pub fn effect_slots_to_bypass(slots: &[EffectSlotUi]) -> reelsynth::FxBypass {
    let mut bypass = reelsynth::FxBypass::default();
    for slot in slots {
        match slot.effect_type {
            EffectType::Chorus => bypass.chorus_bypassed = slot.bypassed,
            EffectType::Delay => bypass.delay_bypassed = slot.bypassed,
            EffectType::Reverb => bypass.reverb_bypassed = slot.bypassed,
            _ => {}
        }
    }
    bypass
}

pub fn effect_slots_from_bypass(bypass: &reelsynth::FxBypass) -> Vec<EffectSlotUi> {
    effect_slots_from_patch(&reelsynth::effects_from_bypass(bypass))
}

pub struct EffectRackState<'a> {
    pub open: &'a mut bool,
    pub slots: &'a mut Vec<EffectSlotUi>,
}

pub struct FxRackResult {
    pub changed: bool,
}

pub fn draw_effect_rack(ui: &mut Ui, rect: Rect, mut state: EffectRackState<'_>) -> FxRackResult {
    let tokens = Tokens::default();
    let mut changed = false;

    ui.allocate_ui_at_rect(rect, |ui| {
        egui::Frame::none()
            .fill(tokens.bg_muted)
            .stroke(egui::Stroke::new(1.0_f32, tokens.border))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let active = state.slots.iter().filter(|s| s.is_active()).count();
                let mut meta = format!("{active} active");
                if active > CPU_WARN_ACTIVE_SLOTS {
                    meta.push_str(" · CPU ⚠");
                }
                let header = section_header(ui, "Effects", &meta, *state.open);
                if header.clicked() {
                    *state.open = !*state.open;
                }

                if *state.open {
                    if active > CPU_WARN_ACTIVE_SLOTS {
                        ui.label(
                            egui::RichText::new(format!(
                                "⚠ {active} active FX slots — may increase CPU usage"
                            ))
                            .size(10.0)
                            .color(Color32::from_rgb(0xe8, 0xa8, 0x40)),
                        );
                    }

                    egui::Frame::none()
                        .inner_margin(egui::Margin::symmetric(SPACE_SM, GRID_UNIT))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = GRID_UNIT;
                                for idx in 0..state.slots.len() {
                                    if draw_fx_slot(ui, &mut state.slots, idx).changed {
                                        changed = true;
                                    }
                                }
                                if draw_add_slot(ui).clicked() {
                                    state
                                        .slots
                                        .push(EffectSlotUi::from_slot(&EffectSlot::chorus()));
                                    changed = true;
                                }
                            });
                        });
                }
            });
    });

    FxRackResult { changed }
}

fn section_header(ui: &mut Ui, title: &str, meta: &str, open: bool) -> egui::Response {
    let tokens = Tokens::default();
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), FX_SECTION_HEADER), egui::Sense::click());
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
        let meta_color = if meta.contains('⚠') {
            Color32::from_rgb(0xe8, 0xa8, 0x40)
        } else {
            tokens.text_muted
        };
        painter.text(
            egui::pos2(rect.max.x - SPACE_SM, rect.center().y),
            egui::Align2::RIGHT_CENTER,
            meta,
            FontId::monospace(10.0),
            meta_color,
        );
    }
    response
}

struct FxSlotResult {
    changed: bool,
}

fn draw_fx_slot(ui: &mut Ui, slots: &mut Vec<EffectSlotUi>, idx: usize) -> FxSlotResult {
    let tokens = Tokens::default();
    let mut changed = false;
    let slot_h = 72.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(FX_SLOT_WIDTH, slot_h),
        egui::Sense::click(),
    );

    let active = slots[idx].is_active();

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let stroke = if active {
            Color32::from_rgb(0x2a, 0x6b, 0x8a)
        } else {
            tokens.border
        };
        let fill = if response.hovered() {
            tokens.surface2
        } else {
            tokens.bg_muted
        };
        painter.rect_filled(rect, RADIUS_SM, fill);
        painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0_f32, stroke));

        let name = slots[idx].effect_type.label();
        let detail = slots[idx].detail();
        painter.text(
            egui::pos2(rect.min.x + GRID_UNIT, rect.min.y + GRID_UNIT),
            egui::Align2::LEFT_TOP,
            name,
            FontId::proportional(11.0),
            tokens.text,
        );
        painter.text(
            egui::pos2(rect.min.x + GRID_UNIT, rect.min.y + GRID_UNIT + 18.0),
            egui::Align2::LEFT_TOP,
            detail,
            FontId::proportional(10.0),
            tokens.text_muted,
        );
    }

    if response.clicked() {
        slots[idx].bypassed = !slots[idx].bypassed;
        changed = true;
    }

    // Controls row below card
    ui.allocate_ui_at_rect(
        Rect::from_min_size(
            egui::pos2(rect.min.x, rect.max.y + 2.0),
            egui::vec2(FX_SLOT_WIDTH, 18.0),
        ),
        |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                if idx > 0 && button_icon(ui, "◀").clicked() {
                    slots.swap(idx, idx - 1);
                    changed = true;
                }
                if idx + 1 < slots.len() && button_icon(ui, "▶").clicked() {
                    slots.swap(idx, idx + 1);
                    changed = true;
                }
                if slots.len() > 1 && button_icon(ui, "✕").clicked() {
                    slots.remove(idx);
                    changed = true;
                    return;
                }
                egui::ComboBox::from_id_source(format!("fx_type_{idx}"))
                    .selected_text(slots[idx].effect_type.label())
                    .width(72.0)
                    .show_ui(ui, |ui| {
                        for ty in EffectType::ALL {
                            if ui
                                .selectable_value(
                                    &mut slots[idx].effect_type,
                                    ty.clone(),
                                    ty.label(),
                                )
                                .clicked()
                            {
                                let bypassed = slots[idx].bypassed;
                                let mix = slots[idx].mix;
                                slots[idx] = EffectSlotUi::from_slot(&EffectSlot::for_type(ty));
                                slots[idx].bypassed = bypassed;
                                slots[idx].mix = mix;
                                changed = true;
                            }
                        }
                    });
            });
        },
    );

    FxSlotResult { changed }
}

fn draw_add_slot(ui: &mut Ui) -> egui::Response {
    let tokens = Tokens::default();
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(48.0, 72.0),
        egui::Sense::click(),
    );
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS_SM, tokens.bg_muted);
        painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0_f32, tokens.border));
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "+",
            FontId::proportional(18.0),
            tokens.text_muted,
        );
    }
    response
}

#[cfg(test)]
mod bridge_tests {
    use super::*;
    use reelsynth::fx::{EffectSlot, EffectType, FxBypass};

    #[test]
    fn fx_slot_ui_roundtrip() {
        let slot = EffectSlot::delay();
        let ui = EffectSlotUi::from_slot(&slot);
        let restored = ui.to_slot();
        assert_eq!(restored.effect_type, slot.effect_type);
        assert_eq!(restored.time_ms, slot.time_ms);
        assert!((restored.mix - slot.mix).abs() < 1e-5);
    }

    #[test]
    fn bypass_migration_roundtrip() {
        let bypass = FxBypass {
            chorus_bypassed: true,
            delay_bypassed: false,
            reverb_bypassed: true,
        };
        let slots = effect_slots_from_bypass(&bypass);
        assert_eq!(slots.len(), 3);
        assert!(slots[0].bypassed);
        assert!(!slots[1].bypassed);
        let back = effect_slots_to_bypass(&slots);
        assert_eq!(back.chorus_bypassed, bypass.chorus_bypassed);
        assert_eq!(back.delay_bypassed, bypass.delay_bypassed);
    }

    #[test]
    fn effect_type_labels() {
        let mut slot = EffectSlot::for_type(EffectType::Distortion);
        slot.bypassed = false;
        let ui = EffectSlotUi::from_slot(&slot);
        assert!(ui.detail().contains("Drive"));
    }
}
