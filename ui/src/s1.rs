use std::collections::HashSet;

use egui::{Color32, FontId, Rect, Ui};
use reelsynth::{Patch, ScopeLiveTaps, WavetableBank};
use reelsynth_ui_theme::{heading_font, Tokens};

use crate::layout::{
    S1Layout, S1LayoutOptions, GRID_UNIT, SPACE_SM, WT_MORPH_HEIGHT,
    WT_STRIP_HEIGHT, WT_VIEW_MIN_HEIGHT,
};
use crate::fx_rack::{draw_fx_rack, default_fx_slots, FxRackState, FxSlotUi};
use crate::mod_matrix::{draw_mod_matrix, default_mod_routes, ModMatrixState, ModRouteUi};
use crate::osc::{draw_osc_column, osc_type_index, warp_mode_index, OscColumnState};
use crate::widgets::{
    adsr_graph, format_depth, format_env_time, format_lfo_rate, format_sustain, tab_bar, Knob,
    KnobSize, KnobStyle, PianoKeyboard, panel, panel_disabled,
};
use crate::scope::{draw_scope_strip, ScopeStripInput, ScopeStripState, SCOPE_STRIP_HEIGHT};
use crate::wt::{morph_amount_for_position, WtEditTool, WtMorph, WtStrip, WtView2d, WtView3d, FACTORY_BANKS};

#[derive(Debug, Clone, Copy, Default)]
pub struct S1ShellConfig {
    /// S2+: show 2D waveform + 3D mesh panels in center column.
    pub show_wt_editor: bool,
    /// S3+: reveal 280px osc column (Osc1–3) and live ADSR/LFO rail.
    pub show_osc_column: bool,
    /// S4+: modulation matrix section below main columns.
    pub show_mod_matrix: bool,
    /// S5+: FX rack section below mod matrix.
    pub show_fx_rack: bool,
}

#[derive(Default)]
pub struct S1Actions {
    pub params_changed: bool,
    pub note_on: Option<u8>,
    pub note_off: Option<u8>,
    pub open_preset: bool,
    pub save_preset: bool,
    pub import_wt_file: bool,
    pub save_wt_file: bool,
    pub import_factory_wt: Option<String>,
    pub import_vital_wt: bool,
    pub import_wav_folder: bool,
    pub import_serum_fxp: bool,
    pub frame_edited: bool,
    pub midi_device_selected: Option<usize>,
}

pub struct S1MidiDevices<'a> {
    pub names: &'a [String],
    pub selected: usize,
}

pub struct S1State {
    pub wt_position: f32,
    pub osc_position: [f32; 3],
    pub wt_bank_name: String,
    pub wt_edit_tool: WtEditTool,
    pub wt_morph_a: f32,
    pub wt_morph_b: f32,
    pub wt_morph_amount: f32,
    pub osc_morph_a: [f32; 3],
    pub osc_morph_b: [f32; 3],
    pub osc_morph_amount: [f32; 3],
    pub osc_tab: usize,
    pub osc_type: [usize; 3],
    pub osc_level: [f32; 3],
    pub osc_pan: [f32; 3],
    pub osc_coarse: [f32; 3],
    pub osc_unison: [u32; 3],
    pub osc_pulse_width: [f32; 3],
    pub osc_warp_mode: [usize; 3],
    pub osc_warp_amount: [f32; 3],
    pub osc_fm_source: [usize; 3],
    pub osc_fm_algorithm: [usize; 3],
    pub osc_fm_ratio: [f32; 3],
    pub osc_fm_index: [f32; 3],
    pub unison_stereo_spread: f32,
    pub sub_level: f32,
    pub noise_level: f32,
    pub macro_values: [f32; 4],
    pub filter_cutoff: f32,
    pub filter_resonance: f32,
    pub filter_key_tracking: f32,
    pub filter_drive: f32,
    pub filter2_cutoff: f32,
    pub filter2_resonance: f32,
    pub filter2_mode: usize,
    pub filter2_drive: f32,
    pub filter_mode: usize,
    pub env_attack: f32,
    pub env_decay: f32,
    pub env_sustain: f32,
    pub env_release: f32,
    pub filt_env_attack: f32,
    pub filt_env_decay: f32,
    pub filt_env_sustain: f32,
    pub filt_env_release: f32,
    pub lfo_rate: f32,
    pub lfo_depth: f32,
    pub lfo_shape: usize,
    pub lfo2_rate: f32,
    pub lfo2_depth: f32,
    pub lfo2_shape: usize,
    pub mod_matrix_open: bool,
    pub fx_rack_open: bool,
    pub mod_routes: Vec<ModRouteUi>,
    pub fx_slots: Vec<FxSlotUi>,
    pub mod_route_total: usize,
    pub keys_down: HashSet<u8>,
    pub piano_visible: bool,
    pub preset_name: String,
    pub preset_category: String,
    pub status: String,
    pub midi_device: String,
}

