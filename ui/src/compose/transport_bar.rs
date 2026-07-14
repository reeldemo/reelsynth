//! Compose mode transport controls.

use egui::{Rect, Ui};
use reelsynth::QuantizeDivision;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::layout::{GRID_UNIT, SPACE_SM};
use crate::region::region;
use crate::widgets::{button_toggle, button_tool};

use super::ComposeUi;

pub struct TransportBarActions {
    pub play: bool,
    pub stop: bool,
    pub record: bool,
    pub params_changed: bool,
}

impl Default for TransportBarActions {
    fn default() -> Self {
        Self {
            play: false,
            stop: false,
            record: false,
            params_changed: false,
        }
    }
}

pub fn draw_transport_bar(
    ui: &mut Ui,
    rect: Rect,
    compose: &mut ComposeUi,
) -> TransportBarActions {
    let tokens = Tokens::default();
    let mut actions = TransportBarActions::default();

    region(ui, rect, |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(SPACE_SM, GRID_UNIT * 0.5))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = GRID_UNIT * 0.5;

                    if button_tool(ui, "▶", false, true).clicked() {
                        compose.transport.playing = true;
                        compose.transport.recording = false;
                        actions.play = true;
                    }
                    if button_tool(ui, "■", compose.transport.playing, true).clicked() {
                        compose.transport.playing = false;
                        compose.transport.recording = false;
                        actions.stop = true;
                    }
                    if button_tool(
                        ui,
                        "●",
                        compose.transport.recording,
                        compose.armed_track().is_some(),
                    )
                    .clicked()
                    {
                        compose.transport.recording = !compose.transport.recording;
                        if compose.transport.recording {
                            compose.transport.playing = true;
                        }
                        actions.record = true;
                    }

                    ui.separator();

                    if button_toggle(ui, "Loop", compose.transport.loop_enabled).clicked() {
                        compose.transport.loop_enabled = !compose.transport.loop_enabled;
                        compose.project.loop_region.enabled = compose.transport.loop_enabled;
                        actions.params_changed = true;
                    }
                    if button_toggle(ui, "Metro", compose.transport.metronome).clicked() {
                        compose.transport.metronome = !compose.transport.metronome;
                        actions.params_changed = true;
                    }

                    ui.separator();

                    ui.label(
                        egui::RichText::new("BPM")
                            .size(10.0)
                            .color(tokens.text_muted),
                    );
                    let mut bpm = compose.project.bpm;
                    if ui
                        .add(
                            egui::DragValue::new(&mut bpm)
                                .speed(0.5)
                                .range(20.0..=300.0)
                                .fixed_decimals(1),
                        )
                        .changed()
                    {
                        compose.project.bpm = bpm;
                        actions.params_changed = true;
                    }

                    ui.separator();

                    ui.label(
                        egui::RichText::new("Snap")
                            .size(10.0)
                            .color(tokens.text_muted),
                    );
                    for div in [
                        QuantizeDivision::Quarter,
                        QuantizeDivision::Eighth,
                        QuantizeDivision::Sixteenth,
                        QuantizeDivision::EighthTriplet,
                    ] {
                        let label = match div {
                            QuantizeDivision::Quarter => "1/4",
                            QuantizeDivision::Eighth => "1/8",
                            QuantizeDivision::Sixteenth => "1/16",
                            QuantizeDivision::EighthTriplet => "1/8T",
                            QuantizeDivision::SixteenthTriplet => "1/16T",
                        };
                        if button_toggle(ui, label, compose.snap_division == div).clicked() {
                            compose.snap_division = div;
                            compose.project.quantize.division = div;
                            actions.params_changed = true;
                        }
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let beats = compose.transport.playhead_beats;
                        let bar =
                            (beats / compose.project.time_sig_num as f32).floor() as u32 + 1;
                        let beat =
                            (beats % compose.project.time_sig_num as f32).floor() as u32 + 1;
                        ui.label(
                            egui::RichText::new(format!(
                                "{bar}.{} · {:.1} BPM",
                                beat, compose.project.bpm
                            ))
                            .font(egui::FontId::monospace(11.0))
                            .color(ACCENT_UI),
                        );
                    });
                });
            });
    });

    actions
}
