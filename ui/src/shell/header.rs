use egui::{Rect, Ui};
use reelsynth::Patch;
use reelsynth_ui_theme::Tokens;

use super::*;
use crate::audit_registry::{record_region, record_used, AuditId};
use crate::fx_rack::{draw_effect_rack_sidebar, EffectRackState};
use crate::layout::{osc_column_split_heights, OSC_SIDEBAR_STACK_GAP, UiScale};
use crate::mod_matrix::{draw_mod_matrix_sidebar, ModMatrixState};
use crate::layout_audit::{
    header_left_cluster_rect_id, header_right_cluster_rect_id, header_used_rect_id,
    osc_fx_allocated_rect_id, osc_fx_used_rect_id, osc_mod_allocated_rect_id,
    osc_mod_used_rect_id, osc_used_rect_id,
};
use crate::osc_column::{draw_osc_column, OscColumnInput, OscColumnState};
use crate::region::region;
use crate::state::ShellMode;
use crate::widgets::{button_ghost, button_toggle, menu_action, menu_divider, menu_section_label, menu_selectable, reel_combo, select_value_text, styled_menu_body};
use crate::performance::draw_performance_header;
use crate::state::OscStripContext;
use crate::wt::morph_amount_for_position;

pub(super) fn draw_header(
    ui: &mut Ui,
    rect: Rect,
    state: &mut UiState,
    midi: &ShellMidiDevices<'_>,
    audio: &ShellAudioDevices<'_>,
    actions: &mut ShellActions,
    mut app_settings: Option<&mut ShellAppSettings>,
) {
    let tokens = Tokens::default();
    region(ui, rect, |ui| {
        ui.set_min_height(rect.height());
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(SPACE_SM, 0.0))
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_height(rect.height());
                    ui.spacing_mut().item_spacing.x = GRID_UNIT;

                    let brand = ui.label(
                        egui::RichText::new("ReelSynth")
                            .font(heading_font(14.0))
                            .color(tokens.text)
                            .extra_letter_spacing(0.04),
                    );
                    record_used(ui.ctx(), AuditId::HeaderBrand, brand.rect);

                    ui.add_space(GRID_UNIT);

                    let open = button_ghost(ui, "Open");
                    if open.clicked() {
                        actions.open_preset = true;
                    }
                    record_used(ui.ctx(), AuditId::HeaderOpenBtn, open.rect);

                    let save = button_ghost(ui, "Save");
                    if save.clicked() {
                        actions.save_preset = true;
                    }
                    record_used(ui.ctx(), AuditId::HeaderSaveBtn, save.rect);

                    ui.add_space(GRID_UNIT);

                    let design = button_toggle(ui, "Design", state.shell_mode == ShellMode::Design);
                    if design.clicked() {
                        state.shell_mode = ShellMode::Design;
                    }
                    record_used(ui.ctx(), AuditId::HeaderModeDesign, design.rect);

                    let compose_btn =
                        button_toggle(ui, "Compose", state.shell_mode == ShellMode::Compose);
                    if compose_btn.clicked() {
                        state.shell_mode = ShellMode::Compose;
                        state.compose.ensure_editable_clip();
                    }
                    record_used(ui.ctx(), AuditId::HeaderModeCompose, compose_btn.rect);

                    ui.add_space(GRID_UNIT);
                    let perf_actions = draw_performance_header(ui, state);
                    if perf_actions.params_changed {
                        actions.params_changed = true;
                    }
                    actions.chord_degree_on = perf_actions.chord_degree_on;
                    actions.chord_degree_off = perf_actions.chord_degree_off;

                    let wt_menu = ui.menu_button(header_menu_label("WT"), |ui| {
                        styled_menu_body(ui, |ui| {
                        if menu_action(ui, "Open .reelwt…").clicked() {
                            actions.import_wt_file = true;
                            ui.close_menu();
                        }
                        if menu_action(ui, "Save .reelwt…").clicked() {
                            actions.save_wt_file = true;
                            ui.close_menu();
                        }
                        menu_divider(ui);
                        menu_section_label(ui, "Factory wavetables");
                        for entry in FACTORY_BANKS {
                            if menu_action(ui, entry.label).clicked() {
                                actions.import_factory_wt = Some(entry.id.to_string());
                                ui.close_menu();
                            }
                        }
                        menu_divider(ui);
                        menu_section_label(ui, "Import");
                        if menu_action(ui, "Vital (.vitaltable)…").clicked() {
                            actions.import_vital_wt = true;
                            ui.close_menu();
                        }
                        if menu_action(ui, "WAV folder…").clicked() {
                            actions.import_wav_folder = true;
                            ui.close_menu();
                        }
                        if menu_action(ui, "Serum (.fxp)…").clicked() {
                            actions.import_serum_fxp = true;
                            ui.close_menu();
                        }
                        });
                    });
                    record_used(ui.ctx(), AuditId::HeaderWtMenu, wt_menu.response.rect);

                    if let Some(settings) = app_settings.as_deref_mut() {
                        let settings_menu = ui.menu_button(header_menu_label("Settings"), |ui| {
                            styled_menu_body(ui, |ui| {
                                ui.set_min_width(220.0);
                                menu_section_label(ui, "Overtone");
                                let result = crate::overtone_rack::draw_overtone_chain_menu(
                                    ui,
                                    &mut state.overtone_slots,
                                );
                                if result.changed {
                                    actions.params_changed = true;
                                }
                                menu_divider(ui);
                                menu_section_label(ui, "Graphics");
                                let backend_label = settings.backend_label();
                                reel_combo(
                                    ui,
                                    "settings_graphics_backend",
                                    select_value_text(backend_label),
                                    180.0,
                                    |ui| {
                                        for (i, label) in
                                            ShellAppSettings::BACKEND_LABELS.iter().enumerate()
                                        {
                                            if menu_selectable(
                                                ui,
                                                settings.graphics_backend_idx == i,
                                                *label,
                                            )
                                            .clicked()
                                            {
                                                if settings.graphics_backend_idx != i {
                                                    settings.graphics_backend_idx = i;
                                                    settings.pending_backend_restart = true;
                                                    settings.dirty = true;
                                                }
                                            }
                                        }
                                    },
                                );
                                if ui
                                    .checkbox(&mut settings.gpu_waveforms, "GPU waveforms")
                                    .changed()
                                {
                                    settings.dirty = true;
                                }
                                if settings.pending_backend_restart {
                                    ui.label(
                                        egui::RichText::new(
                                            "Restart required for graphics backend",
                                        )
                                        .size(10.0)
                                        .color(Color32::from_rgb(0xde, 0xa0, 0x4a)),
                                    );
                                }
                                menu_divider(ui);
                                menu_section_label(ui, "Input");
                                if ui
                                    .checkbox(
                                        &mut settings.auto_midi_keyboard,
                                        "Auto-connect MIDI keyboard",
                                    )
                                    .changed()
                                {
                                    settings.dirty = true;
                                }
                                if ui
                                    .checkbox(
                                        &mut settings.auto_audio_output,
                                        "Auto-select new audio output",
                                    )
                                    .changed()
                                {
                                    settings.dirty = true;
                                }
                                let layout_label = settings.layout_label();
                                reel_combo(
                                    ui,
                                    "settings_keyboard_layout",
                                    select_value_text(layout_label),
                                    180.0,
                                    |ui| {
                                        for (i, label) in
                                            ShellAppSettings::LAYOUT_LABELS.iter().enumerate()
                                        {
                                            if menu_selectable(
                                                ui,
                                                settings.keyboard_layout_idx == i,
                                                *label,
                                            )
                                            .clicked()
                                            {
                                                if settings.keyboard_layout_idx != i {
                                                    settings.keyboard_layout_idx = i;
                                                    settings.dirty = true;
                                                }
                                            }
                                        }
                                    },
                                );
                                ui.label(
                                    egui::RichText::new(format!(
                                        "Detected: {}",
                                        settings.detected_keyboard_label
                                    ))
                                    .size(10.0)
                                    .color(tokens.text_muted),
                                );
                            });
                        });
                        record_used(
                            ui.ctx(),
                            AuditId::HeaderSettingsMenu,
                            settings_menu.response.rect,
                        );
                    } else {
                        let overtone_menu = ui.menu_button(header_menu_label("Overtone"), |ui| {
                            styled_menu_body(ui, |ui| {
                                let result = crate::overtone_rack::draw_overtone_chain_menu(
                                    ui,
                                    &mut state.overtone_slots,
                                );
                                if result.changed {
                                    actions.params_changed = true;
                                }
                            });
                        });
                        let _ = overtone_menu;
                    }

                    let left_cluster = {
                        let r = ui.min_rect();
                        // Cursor is past the last left control; prefer that over min_rect.max.x
                        // so popups / wide child allocations don't inflate the cluster.
                        Rect::from_min_max(r.min, egui::pos2(ui.cursor().min.x, r.max.y))
                    };
                    ui.ctx().data_mut(|d| {
                        d.insert_temp(header_left_cluster_rect_id(), left_cluster);
                    });
                    record_region(
                        ui.ctx(),
                        AuditId::HeaderLeftCluster,
                        left_cluster,
                        left_cluster,
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Cap to remaining space but size the audit cluster to content,
                        // not the full leftover strip (which falsely overlaps the left).
                        ui.set_max_width(ui.available_width().max(0.0));

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
                            let _status = ui.label(
                                egui::RichText::new(truncate_status(&state.status, 32))
                                    .font(FontId::monospace(11.0))
                                    .color(tokens.text_muted),
                            );
                        });

                        let toggle = button_toggle(ui, "Piano", state.piano_visible);
                        if toggle.clicked() {
                            state.piano_visible = !state.piano_visible;
                        }
                        record_used(ui.ctx(), AuditId::HeaderPianoToggle, toggle.rect);

                        let midi_label = midi
                            .names
                            .get(midi.selected)
                            .map(String::as_str)
                            .unwrap_or("MIDI");
                        let midi_resp = reel_combo(
                            ui,
                            "s1_midi_device",
                            select_value_text(midi_label),
                            120.0,
                            |ui| {
                                for (idx, name) in midi.names.iter().enumerate() {
                                    if menu_selectable(ui, midi.selected == idx, name).clicked() {
                                        actions.midi_device_selected = Some(idx);
                                    }
                                }
                            },
                        );
                        record_region(
                            ui.ctx(),
                            AuditId::HeaderMidiCombo,
                            midi_resp.response.rect,
                            midi_resp.response.rect,
                        );

                        let audio_label = audio
                            .names
                            .get(audio.selected)
                            .map(String::as_str)
                            .unwrap_or("Audio");
                        let audio_resp = reel_combo(
                            ui,
                            "s1_audio_device",
                            select_value_text(audio_label),
                            120.0,
                            |ui| {
                                if audio.names.is_empty() {
                                    let _ = menu_selectable(ui, true, "No output devices");
                                } else {
                                    for (idx, name) in audio.names.iter().enumerate() {
                                        if menu_selectable(ui, audio.selected == idx, name)
                                            .clicked()
                                        {
                                            actions.audio_device_selected = Some(idx);
                                        }
                                    }
                                }
                            },
                        );
                        record_region(
                            ui.ctx(),
                            AuditId::HeaderAudioCombo,
                            audio_resp.response.rect,
                            audio_resp.response.rect,
                        );

                        let right_cluster = ui.min_rect();
                        ui.ctx().data_mut(|d| {
                            d.insert_temp(header_right_cluster_rect_id(), right_cluster);
                        });
                        record_region(
                            ui.ctx(),
                            AuditId::HeaderRightCluster,
                            right_cluster,
                            right_cluster,
                        );
                    });
                });
            });
        let used = ui.min_rect();
        ui.ctx()
            .data_mut(|d| d.insert_temp(header_used_rect_id(), used));
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
    let active = state.active_osc().clone();
    state.wt_morph_a = active.morph_a;
    state.wt_morph_b = active.morph_b;
    state.wt_morph_amount = active.morph_amount;
}

