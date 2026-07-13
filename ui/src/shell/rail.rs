use egui::{Rect, Ui};
use reelsynth_ui_theme::Tokens;

use super::*;
use super::footer::{draw_level_meter, format_cutoff};
use super::header::sync_osc_position_from_wt;
use crate::layout::{CENTER_GAP, UiScale};
use crate::layout_audit::rail_used_rect_id;
use crate::region::region;
use crate::widgets::{labeled_cycle, tab_bar, Knob, KnobSize, KnobStyle, panel, panel_disabled};

pub(super) fn draw_rail(
    ui: &mut Ui,
    rect: Rect,
    state: &mut UiState,
    config: &ShellConfig,
    actions: &mut ShellActions,
    scale: UiScale,
) {
    let s = scale.ui();
    let gap = CENTER_GAP * s;
    region(ui, rect, |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::same(SPACE_SM * s * 0.75))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = gap;
                draw_rail_panels(ui, state, config, actions, scale);
                // Store used rect for widget-overflow regression tests.
                let used = ui.min_rect();
                ui.ctx().data_mut(|d| d.insert_temp(rail_used_rect_id(), used));
            });
    });
}

fn draw_rail_panels(
    ui: &mut Ui,
    state: &mut UiState,
    config: &ShellConfig,
    actions: &mut ShellActions,
    scale: UiScale,
) {
    let s = scale.ui();
    let knob_sm = if s < 0.82 { KnobSize::Sm } else { KnobSize::Sm };
    let knob_md = if s < 0.82 { KnobSize::Sm } else { KnobSize::Md };
    let min_panel_h = 92.0 * s;

    if !config.show_osc_column {
        panel(ui, "Performance", |ui| {
            ui.horizontal_centered(|ui| {
                let wt_frame = state.wt_position.round() as i32;
                let r = Knob::new(&mut state.wt_position, 0.0..=255.0, "WT Position")
                    .size(knob_md)
                    .scale(s)
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
            ui.add_space(GRID_UNIT * s);
        }
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(SPACE_SM * s, SPACE_SM * s);
            let knob_size = if config.show_osc_column { knob_sm } else { knob_md };
            let cutoff_text = format_cutoff(state.filter_cutoff);
            let r1 = Knob::new(&mut state.filter_cutoff, 40.0..=12000.0, "Cutoff")
                .size(knob_size)
                .scale(s)
                .style(KnobStyle::Wired)
                .logarithmic(true)
                .value_text(cutoff_text)
                .show(ui);
            let res_label = if config.show_osc_column { "Res" } else { "Resonance" };
            let res_text = format!("{:.2}", state.filter_resonance);
            let r2 = Knob::new(&mut state.filter_resonance, 0.0..=0.95, res_label)
                .size(knob_size)
                .scale(s)
                .style(KnobStyle::Wired)
                .value_text(res_text)
                .show(ui);
            let drive_text = format!("{:.0}%", state.filter_drive * 100.0);
            let r_drive = Knob::new(&mut state.filter_drive, 0.0..=1.0, "Drive")
                .size(knob_sm)
                .scale(s)
                .style(KnobStyle::Wired)
                .value_text(drive_text)
                .show(ui);
            if config.show_osc_column {
                let key_text = format!("{:.0}%", state.filter_key_tracking * 100.0);
                let r3 = Knob::new(&mut state.filter_key_tracking, 0.0..=1.0, "Key")
                    .size(knob_sm)
                    .scale(s)
                    .style(KnobStyle::Wired)
                    .value_text(key_text)
                    .show(ui);
                let f2_text = format_cutoff(state.filter2_cutoff);
                let r4 = Knob::new(&mut state.filter2_cutoff, 40.0..=12000.0, "F2 Cut")
                    .size(knob_sm)
                    .scale(s)
                    .style(KnobStyle::Wired)
                    .logarithmic(true)
                    .value_text(f2_text)
                    .show(ui);
                if r3.changed || r4.changed {
                    actions.params_changed = true;
                }
            }
            if r1.changed || r2.changed || r_drive.changed {
                actions.params_changed = true;
            }
        });
    });

    if config.show_osc_column {
        // Budget panels to avoid clipping at the minimum window size.
        if ui.available_height() > min_panel_h * 4.6 {
            panel(ui, "Filter Envelope", |ui| {
                adsr_graph(
                    ui,
                    state.filt_env_attack,
                    state.filt_env_decay,
                    state.filt_env_sustain,
                    state.filt_env_release,
                    s,
                );
                ui.add_space(GRID_UNIT * s);
                env_knobs(
                    ui,
                    &mut state.filt_env_attack,
                    &mut state.filt_env_decay,
                    &mut state.filt_env_sustain,
                    &mut state.filt_env_release,
                    actions,
                    s,
                );
            });
        }

        if ui.available_height() > min_panel_h * 3.3 {
            panel(ui, "Amp Envelope", |ui| {
                adsr_graph(
                    ui,
                    state.env_attack,
                    state.env_decay,
                    state.env_sustain,
                    state.env_release,
                    s,
                );
                ui.add_space(GRID_UNIT * s);
                env_knobs(
                    ui,
                    &mut state.env_attack,
                    &mut state.env_decay,
                    &mut state.env_sustain,
                    &mut state.env_release,
                    actions,
                    s,
                );
            });
        }

        if ui.available_height() > min_panel_h * 2.0 {
            panel(ui, "LFOs", |ui| {
                ui.spacing_mut().item_spacing.x = SPACE_SM * s;
                let w = (ui.available_width() - SPACE_SM * s).max(0.0) * 0.5;
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(w, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.label(
                                egui::RichText::new("LFO 1")
                                    .size(10.0)
                                    .color(Tokens::default().text_muted),
                            );
                            lfo_panel(
                                ui,
                                &mut state.lfo_rate,
                                &mut state.lfo_depth,
                                &mut state.lfo_shape,
                                KnobStyle::Wired,
                                actions,
                                s,
                            );
                        },
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(w, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            ui.label(
                                egui::RichText::new("LFO 2")
                                    .size(10.0)
                                    .color(Tokens::default().text_muted),
                            );
                            lfo_panel(
                                ui,
                                &mut state.lfo2_rate,
                                &mut state.lfo2_depth,
                                &mut state.lfo2_shape,
                                KnobStyle::Normal,
                                actions,
                                s,
                            );
                        },
                    );
                });
            });
        }

        if ui.available_height() > 56.0 * s {
            draw_level_meter(ui);
        }
    } else {
        panel_disabled(ui, "Amp Envelope", |ui| {
            ui.horizontal_centered(|ui| {
                for label in ["A", "D", "S", "R"] {
                    let mut v = 0.0_f32;
                    Knob::new(&mut v, 0.0..=1.0, label)
                        .size(KnobSize::Sm)
                        .scale(s)
                        .style(KnobStyle::Disabled)
                        .value_text("—")
                        .show(ui);
                }
            });
        });

        panel_disabled(ui, "LFO", |ui| {
            ui.horizontal_centered(|ui| {
                for label in ["Rate", "Depth"] {
                    let mut v = 0.0_f32;
                    Knob::new(&mut v, 0.0..=1.0, label)
                        .size(KnobSize::Sm)
                        .scale(s)
                        .style(KnobStyle::Disabled)
                        .value_text("—")
                        .show(ui);
                }
            });
        });
    }
}