pub struct ScopeStripContext<'a> {
    pub banks: &'a [WavetableBank],
    pub bank_for_osc: &'a dyn Fn(usize) -> usize,
    pub live: Option<&'a ScopeLiveTaps>,
    pub is_playing: bool,
    pub now_secs: f64,
    pub state: &'a mut ScopeStripState,
}

impl Default for S1State {
    fn default() -> Self {
        Self {
            wt_position: 108.0,
            osc_position: [108.0, 0.0, 0.0],
            wt_bank_name: "Saw Morph".into(),
            wt_edit_tool: WtEditTool::Select,
            wt_morph_a: 0.0,
            wt_morph_b: 255.0,
            wt_morph_amount: morph_amount_for_position(0.0, 255.0, 108.0),
            osc_morph_a: [0.0; 3],
            osc_morph_b: [255.0; 3],
            osc_morph_amount: [0.0; 3],
            osc_tab: 0,
            osc_type: [0, 0, 0],
            osc_level: [0.85, 0.0, 0.0],
            osc_pan: [0.0, 0.0, 0.0],
            osc_coarse: [0.0, 0.0, 0.0],
            osc_unison: [3, 1, 1],
            osc_pulse_width: [0.5, 0.5, 0.5],
            osc_warp_mode: [0, 0, 0],
            osc_warp_amount: [0.0, 0.0, 0.0],
            osc_fm_source: [0, 0, 0],
            osc_fm_algorithm: [0, 0, 0],
            osc_fm_ratio: [1.0, 1.0, 1.0],
            osc_fm_index: [0.0, 0.0, 0.0],
            unison_stereo_spread: 0.7,
            sub_level: 0.0,
            noise_level: 0.0,
            macro_values: [0.5; 4],
            filter_cutoff: 1200.0,
            filter_resonance: 0.3,
            filter_key_tracking: 0.5,
            filter_drive: 0.0,
            filter2_cutoff: 2400.0,
            filter2_resonance: 0.25,
            filter2_mode: 0,
            filter2_drive: 0.0,
            filter_mode: 0,
            env_attack: 0.012,
            env_decay: 0.22,
            env_sustain: 0.6,
            env_release: 0.4,
            filt_env_attack: 0.005,
            filt_env_decay: 0.35,
            filt_env_sustain: 0.2,
            filt_env_release: 0.5,
            lfo_rate: 2.4,
            lfo_depth: 0.0,
            lfo_shape: 0,
            lfo2_rate: 1.0,
            lfo2_depth: 0.0,
            lfo2_shape: 0,
            mod_matrix_open: true,
            fx_rack_open: true,
            mod_routes: default_mod_routes(),
            fx_slots: default_fx_slots(),
            mod_route_total: 24,
            keys_down: HashSet::new(),
            piano_visible: true,
            preset_name: "Factory Lead".into(),
            preset_category: "Bass · Wavetable · Saw Morph".into(),
            status: "Audio OK — click keys or use QWERTY row (Z–M)".into(),
            midi_device: "Default".into(),
        }
    }
}

