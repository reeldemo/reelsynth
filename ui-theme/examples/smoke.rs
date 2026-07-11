//! Branded egui smoke test — S-brand exit gate.

fn main() -> eframe::Result<()> {
    eframe::run_native(
        "ReelSynth",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([480.0, 320.0])
                .with_title("ReelSynth — theme preview"),
            ..Default::default()
        },
        Box::new(|cc| {
            Ok(Box::new(SmokeApp {
                started: false,
                wt_position: 0.42,
                filter_cutoff: 1200.0,
            }))
        }),
    )
}

struct SmokeApp {
    started: bool,
    wt_position: f32,
    filter_cutoff: f32,
}

impl eframe::App for SmokeApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.started {
            reelsynth_ui_theme::apply(ctx);
            self.started = true;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("ReelSynth");
            ui.label("Majico palette:0 · IBM Plex Sans / Inter");
            ui.add_space(12.0);
            ui.horizontal(|ui| {
                let _ = ui.button("Play");
                let _ = ui.button("Export");
            });
            ui.add_space(16.0);
            ui.separator();
            ui.label("Wavetable position");
            ui.add(egui::Slider::new(&mut self.wt_position, 0.0..=1.0));
            ui.label("Filter cutoff");
            ui.add(egui::Slider::new(&mut self.filter_cutoff, 20.0..=20000.0).logarithmic(true));
        });
    }
}
