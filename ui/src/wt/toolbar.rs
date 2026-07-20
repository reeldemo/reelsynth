//! Draw/edit tool strip above the Selected waveform view.

use egui::{Sense, Ui};
use reelsynth_ui_theme::Tokens;

use crate::layout::{RADIUS_SM, WT_TOOLBAR_HEIGHT};
use crate::quant_interp::{toolbar_curve_label, toolbar_segment_label, WtQuantInterp};
use crate::region::region;
use crate::widgets::button_tool;
use crate::wt::QuantSeamMode;

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
        matches!(self, Self::Select | Self::Shape)
    }

    /// Tools shown in the Design Selected toolbar (Curve morph editor removed from UI).
    fn toolbar_candidates() -> [Self; 2] {
        [Self::Select, Self::Shape]
    }
}

pub struct WtToolbarResponse {
    pub tool_changed: bool,
    pub analyze_requested: bool,
    pub assign_shape: Option<FrameShapeTemplate>,
    /// Curve-wide Interp changed (apply to all segments).
    pub interp_changed: bool,
    /// Per-segment Interp changed for the selected knob's outgoing segment.
    pub segment_interp_changed: bool,
    /// Wrap-seam reduction mode changed.
    pub seam_changed: bool,
    /// Artistic crackle amount (patch.crackle) changed.
    pub crackle_changed: bool,
}

pub struct WtToolbar;

impl WtToolbar {
    pub fn show(
        ui: &mut Ui,
        tool: &mut WtEditTool,
        wave_quant: u8,
        curve_interp: &mut WtQuantInterp,
    ) -> bool {
        Self::show_with_analyze(ui, tool, wave_quant, curve_interp, None, None, None, None)
            .tool_changed
    }

