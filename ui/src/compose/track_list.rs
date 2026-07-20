//! Track list — mute / solo / arm / select.

use egui::{Rect, Ui};
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::audit_registry::{record_region, AuditId};
use crate::layout::{GRID_UNIT, SPACE_SM};
use crate::region::region;
use crate::widgets::button_tool;

use super::ComposeUi;

pub struct TrackListActions {
    pub selection_changed: bool,
    pub track_state_changed: bool,
}

impl Default for TrackListActions {
    fn default() -> Self {
        Self {
            selection_changed: false,
            track_state_changed: false,
        }
    }
}

pub fn draw_track_list(ui: &mut Ui, rect: Rect, compose: &mut ComposeUi) -> TrackListActions {
    let tokens = Tokens::default();
    let mut actions = TrackListActions::default();

    region(ui, rect, |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(SPACE_SM * 0.5, GRID_UNIT))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Tracks")
                        .size(10.0)
                        .color(tokens.text_muted),
                );
                ui.add_space(GRID_UNIT * 0.5);

                egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        let track_count = compose.project.tracks.len();
                        for ti in 0..track_count {
                            let row_before = ui.min_rect();
                            let selected = compose.selected_track == ti;
                            let (name, mute, solo, arm) = {
                                let t = &compose.project.tracks[ti];
                                (t.name.clone(), t.mute, t.solo, t.arm)
                            };

                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = 2.0;

                                let sel_resp = ui.selectable_label(selected, "");
                                if sel_resp.clicked() {
                                    compose.selected_track = ti;
                                    compose.ensure_editable_clip();
                                    compose.selected_notes.clear();
                                    actions.selection_changed = true;
                                }

                                if button_tool(ui, "M", mute, true).clicked() {
                                    compose.project.tracks[ti].mute = !mute;
                                    actions.track_state_changed = true;
                                }
                                if button_tool(ui, "S", solo, true).clicked() {
                                    compose.project.tracks[ti].solo = !solo;
                                    actions.track_state_changed = true;
                                }
                                if button_tool(ui, "R", arm, true).clicked() {
                                    let was_armed = arm;
                                    for t in &mut compose.project.tracks {
                                        t.arm = false;
                                    }
                                    compose.project.tracks[ti].arm = !was_armed;
                                    actions.track_state_changed = true;
                                }

                                let label_color = if selected {
                                    ACCENT_UI
                                } else if arm {
                                    tokens.text
                                } else {
                                    tokens.text_secondary
                                };
                                if ui
                                    .selectable_label(
                                        selected,
                                        egui::RichText::new(&name).size(11.0).color(label_color),
                                    )
                                    .clicked()
                                {
                                    compose.selected_track = ti;
                                    compose.ensure_editable_clip();
                                    compose.selected_notes.clear();
                                    actions.selection_changed = true;
                                }
                            });
                            record_region(
                                ui.ctx(),
                                AuditId::ComposeTrackRow(ti),
                                row_before,
                                ui.min_rect(),
                            );
                        }
                    });
            });
    });

    actions
}
