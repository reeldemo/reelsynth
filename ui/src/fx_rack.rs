//! FX rack section (S5/S6) — reorderable slot cards per COMPONENT_SPEC.

use egui::{Color32, FontId, Rect, Ui};
use reelsynth::{EffectSlot, EffectType};
use reelsynth_ui_theme::{heading_font, Tokens, ACCENT_UI};

use crate::layout::{UiScale, GRID_UNIT, RADIUS_SM, SPACE_SM};
use crate::region::region;
use crate::widgets::{button_icon, menu_selectable, reel_combo, select_value_text};

pub const FX_SLOT_WIDTH: f32 = 148.0;
pub const FX_SECTION_HEADER: f32 = 24.0;
const CPU_WARN_ACTIVE_SLOTS: usize = 4;

#[derive(Debug, Clone, Copy)]
struct FxMetrics {
    slot_width: f32,
    card_height: f32,
    controls_height: f32,
    column_height: f32,
    add_width: f32,
    header_h: f32,
}

impl FxMetrics {
    fn from_scale(scale: UiScale, body_h: f32) -> Self {
        let s = scale.ui();
        let header_h = FX_SECTION_HEADER * s;
        let controls_h = 18.0 * s;
        let gap = 2.0 * s;
        let body = (body_h - header_h).max(40.0 * s);
        let card_h = (body - controls_h - gap).clamp(44.0 * s, 60.0 * s);
        let column_h = card_h + gap + controls_h;
        Self {
            slot_width: FX_SLOT_WIDTH * s,
            card_height: card_h,
            controls_height: controls_h,
            column_height: column_h,
            add_width: 40.0 * s,
            header_h,
        }
    }
}

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
            EffectType::Chorus => format!("Mix {:.0}% · {:.1} Hz", self.mix * 100.0, self.rate),
            EffectType::Delay => format!("{:.0} ms · FB {:.0}%", self.time_ms, self.feedback * 100.0),
            EffectType::Reverb => format!("Size {:.0}% · Mix {:.0}%", self.size * 100.0, self.mix * 100.0),
            EffectType::Distortion => format!("Drive {:.0}% · Mix {:.0}%", self.drive * 100.0, self.mix * 100.0),
            EffectType::Compressor => format!("{:.0} dB · {:.1}:1", self.threshold, self.ratio),
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

pub fn draw_effect_rack(
    ui: &mut Ui,
    rect: Rect,
    mut state: EffectRackState<'_>,
    scale: UiScale,
) -> FxRackResult {
    draw_effect_rack_inner(ui, rect, &mut state, scale, RackLayout::Horizontal)
}

/// Narrow-column layout: 2-column grid of effect slots (left osc column).
pub fn draw_effect_rack_sidebar(
    ui: &mut Ui,
    rect: Rect,
    mut state: EffectRackState<'_>,
    scale: UiScale,
) -> FxRackResult {
    draw_effect_rack_inner(ui, rect, &mut state, scale, RackLayout::Grid2x2)
}

#[derive(Clone, Copy)]
enum RackLayout {
    Horizontal,
    Grid2x2,
}

fn draw_effect_rack_inner(
    ui: &mut Ui,
    rect: Rect,
    state: &mut EffectRackState<'_>,
    scale: UiScale,
    layout: RackLayout,
) -> FxRackResult {
    let tokens = Tokens::default();
    let mut changed = false;
    let metrics = FxMetrics::from_scale(scale, rect.height());

    region(ui, rect, |ui| {
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
                let header = section_header(ui, "Effects", &meta, *state.open, metrics.header_h);
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
                        .inner_margin(egui::Margin::symmetric(SPACE_SM * scale.ui(), GRID_UNIT * scale.ui()))
                        .show(ui, |ui| {
                            match layout {
                                RackLayout::Horizontal => {
                                    draw_effect_rack_horizontal(ui, state, scale, metrics, &mut changed);
                                }
                                RackLayout::Grid2x2 => {
                                    draw_effect_rack_grid(ui, state, scale, metrics, &mut changed);
                                }
                            }
                        });
                }
            });
    });

    FxRackResult { changed }
}

fn draw_effect_rack_horizontal(
    ui: &mut Ui,
    state: &mut EffectRackState<'_>,
    scale: UiScale,
    metrics: FxMetrics,
    changed: &mut bool,
) {
    let s = scale.ui();
    let gap = GRID_UNIT * s;
    let slot_count = state.slots.len().max(1);
    let add_w = metrics.add_width;
    let gaps = gap * slot_count as f32;
    let avail = ui.available_width();
    let flex_slot_w = ((avail - add_w - gaps) / slot_count as f32)
        .clamp(96.0 * s, FX_SLOT_WIDTH * s * 1.35);
    let mut flex_metrics = metrics;
    flex_metrics.slot_width = flex_slot_w;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = gap;
        ui.set_min_height(flex_metrics.column_height);
        for idx in 0..state.slots.len() {
            if draw_fx_slot_column(ui, &mut state.slots, idx, flex_metrics).changed {
                *changed = true;
            }
        }
        if draw_add_slot(ui, flex_metrics).clicked() {
            state
                .slots
                .push(EffectSlotUi::from_slot(&EffectSlot::chorus()));
            *changed = true;
        }
    });
}