pub fn draw_s1(
    ui: &mut Ui,
    screen: Rect,
    state: &mut S1State,
    bank: Option<&mut WavetableBank>,
    preview_patch: &Patch,
    midi: &S1MidiDevices<'_>,
    config: &S1ShellConfig,
    scope: Option<ScopeStripContext<'_>>,
) -> S1Actions {
    let layout = S1Layout::compute_with_options(
        screen,
        S1LayoutOptions {
            piano_visible: state.piano_visible,
            show_osc_column: config.show_osc_column,
            show_mod_matrix: config.show_mod_matrix,
            mod_matrix_open: state.mod_matrix_open,
            show_fx_rack: config.show_fx_rack,
            fx_rack_open: state.fx_rack_open,
        },
    );
    let tokens = Tokens::default();
    let mut actions = S1Actions::default();

    let painter = ui.painter_at(screen);
    let border = egui::Stroke::new(1.0_f32, tokens.border);
    painter.rect_filled(layout.header, 0.0, tokens.surface2);
    painter.line_segment(
        [layout.header.left_bottom(), layout.header.right_bottom()],
        border,
    );
    painter.rect_filled(layout.main, 0.0, tokens.bg);
    if layout.osc.is_positive() {
        painter.rect_filled(layout.osc, 0.0, tokens.bg);
        painter.line_segment(
            [layout.osc.right_top(), layout.osc.right_bottom()],
            border,
        );
    }
    painter.rect_filled(layout.rail, 0.0, tokens.bg);
    if layout.rail.is_positive() {
        painter.line_segment(
            [layout.rail.left_top(), layout.rail.left_bottom()],
            border,
        );
    }
    if layout.mod_matrix.is_positive() {
        painter.line_segment(
            [layout.mod_matrix.left_top(), layout.mod_matrix.right_top()],
            border,
        );
    }
    if layout.fx_rack.is_positive() {
        painter.line_segment(
            [layout.fx_rack.left_top(), layout.fx_rack.right_top()],
            border,
        );
    }
    if state.piano_visible && layout.piano_wrap.is_positive() {
        painter.rect_filled(layout.piano_wrap, 0.0, tokens.surface2);
        painter.line_segment(
            [layout.piano_wrap.left_top(), layout.piano_wrap.right_top()],
            border,
        );
    }
    painter.rect_filled(layout.footer, 0.0, tokens.surface2);
    painter.line_segment(
        [layout.footer.left_top(), layout.footer.right_top()],
        border,
    );

    draw_header(ui, layout.header, state, midi, &mut actions);
    if layout.osc.is_positive() {
        draw_osc(ui, layout.osc, state, &mut actions);
    }
    draw_center(ui, layout.center, state, bank, preview_patch, config, scope, &mut actions);
    draw_rail(ui, layout.rail, state, config, &mut actions);

    if layout.mod_matrix.is_positive() {
        let result = draw_mod_matrix(
            ui,
            layout.mod_matrix,
            ModMatrixState {
                open: &mut state.mod_matrix_open,
                routes: &mut state.mod_routes,
                total_routes: state.mod_route_total,
            },
        );
        if result.changed {
            actions.params_changed = true;
        }
    }

    if layout.fx_rack.is_positive() {
        let result = draw_fx_rack(
            ui,
            layout.fx_rack,
            FxRackState {
                open: &mut state.fx_rack_open,
                slots: &mut state.fx_slots,
            },
        );
        if result.changed {
            actions.params_changed = true;
        }
    }

    if state.piano_visible && layout.piano_wrap.is_positive() {
        draw_piano_wrap(ui, layout.piano_wrap, state, &mut actions);
    }

    draw_footer(ui, layout.footer, state);

    actions
}

