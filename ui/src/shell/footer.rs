use egui::{Rect, Ui};
use reelsynth_ui_theme::Tokens;

use super::*;
use crate::layout::UiScale;
use crate::layout_audit::{footer_used_rect_id, piano_used_rect_id};
use crate::performance::draw_arp_panel;
use crate::performance::draw_chord_grid;
use crate::region::region;
use crate::widgets::button_ghost;
use reelsynth::PerformanceLayout;

pub(super) fn draw_level_meter(ui: &mut Ui) {
    let tokens = Tokens::default();
    let (rect, _) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 32.0), egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let painter = ui.painter_at(rect);
        let bar_w = 8.0;
        let gap = 5.0;
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

pub(super) fn draw_piano_wrap(
    ui: &mut Ui,
    rect: Rect,
    state: &mut UiState,
    actions: &mut ShellActions,
    _scale: UiScale,
) {
    region(ui, rect, |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(SPACE_SM, GRID_UNIT))
            .show(ui, |ui| {
                let inner = ui.max_rect();
                let perf = state.performance.to_settings();
                let chords_layout = state.shell_mode == crate::state::ShellMode::Design
                    && perf.layout == PerformanceLayout::Chords;

                if chords_layout {
                    let grid_actions = draw_chord_grid(ui, inner, state);
                    if let Some(deg) = grid_actions.chord_degree_on {
                        actions.chord_degree_on = Some(deg);
                    }
                    if let Some(deg) = grid_actions.chord_degree_off {
                        actions.chord_degree_off = Some(deg);
                    }
                } else {
                    let scale_fold = state.shell_mode == crate::state::ShellMode::Compose
                        || perf.layout == PerformanceLayout::Scale;
                    let piano = PianoKeyboard::new(&state.keys_down)
                        .with_scale_fold(perf.root, perf.scale, scale_fold);
                    let (_, piano) = piano.show_in_rect(ui, inner);
                    if let Some(n) = piano.note_on {
                        actions.note_on = Some(n);
                    }
                    if let Some(n) = piano.note_off {
                        actions.note_off = Some(n);
                    }
                }
            });
        let used = ui.min_rect();
        ui.ctx()
            .data_mut(|d| d.insert_temp(piano_used_rect_id(), used));
    });
}

pub(super) fn draw_footer(ui: &mut Ui, rect: Rect, state: &mut UiState, actions: &mut ShellActions) {
    let tokens = Tokens::default();
    region(ui, rect, |ui| {
        ui.set_min_height(rect.height());
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(SPACE_SM, 0.0))
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                    ui.set_min_height(rect.height());
                    ui.spacing_mut().item_spacing.x = GRID_UNIT;

                    ui.label(
                        egui::RichText::new("Performance")
                            .size(10.0)
                            .color(tokens.text_muted),
                    );

                    let arp_actions = draw_arp_panel(ui, state);
                    if arp_actions.params_changed {
                        actions.params_changed = true;
                    }

                    ui.label(
                        egui::RichText::new("Hz")
                            .size(10.0)
                            .color(tokens.text_muted),
                    );
                    let resp = ui.add(
                        egui::TextEdit::singleline(&mut state.custom_hz_input)
                            .desired_width(56.0)
                            .font(FontId::monospace(10.0)),
                    );
                    if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Ok(freq) = state.custom_hz_input.trim().parse::<f32>() {
                            if freq > 0.0 {
                                actions.note_on_freq = Some((freq, 0.9));
                            }
                        }
                    }
                    if button_ghost(ui, "Play Hz").clicked() {
                        if let Ok(freq) = state.custom_hz_input.trim().parse::<f32>() {
                            if freq > 0.0 {
                                actions.note_on_freq = Some((freq, 0.9));
                            }
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.set_width(ui.available_width());
                        let wt = state.wt_position.round() as i32;
                        ui.label(
                            egui::RichText::new(format!(
                                "WT {wt} · Cutoff {}",
                                format_cutoff(state.filter_cutoff)
                            ))
                            .font(FontId::monospace(10.0))
                            .color(tokens.text_muted),
                        );
                    });
                });
            });
        let used = ui.min_rect();
        ui.ctx()
            .data_mut(|d| d.insert_temp(footer_used_rect_id(), used));
    });
}

pub(super) fn format_cutoff(hz: f32) -> String {
    if hz >= 1000.0 {
        format!("{:.1} kHz", hz / 1000.0)
    } else {
        format!("{:.0} Hz", hz)
    }
}