pub(super) fn sync_morph_from_active_tab(state: &mut UiState) {
    let idx = state.active_osc_index();
    let morph_a = state.wt_morph_a;
    let morph_b = state.wt_morph_b;
    let morph_amount = state.wt_morph_amount;
    state.oscillators[idx].morph_a = morph_a;
    state.oscillators[idx].morph_b = morph_b;
    state.oscillators[idx].morph_amount = morph_amount;
}

pub(super) fn sync_wt_from_osc(state: &mut UiState, num_frames: usize) {
    use crate::wt::position_from_osc_ui;

    let idx = state.active_osc_index();
    state.wt_position = position_from_osc_ui(&state.oscillators[idx], num_frames);
}

pub(super) fn sync_osc_from_wt(state: &mut UiState, num_frames: usize) {
    use crate::wt::sync_slot_from_position;

    let idx = state.active_osc_index();
    state.oscillators[idx].position = state.wt_position;
    sync_slot_from_position(&mut state.oscillators[idx], num_frames);
}

/// Legacy alias — sync position + slot from wt_position.
pub(super) fn sync_osc_position_from_wt(state: &mut UiState) {
    sync_osc_from_wt(state, 256);
}

/// Legacy alias — sync wt_position from active osc slots.
#[allow(dead_code)]
pub(super) fn sync_wt_position_from_osc(state: &mut UiState) {
    sync_wt_from_osc(state, 256);
}