    pub fn show_with_analyze(
        ui: &mut Ui,
        tool: &mut WtEditTool,
        wave_quant: u8,
        curve_interp: &mut WtQuantInterp,
        selected_slot: Option<usize>,
        segment_interp: Option<&mut WtQuantInterp>,
        seam_mode: Option<&mut QuantSeamMode>,
        crackle_amount: Option<&mut f32>,
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
        let mut segment_interp_changed = false;
        let mut seam_changed = false;
        let mut crackle_changed = false;

        if !ui.is_rect_visible(rect) {
            return WtToolbarResponse {
                tool_changed,
                analyze_requested,
                assign_shape,
                interp_changed,
                segment_interp_changed,
                seam_changed,
                crackle_changed,
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
            ui.set_clip_rect(rect);
            // Coerce legacy Curve/Pencil selection to Select (removed from toolbar).
            if matches!(
                *tool,
                WtEditTool::Curve | WtEditTool::Pencil | WtEditTool::Line | WtEditTool::Smooth
            ) {
                *tool = WtEditTool::Select;
                tool_changed = true;
            }
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 2.0;
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    for candidate in WtEditTool::toolbar_candidates() {
                        let hover = match candidate {
                            WtEditTool::Select => {
                                if wave_quant > 0 {
                                    "Drag knobs to reshape this frame · drag background to scan"
                                } else {
                                    "Drag waveform to reshape this frame · drag background to scan"
                                }
                            }
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
                        if btn.clicked() && candidate.enabled() {
                            *tool = candidate;
                            tool_changed = true;
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
                            if ui.button(label).on_hover_text(tip).clicked() {
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
                });
                if wave_quant > 0 {
                    ui.horizontal_wrapped(|ui| {
                        const COMBO_W: f32 = 72.0;
                        ui.spacing_mut().item_spacing.x = 4.0;
                        let combo = egui::ComboBox::from_id_salt("wt_quant_interp_curve")
                            .selected_text(toolbar_curve_label(*curve_interp))
                            .width(COMBO_W)
                            .show_ui(ui, |ui| {
                                for (idx, &label) in WtQuantInterp::LABELS.iter().enumerate() {
                                    let mode = WtQuantInterp::from_index(idx);
                                    if ui
                                        .selectable_label(curve_interp.index() == idx, label)
                                        .on_hover_text(mode.tooltip())
                                        .clicked()
                                        && *curve_interp != mode
                                    {
                                        *curve_interp = mode;
                                        interp_changed = true;
                                    }
                                }
                            });
                        combo.response.on_hover_text(
                            "Curve default — apply this interp to all segments on this layer",
                        );

                        if let Some(seg) = segment_interp {
                            let slot = selected_slot.unwrap_or(0);
                            let combo = egui::ComboBox::from_id_salt("wt_quant_interp_seg")
                                .selected_text(toolbar_segment_label(slot, *seg))
                                .width(COMBO_W)
                                .show_ui(ui, |ui| {
                                    for (idx, &label) in WtQuantInterp::LABELS.iter().enumerate() {
                                        let mode = WtQuantInterp::from_index(idx);
                                        if ui
                                            .selectable_label(seg.index() == idx, label)
                                            .on_hover_text(mode.tooltip())
                                            .clicked()
                                            && *seg != mode
                                        {
                                            *seg = mode;
                                            segment_interp_changed = true;
                                        }
                                    }
                                });
                            combo.response.on_hover_text(format!(
                                "Segment {} → {} interp (selected knob)",
                                slot + 1,
                                slot + 2
                            ));
                        } else if let Some(slot) = selected_slot {
                            let slot_count = crate::wt::effective_quant_count(wave_quant).max(1);
                            if slot + 1 >= slot_count {
                                ui.label(
                                    egui::RichText::new("end · wrap")
                                        .size(10.0)
                                        .color(tokens.text_secondary),
                                )
                                .on_hover_text(
                                    "Last Quant knob is the wrap point (linked with first when Seam ≠ Off)",
                                );
                            }
                        }
                        if let Some(seam) = seam_mode {
                            let combo = egui::ComboBox::from_id_salt("wt_quant_seam")
                                .selected_text(seam.label())
                                .width(COMBO_W + 8.0)
                                .show_ui(ui, |ui| {
                                    for (idx, &label) in QuantSeamMode::LABELS.iter().enumerate() {
                                        let mode = QuantSeamMode::from_index(idx);
                                        if ui
                                            .selectable_label(seam.index() == idx, label)
                                            .on_hover_text(mode.tooltip())
                                            .clicked()
                                            && *seam != mode
                                        {
                                            *seam = mode;
                                            seam_changed = true;
                                        }
                                    }
                                });
                            combo.response.on_hover_text(
                                "Wrap crackle reduction — Adaptive fades only as needed",
                            );
                        }
                        if let Some(crackle) = crackle_amount {
                            crackle_changed |= paint_crackle_slider(ui, crackle, &tokens);
                        }
                    });
                } else if let Some(crackle) = crackle_amount {
                    ui.horizontal(|ui| {
                        crackle_changed |= paint_crackle_slider(ui, crackle, &tokens);
                    });
                }
            });
        });

        WtToolbarResponse {
            tool_changed,
            analyze_requested,
            assign_shape,
            interp_changed,
            segment_interp_changed,
            seam_changed,
            crackle_changed,
        }
    }
}

fn paint_crackle_slider(ui: &mut Ui, crackle: &mut f32, tokens: &Tokens) -> bool {
    ui.label(
        egui::RichText::new("Crackle")
            .size(10.0)
            .color(tokens.text_secondary),
    );
    let resp = ui
        .add_sized(
            egui::vec2(56.0, 14.0),
            egui::Slider::new(crackle, 0.0..=1.0).show_value(false),
        )
        .on_hover_text("0 = eliminate (clean default) · 1 = amplify wrap grit · modulatable");
    if resp.changed() {
        *crackle = crackle.clamp(0.0, 1.0);
        true
    } else {
        false
    }
}