fn env_knobs(
    ui: &mut Ui,
    attack: &mut f32,
    decay: &mut f32,
    sustain: &mut f32,
    release: &mut f32,
    actions: &mut ShellActions,
    scale: f32,
) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_SM * scale;
        let a_text = format_env_time(*attack);
        let r_a = Knob::new(attack, 0.001..=2.0, "A")
            .size(KnobSize::Sm)
            .scale(scale)
            .style(KnobStyle::Wired)
            .value_text(a_text)
            .show(ui);
        let d_text = format_env_time(*decay);
        let r_d = Knob::new(decay, 0.001..=2.0, "D")
            .size(KnobSize::Sm)
            .scale(scale)
            .style(KnobStyle::Wired)
            .value_text(d_text)
            .show(ui);
        let s_text = format_sustain(*sustain);
        let r_s = Knob::new(sustain, 0.0..=1.0, "S")
            .size(KnobSize::Sm)
            .scale(scale)
            .style(KnobStyle::Wired)
            .value_text(s_text)
            .show(ui);
        let r_text = format_env_time(*release);
        let r_r = Knob::new(release, 0.001..=3.0, "R")
            .size(KnobSize::Sm)
            .scale(scale)
            .style(KnobStyle::Wired)
            .value_text(r_text)
            .show(ui);
        if r_a.changed || r_d.changed || r_s.changed || r_r.changed {
            actions.params_changed = true;
        }
    });
}

fn lfo_panel(
    ui: &mut Ui,
    rate: &mut f32,
    depth: &mut f32,
    shape: &mut usize,
    style: KnobStyle,
    actions: &mut ShellActions,
    scale: f32,
) {
    ui.horizontal_centered(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_SM * scale;
        let rate_text = format_lfo_rate(*rate);
        let r1 = Knob::new(rate, 0.05..=20.0, "Rate")
            .size(KnobSize::Sm)
            .scale(scale)
            .style(style)
            .value_text(rate_text)
            .show(ui);
        let depth_text = format_depth(*depth);
        let r2 = Knob::new(depth, 0.0..=1.0, "Depth")
            .size(KnobSize::Sm)
            .scale(scale)
            .style(style)
            .value_text(depth_text)
            .show(ui);
        if r1.changed || r2.changed {
            actions.params_changed = true;
        }
    });
    let shapes = ["Sine", "Tri", "Saw", "S&H"];
    let label = shapes[(*shape).min(3)];
    if labeled_cycle(ui, "Shape", label).clicked() {
        *shape = (*shape + 1) % shapes.len();
        actions.params_changed = true;
    }
}
