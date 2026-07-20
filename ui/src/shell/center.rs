use egui::{Rect, Ui};
use reelsynth::Patch;

use super::*;
use super::header::{sync_morph_from_active_tab, sync_osc_from_wt};
use crate::audit_registry::{record_region, AuditId};
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
        let layer_first = state.shell_mode == ShellMode::Design && config.show_wt_editor;
        let regions = compute_center_regions(inner, config, s, piano_in_center, layer_first);
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
            record_region(ui.ctx(), AuditId::CenterScope, scope_rect, used);
            });
        }

        if config.show_wt_editor
            && morph_rect.is_positive()
            && state.shell_mode != ShellMode::Design
        {
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
            let used = ui.min_rect().intersect(morph_rect);
            ui.ctx()
                .data_mut(|d| d.insert_temp(center_morph_used_rect_id(), used));
            record_region(ui.ctx(), AuditId::CenterWtMorph, morph_rect, used);
            });
        }

        if config.show_wt_editor && views_rect.is_positive() {
            let views_h = views_rect.height().max(WT_VIEW_MIN_HEIGHT * s * 0.5);
            crate::wt::set_quant_seam_mode(state.wt_quant_seam);
            crate::wt::set_crackle_amount(state.patch_crackle);
            region(ui, views_rect, |ui| {
            ui.set_width(views_rect.width());
            ui.painter().rect_filled(views_rect, 8.0, Tokens::default().bg);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GRID_UNIT;
                let idx = state.active_osc_index();
                let col_w = ((ui.available_width() - GRID_UNIT * 2.0 - 4.0) / 3.0)
                    .floor()
                    .max(48.0);

                // Col 1 — Result
                ui.allocate_ui_with_layout(
                    egui::vec2(col_w, views_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let osc = &mut state.oscillators[idx];
                        let wave_quant = osc.wave_quant;
                        let view = WtViewResult {
                            wt_position: &state.wt_position,
                            bank: bank.as_deref_mut(),
                            wave_layers: &mut osc.wave_layers,
                            selected_layer_idx: &mut state.selected_layer_idx,
                            stack_mode: &mut osc.stack_mode,
                            wave_quant,
                            wavetable_id: Some(bank_name.clone()),
                            active_osc: idx,
                            curve_view: &mut state.wt_curve_view,
                        };
                        let resp = view.show(ui);
                        if resp.frame_edited {
                            actions.frame_edited = true;
                        }
                        if let Some(hint) = resp.status_hint.as_deref() {
                            state.status = hint.to_string();
                        }
                        if resp.stack_changed {
                            sync_osc_from_wt(state, num_frames);
                            sync_morph_from_active_tab(state);
                            actions.params_changed = true;
                        }
                    },
                );

                // Col 2 — Layers (bank borrow released from col 1)
                ui.allocate_ui_with_layout(
                    egui::vec2(col_w, views_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let osc = &mut state.oscillators[idx];
                        let stack_mode = osc.stack_mode.clone();
                        let wave_quant = osc.wave_quant;
                        let view_stack = WtView3dStack {
                            layers: &mut osc.wave_layers,
                            stack_mode: &stack_mode,
                            bank: bank.as_deref_mut(),
                            wt_pos_offset: 0.0,
                            wt_position: &mut state.wt_position,
                            selected_layer: &mut state.selected_layer_idx,
                            view_mode: Some(&mut state.wt_view_3d_mode),
                            show_mode_toggle: false,
                            active_osc: idx,
                            time: 0.0,
                            wave_quant,
                            curve_view: &mut state.wt_curve_view,
                        };
                        let stack_resp = view_stack.show(ui);
                        if stack_resp.frame_edited {
                            actions.frame_edited = true;
                        }
                        if let Some(hint) = stack_resp.status_hint.as_deref() {
                            state.status = hint.to_string();
                        }
                        if stack_resp.layer_selected || stack_resp.wt_position_changed {
                            sync_osc_from_wt(state, num_frames);
                            sync_morph_from_active_tab(state);
                            actions.params_changed = true;
                        }
                    },
                );

                // Col 3 — Selected layer edit
                ui.allocate_ui_with_layout(
                    egui::vec2(col_w, views_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let osc = &mut state.oscillators[idx];
                        let wave_quant = osc.wave_quant;
                        let view_sel = WtSelectedLayerView {
                            wt_position: &mut state.wt_position,
                            bank: bank.as_deref_mut(),
                            tool: &mut state.wt_edit_tool,
                            wave_quant,
                            quant_interp: &mut state.wt_quant_interp,
                            selected_quant_slot: &mut state.selected_quant_slot,
                            wave_slot: &mut osc.wave_slot,
                            wave_slots: &mut osc.wave_slots,
                            wave_layers: &mut osc.wave_layers,
                            selected_layer_idx: &mut state.selected_layer_idx,
                            shape_control_points: state.shape_control_points,
                            analyze_dialog_open: Some(&mut state.analyze_dialog_open),
                            curve_view: &mut state.wt_curve_view,
                            quant_seam: &mut state.wt_quant_seam,
                            patch_crackle: &mut state.patch_crackle,
                        };
                        let sel_resp = view_sel.show(ui);
                        if sel_resp.frame_edited {
                            actions.frame_edited = true;
                        }
                        if let Some(hint) = sel_resp.status_hint.as_deref() {
                            state.status = hint.to_string();
                        }
                        if sel_resp.stack_changed || sel_resp.analyze_requested {
                            sync_osc_from_wt(state, num_frames);
                            sync_morph_from_active_tab(state);
                            actions.params_changed = true;
                        }
                        if sel_resp.params_changed {
                            actions.params_changed = true;
                        }
                    },
                );
            });
            let used = ui.min_rect().intersect(views_rect);
            ui.ctx()
                .data_mut(|d| d.insert_temp(center_views_used_rect_id(), used));
            record_region(ui.ctx(), AuditId::CenterWtViews, views_rect, used);
            });
        }

        if state.analyze_dialog_open {
            let dialog_before = ui.min_rect();
            if draw_analyze_dialog(ui, state, bank.as_deref(), num_frames) {
                actions.params_changed = true;
            }
            record_region(
                ui.ctx(),
                AuditId::CenterWt2dAnalyzeDialog,
                dialog_before,
                ui.min_rect(),
            );
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
                record_region(ui.ctx(), AuditId::CenterPiano, piano_rect, used);
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
                edit_tool: state.wt_edit_tool,
                wave_layers: &mut osc.wave_layers,
                selected_layer_idx: &mut state.selected_layer_idx,
                strip_mode: if state.shell_mode == ShellMode::Design {
                    StripMode::Layers
                } else {
                    StripMode::Frames
                },
                show_layer_chips: true,
            };
            let strip_resp = strip.show(ui);
            if strip_resp.changed {
                sync_osc_from_wt(state, num_frames);
                state.wt_morph_amount =
                    morph_amount_for_position(state.wt_morph_a, state.wt_morph_b, state.wt_position);
                sync_morph_from_active_tab(state);
                actions.params_changed = true;
            }
            if strip_resp.params_changed {
                actions.params_changed = true;
            }
            let used = ui.min_rect().intersect(strip_rect);
            ui.ctx()
                .data_mut(|d| d.insert_temp(center_strip_used_rect_id(), used));
            record_region(ui.ctx(), AuditId::CenterWtStrip, strip_rect, used);
            });
        }

        let used = ui.min_rect();
        ui.ctx()
            .data_mut(|d| d.insert_temp(center_used_rect_id(), used));
        record_region(ui.ctx(), AuditId::CenterColumn, rect, used);
    });
}

