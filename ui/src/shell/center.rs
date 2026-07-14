use egui::{Rect, Ui};
use reelsynth::Patch;

use super::*;
use super::header::{sync_morph_from_active_tab, sync_osc_from_wt, sync_wt_from_osc};
use crate::ambient::paint_ambient_waves;
use crate::center_layout::compute_center_regions;
use crate::layout::{embed_piano_in_center, ShellLayoutOptions, UiScale};
use crate::layout_audit::{
    center_morph_used_rect_id, center_piano_used_rect_id, center_scope_used_rect_id,
    center_strip_used_rect_id, center_used_rect_id, center_views_used_rect_id,
};
use crate::region::region;
use crate::wt::resolved_slots_for_ui;

pub(super) fn draw_center(
    ui: &mut Ui,
    rect: Rect,
    state: &mut UiState,
    mut bank: Option<&mut WavetableBank>,
    preview_patch: &Patch,
    config: &ShellConfig,
    scope: Option<ScopeStripContext<'_>>,
    actions: &mut ShellActions,
    scale: UiScale,
) {
    region(ui, rect, |ui| {
        let s = scale.ui();
        let inner = rect.shrink(SPACE_SM * s * 0.75);
        let layout_opts = ShellLayoutOptions {
            piano_visible: state.piano_visible,
            show_osc_column: config.show_osc_column,
            show_mod_matrix: config.show_mod_matrix,
            mod_matrix_open: state.mod_matrix_open,
            show_fx_rack: config.show_fx_rack,
            fx_rack_open: state.fx_rack_open,
        };
        let piano_in_center = embed_piano_in_center(layout_opts);

        let time = ui.input(|i| i.time);
        let regions = compute_center_regions(inner, config, s, piano_in_center);
        let scope_rect = regions.scope;
        let strip_rect = regions.wt_strip;
        let morph_rect = regions.morph;
        let views_rect = regions.wt_views;
        let piano_rect = regions.piano;

        let bank_name = state.wt_bank_name.clone();
        let num_frames = bank.as_ref().map(|b| b.num_frames).unwrap_or(256);
        let active_idx = state.active_osc_index();
        let resolved_slots = resolved_slots_for_ui(state.active_osc(), num_frames);
        let wave_quant = state.oscillators[active_idx].wave_quant;

        if scope_rect.is_positive() {
            region(ui, scope_rect, |ui| {
            if let Some(ctx) = scope {
                draw_scope_strip(
                    ui,
                    scope_rect,
                    ScopeStripInput {
                        patch: preview_patch,
                        banks: ctx.banks,
                        bank_for_osc: ctx.bank_for_osc,
                        live: ctx.live,
                        is_playing: ctx.is_playing,
                        now_secs: ctx.now_secs,
                        state: ctx.state,
                    },
                );
            } else if let Some(b) = bank.as_deref() {
                let bank_for_osc: &dyn Fn(usize) -> usize = &|_| 0;
                let mut strip_state = ScopeStripState::default();
                draw_scope_strip(
                    ui,
                    scope_rect,
                    ScopeStripInput {
                        patch: preview_patch,
                        banks: std::slice::from_ref(b),
                        bank_for_osc: &bank_for_osc,
                        live: None,
                        is_playing: false,
                        now_secs: time,
                        state: &mut strip_state,
                    },
                );
            }
            let used = ui.min_rect();
            ui.ctx()
                .data_mut(|d| d.insert_temp(center_scope_used_rect_id(), used));
            });
        }

        if config.show_wt_editor && morph_rect.is_positive() {
            region(ui, morph_rect, |ui| {
            let morph = WtMorph {
                frame_a: &mut state.wt_morph_a,
                frame_b: &mut state.wt_morph_b,
                amount: &mut state.wt_morph_amount,
                position: &mut state.wt_position,
                wave_quant,
                wave_slots: &resolved_slots,
            };
            if morph.show(ui).changed {
                sync_osc_from_wt(state, num_frames);
                sync_morph_from_active_tab(state);
                actions.params_changed = true;
            }
            let used = ui.min_rect();
            ui.ctx()
                .data_mut(|d| d.insert_temp(center_morph_used_rect_id(), used));
            });
        }

        if config.show_wt_editor && views_rect.is_positive() {
            let views_h = views_rect.height().max(WT_VIEW_MIN_HEIGHT * s * 0.5);
            region(ui, views_rect, |ui| {
            ui.painter().rect_filled(views_rect, 8.0, Tokens::default().bg);
            if config.show_osc_column {
                paint_ambient_waves(ui.painter(), views_rect, time);
            }
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GRID_UNIT;
                let half_w = (ui.available_width() - GRID_UNIT) * 0.5;
                ui.allocate_ui_with_layout(
                    egui::vec2(half_w, views_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let view = WtView2d {
                            position: &mut state.wt_position,
                            bank: bank.as_deref_mut(),
                            bank_name: Some(bank_name.as_str()),
                            tool: &mut state.wt_edit_tool,
                            morph_amount: Some(&mut state.wt_morph_amount),
                            patch: Some(preview_patch),
                            macro_values: Some(&state.macro_values),
                            animate: true,
                            time: time as f32,
                        };
                        let view2d_resp = view.show(ui);
                        if view2d_resp.frame_edited {
                            actions.frame_edited = true;
                        }
                        if view2d_resp.changed() {
                            if view2d_resp.morph_changed {
                                state.wt_position = morph_position(
                                    state.wt_morph_a,
                                    state.wt_morph_b,
                                    state.wt_morph_amount,
                                );
                            } else if view2d_resp.position_changed {
                                state.wt_morph_amount = morph_amount_for_position(
                                    state.wt_morph_a,
                                    state.wt_morph_b,
                                    state.wt_position,
                                );
                            }
                            sync_osc_from_wt(state, num_frames);
                            sync_morph_from_active_tab(state);
                            actions.params_changed = true;
                        }
                    },
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(half_w, views_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let view3d = WtView3d {
                            position: &mut state.wt_position,
                            bank: bank.as_deref(),
                            morph_amount: Some(&mut state.wt_morph_amount),
                            time: time as f32,
                        };
                        let view3d_resp = view3d.show(ui);
                        if view3d_resp.changed() {
                            if view3d_resp.morph_changed {
                                state.wt_position = morph_position(
                                    state.wt_morph_a,
                                    state.wt_morph_b,
                                    state.wt_morph_amount,
                                );
                            } else if view3d_resp.position_changed {
                                state.wt_morph_amount = morph_amount_for_position(
                                    state.wt_morph_a,
                                    state.wt_morph_b,
                                    state.wt_position,
                                );
                            }
                            sync_osc_from_wt(state, num_frames);
                            sync_morph_from_active_tab(state);
                            actions.params_changed = true;
                        }
                    },
                );
            });
            let used = ui.min_rect();
            ui.ctx()
                .data_mut(|d| d.insert_temp(center_views_used_rect_id(), used));
            });
        }

        if piano_rect.is_positive() && state.piano_visible {
            region(ui, piano_rect, |ui| {
                let inner = ui.max_rect();
                let (_, piano) = PianoKeyboard::compact(&state.keys_down).show_in_rect(ui, inner);
                if let Some(n) = piano.note_on {
                    actions.note_on = Some(n);
                }
                if let Some(n) = piano.note_off {
                    actions.note_off = Some(n);
                }
                let used = ui.min_rect();
                ui.ctx()
                    .data_mut(|d| d.insert_temp(center_piano_used_rect_id(), used));
            });
        }

        if strip_rect.is_positive() {
            region(ui, strip_rect, |ui| {
            let idx = state.active_osc_index();
            let osc = &mut state.oscillators[idx];
            let strip = WtStrip {
                position: &mut state.wt_position,
                wave_quant: osc.wave_quant,
                wave_slot: &mut osc.wave_slot,
                wave_slot_fine: &mut osc.wave_slot_fine,
                wave_slots: &resolved_slots,
                bank: bank.as_deref(),
                bank_name: Some(bank_name.as_str()),
                visible_frames: 16,
            };
            if strip.show(ui).changed {
                sync_osc_from_wt(state, num_frames);
                state.wt_morph_amount =
                    morph_amount_for_position(state.wt_morph_a, state.wt_morph_b, state.wt_position);
                sync_morph_from_active_tab(state);
                actions.params_changed = true;
            }
            let used = ui.min_rect();
            ui.ctx()
                .data_mut(|d| d.insert_temp(center_strip_used_rect_id(), used));
            });
        }

        let used = ui.min_rect();
        ui.ctx()
            .data_mut(|d| d.insert_temp(center_used_rect_id(), used));

        if config.show_osc_column || config.show_wt_editor {
            ui.ctx().request_repaint();
        }
    });
}
