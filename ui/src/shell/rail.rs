use egui::{Rect, Ui};
use reelsynth::Patch;
use reelsynth_ui_theme::Tokens;

use super::*;
use super::footer::{draw_level_meter, format_cutoff};
use super::header::sync_osc_position_from_wt;
use crate::widgets::{labeled_cycle, tab_bar, Knob, KnobSize, KnobStyle, panel, panel_disabled};

pub(super) fn draw_rail(
    ui: &mut Ui,
    rect: Rect,
    state: &mut UiState,
    config: &ShellConfig,
    actions: &mut ShellActions,
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
                            let shapes = ["Sine", "Tri", "Saw", "S&H"];
                            let label = shapes[state.lfo_shape.min(3)];
                            if labeled_cycle(ui, "Shape", label).clicked() {
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
                            let shapes = ["Sine", "Tri", "Saw", "S&H"];
                            let label = shapes[state.lfo2_shape.min(3)];
                            if labeled_cycle(ui, "Shape", label).clicked() {
                                state.lfo2_shape = (state.lfo2_shape + 1) % shapes.len();
                                actions.params_changed = true;
                            }
                        });
                    });

                    draw_level_meter(ui);
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