fn draw_analyze_dialog(
    ui: &mut Ui,
    state: &mut UiState,
    bank: Option<&WavetableBank>,
    num_frames: usize,
) -> bool {
    use crate::oscillator_ui::WaveLayerUi;
    use crate::wt::frame_index;
    use reelsynth::decompose_frame;

    let Some(bank) = bank else {
        state.analyze_dialog_open = false;
        return false;
    };

    let frame_idx = frame_index(state.wt_position, num_frames.max(1));
    let frame_data = bank.frame(frame_idx);
    if frame_data.len() != 2048 {
        state.analyze_dialog_open = false;
        return false;
    }
    let mut frame = [0.0f32; 2048];
    frame.copy_from_slice(frame_data);

    let mut changed = false;
    let mut should_close = false;
    egui::Window::new("Analyze → Stack")
        .collapsible(false)
        .resizable(false)
        .show(ui.ctx(), |ui| {
            ui.label(format!("Decompose frame {frame_idx} into sine wave layers"));
            ui.add(
                egui::Slider::new(&mut state.analyze_harmonics, 1..=32).text("Harmonics"),
            );
            ui.add(
                egui::Slider::new(&mut state.analyze_min_mag, 0.001..=0.1)
                    .logarithmic(true)
                    .text("Min magnitude"),
            );
            ui.checkbox(&mut state.analyze_append, "Append (instead of replace)");
            ui.horizontal(|ui| {
                if ui.button("Analyze").clicked() {
                    let decomposed =
                        decompose_frame(&frame, state.analyze_harmonics, state.analyze_min_mag);
                    let new_layers: Vec<WaveLayerUi> = decomposed
                        .into_iter()
                        .map(|l| WaveLayerUi {
                            source_type: l.source_type,
                            level: l.level,
                            detune: l.detune,
                            phase: l.phase,
                            enabled: true,
                            ..WaveLayerUi::default()
                        })
                        .collect();
                    let idx = state.active_osc_index();
                    if state.analyze_append {
                        state.oscillators[idx].wave_layers.extend(new_layers);
                    } else {
                        state.oscillators[idx].wave_layers = new_layers;
                    }
                    state.oscillators[idx].stack_mode = "add".into();
                    state.wt_view_3d_mode = WtView3dMode::Stack;
                    should_close = true;
                    changed = true;
                }
                if ui.button("Cancel").clicked() {
                    should_close = true;
                }
            });
        });
    if should_close {
        state.analyze_dialog_open = false;
    }
    changed
}
