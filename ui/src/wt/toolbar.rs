//! Draw/edit tool strip above the 2D waveform view.

use egui::{Sense, Ui};
use reelsynth_ui_theme::Tokens;

use crate::layout::{RADIUS_SM, WT_TOOLBAR_HEIGHT};
use crate::widgets::button_tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WtEditTool {
    #[default]
    Select,
    Pencil,
    Line,
    Smooth,
}

impl WtEditTool {
    fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Pencil => "Pencil",
            Self::Line => "Line",
            Self::Smooth => "Smooth",
        }
    }

    fn enabled(self) -> bool {
        matches!(self, Self::Select | Self::Pencil)
    }
}

pub struct WtToolbar;

impl WtToolbar {
    pub fn show(ui: &mut Ui, tool: &mut WtEditTool) -> bool {
        let tokens = Tokens::default();
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), WT_TOOLBAR_HEIGHT),
            Sense::hover(),
        );

        if !ui.is_rect_visible(rect) {
            return false;
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS_SM, tokens.surface2);
        painter.rect_stroke(
            rect,
            RADIUS_SM,
            egui::Stroke::new(1.0_f32, tokens.border),
        );

        let mut changed = false;
        ui.allocate_ui_at_rect(rect.shrink2(egui::vec2(4.0, 2.0)), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for candidate in [
                    WtEditTool::Select,
                    WtEditTool::Pencil,
                    WtEditTool::Line,
                    WtEditTool::Smooth,
                ] {
                    if button_tool(
                        ui,
                        candidate.label(),
                        *tool == candidate,
                        candidate.enabled(),
                    )
                    .clicked()
                    {
                        if candidate.enabled() {
                            *tool = candidate;
                            changed = true;
                        }
                    }
                }
                ui.add_space(6.0);
                let hint = match *tool {
                    WtEditTool::Pencil => "Drag on waveform to sculpt frame",
                    WtEditTool::Select => "Click strip or knob to change position",
                    _ => "",
                };
                if !hint.is_empty() {
                    ui.label(
                        egui::RichText::new(hint)
                            .size(10.0)
                            .color(tokens.text_muted),
                    );
                }
            });
        });

        changed
    }
}