fn draw_header(
    ui: &mut Ui,
    rect: Rect,
    state: &mut S1State,
    midi: &S1MidiDevices<'_>,
    actions: &mut S1Actions,
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
                            .font(heading_font(15.0))
                            .color(tokens.text)
                            .extra_letter_spacing(0.04),
                    );

                    ui.add_space(GRID_UNIT);

                    if header_btn(ui, "Open", true).clicked() {
                        actions.open_preset = true;
                    }
                    if header_btn(ui, "Save", true).clicked() {
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
                            .width(160.0)
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

fn header_btn(ui: &mut Ui, label: &str, ghost: bool) -> egui::Response {
    let tokens = Tokens::default();
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(11.0),
        if ghost { tokens.text } else { tokens.accent_on },
    );
    let size = egui::vec2(galley.size().x + 24.0, galley.size().y + 12.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let fill = if ghost {
            Color32::TRANSPARENT
        } else {
            tokens.accent
        };
        let stroke = if ghost {
            tokens.border
        } else {
            Color32::from_rgb(0x2a, 0x6b, 0x8a)
        };
        let text_color = if ghost {
            tokens.text
        } else {
            tokens.accent_on
        };
        if response.hovered() {
            painter.rect_filled(rect, 6.0, tokens.bg_muted);
        } else {
            painter.rect_filled(rect, 6.0, fill);
        }
        painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0_f32, stroke));
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.0),
            text_color,
        );
    }
    response
}

fn header_menu_label(label: &str) -> egui::WidgetText {
    egui::RichText::new(label).size(11.0).into()
}

fn sync_morph_to_active_tab(state: &mut S1State) {
    let idx = state.osc_tab.min(2);
    state.wt_morph_a = state.osc_morph_a[idx];
    state.wt_morph_b = state.osc_morph_b[idx];
    state.wt_morph_amount = state.osc_morph_amount[idx];
}

fn sync_morph_from_active_tab(state: &mut S1State) {
    let idx = state.osc_tab.min(2);
    state.osc_morph_a[idx] = state.wt_morph_a;
    state.osc_morph_b[idx] = state.wt_morph_b;
    state.osc_morph_amount[idx] = state.wt_morph_amount;
}

fn sync_wt_position_from_osc(state: &mut S1State) {
    let idx = state.osc_tab.min(2);
    state.wt_position = state.osc_position[idx];
}

fn sync_osc_position_from_wt(state: &mut S1State) {
    let idx = state.osc_tab.min(2);
    state.osc_position[idx] = state.wt_position;
}

