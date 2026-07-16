//! Shared egui editor host for plugin + minimal spike binary (S6).

use eframe::egui;
use reelsynth::Patch;
use reelsynth_ui::{
    draw_shell, APP_HEIGHT_FULL, ShellMidiDevices, ShellConfig, UiState,
};

/// Configuration for the plugin editor surface.
#[derive(Debug, Clone)]
pub struct PluginEditorConfig {
    pub show_wt_editor: bool,
    pub show_osc_column: bool,
    pub show_mod_matrix: bool,
    pub show_fx_rack: bool,
    pub title: String,
}

impl Default for PluginEditorConfig {
    fn default() -> Self {
        Self {
            show_wt_editor: true,
            show_osc_column: true,
            show_mod_matrix: true,
            show_fx_rack: true,
            title: "ReelSynth (plugin editor spike)".into(),
        }
    }
}

/// Minimal egui host embedding the shared `reelsynth-app` S1 shell.
pub struct PluginEditorApp {
    pub state: UiState,
    pub config: PluginEditorConfig,
    midi_names: Vec<String>,
}

impl PluginEditorApp {
    pub fn new(config: PluginEditorConfig) -> Self {
        Self {
            state: UiState {
                status: "Plugin editor spike — UI only (no audio I/O)".into(),
                ..UiState::default()
            },
            config,
            midi_names: vec!["None".into()],
        }
    }

    pub fn run_native(config: PluginEditorConfig) -> eframe::Result<()> {
        let title = config.title.clone();
        let window_title = title.clone();
        eframe::run_native(
            &title,
            eframe::NativeOptions {
                viewport: egui::ViewportBuilder::default()
                    .with_inner_size([1280.0, APP_HEIGHT_FULL])
                    .with_min_inner_size([1024.0, 640.0])
                    .with_title(window_title),
                ..Default::default()
            },
            Box::new(move |cc| {
                reelsynth_ui_theme::apply(&cc.egui_ctx);
                Ok(Box::new(PluginEditorApp::new(config)))
            }),
        )
    }
}

impl eframe::App for PluginEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill: reelsynth_ui_theme::Tokens::default().bg,
                ..Default::default()
            })
            .show(ctx, |ui| {
                let midi = ShellMidiDevices {
                    names: &self.midi_names,
                    selected: 0,
                };
                let shell = ShellConfig {
                    show_wt_editor: self.config.show_wt_editor,
                    show_osc_column: self.config.show_osc_column,
                    show_mod_matrix: self.config.show_mod_matrix,
                    show_fx_rack: self.config.show_fx_rack,
                };
                let preview = Patch::default_mono();
                let actions = draw_shell(
                    ui,
                    ui.max_rect(),
                    &mut self.state,
                    None,
                    &preview,
                    &midi,
                    &shell,
                    None,
                    None,
                    None,
                );

                if let Some(n) = actions.note_on {
                    self.state.keys_down.insert(n);
                }
                if let Some(n) = actions.note_off {
                    self.state.keys_down.remove(&n);
                }
            });
    }
}
