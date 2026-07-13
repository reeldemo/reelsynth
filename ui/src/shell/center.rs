use egui::{Rect, Ui};
use reelsynth::Patch;

use super::*;
use super::header::{sync_morph_from_active_tab, sync_osc_position_from_wt};
use crate::ambient::paint_ambient_waves;
use crate::center_layout::compute_center_regions;
use crate::fx_rack::{draw_effect_rack, EffectRackState};
use crate::layout::{embed_mod_fx_in_center, embed_piano_in_center, UiScale};
use crate::layout_audit::{
    center_fx_used_rect_id, center_mod_used_rect_id, center_morph_used_rect_id,
    center_piano_used_rect_id, center_scope_used_rect_id, center_strip_used_rect_id,
    center_used_rect_id, center_views_used_rect_id,
};
use crate::mod_matrix::{draw_mod_matrix, ModMatrixState};
use crate::region::region;

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
        let embedded = embed_mod_fx_in_center(layout_opts);

        let time = ui.input(|i| i.time);
        let regions = compute_center_regions(
            inner,
            config,
            s,
            embedded,
            embed_piano_in_center(layout_opts),
        );
        let scope_rect = regions.scope;
        let strip_rect = regions.wt_strip;
        let morph_rect = regions.morph;
        let mod_rect = regions.mod_matrix;
        let fx_rect = regions.fx_rack;
        let views_rect = regions.wt_views;
        let piano_rect = regions.piano;

        let bank_name = state.wt_bank_name.clone();

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
            };
            if morph.show(ui).changed {
                sync_osc_position_from_wt(state);
                sync_morph_from_active_tab(state);
                actions.params_changed = true;
            }
            let used = ui.min_rect();
            ui.ctx()
                .data_mut(|d| d.insert_temp(center_morph_used_rect_id(), used));
            });
        }

        // Compact animated WT preview when mod/FX are embedded in center.
        if config.show_wt_editor && views_rect.is_positive() && embedded {
            let views_h = views_rect.height();
            region(ui, views_rect, |ui| {
            ui.painter().rect_filled(views_rect, 8.0, Tokens::default().bg);
            paint_ambient_waves(ui.painter(), views_rect, time);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GRID_UNIT;
                let half_w = (ui.available_width() - GRID_UNIT) * 0.5;
                ui.allocate_ui_with_layout(
                    egui::vec2(half_w, views_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let view = WtView2d {
                            position: state.wt_position,
                            bank: bank.as_deref_mut(),
                            bank_name: Some(bank_name.as_str()),
                            tool: &mut state.wt_edit_tool,
                            animate: true,
                            time: time as f32,
                        };
                        if view.show(ui).frame_edited {
                            actions.frame_edited = true;
                        }
                    },
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(half_w, views_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        WtView3d {
                            position: state.wt_position,
                            bank: bank.as_deref(),
                            time: time as f32,
                        }
                        .show(ui);
                    },
                );
            });
            let used = ui.min_rect();
            ui.ctx()
                .data_mut(|d| d.insert_temp(center_views_used_rect_id(), used));
            });
        } else if config.show_wt_editor && views_rect.is_positive() {
            let views_h = views_rect.height().max(WT_VIEW_MIN_HEIGHT * s * 0.5);
            region(ui, views_rect, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GRID_UNIT;
                let half_w = (ui.available_width() - GRID_UNIT) * 0.5;
                ui.allocate_ui_with_layout(
                    egui::vec2(half_w, views_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let view = WtView2d {
                            position: state.wt_position,
                            bank: bank.as_deref_mut(),
                            bank_name: Some(bank_name.as_str()),
                            tool: &mut state.wt_edit_tool,
                            animate: true,
                            time: time as f32,
                        };
                        if view.show(ui).frame_edited {
                            actions.frame_edited = true;
                        }
                    },
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(half_w, views_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        WtView3d {
                            position: state.wt_position,
                            bank: bank.as_deref(),
                            time: time as f32,
                        }
                        .show(ui);
                    },
                );
            });
            let used = ui.min_rect();
            ui.ctx()
                .data_mut(|d| d.insert_temp(center_views_used_rect_id(), used));
            });
        }

        if embedded && mod_rect.is_positive() && config.show_mod_matrix {
            region(ui, mod_rect, |ui| {
            paint_ambient_waves(ui.painter(), mod_rect, time);
            let result = draw_mod_matrix(
                ui,
                mod_rect,
                ModMatrixState {
                    open: &mut state.mod_matrix_open,
                    routes: &mut state.mod_routes,
                    total_routes: state.mod_route_total,
                },
                scale,
            );
            if result.changed {
                actions.params_changed = true;
            }
            let used = ui.min_rect();
            ui.ctx()
                .data_mut(|d| d.insert_temp(center_mod_used_rect_id(), used));
            });
        }

        if embedded && fx_rect.is_positive() && config.show_fx_rack {
            region(ui, fx_rect, |ui| {
            paint_ambient_waves(ui.painter(), fx_rect, time + 1.5);
            let result = draw_effect_rack(
                ui,
                fx_rect,
                EffectRackState {
                    open: &mut state.fx_rack_open,
                    slots: &mut state.fx_slots,
                },
                scale,
            );
            if result.changed {
                actions.params_changed = true;
            }
            let used = ui.min_rect();
            ui.ctx()
                .data_mut(|d| d.insert_temp(center_fx_used_rect_id(), used));
            });
        }

        if piano_rect.is_positive() && state.piano_visible {
            region(ui, piano_rect, |ui| {
                let inner = ui.max_rect();
                let (_, piano) = PianoKeyboard::new(&state.keys_down).show_in_rect(ui, inner);
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
            let strip = WtStrip {
                position: &mut state.wt_position,
                bank: bank.as_deref(),
                bank_name: Some(bank_name.as_str()),
                visible_frames: 16,
            };
            if strip.show(ui).changed {
                sync_osc_position_from_wt(state);
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

        // Whole-center used rect (after subpanels).
        let used = ui.min_rect();
        ui.ctx()
            .data_mut(|d| d.insert_temp(center_used_rect_id(), used));

        if embedded || config.show_wt_editor {
            ui.ctx().request_repaint();
        }
    });
}