fn draw_osc(ui: &mut Ui, rect: Rect, state: &mut S1State, actions: &mut S1Actions) {
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

fn draw_center(
    ui: &mut Ui,
    rect: Rect,
    state: &mut S1State,
    mut bank: Option<&mut WavetableBank>,
    preview_patch: &Patch,
    config: &S1ShellConfig,
    scope: Option<ScopeStripContext<'_>>,
    actions: &mut S1Actions,
) {
    let inner = rect.shrink(SPACE_SM);
    let morph_h = if config.show_wt_editor {
        WT_MORPH_HEIGHT + GRID_UNIT
    } else {
        0.0
    };
    let views_h = if config.show_wt_editor {
        WT_VIEW_MIN_HEIGHT + GRID_UNIT
    } else {
        0.0
    };

    let scope_rect = Rect::from_min_max(
        inner.min,
        egui::pos2(inner.max.x, inner.min.y + SCOPE_STRIP_HEIGHT),
    );
    let content_top = scope_rect.max.y + GRID_UNIT;

    let (strip_rect, morph_rect, views_rect) = if config.show_osc_column {
        let strip_rect = Rect::from_min_max(
            egui::pos2(inner.min.x, content_top),
            egui::pos2(inner.max.x, content_top + WT_STRIP_HEIGHT),
        );
        let morph_rect = if config.show_wt_editor {
            Rect::from_min_max(
                egui::pos2(inner.min.x, strip_rect.max.y + GRID_UNIT),
                egui::pos2(inner.max.x, strip_rect.max.y + GRID_UNIT + WT_MORPH_HEIGHT),
            )
        } else {
            Rect::NOTHING
        };
        let views_top = if config.show_wt_editor {
            morph_rect.max.y + GRID_UNIT
        } else {
            strip_rect.max.y + GRID_UNIT
        };
        let views_rect = if config.show_wt_editor {
            Rect::from_min_max(
                egui::pos2(inner.min.x, views_top),
                inner.max,
            )
        } else {
            Rect::NOTHING
        };
        (strip_rect, morph_rect, views_rect)
    } else {
        let views_rect = if config.show_wt_editor {
            Rect::from_min_max(
                egui::pos2(inner.min.x, inner.max.y - views_h),
                inner.max,
            )
        } else {
            Rect::NOTHING
        };
        let morph_rect = if config.show_wt_editor {
            Rect::from_min_max(
                egui::pos2(inner.min.x, views_rect.min.y - morph_h),
                egui::pos2(inner.max.x, views_rect.min.y - GRID_UNIT),
            )
        } else {
            Rect::NOTHING
        };
        let strip_bottom = if config.show_wt_editor {
            morph_rect.min.y - GRID_UNIT
        } else {
            inner.max.y
        };
        let strip_rect = Rect::from_min_max(
            egui::pos2(inner.min.x, strip_bottom - WT_STRIP_HEIGHT),
            egui::pos2(inner.max.x, strip_bottom),
        );
        (strip_rect, morph_rect, views_rect)
    };

    let bank_name = state.wt_bank_name.clone();

    if scope_rect.is_positive() {
        ui.allocate_ui_at_rect(scope_rect, |ui| {
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
                        now_secs: ui.input(|i| i.time),
                        state: &mut strip_state,
                    },
                );
            }
        });
    }

    if config.show_wt_editor && morph_rect.is_positive() {
        ui.allocate_ui_at_rect(morph_rect, |ui| {
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
        });
    }

    if config.show_wt_editor && views_rect.is_positive() {
        ui.allocate_ui_at_rect(views_rect, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GRID_UNIT;
                let half_w = (ui.available_width() - GRID_UNIT) * 0.5;
                ui.allocate_ui_with_layout(
                    egui::vec2(half_w, WT_VIEW_MIN_HEIGHT),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let view = WtView2d {
                            position: state.wt_position,
                            bank: bank.as_deref_mut(),
                            bank_name: Some(bank_name.as_str()),
                            tool: &mut state.wt_edit_tool,
                        };
                        if view.show(ui).frame_edited {
                            actions.frame_edited = true;
                        }
                    },
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(half_w, WT_VIEW_MIN_HEIGHT),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        WtView3d {
                            position: state.wt_position,
                            bank: bank.as_deref(),
                        }
                        .show(ui);
                    },
                );
            });
        });
    }

    ui.allocate_ui_at_rect(strip_rect, |ui| {
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
    });
}