fn draw_effect_rack_grid(
    ui: &mut Ui,
    state: &mut EffectRackState<'_>,
    scale: UiScale,
    metrics: FxMetrics,
    changed: &mut bool,
) {
    let s = scale.ui();
    let gap = GRID_UNIT * s * 0.75;
    let total_w = ui.available_width();
    let col_w = ((total_w - gap) * 0.5).max(72.0 * s);
    let card_h = metrics.card_height.max(48.0 * s);
    let controls_h = metrics.controls_height;
    let column_h = card_h + gap * 0.5 + controls_h;
    let grid_metrics = FxMetrics {
        slot_width: col_w,
        card_height: card_h,
        controls_height: controls_h,
        column_height: column_h,
        add_width: col_w,
        header_h: metrics.header_h,
    };

    let cell_count = state.slots.len() + 1;
    let rows = cell_count.div_ceil(2);
    for row in 0..rows {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = gap;
            for col in 0..2 {
                let cell = row * 2 + col;
                ui.allocate_ui_with_layout(
                    egui::vec2(col_w, column_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_width(col_w);
                        ui.set_max_width(col_w);
                        if cell < state.slots.len() {
                            if draw_fx_slot_column(ui, &mut state.slots, cell, grid_metrics)
                                .changed
                            {
                                *changed = true;
                            }
                        } else if cell == state.slots.len() {
                            if draw_add_slot(ui, grid_metrics).clicked() {
                                state.slots.push(EffectSlotUi::from_slot(
                                    &EffectSlot::chorus(),
                                ));
                                *changed = true;
                            }
                        }
                    },
                );
            }
        });
        if row + 1 < rows {
            ui.add_space(gap * 0.5);
        }
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

fn draw_fx_slot_column(
    ui: &mut Ui,
    slots: &mut Vec<EffectSlotUi>,
    idx: usize,
    metrics: FxMetrics,
) -> FxSlotResult {
    let tokens = Tokens::default();
    let mut changed = false;

    let column = ui.vertical(|ui| {
        ui.set_width(metrics.slot_width);
        ui.set_min_height(metrics.column_height);

        let (card_rect, response) = ui.allocate_exact_size(
            egui::vec2(metrics.slot_width, metrics.card_height),
            egui::Sense::click(),
        );

        let active = slots[idx].is_active();
        if ui.is_rect_visible(card_rect) {
            let painter = ui.painter_at(card_rect);
            let stroke = if active { ACCENT_UI } else { tokens.border };
            let fill = if response.hovered() {
                tokens.surface2
            } else {
                tokens.bg_muted
            };
            painter.rect_filled(card_rect, RADIUS_SM, fill);
            painter.rect_stroke(card_rect, RADIUS_SM, egui::Stroke::new(1.0_f32, stroke));
            painter.text(
                egui::pos2(card_rect.min.x + GRID_UNIT, card_rect.min.y + 6.0),
                egui::Align2::LEFT_TOP,
                slots[idx].effect_type.label(),
                FontId::proportional(11.0),
                tokens.text,
            );
            painter.text(
                egui::pos2(card_rect.min.x + GRID_UNIT, card_rect.min.y + 22.0),
                egui::Align2::LEFT_TOP,
                slots[idx].detail(),
                FontId::proportional(10.0),
                tokens.text_muted,
            );
        }

        if response.clicked() {
            slots[idx].bypassed = !slots[idx].bypassed;
            changed = true;
        }

        ui.allocate_ui_with_layout(
            egui::vec2(metrics.slot_width, metrics.controls_height),
            egui::Layout::left_to_right(egui::Align::Center),
            |ui| {
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
                reel_combo(
                    ui,
                    format!("fx_type_{idx}"),
                    select_value_text(slots[idx].effect_type.label()),
                    metrics.slot_width - 56.0,
                    |ui| {
                        for ty in EffectType::ALL {
                            if menu_selectable(
                                ui,
                                slots[idx].effect_type == ty,
                                ty.label(),
                            )
                            .clicked()
                            {
                                let bypassed = slots[idx].bypassed;
                                let mix = slots[idx].mix;
                                slots[idx] = EffectSlotUi::from_slot(&EffectSlot::for_type(ty.clone()));
                                slots[idx].bypassed = bypassed;
                                slots[idx].mix = mix;
                                changed = true;
                            }
                        }
                    },
                );
            },
        );
    });

    let _ = column;
    FxSlotResult { changed }
}

fn draw_add_slot(ui: &mut Ui, metrics: FxMetrics) -> egui::Response {
    let tokens = Tokens::default();
    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(metrics.add_width, metrics.card_height),
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
