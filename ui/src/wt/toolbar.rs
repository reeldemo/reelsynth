//! Draw/edit tool strip above the 2D waveform view.

use egui::{Sense, Ui};
use reelsynth_ui_theme::Tokens;

use crate::layout::{RADIUS_SM, WT_TOOLBAR_HEIGHT};
use crate::region::region;
use crate::widgets::button_tool;

use super::quant_handles::WtQuantInterp;

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

/// Basic cycle templates applied to the strip-selected frame (click-to-assign).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameShapeTemplate {
    Saw,
    Square,
    Sine,
    Tri,
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
    pub assign_shape: Option<FrameShapeTemplate>,
    pub interp_changed: bool,
}

pub struct WtToolbar;

impl WtToolbar {
    pub fn show(ui: &mut Ui, tool: &mut WtEditTool, wave_quant: u8, quant_interp: &mut WtQuantInterp) -> bool {
        Self::show_with_analyze(ui, tool, wave_quant, quant_interp).tool_changed
    }

    pub fn show_with_analyze(
        ui: &mut Ui,
        tool: &mut WtEditTool,
        wave_quant: u8,
        quant_interp: &mut WtQuantInterp,
    ) -> WtToolbarResponse {
        let tokens = Tokens::default();
        let (rect, _) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), WT_TOOLBAR_HEIGHT),
            Sense::hover(),
        );

        let mut tool_changed = false;
        let mut analyze_requested = false;
        let mut assign_shape = None;
        let mut interp_changed = false;

        if !ui.is_rect_visible(rect) {
            return WtToolbarResponse {
                tool_changed,
                analyze_requested,
                assign_shape,
                interp_changed,
            };
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS_SM, tokens.surface2);
        painter.rect_stroke(
            rect,
            RADIUS_SM,
            egui::Stroke::new(1.0_f32, tokens.border),
        );

        // Clip interactions to the allocated strip so extras never expand the
        // Design half-pane used-rect past the center column.
        region(ui, rect.shrink2(egui::vec2(4.0, 2.0)), |ui| {
            ui.set_clip_rect(rect);
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for candidate in [
                    WtEditTool::Select,
                    WtEditTool::Curve,
                    WtEditTool::Shape,
                ] {
                    if candidate == WtEditTool::Pencil {
                        continue;
                    }
                    let hover = match candidate {
                        WtEditTool::Select => {
                            if wave_quant > 0 {
                                "Drag knobs to reshape this frame · drag background to scan"
                            } else {
                                "Drag waveform to reshape this frame · drag background to scan"
                            }
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
                ui.menu_button("Shape", |ui| {
                    for (label, tip, kind) in [
                        ("Saw", "Set active layer to saw", FrameShapeTemplate::Saw),
                        ("Square", "Set active layer to square", FrameShapeTemplate::Square),
                        ("Sine", "Set active layer to sine", FrameShapeTemplate::Sine),
                        ("Triangle", "Set active layer to triangle", FrameShapeTemplate::Tri),
                    ] {
                        if ui
                            .button(label)
                            .on_hover_text(tip)
                            .clicked()
                        {
                            assign_shape = Some(kind);
                            ui.close_menu();
                        }
                    }
                })
                .response
                .on_hover_text("Set basic shape on the active layer");
                ui.add_space(2.0);
                if ui
                    .small_button("FFT")
                    .on_hover_text("Decompose frame into sine harmonics (engine layers)")
                    .clicked()
                {
                    analyze_requested = true;
                }
                if wave_quant > 0 {
                    const COMBO_W: f32 = 84.0;
                    ui.add_space((ui.available_width() - COMBO_W).max(0.0));
                    let combo = egui::ComboBox::from_id_salt("wt_quant_interp")
                        .selected_text(quant_interp.label())
                        .width(COMBO_W - 4.0)
                        .show_ui(ui, |ui| {
                            for (idx, &label) in WtQuantInterp::LABELS.iter().enumerate() {
                                let mode = WtQuantInterp::from_index(idx);
                                if ui
                                    .selectable_label(quant_interp.index() == idx, label)
                                    .on_hover_text(mode.tooltip())
                                    .clicked()
                                {
                                    if *quant_interp != mode {
                                        *quant_interp = mode;
                                        interp_changed = true;
                                    }
                                }
                            }
                        });
                    combo.response.on_hover_text(
                        "Interpolation between quant knobs when reshaping the frame",
                    );
                }
            });
        });

        WtToolbarResponse {
            tool_changed,
            analyze_requested,
            assign_shape,
            interp_changed,
        }
    }
}