fn draw_rail(
    ui: &mut Ui,
    rect: Rect,
    state: &mut S1State,
    config: &S1ShellConfig,
    actions: &mut S1Actions,
) {
    ui.allocate_ui_at_rect(rect, |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::same(SPACE_SM))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = SPACE_SM;

                if !config.show_osc_column {
                    panel(ui, "Performance", |ui| {
                        ui.horizontal_centered(|ui| {
                            let wt_frame = state.wt_position.round() as i32;
                            let r = Knob::new(&mut state.wt_position, 0.0..=255.0, "WT Position")
                                .size(KnobSize::Lg)
                                .style(KnobStyle::Wired)
                                .value_text(format!("{wt_frame}"))
                                .show(ui);
                            if r.changed {
                                sync_osc_position_from_wt(state);
                                state.wt_morph_amount = morph_amount_for_position(
                                    state.wt_morph_a,
                                    state.wt_morph_b,
                                    state.wt_position,
                                );
                                actions.params_changed = true;
                            }
                        });
                    });
                }

                panel(ui, "Filter", |ui| {
                    if config.show_osc_column {
                        let prev = state.filter_mode;
                        tab_bar(ui, &["LP", "HP", "BP", "Notch"], &mut state.filter_mode);
                        if prev != state.filter_mode {
                            actions.params_changed = true;
                        }
                        ui.add_space(GRID_UNIT);
                    }
                    ui.horizontal_centered(|ui| {
                        ui.spacing_mut().item_spacing.x = SPACE_SM;
                        let cutoff_text = format_cutoff(state.filter_cutoff);
                        let r1 = Knob::new(&mut state.filter_cutoff, 40.0..=12000.0, "Cutoff")
                            .size(if config.show_osc_column {
                                KnobSize::Md
                            } else {
                                KnobSize::Lg
                            })
                            .style(KnobStyle::Wired)
                            .logarithmic(true)
                            .value_text(cutoff_text)
                            .show(ui);
                        let res_label = if config.show_osc_column {
                            "Res"
                        } else {
                            "Resonance"
                        };
                        let res_text = format!("{:.2}", state.filter_resonance);
                        let r2 = Knob::new(&mut state.filter_resonance, 0.0..=0.95, res_label)
                            .size(if config.show_osc_column {
                                KnobSize::Md
                            } else {
                                KnobSize::Lg
                            })
                            .style(KnobStyle::Wired)
                            .value_text(res_text)
                            .show(ui);
                        let drive_text = format!("{:.0}%", state.filter_drive * 100.0);
                        let r_drive = Knob::new(&mut state.filter_drive, 0.0..=1.0, "Drive")
                            .size(KnobSize::Sm)
                            .style(KnobStyle::Wired)
                            .value_text(drive_text)
                            .show(ui);
                        if r1.changed || r2.changed || r_drive.changed {
                            actions.params_changed = true;
                        }
                    });
                    if config.show_osc_column {
                        ui.add_space(GRID_UNIT);
                        let kt_text = format!("{:.0}%", state.filter_key_tracking * 100.0);
                        let r3 = Knob::new(&mut state.filter_key_tracking, 0.0..=1.0, "Key")
                            .size(KnobSize::Sm)
                            .style(KnobStyle::Wired)
                            .value_text(kt_text)
                            .show(ui);
                        let f2_text = format_cutoff(state.filter2_cutoff);
                        let r4 = Knob::new(&mut state.filter2_cutoff, 40.0..=12000.0, "F2 Cut")
                            .size(KnobSize::Sm)
                            .style(KnobStyle::Wired)
                            .logarithmic(true)
                            .value_text(f2_text)
                            .show(ui);
                        if r3.changed || r4.changed {
                            actions.params_changed = true;
                        }
                    }
                });

                if config.show_osc_column {
                    panel(ui, "Filter Envelope", |ui| {
                        adsr_graph(
                            ui,
                            state.filt_env_attack,
                            state.filt_env_decay,
                            state.filt_env_sustain,
                            state.filt_env_release,
                        );
                        ui.add_space(GRID_UNIT);
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            let attack_text = format_env_time(state.filt_env_attack);
                            let decay_text = format_env_time(state.filt_env_decay);
                            let sustain_text = format_sustain(state.filt_env_sustain);
                            let release_text = format_env_time(state.filt_env_release);
                            let r_a = Knob::new(&mut state.filt_env_attack, 0.001..=2.0, "A")
                                .size(KnobSize::Sm)
                                .style(KnobStyle::Wired)
                                .value_text(attack_text)
                                .show(ui);
                            let r_d = Knob::new(&mut state.filt_env_decay, 0.001..=2.0, "D")
                                .size(KnobSize::Sm)
                                .style(KnobStyle::Wired)
                                .value_text(decay_text)
                                .show(ui);
                            let r_s = Knob::new(&mut state.filt_env_sustain, 0.0..=1.0, "S")
                                .size(KnobSize::Sm)
                                .style(KnobStyle::Wired)
                                .value_text(sustain_text)
                                .show(ui);
                            let r_r = Knob::new(&mut state.filt_env_release, 0.001..=3.0, "R")
                                .size(KnobSize::Sm)
                                .style(KnobStyle::Wired)
                                .value_text(release_text)
                                .show(ui);
                            if r_a.changed || r_d.changed || r_s.changed || r_r.changed {
                                actions.params_changed = true;
                            }
                        });
                    });
                }

                if config.show_osc_column {
                    panel(ui, "Amp Envelope", |ui| {
                        adsr_graph(
                            ui,
                            state.env_attack,
                            state.env_decay,
                            state.env_sustain,
                            state.env_release,
                        );
                        ui.add_space(GRID_UNIT);
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            let attack_text = format_env_time(state.env_attack);
                            let decay_text = format_env_time(state.env_decay);
                            let sustain_text = format_sustain(state.env_sustain);
                            let release_text = format_env_time(state.env_release);
                            let r_a = Knob::new(&mut state.env_attack, 0.001..=2.0, "A")
                                .size(KnobSize::Sm)
                                .style(KnobStyle::Wired)
                                .value_text(attack_text)
                                .show(ui);
                            let r_d = Knob::new(&mut state.env_decay, 0.001..=2.0, "D")
                                .size(KnobSize::Sm)
                                .style(KnobStyle::Wired)
                                .value_text(decay_text)
                                .show(ui);
                            let r_s = Knob::new(&mut state.env_sustain, 0.0..=1.0, "S")
                                .size(KnobSize::Sm)
                                .style(KnobStyle::Wired)
                                .value_text(sustain_text)
                                .show(ui);
                            let r_r = Knob::new(&mut state.env_release, 0.001..=3.0, "R")
                                .size(KnobSize::Sm)
                                .style(KnobStyle::Wired)
                                .value_text(release_text)
                                .show(ui);
                            if r_a.changed || r_d.changed || r_s.changed || r_r.changed {
                                actions.params_changed = true;
                            }
                        });
                    });

                    panel(ui, "LFO 1", |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            let rate_text = format_lfo_rate(state.lfo_rate);
                            let depth_text = format_depth(state.lfo_depth);
                            let r1 = Knob::new(&mut state.lfo_rate, 0.05..=20.0, "Rate")
                                .size(KnobSize::Sm)
                                .style(KnobStyle::Wired)
                                .value_text(rate_text)
                                .show(ui);
                            let r2 = Knob::new(&mut state.lfo_depth, 0.0..=1.0, "Depth")
                                .size(KnobSize::Sm)
                                .style(KnobStyle::Wired)
                                .value_text(depth_text)
                                .show(ui);
                            if r1.changed || r2.changed {
                                actions.params_changed = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Shape")
                                    .size(10.0)
                                    .color(Tokens::default().text_muted),
                            );
                            let shapes = ["Sine", "Tri", "Saw", "S&H"];
                            let label = shapes[state.lfo_shape.min(3)];
                            if ui.button(label).clicked() {
                                state.lfo_shape = (state.lfo_shape + 1) % shapes.len();
                                actions.params_changed = true;
                            }
                        });
                    });

                    panel(ui, "LFO 2", |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            let rate_text = format_lfo_rate(state.lfo2_rate);
                            let depth_text = format_depth(state.lfo2_depth);
                            let r1 = Knob::new(&mut state.lfo2_rate, 0.05..=20.0, "Rate")
                                .size(KnobSize::Sm)
                                .style(KnobStyle::Normal)
                                .value_text(rate_text)
                                .show(ui);
                            let r2 = Knob::new(&mut state.lfo2_depth, 0.0..=1.0, "Depth")
                                .size(KnobSize::Sm)
                                .style(KnobStyle::Normal)
                                .value_text(depth_text)
                                .show(ui);
                            if r1.changed || r2.changed {
                                actions.params_changed = true;
                            }
                        });
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new("Shape")
                                    .size(10.0)
                                    .color(Tokens::default().text_muted),
                            );
                            let shapes = ["Sine", "Tri", "Saw", "S&H"];
                            let label = shapes[state.lfo2_shape.min(3)];
                            if ui.button(label).clicked() {
                                state.lfo2_shape = (state.lfo2_shape + 1) % shapes.len();
                                actions.params_changed = true;
                            }
                        });
                    });

                    draw_meter_stub(ui);
                } else {
                    panel_disabled(ui, "Amp Envelope", |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            for label in ["A", "D", "S", "R"] {
                                let mut v = 0.0_f32;
                                Knob::new(&mut v, 0.0..=1.0, label)
                                    .size(KnobSize::Sm)
                                    .style(KnobStyle::Disabled)
                                    .value_text("—")
                                    .show(ui);
                            }
                        });
                    });

                    panel_disabled(ui, "LFO", |ui| {
                        ui.horizontal_centered(|ui| {
                            ui.spacing_mut().item_spacing.x = SPACE_SM;
                            for label in ["Rate", "Depth"] {
                                let mut v = 0.0_f32;
                                Knob::new(&mut v, 0.0..=1.0, label)
                                    .size(KnobSize::Sm)
                                    .style(KnobStyle::Disabled)
                                    .value_text("—")
                                    .show(ui);
                            }
                        });
                    });
                }
            });
    });
}