pub(super) fn draw_osc(
    ui: &mut Ui,
    rect: Rect,
    state: &mut UiState,
    preview_patch: &Patch,
    osc_preview: Option<OscStripContext<'_>>,
    config: &ShellConfig,
    actions: &mut ShellActions,
    scale: UiScale,
) {
    region(ui, rect, |ui| {
        let s = scale.ui();
        let stack = osc_column_split_heights(
            rect.height(),
            s,
            state.fx_slots.len(),
            config.show_fx_rack,
            config.show_mod_matrix,
        );

        let mut y = rect.min.y;
        let osc_rect = Rect::from_min_max(rect.min, egui::pos2(rect.max.x, y + stack.osc));
        y += stack.osc;
        let fx_rect = if stack.fx > 0.0 {
            let r = Rect::from_min_max(
                egui::pos2(rect.min.x, y),
                egui::pos2(rect.max.x, y + stack.fx),
            );
            y += stack.fx;
            r
        } else {
            Rect::NOTHING
        };
        let mod_rect = if stack.mod_matrix > 0.0 {
            let mod_top = y
                + if fx_rect.is_positive() {
                    OSC_SIDEBAR_STACK_GAP * s
                } else {
                    0.0
                };
            Rect::from_min_max(
                egui::pos2(rect.min.x, mod_top),
                egui::pos2(rect.max.x, mod_top + stack.mod_matrix),
            )
        } else {
            Rect::NOTHING
        };

        region(ui, osc_rect, |ui| {
            egui::ScrollArea::vertical()
                .id_salt("osc_column_scroll")
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let prev_tab = state.osc_tab;
                    let result = draw_osc_column(
                        ui,
                        OscColumnState {
                            oscillators: &mut state.oscillators,
                            osc_tab: &mut state.osc_tab,
                            unison_stereo_spread: &mut state.unison_stereo_spread,
                            sub_level: &mut state.sub_level,
                            noise_level: &mut state.noise_level,
                            macro_values: &mut state.macro_values,
                            selected_layer_idx: &mut state.selected_layer_idx,
                        },
                        OscColumnInput {
                            patch: preview_patch,
                            preview: osc_preview,
                        },
                        s,
                    );
                    if state.osc_tab != prev_tab {
                        sync_morph_from_active_tab(state);
                        sync_wt_from_osc(state, 256);
                        sync_morph_to_active_tab(state);
                    }
                    if result.changed {
                        sync_wt_from_osc(state, 256);
                        sync_morph_from_active_tab(state);
                        state.wt_morph_amount = morph_amount_for_position(
                            state.wt_morph_a,
                            state.wt_morph_b,
                            state.wt_position,
                        );
                        let idx = state.active_osc_index();
                        state.oscillators[idx].morph_amount = state.wt_morph_amount;
                        actions.params_changed = true;
                    }
                    if result.osc_count_changed {
                        actions.params_changed = true;
                    }
                });
        });

        if fx_rect.is_positive() {
            let fx_result = draw_effect_rack_sidebar(
                ui,
                fx_rect,
                EffectRackState {
                    open: &mut state.fx_rack_open,
                    slots: &mut state.fx_slots,
                },
                scale,
            );
            if fx_result.changed {
                actions.params_changed = true;
            }
            let used = ui.min_rect().intersect(fx_rect);
            ui.ctx().data_mut(|d| {
                d.insert_temp(osc_fx_allocated_rect_id(), fx_rect);
                d.insert_temp(osc_fx_used_rect_id(), used);
            });
            record_region(ui.ctx(), AuditId::OscFxPanel, fx_rect, used);
        }


        if mod_rect.is_positive() {
            let mod_result = draw_mod_matrix_sidebar(
                ui,
                mod_rect,
                ModMatrixState {
                    open: &mut state.mod_matrix_open,
                    routes: &mut state.mod_routes,
                    total_routes: state.mod_route_total,
                },
                scale,
            );
            if mod_result.changed {
                actions.params_changed = true;
            }
            let used = ui.min_rect().intersect(mod_rect);
            ui.ctx().data_mut(|d| {
                d.insert_temp(osc_mod_allocated_rect_id(), mod_rect);
                d.insert_temp(osc_mod_used_rect_id(), used);
            });
            record_region(ui.ctx(), AuditId::OscModPanel, mod_rect, used);
        }

        let used = ui.min_rect().intersect(rect);
        ui.ctx().data_mut(|d| d.insert_temp(osc_used_rect_id(), used));
        record_region(ui.ctx(), AuditId::OscColumn, rect, used);
    });
}
