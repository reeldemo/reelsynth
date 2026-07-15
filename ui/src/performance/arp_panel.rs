//! Compact arpeggiator controls for performance footer / header.

use egui::Ui;
use reelsynth_ui_theme::Tokens;

use crate::widgets::{button_toggle, menu_divider, menu_section_label, menu_selectable, reel_combo, select_value_text, styled_menu_body};
use crate::UiState;

use super::{ArpUi, INPUT_MODE_NAMES, RATE_NAMES, STYLE_NAMES};

pub struct ArpPanelActions {
    pub params_changed: bool,
}

impl Default for ArpPanelActions {
    fn default() -> Self {
        Self {
            params_changed: false,
        }
    }
}

fn arp_summary(arp: &ArpUi) -> String {
    if !arp.enabled {
        return "Off".into();
    }
    let style = STYLE_NAMES[arp.direction.min(STYLE_NAMES.len().saturating_sub(1))];
    let rate = RATE_NAMES[arp.rate.min(RATE_NAMES.len().saturating_sub(1))];
    format!("{style} · {rate}")
}

/// Arp on/off toggle plus settings dropdown.
pub fn draw_arp_panel(ui: &mut Ui, state: &mut UiState) -> ArpPanelActions {
    let tokens = Tokens::default();
    let mut actions = ArpPanelActions::default();
    let arp = &mut state.performance.arp;

    if button_toggle(ui, "Arp", arp.enabled).clicked() {
        arp.enabled = !arp.enabled;
        actions.params_changed = true;
    }

    ui.label(
        egui::RichText::new("Arp")
            .size(10.0)
            .color(tokens.text_muted),
    );
    let summary = arp_summary(arp);
    reel_combo(ui, "arp_settings", select_value_text(&summary), 108.0, |ui| {
        styled_menu_body(ui, |ui| {
            let arp = &mut state.performance.arp;

            menu_section_label(ui, "Input");
            for (idx, name) in INPUT_MODE_NAMES.iter().enumerate() {
                if menu_selectable(ui, arp.input_mode == idx, name).clicked() {
                    arp.input_mode = idx;
                    actions.params_changed = true;
                }
            }

            menu_divider(ui);
            menu_section_label(ui, "Style");
            for (idx, name) in STYLE_NAMES.iter().enumerate() {
                if menu_selectable(ui, arp.direction == idx, name).clicked() {
                    arp.direction = idx;
                    actions.params_changed = true;
                }
            }

            menu_divider(ui);
            menu_section_label(ui, "Rate");
            for (idx, name) in RATE_NAMES.iter().enumerate() {
                if menu_selectable(ui, arp.rate == idx, name).clicked() {
                    arp.rate = idx;
                    actions.params_changed = true;
                }
            }

            menu_divider(ui);
            menu_section_label(ui, "Octaves");
            for oct in 1..=4u8 {
                let label = oct.to_string();
                if menu_selectable(ui, arp.octave_spread == oct, &label).clicked() {
                    arp.octave_spread = oct;
                    actions.params_changed = true;
                }
            }

            menu_divider(ui);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Gate")
                        .size(10.0)
                        .color(tokens.text_muted),
                );
                if ui
                    .add(
                        egui::Slider::new(&mut arp.gate, 0.1..=1.0)
                            .show_value(false)
                            .fixed_decimals(2),
                    )
                    .changed()
                {
                    actions.params_changed = true;
                }
            });

            if menu_selectable(ui, arp.latch, "Latch (hold after release)").clicked() {
                arp.latch = !arp.latch;
                actions.params_changed = true;
            }
        });
    });

    actions
}