fn draw_meter_stub(ui: &mut Ui) {
    let tokens = Tokens::default();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 48.0), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let bar_w = 10.0;
        let gap = 6.0;
        let cx = rect.center().x;
        for (i, &level) in [0.62_f32, 0.48_f32].iter().enumerate() {
            let x = cx + (i as f32 - 0.5) * (bar_w + gap);
            let bar_h = rect.height() * level;
            let bar = egui::Rect::from_min_max(
                egui::pos2(x - bar_w * 0.5, rect.max.y - bar_h),
                egui::pos2(x + bar_w * 0.5, rect.max.y),
            );
            painter.rect_filled(bar, 2.0, tokens.accent.gamma_multiply(0.85));
        }
    }
}

fn draw_piano_wrap(ui: &mut Ui, rect: Rect, state: &mut S1State, actions: &mut S1Actions) {
    ui.allocate_ui_at_rect(rect, |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(SPACE_SM, GRID_UNIT))
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                    let (_, piano) = PianoKeyboard::new(&state.keys_down).show(ui);
                    if let Some(n) = piano.note_on {
                        actions.note_on = Some(n);
                    }
                    if let Some(n) = piano.note_off {
                        actions.note_off = Some(n);
                    }
                });
            });
    });
}

fn draw_footer(ui: &mut Ui, rect: Rect, state: &S1State) {
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
                        egui::RichText::new("Performance")
                            .size(11.0)
                            .color(tokens.text_muted),
                    );

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.set_width(ui.available_width());
                        let wt = state.wt_position.round() as i32;
                        ui.label(
                            egui::RichText::new(format!(
                                "WT {wt} · Cutoff {}",
                                format_cutoff(state.filter_cutoff)
                            ))
                            .font(FontId::monospace(11.0))
                            .color(tokens.text_muted),
                        );
                    });
                });
            });
    });
}

fn draw_piano_toggle(ui: &mut Ui, on: bool) -> egui::Response {
    let tokens = Tokens::default();
    let label = "Piano";
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        FontId::proportional(11.0),
        if on { tokens.accent_on } else { tokens.text_muted },
    );
    let size = egui::vec2(galley.size().x + 20.0, galley.size().y + 8.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let fill = if on { tokens.accent } else { tokens.bg_muted };
        let stroke = if on {
            Color32::from_rgb(0x2a, 0x6b, 0x8a)
        } else {
            tokens.border
        };
        painter.rect_filled(rect, 6.0, fill);
        painter.rect_stroke(rect, 6.0, egui::Stroke::new(1.0_f32, stroke));
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            FontId::proportional(11.0),
            if on { tokens.accent_on } else { tokens.text_muted },
        );
    }
    response
}

fn format_cutoff(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1} kHz", hz / 1000.0)
    } else {
        format!("{:.0} Hz", hz)
    }
}
