//! Scene launch grid — 8 scenes × track columns.

use egui::{Rect, Sense, Ui};
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::layout::GRID_UNIT;
use crate::region::region;

use super::ComposeUi;

pub struct SceneGridActions {
    pub scene_launched: Option<usize>,
}

impl Default for SceneGridActions {
    fn default() -> Self {
        Self {
            scene_launched: None,
        }
    }
}

pub fn draw_scene_grid(ui: &mut Ui, rect: Rect, compose: &mut ComposeUi) -> SceneGridActions {
    let tokens = Tokens::default();
    let mut actions = SceneGridActions::default();

    region(ui, rect, |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(GRID_UNIT, GRID_UNIT * 0.5))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("Scenes")
                        .size(10.0)
                        .color(tokens.text_muted),
                );
                ui.add_space(GRID_UNIT * 0.25);

                let scene_count = compose.project.scenes.len();
                let track_count = compose.project.tracks.len();
                if scene_count == 0 || track_count == 0 {
                    return;
                }

                let cell_w = (ui.available_width() / track_count as f32).max(32.0);
                let cell_h = 22.0;

                for (si, scene) in compose.project.scenes.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 2.0;
                        ui.label(
                            egui::RichText::new(&scene.name)
                                .size(9.0)
                                .color(tokens.text_secondary),
                        );
                        for slot in scene.slots.iter().take(track_count) {
                            let filled = slot.is_some();
                            let launched = compose.launched_scene == Some(si);
                            let (cell_rect, resp) = ui.allocate_exact_size(
                                egui::vec2(cell_w, cell_h),
                                Sense::click(),
                            );
                            let fill = if launched && filled {
                                ACCENT_UI.gamma_multiply(0.7)
                            } else if filled {
                                tokens.accent.gamma_multiply(0.5)
                            } else {
                                tokens.surface2
                            };
                            ui.painter_at(cell_rect).rect_filled(cell_rect, 4.0, fill);
                            ui.painter_at(cell_rect).rect_stroke(
                                cell_rect,
                                4.0,
                                egui::Stroke::new(1.0_f32, tokens.border_strong),
                            );
                            if filled {
                                ui.painter_at(cell_rect).text(
                                    cell_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "▶",
                                    egui::FontId::proportional(10.0),
                                    tokens.text,
                                );
                            }
                            if resp.clicked() {
                                compose.launched_scene = Some(si);
                                compose.active_scene_slots = scene.slots.clone();
                                actions.scene_launched = Some(si);
                            }
                        }
                    });
                }
            });
    });

    actions
}
