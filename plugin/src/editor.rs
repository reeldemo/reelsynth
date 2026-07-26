//! Shared egui external editor — full ReelSynth UI connected to the Live VST via IPC.

use crate::ipc::IpcClient;
use crate::plugin_state::PluginStateV1;
use eframe::egui::{self, Key};
use reelsynth::{Patch, WavetableBank};
use reelsynth_ui::{
    draw_shell, patch_from_state, sync_state_from_patch, ShellAudioDevices, ShellConfig,
    ShellMidiDevices, UiState, APP_HEIGHT_FULL,
};

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
            title: "ReelSynth Editor (Ableton)".into(),
        }
    }
}

pub struct PluginEditorApp {
    pub state: UiState,
    pub config: PluginEditorConfig,
    midi_names: Vec<String>,
    audio_names: Vec<String>,
    client: Option<IpcClient>,
    patch: Patch,
    bank: WavetableBank,
    push_cooldown: u8,
    reconnect_tick: u32,
}

impl PluginEditorApp {
    pub fn new(config: PluginEditorConfig) -> Self {
        let mut app = Self {
            state: UiState {
                status: "Connecting to Ableton ReelSynth…".into(),
                ..UiState::default()
            },
            config,
            midi_names: vec!["Host MIDI (Live)".into()],
            audio_names: vec!["Host audio (Live)".into()],
            client: None,
            patch: Patch::default_mono(),
            bank: WavetableBank::factory_saw_morph(),
            push_cooldown: 0,
            reconnect_tick: 0,
        };
        app.try_connect();
        app
    }

    fn try_connect(&mut self) {
        match IpcClient::connect_from_manifest() {
            Ok(mut client) => match client.get_state() {
                Ok(state) => {
                    if let Ok((patch, bank)) = PluginStateV1::from_json(
                        &state.to_json().unwrap_or_else(|_| "{}".into()),
                    ) {
                        sync_state_from_patch(&mut self.state, &patch);
                        self.patch = patch;
                        self.bank = bank;
                    }
                    self.state.status =
                        "Connected — piano / Z–M keys play through Live. Edits push to the instrument."
                            .into();
                    self.client = Some(client);
                }
                Err(e) => {
                    self.state.status = format!("Connected but GetState failed: {e}");
                    self.client = Some(client);
                }
            },
            Err(e) => {
                self.state.status = format!("{e}  |  Retry: click status or relaunch editor.");
                self.client = None;
            }
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

    fn push_state(&mut self) {
        let state = PluginStateV1::from_patch_bank(&self.patch, &self.bank);
        if let Some(client) = self.client.as_mut() {
            match client.set_state(state) {
                Ok(()) => {
                    self.state.status =
                        "Pushed patch to Ableton · piano / Z–M play through Live.".into();
                }
                Err(e) => {
                    self.state.status = format!("Push failed: {e} — reconnecting…");
                    self.client = None;
                }
            }
        }
    }

    fn send_note_on(&mut self, note: u8, velocity: f32) {
        self.state.keys_down.insert(note);
        if let Some(client) = self.client.as_mut() {
            if let Err(e) = client.note_on(note, velocity) {
                self.state.status = format!("NoteOn failed: {e}");
                self.client = None;
            }
        }
    }

    fn send_note_off(&mut self, note: u8) {
        self.state.keys_down.remove(&note);
        if let Some(client) = self.client.as_mut() {
            if let Err(e) = client.note_off(note) {
                self.state.status = format!("NoteOff failed: {e}");
                self.client = None;
            }
        }
    }

    fn handle_computer_keys(&mut self, ctx: &egui::Context) {
        let octave = self.state.performance.input_octave_offset;
        let events: Vec<(Key, bool)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key {
                        key,
                        pressed,
                        repeat,
                        ..
                    } if !*repeat => Some((*key, *pressed)),
                    _ => None,
                })
                .collect()
        });
        for (key, pressed) in events {
            let Some(base) = qwerty_note(key) else {
                continue;
            };
            let note = shift_note(base, octave);
            if pressed {
                self.send_note_on(note, 0.9);
            } else {
                self.send_note_off(note);
            }
        }
    }
}

fn qwerty_note(key: Key) -> Option<u8> {
    match key {
        Key::Z => Some(48),
        Key::S => Some(49),
        Key::X => Some(50),
        Key::D => Some(51),
        Key::C => Some(52),
        Key::V => Some(53),
        Key::G => Some(54),
        Key::B => Some(55),
        Key::H => Some(56),
        Key::N => Some(57),
        Key::J => Some(58),
        Key::M => Some(59),
        _ => None,
    }
}

fn shift_note(note: u8, octave_offset: i8) -> u8 {
    let shifted = i16::from(note) + i16::from(octave_offset) * 12;
    shifted.clamp(0, 127) as u8
}

fn freq_to_midi(freq: f32) -> u8 {
    if freq <= 0.0 {
        return 69;
    }
    let midi = 69.0 + 12.0 * (freq / 440.0).log2();
    midi.round().clamp(0.0, 127.0) as u8
}

impl eframe::App for PluginEditorApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.client.is_none() {
            self.reconnect_tick = self.reconnect_tick.wrapping_add(1);
            if self.reconnect_tick % 120 == 0 {
                self.try_connect();
            }
        }
        if self.push_cooldown > 0 {
            self.push_cooldown -= 1;
        }

        self.handle_computer_keys(ctx);

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
                let audio = ShellAudioDevices {
                    names: &self.audio_names,
                    selected: 0,
                };
                let shell = ShellConfig {
                    show_wt_editor: self.config.show_wt_editor,
                    show_osc_column: self.config.show_osc_column,
                    show_mod_matrix: self.config.show_mod_matrix,
                    show_fx_rack: self.config.show_fx_rack,
                };
                let actions = draw_shell(
                    ui,
                    ui.max_rect(),
                    &mut self.state,
                    Some(&mut self.bank),
                    &self.patch,
                    &midi,
                    &audio,
                    &shell,
                    None,
                    None,
                    None,
                );
                if actions.params_changed || actions.frame_edited {
                    self.patch = patch_from_state(&self.state, &self.patch);
                    if self.push_cooldown == 0 {
                        self.push_state();
                        self.push_cooldown = 3;
                    }
                }
                if let Some(n) = actions.note_on {
                    self.send_note_on(n, 0.9);
                }
                if let Some(n) = actions.note_off {
                    self.send_note_off(n);
                }
                if let Some((freq, vel)) = actions.note_on_freq {
                    self.send_note_on(freq_to_midi(freq), vel);
                }
                if ui
                    .interact(
                        ui.max_rect(),
                        ui.id().with("reconnect_click"),
                        egui::Sense::click(),
                    )
                    .clicked()
                    && self.client.is_none()
                {
                    self.try_connect();
                }
            });
        ctx.request_repaint_after(std::time::Duration::from_millis(16));
    }
}
