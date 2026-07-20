//! Interaction prototype — knob drag, piano keys, panel chrome (Gate 2).

use eframe::egui;
use reelsynth_ui::widgets::{Knob, KnobSize, KnobStyle, panel};
use reelsynth::Patch;
use reelsynth_ui::{draw_shell, ShellAudioDevices, ShellMidiDevices, ShellConfig, UiState};
use reelsynth_ui_theme;

struct ProtoApp {
    state: UiState,
    midi_names: Vec<String>,
    audio_names: Vec<String>,
}

impl Default for ProtoApp {
    fn default() -> Self {
        Self {
            state: UiState::default(),
            midi_names: vec!["None".into(), "Demo MIDI".into()],
            audio_names: vec!["Speakers".into()],
        }
    }
}

impl eframe::App for ProtoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill: reelsynth_ui_theme::Tokens::default().bg,
                ..Default::default()
            })
            .show(ctx, |ui| {
                let screen = ui.max_rect();
                let midi = ShellMidiDevices {
                    names: &self.midi_names,
                    selected: 0,
                };
                let audio = ShellAudioDevices {
                    names: &self.audio_names,
                    selected: 0,
                };
                let config = ShellConfig::default();
                let preview = Patch::default_mono();
                let actions = draw_shell(ui, screen, &mut self.state, None, &preview, &midi, &audio, &config, None, None, None);

                if let Some(n) = actions.note_on {
                    self.state.keys_down.insert(n);
                }
                if let Some(n) = actions.note_off {
                    self.state.keys_down.remove(&n);
                }
            });

        // Isolated widget demo in a floating window
        egui::Window::new("Widget demo")
            .default_pos([20.0, 20.0])
            .show(ctx, |ui| {
                panel(ui, "Knob", |ui| {
                    Knob::new(&mut self.state.wt_position, 0.0..=255.0, "WT Position")
                        .size(KnobSize::Lg)
                        .style(KnobStyle::Wired)
                        .show(ui);
                    let mut v = 0.5_f32;
                    Knob::new(&mut v, 0.0..=1.0, "Disabled")
                        .size(KnobSize::Sm)
                        .style(KnobStyle::Disabled)
                        .show(ui);
                });
            });
    }
}

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "ReelSynth UI Proto",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1280.0, 720.0])
                .with_title("ReelSynth UI Proto"),
            ..Default::default()
        },
        Box::new(|cc| {
            reelsynth_ui_theme::apply(&cc.egui_ctx);
            Ok(Box::new(ProtoApp::default()))
        }),
    )
}
