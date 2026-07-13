//! FX rack section (S5) — matches `.rs-fx-rack` slot cards in mockups.

use egui::{Color32, FontId, Rect, Ui};
use reelsynth::FxBypass;
use reelsynth_ui_theme::{heading_font, Tokens};

use crate::layout::{GRID_UNIT, RADIUS_SM, SPACE_SM};

pub const FX_SLOT_WIDTH: f32 = 160.0;
pub const FX_SECTION_HEADER: f32 = 28.0;

#[derive(Debug, Clone)]
pub struct FxSlotUi {
    pub name: &'static str,
    pub detail: String,
    pub active: bool,
    pub bypassed: bool,
}

impl FxSlotUi {
    pub fn empty() -> Self {
        Self {
            name: "+ Slot",
            detail: "Empty".into(),
            active: false,
            bypassed: true,
        }
    }
}

pub fn default_fx_slots() -> Vec<FxSlotUi> {
    vec![
        FxSlotUi {
            name: "Chorus",
            detail: "Mix 24% · Rate 0.8 Hz".into(),
            active: true,
            bypassed: false,
        },
        FxSlotUi {
            name: "Delay",
            detail: "1/8 dotted · FB 32%".into(),
            active: true,
            bypassed: false,
        },
        FxSlotUi {
            name: "Reverb",
            detail: "Bypassed".into(),
            active: false,
            bypassed: true,
        },
        FxSlotUi::empty(),
    ]
}

pub struct FxRackState<'a> {
    pub open: &'a mut bool,
    pub slots: &'a mut [FxSlotUi],
}

pub struct FxRackResult {
    pub changed: bool,
}

pub fn draw_fx_rack(ui: &mut Ui, rect: Rect, state: FxRackState<'_>) -> FxRackResult {
    let tokens = Tokens::default();
    let mut changed = false;

    ui.allocate_ui_at_rect(rect, |ui| {
        egui::Frame::none()
            .fill(tokens.bg_muted)
            .stroke(egui::Stroke::new(1.0_f32, tokens.border))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                let active = state.slots.iter().filter(|s| s.active && !s.bypassed).count();
                let meta = format!("{active} active");
                let header = section_header(ui, "Effects", &meta, *state.open);
                if header.clicked() {
                    *state.open = !*state.open;
                }

                if *state.open {
                    egui::Frame::none()
                        .inner_margin(egui::Margin::symmetric(SPACE_SM, GRID_UNIT))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = GRID_UNIT;
                                for slot in state.slots.iter_mut() {
                                    if draw_fx_slot(ui, slot).changed {
                                        changed = true;
                                    }
                                }
                            });
                        });
                }
            });
    });

    FxRackResult { changed }
}

/// Map UI slot cards to engine bypass flags (S6).
pub fn fx_slots_to_bypass(slots: &[FxSlotUi]) -> FxBypass {
    let mut bypass = FxBypass::default();
    for slot in slots {
        match slot.name {
            "Chorus" => bypass.chorus_bypassed = slot.bypassed,
            "Delay" => bypass.delay_bypassed = slot.bypassed,
            "Reverb" => bypass.reverb_bypassed = slot.bypassed,
            _ => {}
        }
    }
    bypass
}

/// Hydrate UI slots from patch bypass state.
pub fn fx_slots_from_bypass(bypass: &FxBypass) -> Vec<FxSlotUi> {
    let mut slots = default_fx_slots();
    for slot in &mut slots {
        match slot.name {
            "Chorus" => {
                slot.bypassed = bypass.chorus_bypassed;
                slot.active = !bypass.chorus_bypassed;
            }
            "Delay" => {
                slot.bypassed = bypass.delay_bypassed;
                slot.active = !bypass.delay_bypassed;
            }
            "Reverb" => {
                slot.bypassed = bypass.reverb_bypassed;
                slot.active = !bypass.reverb_bypassed;
            }
            _ => {}
        }
        if slot.bypassed && slot.name != "+ Slot" {
            slot.detail = "Bypassed".into();
        }
    }
    slots
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

struct FxSlotResult {
    changed: bool,
}

fn draw_fx_slot(ui: &mut Ui, slot: &mut FxSlotUi) -> FxSlotResult {
    let tokens = Tokens::default();
    let mut changed = false;
    let slot_h = 64.0;
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(FX_SLOT_WIDTH, slot_h),
        egui::Sense::click(),
    );

    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let stroke = if slot.active && !slot.bypassed {
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

        painter.text(
            egui::pos2(rect.min.x + GRID_UNIT, rect.min.y + GRID_UNIT),
            egui::Align2::LEFT_TOP,
            slot.name,
            FontId::proportional(11.0),
            tokens.text,
        );
        painter.text(
            egui::pos2(rect.min.x + GRID_UNIT, rect.min.y + GRID_UNIT + 18.0),
            egui::Align2::LEFT_TOP,
            &slot.detail,
            FontId::proportional(10.0),
            tokens.text_muted,
        );
    }

    if response.clicked() && slot.name != "+ Slot" {
        slot.bypassed = !slot.bypassed;
        slot.active = !slot.bypassed;
        slot.detail = if slot.bypassed {
            "Bypassed".into()
        } else {
            match slot.name {
                "Chorus" => "Mix 24% · Rate 0.8 Hz".into(),
                "Delay" => "1/8 dotted · FB 32%".into(),
                "Reverb" => "Room 42% · Size 68%".into(),
                _ => "Active".into(),
            }
        };
        changed = true;
    }

    FxSlotResult { changed }
}
