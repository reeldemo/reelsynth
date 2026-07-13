use egui::{Rect, Ui};
use reelsynth::Patch;
use reelsynth_ui_theme::Tokens;

use super::*;
use super::footer::draw_piano_toggle;
use crate::widgets::button_ghost;
pub(super) fn draw_header(
    ui: &mut Ui,
    rect: Rect,
    state: &mut UiState,
    midi: &ShellMidiDevices<'_>,
    actions: &mut ShellActions,
) {
    let tokens = Tokens::default();
    ui.allocate_ui_at_rect(rect, |ui| {
        ui.set_min_height(rect.height());
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(SPACE_SM, 0.0))
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_height(rect.height());
                    ui.spacing_mut().item_spacing.x = GRID_UNIT;

                    ui.label(
                        egui::RichText::new("ReelSynth")
                            .font(heading_font(14.0))
                            .color(tokens.text)
                            .extra_letter_spacing(0.04),
                    );

                    ui.add_space(GRID_UNIT);

                    if button_ghost(ui, "Open").clicked() {
                        actions.open_preset = true;
                    }
                    if button_ghost(ui, "Save").clicked() {
                        actions.save_preset = true;
                    }

                    ui.menu_button(header_menu_label("WT"), |ui| {
                        if ui.button("Open .reelwt…").clicked() {
                            actions.import_wt_file = true;
                            ui.close_menu();
                        }
                        if ui.button("Save .reelwt…").clicked() {
                            actions.save_wt_file = true;
                            ui.close_menu();
                        }
                        ui.separator();
                        ui.label(
                            egui::RichText::new("Factory banks")
                                .size(10.0)
                                .color(tokens.text_muted),
                        );
                        for entry in FACTORY_BANKS {
                            if ui.button(entry.label).clicked() {
                                actions.import_factory_wt = Some(entry.id.to_string());
                                ui.close_menu();
                            }
                        }
                        ui.separator();
                        ui.label(
                            egui::RichText::new("Import")
                                .size(10.0)
                                .color(tokens.text_muted),
                        );
                        if ui.button("Vital (.vitaltable)…").clicked() {
                            actions.import_vital_wt = true;
                            ui.close_menu();
                        }
                        if ui.button("WAV folder…").clicked() {
                            actions.import_wav_folder = true;
                            ui.close_menu();
                        }
                        if ui.button("Serum (.fxp)…").clicked() {
                            actions.import_serum_fxp = true;
                            ui.close_menu();
                        }
                    });

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.set_width(ui.available_width());

                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = 6.0;
                            let (dot_rect, _) = ui.allocate_exact_size(
                                egui::vec2(6.0, 6.0),
                                egui::Sense::hover(),
                            );
                            ui.painter_at(dot_rect).circle_filled(
                                dot_rect.center(),
                                3.0,
                                Color32::from_rgb(0x4a, 0xde, 0x80),
                            );
                            ui.label(
                                egui::RichText::new(truncate_status(&state.status, 48))
                                    .font(FontId::monospace(11.0))
                                    .color(tokens.text_muted),
                            );
                        });

                        let toggle = draw_piano_toggle(ui, state.piano_visible);
                        if toggle.clicked() {
                            state.piano_visible = !state.piano_visible;
                        }

                        egui::ComboBox::from_id_source("s1_midi_device")
                            .selected_text(
                                midi.names
                                    .get(midi.selected)
                                    .map(String::as_str)
                                    .unwrap_or("MIDI"),
                            )
                            .width(140.0)
                            .show_ui(ui, |ui| {
                                for (idx, name) in midi.names.iter().enumerate() {
                                    if ui
                                        .selectable_label(midi.selected == idx, name)
                                        .clicked()
                                    {
                                        actions.midi_device_selected = Some(idx);
                                    }
                                }
                            });
                    });
                });
            });
    });
}

fn truncate_status(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_chars.saturating_sub(1)).collect::<String>())
    }
}

fn header_menu_label(label: &str) -> egui::WidgetText {
    egui::RichText::new(label).size(11.0).into()
}

pub(super) fn sync_morph_to_active_tab(state: &mut UiState) {
    let idx = state.osc_tab.min(2);
    state.wt_morph_a = state.osc_morph_a[idx];
    state.wt_morph_b = state.osc_morph_b[idx];
    state.wt_morph_amount = state.osc_morph_amount[idx];
}

pub(super) fn sync_morph_from_active_tab(state: &mut UiState) {
    let idx = state.osc_tab.min(2);
    state.osc_morph_a[idx] = state.wt_morph_a;
    state.osc_morph_b[idx] = state.wt_morph_b;
    state.osc_morph_amount[idx] = state.wt_morph_amount;
}

pub(super) fn sync_wt_position_from_osc(state: &mut UiState) {
    let idx = state.osc_tab.min(2);
    state.wt_position = state.osc_position[idx];
}

pub(super) fn sync_osc_position_from_wt(state: &mut UiState) {
    let idx = state.osc_tab.min(2);
    state.osc_position[idx] = state.wt_position;
}

pub(super) fn draw_osc(ui: &mut Ui, rect: Rect, state: &mut UiState, actions: &mut ShellActions) {
    ui.allocate_ui_at_rect(rect, |ui| {
        let prev_tab = state.osc_tab;
        let result = draw_osc_column(
            ui,
            OscColumnState {
                osc_tab: &mut state.osc_tab,
                osc_type: &mut state.osc_type,
                osc_level: &mut state.osc_level,
                osc_pan: &mut state.osc_pan,
                osc_coarse: &mut state.osc_coarse,
                osc_unison: &mut state.osc_unison,
                osc_position: &mut state.osc_position,
                osc_pulse_width: &mut state.osc_pulse_width,
                osc_warp_mode: &mut state.osc_warp_mode,
                osc_warp_amount: &mut state.osc_warp_amount,
                osc_fm_source: &mut state.osc_fm_source,
                osc_fm_algorithm: &mut state.osc_fm_algorithm,
                osc_fm_ratio: &mut state.osc_fm_ratio,
                osc_fm_index: &mut state.osc_fm_index,
                unison_stereo_spread: &mut state.unison_stereo_spread,
                sub_level: &mut state.sub_level,
                noise_level: &mut state.noise_level,
                macro_values: &mut state.macro_values,
            },
        );
        if state.osc_tab != prev_tab {
            sync_morph_from_active_tab(state);
            sync_wt_position_from_osc(state);
            sync_morph_to_active_tab(state);
        }
        if result.changed {
            sync_wt_position_from_osc(state);
            sync_morph_from_active_tab(state);
            state.wt_morph_amount = morph_amount_for_position(
                state.wt_morph_a,
                state.wt_morph_b,
                state.wt_position,
            );
            state.osc_morph_amount[state.osc_tab.min(2)] = state.wt_morph_amount;
            actions.params_changed = true;
        }
    });
}

