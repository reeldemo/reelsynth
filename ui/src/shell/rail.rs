use egui::{Grid, Rect, Ui};
use reelsynth_ui_theme::Tokens;

use super::*;
use super::footer::{draw_level_meter, format_cutoff};
use super::header::sync_osc_position_from_wt;
use crate::audit_registry::{record_region, record_used, AuditId};
use crate::layout::{CENTER_GAP, KNOB_COL_WIDTH, UiScale};
use crate::layout_audit::{
    rail_filter_allocated_rect_id, rail_filter_used_rect_id, rail_used_rect_id,
};
use crate::region::region;
use crate::widgets::{
    labeled_select, panel_audit, Knob, KnobResponse, KnobSize, KnobStyle, panel, panel_disabled,
};

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
            .inner_margin(egui::Margin::same(SPACE_SM * s))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.set_min_height(rect.height());
                ui.spacing_mut().item_spacing.y = gap;
                if config.show_osc_column {
                    egui::ScrollArea::vertical()
                        .id_salt("rail_scroll")
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            draw_rail_panels(ui, state, config, actions, scale);
                        });
                } else {
                    draw_rail_panels(ui, state, config, actions, scale);
                }
            });

        let used = ui.min_rect().intersect(rect);
        ui.ctx().data_mut(|d| d.insert_temp(rail_used_rect_id(), used));
        record_region(ui.ctx(), AuditId::RailColumn, rect, used);
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

    panel_audit(ui, "Filter", Some(AuditId::RailPanelFilter), |ui| {
        let filter_body_top = ui.cursor().min.y;
        if config.show_osc_column {
            let knobs_start = ui.cursor().min;
            let rack = crate::draw_filter_chain(ui, &mut state.filter_slots, s);
            if rack.changed {
                if let Some(s0) = state.filter_slots.first() {
                    state.filter_cutoff = s0.cutoff;
                    state.filter_resonance = s0.resonance;
                    state.filter_key_tracking = s0.key_tracking;
                    state.filter_drive = s0.drive;
                    state.filter_mode = crate::filter_mode_from_type(&s0.filter_type);
                }
                actions.params_changed = true;
            }
            record_row(ui.ctx(), AuditId::RailFilterKnobs, ui, knobs_start);
            let used = ui.min_rect();
            let allocated = Rect::from_min_max(
                egui::pos2(ui.max_rect().min.x, filter_body_top),
                egui::pos2(ui.max_rect().max.x, used.max.y),
            );
            ui.ctx().data_mut(|d| {
                d.insert_temp(rail_filter_used_rect_id(), used);
                d.insert_temp(rail_filter_allocated_rect_id(), allocated);
            });
        } else {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(SPACE_SM * s, SPACE_SM * s);
                draw_filter_knobs_row(ui, state, config, actions, knob_md, knob_sm, s);
            });
        }
    });

    if config.show_osc_column {
        panel_audit(ui, "Filter Envelope", Some(AuditId::RailPanelFiltEnv), |ui| {
            let graph = adsr_graph(
                ui,
                &mut state.filt_env_attack,
                &mut state.filt_env_decay,
                &mut state.filt_env_sustain,
                &mut state.filt_env_release,
                s * 0.85,
                "filt_env",
            );
            if graph.changed {
                actions.params_changed = true;
            }
            record_used(ui.ctx(), AuditId::RailFiltEnvGraph, graph.response.rect);
            ui.add_space(GRID_UNIT * s * 0.5);
            let knobs_start = ui.cursor().min;
            env_knobs(
                ui,
                &mut state.filt_env_attack,
                &mut state.filt_env_decay,
                &mut state.filt_env_sustain,
                &mut state.filt_env_release,
                actions,
                s,
                "filt_env",
            );
            record_row(ui.ctx(), AuditId::RailFiltEnvKnobs, ui, knobs_start);
        });

        panel_audit(ui, "Amp Envelope", Some(AuditId::RailPanelAmpEnv), |ui| {
            let graph = adsr_graph(
                ui,
                &mut state.env_attack,
                &mut state.env_decay,
                &mut state.env_sustain,
                &mut state.env_release,
                s * 0.85,
                "amp_env",
            );
            if graph.changed {
                actions.params_changed = true;
            }
            record_used(ui.ctx(), AuditId::RailAmpEnvGraph, graph.response.rect);
            ui.add_space(GRID_UNIT * s * 0.5);
            let knobs_start = ui.cursor().min;
            env_knobs(
                ui,
                &mut state.env_attack,
                &mut state.env_decay,
                &mut state.env_sustain,
                &mut state.env_release,
                actions,
                s,
                "amp_env",
            );
            record_row(ui.ctx(), AuditId::RailAmpEnvKnobs, ui, knobs_start);
        });

        panel_audit(ui, "LFOs", Some(AuditId::RailPanelLfos), |ui| {
            ui.spacing_mut().item_spacing.y = GRID_UNIT * s;
            let knob_row = KNOB_COL_WIDTH * s * 2.0 + SPACE_SM * s;
            let min_pair_w = knob_row * 2.0 + SPACE_SM * s;
            let side_by_side = ui.available_width() >= min_pair_w;
            if !side_by_side {
                let lfo1_start = ui.cursor().min;
                lfo_block(
                    ui,
                    "LFO 1",
                    &mut state.lfo_rate,
                    &mut state.lfo_depth,
                    &mut state.lfo_shape,
                    KnobStyle::Wired,
                    actions,
                    s,
                );
                let lfo1_used = clamp_used_below_start(ui.min_rect(), lfo1_start);
                let lfo1_alloc = Rect::from_min_max(lfo1_start, lfo1_used.max);
                if lfo1_alloc.is_positive() && lfo1_used.is_positive() {
                    record_region(
                        ui.ctx(),
                        AuditId::RailLfo1Block,
                        lfo1_alloc,
                        lfo1_used,
                    );
                }
                let lfo2_start = ui.cursor().min;
                lfo_block(
                    ui,
                    "LFO 2",
                    &mut state.lfo2_rate,
                    &mut state.lfo2_depth,
                    &mut state.lfo2_shape,
                    KnobStyle::Normal,
                    actions,
                    s,
                );
                let lfo2_used = clamp_used_below_start(ui.min_rect(), lfo2_start);
                let lfo2_alloc = Rect::from_min_max(lfo2_start, lfo2_used.max);
                if lfo2_alloc.is_positive() && lfo2_used.is_positive() {
                    record_region(
                        ui.ctx(),
                        AuditId::RailLfo2Block,
                        lfo2_alloc,
                        lfo2_used,
                    );
                }
            } else {
                ui.spacing_mut().item_spacing.x = SPACE_SM * s;
                let w = (ui.available_width() - SPACE_SM * s).max(0.0) * 0.5;
                ui.horizontal(|ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(w, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            let lfo1_start = ui.cursor().min;
                            lfo_block(
                                ui,
                                "LFO 1",
                                &mut state.lfo_rate,
                                &mut state.lfo_depth,
                                &mut state.lfo_shape,
                                KnobStyle::Wired,
                                actions,
                                s,
                            );
                            let lfo1_used = clamp_used_below_start(ui.min_rect(), lfo1_start);
                            let lfo1_alloc = Rect::from_min_max(lfo1_start, lfo1_used.max);
                            if lfo1_alloc.is_positive() && lfo1_used.is_positive() {
                                record_region(
                                    ui.ctx(),
                                    AuditId::RailLfo1Block,
                                    lfo1_alloc,
                                    lfo1_used,
                                );
                            }
                        },
                    );
                    ui.allocate_ui_with_layout(
                        egui::vec2(w, 0.0),
                        egui::Layout::top_down(egui::Align::Min),
                        |ui| {
                            let lfo2_start = ui.cursor().min;
                            lfo_block(
                                ui,
                                "LFO 2",
                                &mut state.lfo2_rate,
                                &mut state.lfo2_depth,
                                &mut state.lfo2_shape,
                                KnobStyle::Normal,
                                actions,
                                s,
                            );
                            let lfo2_used = clamp_used_below_start(ui.min_rect(), lfo2_start);
                            let lfo2_alloc = Rect::from_min_max(lfo2_start, lfo2_used.max);
                            if lfo2_alloc.is_positive() && lfo2_used.is_positive() {
                                record_region(
                                    ui.ctx(),
                                    AuditId::RailLfo2Block,
                                    lfo2_alloc,
                                    lfo2_used,
                                );
                            }
                        },
                    );
                });
            }
        });

        if ui.available_height() > 40.0 * s {
            let meter_start = ui.cursor().min;
            draw_level_meter(ui);
            record_row(ui.ctx(), AuditId::RailLevelMeter, ui, meter_start);
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

fn record_row(ctx: &egui::Context, id: AuditId, ui: &Ui, start: egui::Pos2) {
    let end_y = ui.cursor().min.y;
    let rect = egui::Rect::from_min_max(start, egui::pos2(ui.max_rect().max.x, end_y.max(start.y)));
    if rect.is_positive() {
        record_region(ctx, id, rect, rect);
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
    id_salt: &str,
) {
    let gap = SPACE_SM * scale * 0.5;
    Grid::new(ui.id().with(id_salt).with("env_knobs"))
        .num_columns(2)
        .spacing([gap, gap])
        .min_col_width((ui.available_width() - gap) * 0.5)
        .show(ui, |ui| {
            let a_text = format_env_time(*attack);
            let r_a = env_knob_cell(ui, attack, 0.001..=2.0, "A", a_text, scale);
            let d_text = format_env_time(*decay);
            let r_d = env_knob_cell(ui, decay, 0.001..=2.0, "D", d_text, scale);
            ui.end_row();
            let s_text = format_sustain(*sustain);
            let r_s = env_knob_cell(ui, sustain, 0.0..=1.0, "S", s_text, scale);
            let r_text = format_env_time(*release);
            let r_r = env_knob_cell(ui, release, 0.001..=3.0, "R", r_text, scale);
            ui.end_row();
            if r_a.changed || r_d.changed || r_s.changed || r_r.changed {
                actions.params_changed = true;
            }
        });
}

fn env_knob_cell(
    ui: &mut Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
    value_text: String,
    scale: f32,
) -> KnobResponse {
    ui.vertical_centered(|ui| {
        Knob::new(value, range, label)
            .size(KnobSize::Sm)
            .scale(scale)
            .style(KnobStyle::Wired)
            .show_wired_badge(false)
            .value_text(value_text)
            .show(ui)
    })
    .inner
}

#[allow(dead_code)] // reserved for compact / non-chain layouts
fn draw_filter_knobs_compact(
    ui: &mut Ui,
    state: &mut UiState,
    actions: &mut ShellActions,
    scale: f32,
) {
    let gap = SPACE_SM * scale * 0.5;
    Grid::new(ui.id().with("filter_knobs"))
        .num_columns(2)
        .spacing([gap, gap])
        .min_col_width((ui.available_width() - gap) * 0.5)
        .show(ui, |ui| {
            let cutoff_text = format_cutoff(state.filter_cutoff);
            let r1 = filter_knob_cell(
                ui,
                &mut state.filter_cutoff,
                40.0..=12000.0,
                "Cutoff",
                cutoff_text,
                scale,
                true,
            );
            let drive_text = format!("{:.0}%", state.filter_drive * 100.0);
            let r_drive = filter_knob_cell(
                ui,
                &mut state.filter_drive,
                0.0..=1.0,
                "Drive",
                drive_text,
                scale,
                false,
            );
            ui.end_row();
            let res_text = format!("{:.2}", state.filter_resonance);
            let r2 = filter_knob_cell(
                ui,
                &mut state.filter_resonance,
                0.0..=0.95,
                "Res",
                res_text,
                scale,
                false,
            );
            let key_text = format!("{:.0}%", state.filter_key_tracking * 100.0);
            let r3 = filter_knob_cell(
                ui,
                &mut state.filter_key_tracking,
                0.0..=1.0,
                "Key",
                key_text,
                scale,
                false,
            );
            ui.end_row();
            if r1.changed || r2.changed || r_drive.changed || r3.changed {
                actions.params_changed = true;
            }
        });
}

fn filter_knob_cell(
    ui: &mut Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
    label: &str,
    value_text: String,
    scale: f32,
    logarithmic: bool,
) -> KnobResponse {
    ui.vertical_centered(|ui| {
        Knob::new(value, range, label)
            .size(KnobSize::Sm)
            .scale(scale)
            .style(KnobStyle::Wired)
            .show_wired_badge(false)
            .logarithmic(logarithmic)
            .value_text(value_text)
            .show(ui)
    })
    .inner
}

fn draw_filter_knobs_row(
    ui: &mut Ui,
    state: &mut UiState,
    config: &ShellConfig,
    actions: &mut ShellActions,
    knob_md: KnobSize,
    knob_sm: KnobSize,
    scale: f32,
) {
    let cutoff_text = format_cutoff(state.filter_cutoff);
    let r1 = Knob::new(&mut state.filter_cutoff, 40.0..=12000.0, "Cutoff")
        .size(knob_md)
        .scale(scale)
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
        .size(knob_md)
        .scale(scale)
        .style(KnobStyle::Wired)
        .value_text(res_text)
        .show(ui);
    let drive_text = format!("{:.0}%", state.filter_drive * 100.0);
    let r_drive = Knob::new(&mut state.filter_drive, 0.0..=1.0, "Drive")
        .size(knob_sm)
        .scale(scale)
        .style(KnobStyle::Wired)
        .value_text(drive_text)
        .show(ui);
    let mut local_changed = r1.changed || r2.changed || r_drive.changed;
    if config.show_osc_column {
        let key_text = format!("{:.0}%", state.filter_key_tracking * 100.0);
        let r3 = Knob::new(&mut state.filter_key_tracking, 0.0..=1.0, "Key")
            .size(knob_sm)
            .scale(scale)
            .style(KnobStyle::Wired)
            .value_text(key_text)
            .show(ui);
        let f2_text = format_cutoff(state.filter2_cutoff);
        let r4 = Knob::new(&mut state.filter2_cutoff, 40.0..=12000.0, "F2 Cut")
            .size(knob_sm)
            .scale(scale)
            .style(KnobStyle::Wired)
            .logarithmic(true)
            .value_text(f2_text)
            .show(ui);
        local_changed |= r3.changed || r4.changed;
    }
    if local_changed {
        actions.params_changed = true;
        if state.filter_slots.is_empty() {
            state.filter_slots.push(crate::FilterSlotUi::default_new());
        }
        if let Some(s0) = state.filter_slots.first_mut() {
            s0.cutoff = state.filter_cutoff;
            s0.resonance = state.filter_resonance;
            s0.drive = state.filter_drive;
            s0.key_tracking = state.filter_key_tracking;
            s0.filter_type = crate::filter_type_from_mode(state.filter_mode).into();
        }
        if let Some(s1) = state.filter_slots.get_mut(1) {
            s1.cutoff = state.filter2_cutoff;
            s1.resonance = state.filter2_resonance;
            s1.drive = state.filter2_drive;
            s1.filter_type = crate::filter_type_from_mode(state.filter2_mode).into();
        } else if config.show_osc_column && state.filter_slots.len() == 1 {
            let mut s1 = crate::FilterSlotUi::default_new();
            s1.cutoff = state.filter2_cutoff;
            s1.resonance = state.filter2_resonance;
            s1.drive = state.filter2_drive;
            s1.filter_type = crate::filter_type_from_mode(state.filter2_mode).into();
            state.filter_slots.push(s1);
        }
    }
}

fn lfo_block(
    ui: &mut Ui,
    title: &str,
    rate: &mut f32,
    depth: &mut f32,
    shape: &mut usize,
    style: KnobStyle,
    actions: &mut ShellActions,
    scale: f32,
) {
    ui.push_id(title, |ui| {
        ui.label(
            egui::RichText::new(title)
                .size(10.0)
                .color(Tokens::default().text_muted),
        );
        lfo_panel(ui, rate, depth, shape, style, actions, scale);
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
    ui.set_max_width(ui.available_width());
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
    ui.horizontal(|ui| {
        ui.set_max_width(ui.available_width());
        if labeled_select(ui, "Shape", &["Sine", "Tri", "Saw", "S&H"], shape) {
            actions.params_changed = true;
        }
    });
}

fn clamp_used_below_start(used: Rect, start: egui::Pos2) -> Rect {
    Rect::from_min_max(
        egui::pos2(used.min.x, used.min.y.max(start.y)),
        used.max,
    )
}
