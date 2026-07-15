//! Draw/edit tool strip above the 2D waveform view.

use egui::{Sense, Ui};
use reelsynth_ui_theme::Tokens;

use crate::layout::{RADIUS_SM, WT_TOOLBAR_HEIGHT};
use crate::region::region;
use crate::widgets::button_tool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WtEditTool {
    #[default]
    Select,
    Pencil,
    Curve,
    Shape,
    Line,
    Smooth,
}

impl WtEditTool {
    fn label(self) -> &'static str {
        match self {
            Self::Select => "Select",
            Self::Pencil => "Pencil",
            Self::Curve => "Curve",
            Self::Shape => "Shape",
            Self::Line => "Line",
            Self::Smooth => "Smooth",
        }
    }

    fn enabled(self) -> bool {
        matches!(
            self,
            Self::Select | Self::Pencil | Self::Curve | Self::Shape
        )
    }
}

pub struct WtToolbarResponse {
    pub tool_changed: bool,
    pub analyze_requested: bool,
}

pub struct WtToolbar;

impl WtToolbar {
    pub fn show(ui: &mut Ui, tool: &mut WtEditTool) -> bool {
        Self::show_with_analyze(ui, tool).tool_changed
    }

    pub fn show_with_analyze(ui: &mut Ui, tool: &mut WtEditTool) -> WtToolbarResponse {
        let tokens = Tokens::default();
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), WT_TOOLBAR_HEIGHT),
            Sense::hover(),
        );

        let mut tool_changed = false;
        let mut analyze_requested = false;

        if !ui.is_rect_visible(rect) {
            return WtToolbarResponse {
                tool_changed,
                analyze_requested,
            };
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS_SM, tokens.surface2);
        painter.rect_stroke(
            rect,
            RADIUS_SM,
            egui::Stroke::new(1.0_f32, tokens.border),
        );

        region(ui, rect.shrink2(egui::vec2(4.0, 2.0)), |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for candidate in [
                    WtEditTool::Select,
                    WtEditTool::Curve,
                    WtEditTool::Shape,
                    WtEditTool::Pencil,
                ] {
                    let hover = match candidate {
                        WtEditTool::Select => {
                            "Drag waveform to shape · drag background to scan position"
                        }
                        WtEditTool::Pencil => "Freehand draw (advanced)",
                        WtEditTool::Curve => "Edit slot → frame morph curve",
                        WtEditTool::Shape => "Edit control points on the cycle",
                        _ => "",
                    };
                    let mut btn = button_tool(
                        ui,
                        candidate.label(),
                        *tool == candidate,
                        candidate.enabled(),
                    );
                    if !hover.is_empty() {
                        btn = btn.on_hover_text(hover);
                    }
                    if btn.clicked() {
                        if candidate.enabled() {
                            *tool = candidate;
                            tool_changed = true;
                        }
                    }
                }
                ui.add_space(4.0);
                if ui
                    .small_button("FFT→Stack")
                    .on_hover_text("Decompose frame into sine harmonics")
                    .clicked()
                {
                    analyze_requested = true;
                }
            });
        });

        WtToolbarResponse {
            tool_changed,
            analyze_requested,
        }
    }
}
