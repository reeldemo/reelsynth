//! Interaction prototype — knob drag, piano keys, panel chrome (Gate 2).

use eframe::egui;
use reelsynth_ui::widgets::{Knob, KnobSize, KnobStyle, panel};
use reelsynth_ui::{draw_s1, S1State};
use reelsynth_ui_theme;

struct ProtoApp {
    themed: bool,
    state: S1State,
}

impl Default for ProtoApp {
    fn default() -> Self {
        Self {
            themed: false,
            state: S1State::default(),
        }
    }
}

impl eframe::App for ProtoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.themed {
            reelsynth_ui_theme::apply(ctx);
            self.themed = true;
        }

        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill: reelsynth_ui_theme::Tokens::default().bg,
                ..Default::default()
            })
            .show(ctx, |ui| {
                let screen = ui.max_rect();
                let actions = draw_s1(ui, screen, &mut self.state, None);

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
        Box::new(|_cc| Ok(Box::new(ProtoApp::default()))),
    )
}
