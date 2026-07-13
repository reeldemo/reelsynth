//! ReelSynth standalone application state.

use crate::audio_commands::AudioCmd;
use crate::audio_host::AudioHandle;
use crate::midi_host::{MidiDevices, MidiInputHandle};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use reelsynth::import::{import_serum_fxp, import_vital, import_wav_folder};
use reelsynth::{load_preset, resolve_bank_for_preset, Envelope, MidiEvent, Patch, ScopeMonitor, WavetableBank};
use reelsynth_ui::{
    draw_shell, effect_slots_to_patch, factory_bank, factory_label, fm_source_from_index,
    filter_type_from_mode, lfo_shape_from_index, mod_slots_to_patch, osc_type_from_index,
    patch_from_state, sync_state_from_patch, warp_mode_from_index, OscStripContext,
    OscStripPreviewState, ShellConfig, ShellMidiDevices, UiState, ScopeStripContext,
    ScopeStripState,
};
use std::path::{Path, PathBuf};
use std::sync::Arc;

fn resolve_bank(path: &Path, preset: &Patch) -> Result<WavetableBank, String> {
    resolve_bank_for_preset(path, preset).or_else(|_| match preset.wavetable_id.as_deref() {
        Some("saw_morph") => Ok(WavetableBank::factory_saw_morph()),
        Some(id) => Err(format!("could not resolve wavetable for id {id}")),
        None => Ok(WavetableBank::factory_saw_morph()),
    })
}


pub struct ReelSynthApp {
    audio: Option<Arc<AudioHandle>>,
    state: UiState,
    current_patch: Patch,
    preset_path: Option<PathBuf>,
    wt_path: Option<PathBuf>,
    midi_devices: MidiDevices,
    midi_selected: usize,
    midi_handle: MidiInputHandle,
    midi_event_tx: Sender<MidiEvent>,
    midi_event_rx: Receiver<MidiEvent>,
    scope: ScopeMonitor,
    scope_strip_state: ScopeStripState,
    osc_strip_state: OscStripPreviewState,
}

impl ReelSynthApp {
    pub fn new(
        audio: Option<Arc<AudioHandle>>,
        midi_devices: MidiDevices,
        midi_event_tx: Sender<MidiEvent>,
        midi_event_rx: Receiver<MidiEvent>,
    ) -> Self {
        let status = if audio.is_some() {
            "Audio OK — click keys, QWERTY (Z–M), or MIDI".into()
        } else {
            "No audio — UI only".into()
        };
        let midi_handle = MidiInputHandle::disconnected();
        let mut state = UiState {
            status,
            ..UiState::default()
        };
        let mut current_patch = Patch::default_mono();
        sync_state_from_patch(&mut state, &current_patch);
        current_patch.mod_matrix = mod_slots_to_patch(&state.mod_routes);
        current_patch.effects = effect_slots_to_patch(&state.fx_slots);
        let scope = audio.as_ref().map(|a| a.scope()).unwrap_or_default();
        Self {
            audio,
            state,
            current_patch,
            preset_path: None,
            wt_path: None,
            midi_devices,
            midi_selected: 0,
            midi_handle,
            midi_event_tx,
            midi_event_rx,
            scope,
            scope_strip_state: ScopeStripState::default(),
            osc_strip_state: OscStripPreviewState::default(),
        }
    }

    fn note_on(&mut self, note: u8, velocity: f32) {
        if self.state.keys_down.insert(note) {
            if let Some(a) = &self.audio {
                a.send(AudioCmd::Midi(MidiEvent::note_on(0, note, velocity)));
            }
        }
    }

    fn note_off(&mut self, note: u8) {
        if self.state.keys_down.remove(&note) {
            if let Some(a) = &self.audio {
                a.send(AudioCmd::Midi(MidiEvent::note_off(0, note)));
            }
        }
    }

    fn sync_params(&mut self) {
        if let Some(a) = &self.audio {
            a.send(AudioCmd::SetWtPosition(self.state.wt_position));
            a.send(AudioCmd::SetFilterCutoff(self.state.filter_cutoff));
            a.send(AudioCmd::SetFilterResonance(self.state.filter_resonance));
            a.send(AudioCmd::SetFilterType(
                filter_type_from_mode(self.state.filter_mode).into(),
            ));
            a.send(AudioCmd::SetFilterKeyTracking(self.state.filter_key_tracking));
            a.send(AudioCmd::SetFilterDrive(self.state.filter_drive));
            a.send(AudioCmd::SetFilter2 {
                cutoff: self.state.filter2_cutoff,
                resonance: self.state.filter2_resonance,
                filter_type: filter_type_from_mode(self.state.filter2_mode).into(),
                drive: self.state.filter2_drive,
            });
            a.send(AudioCmd::SetUnisonStereoSpread(self.state.unison_stereo_spread));
            a.send(AudioCmd::SetEnvelope(Envelope {
                attack: self.state.env_attack,
                decay: self.state.env_decay,
                sustain: self.state.env_sustain,
                release: self.state.env_release,
            }));
            a.send(AudioCmd::SetFilterEnvelope(Envelope {
                attack: self.state.filt_env_attack,
                decay: self.state.filt_env_decay,
                sustain: self.state.filt_env_sustain,
                release: self.state.filt_env_release,
            }));
            a.send(AudioCmd::SetLfo {
                rate: self.state.lfo_rate,
                depth: self.state.lfo_depth,
                shape: lfo_shape_from_index(self.state.lfo_shape).into(),
            });
            a.send(AudioCmd::SetLfo2 {
                rate: self.state.lfo2_rate,
                depth: self.state.lfo2_depth,
                shape: lfo_shape_from_index(self.state.lfo2_shape).into(),
            });
            let mut macros = self.current_patch.macros.clone();
            for (i, mac) in macros.iter_mut().enumerate().take(4) {
                mac.value = self.state.macro_values[i];
            }
            a.send(AudioCmd::SetMacros(macros));
            for (i, osc) in self.state.oscillators.iter().enumerate() {
                a.send(AudioCmd::SetOsc {
                    index: i,
                    level: osc.level,
                    detune: osc.coarse,
                    unison: osc.unison,
                    position: osc.position,
                    pan: osc.pan,
                    osc_type: osc_type_from_index(osc.osc_type).into(),
                    pulse_width: osc.pulse_width,
                    morph_a: osc.morph_a,
                    morph_b: osc.morph_b,
                    morph_amount: osc.morph_amount,
                    warp_mode: warp_mode_from_index(osc.warp_mode).into(),
                    warp_amount: osc.warp_amount,
                    fm_source: fm_source_from_index(osc.fm_source).into(),
                    fm_ratio: osc.fm_ratio,
                    fm_index: osc.fm_index,
                });
                a.send(AudioCmd::SetOscFm {
                    index: i,
                    fm_source: fm_source_from_index(osc.fm_source).into(),
                    fm_ratio: osc.fm_ratio,
                    fm_index: osc.fm_index,
                });
            }
            let patch = patch_from_state(&self.state, &self.current_patch);
            a.send(AudioCmd::SetPatch(patch.clone()));
            self.current_patch = patch;
        } else {
            self.current_patch = patch_from_state(&self.state, &self.current_patch);
        }
    }

    fn connect_midi(&mut self, index: usize) {
        self.midi_selected = index;
        self.midi_handle = match MidiInputHandle::connect(
            &self.midi_devices,
            index,
            self.midi_event_tx.clone(),
        ) {
            Ok(h) => {
                let label = self
                    .midi_devices
                    .names
                    .get(index)
                    .cloned()
                    .unwrap_or_else(|| "MIDI".into());
                self.state.midi_device = if index == 0 {
                    "None".into()
                } else {
                    label.clone()
                };
                if index == 0 {
                    self.state.status = "MIDI disconnected".into();
                } else {
                    self.state.status = format!("MIDI: {label}");
                }
                h
            }
            Err(e) => {
                self.state.status = e;
                MidiInputHandle::disconnected()
            }
        };
    }

    fn open_preset(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("ReelSynth Preset", &["reelpreset"])
            .pick_file()
        else {
            return;
        };

        match load_preset(&path) {
            Ok(patch) => match resolve_bank(&path, &patch) {
                Ok(bank) => {
                    if let Some(a) = &self.audio {
                        a.send(AudioCmd::LoadPreset {
                            patch: patch.clone(),
                            bank,
                        });
                    }
                    sync_state_from_patch(&mut self.state, &patch);
                    self.current_patch = patch;
                    self.preset_path = Some(path);
                    self.state.status = format!(
                        "Loaded {}",
                        self.preset_path
                            .as_ref()
                            .and_then(|p| p.file_name())
                            .and_then(|n| n.to_str())
                            .unwrap_or("preset")
                    );
                }
                Err(e) => self.state.status = e,
            },
            Err(e) => self.state.status = format!("Open failed: {e}"),
        }
    }

    fn save_preset(&mut self) {
        let path = if let Some(p) = &self.preset_path {
            Some(p.clone())
        } else {
            let default_name = format!(
                "{}.reelpreset",
                self.state
                    .preset_name
                    .replace(['/', '\\'], "_")
                    .trim()
            );
            rfd::FileDialog::new()
                .add_filter("ReelSynth Preset", &["reelpreset"])
                .set_file_name(&default_name)
                .save_file()
        };

        let Some(mut path) = path else {
            return;
        };

        if path.extension().is_none() {
            path.set_extension("reelpreset");
        }

        self.current_patch = patch_from_state(&self.state, &self.current_patch);
        match self.current_patch.to_json() {
            Ok(json) => match std::fs::write(&path, json) {
                Ok(()) => {
                    self.preset_path = Some(path.clone());
                    self.state.status = format!(
                        "Saved {}",
                        path.file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("preset")
                    );
                }
                Err(e) => self.state.status = format!("Save failed: {e}"),
            },
            Err(e) => self.state.status = format!("Serialize failed: {e}"),
        }
    }

    fn load_bank(&mut self, bank: WavetableBank, name: String, wt_id: Option<String>) {
        if let Some(id) = wt_id {
            self.current_patch.wavetable_id = Some(id);
        }
        self.state.wt_bank_name = name;
        if let Some(a) = &self.audio {
            let patch = patch_from_state(&self.state, &self.current_patch);
            a.send(AudioCmd::LoadPreset {
                patch: patch.clone(),
                bank,
            });
            self.current_patch = patch;
        }
    }

    fn import_wt_file(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("ReelSynth Wavetable", &["reelwt"])
            .pick_file()
        else {
            return;
        };

        match WavetableBank::read_file(path.to_str().unwrap_or_default()) {
            Ok(bank) => {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Wavetable")
                    .replace('_', " ");
                self.wt_path = Some(path.clone());
                self.load_bank(bank, name, None);
                self.state.status = format!(
                    "Loaded WT {}",
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("wavetable")
                );
            }
            Err(e) => self.state.status = format!("WT open failed: {e}"),
        }
    }

    fn import_factory_wt(&mut self, id: &str) {
        let Some(bank) = factory_bank(id) else {
            self.state.status = format!("Unknown factory bank: {id}");
            return;
        };
        let label = factory_label(id).unwrap_or(id).to_string();
        self.wt_path = None;
        self.load_bank(bank, label, Some(id.to_string()));
        self.state.status = format!("Loaded factory WT: {id}");
    }

    fn import_vital_wt(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Vital Wavetable", &["vitaltable", "json"])
            .pick_file()
        else {
            return;
        };
        match import_vital(path.to_str().unwrap_or_default()) {
            Ok(bank) => {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Vital")
                    .replace('_', " ");
                self.wt_path = None;
                self.load_bank(bank, name, None);
                self.state.status = format!(
                    "Imported Vital {}",
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("wavetable")
                );
            }
            Err(e) => self.state.status = format!("Vital import failed: {e}"),
        }
    }

    fn import_wav_folder(&mut self) {
        let Some(path) = rfd::FileDialog::new().pick_folder() else {
            return;
        };
        match import_wav_folder(path.to_str().unwrap_or_default()) {
            Ok(bank) => {
                let name = path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("WAV")
                    .replace('_', " ");
                self.wt_path = None;
                self.load_bank(bank, name, None);
                self.state.status = format!(
                    "Imported WAV folder {}",
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("cycles")
                );
            }
            Err(e) => self.state.status = format!("WAV import failed: {e}"),
        }
    }

    fn import_serum_fxp(&mut self) {
        let Some(path) = rfd::FileDialog::new()
            .add_filter("Serum Preset", &["fxp"])
            .pick_file()
        else {
            return;
        };
        match import_serum_fxp(path.to_str().unwrap_or_default()) {
            Ok(bank) => {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("Serum")
                    .replace('_', " ");
                self.wt_path = None;
                self.load_bank(bank, name, None);
                self.state.status = format!(
                    "Imported Serum {}",
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("patch")
                );
            }
            Err(e) => self.state.status = format!("Serum import failed: {e}"),
        }
    }

    fn sync_bank_edit(&mut self) {
        if let Some(a) = &self.audio {
            if let Ok(bank) = a.bank().read() {
                a.send(AudioCmd::UpdateBank((*bank).clone()));
            }
        }
    }

    fn save_wt_file(&mut self) {
        let bank = match self.bank_for_ui() {
            Some(b) => b,
            None => {
                self.state.status = "No wavetable loaded".into();
                return;
            }
        };

        let path = if let Some(p) = &self.wt_path {
            Some(p.clone())
        } else {
            let default_name = format!(
                "{}.reelwt",
                self.state
                    .wt_bank_name
                    .replace(['/', '\\'], "_")
                    .trim()
            );
            rfd::FileDialog::new()
                .add_filter("ReelSynth Wavetable", &["reelwt"])
                .set_file_name(&default_name)
                .save_file()
        };

        let Some(mut path) = path else {
            return;
        };

        if path.extension().is_none() {
            path.set_extension("reelwt");
        }

        match bank.write_file(path.to_str().unwrap_or_default()) {
            Ok(()) => {
                self.wt_path = Some(path.clone());
                self.state.status = format!(
                    "Saved WT {}",
                    path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("wavetable")
                );
            }
            Err(e) => self.state.status = format!("WT save failed: {e}"),
        }
    }

    fn bank_for_ui(&self) -> Option<WavetableBank> {
        self.audio
            .as_ref()
            .and_then(|a| a.bank().read().ok().map(|g| (*g).clone()))
    }
}

fn keyboard_note(key: egui::Key) -> Option<u8> {
    use egui::Key;
    Some(match key {
        Key::Z => 48,
        Key::S => 49,
        Key::X => 50,
        Key::D => 51,
        Key::C => 52,
        Key::V => 53,
        Key::G => 54,
        Key::B => 55,
        Key::H => 56,
        Key::N => 57,
        Key::J => 58,
        Key::M => 59,
        _ => return None,
    })
}

impl eframe::App for ReelSynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(event) = self.midi_event_rx.try_recv() {
            if let Some(a) = &self.audio {
                a.send(AudioCmd::Midi(event));
            }
            if let MidiEvent::NoteOn { note, velocity, .. } = event {
                self.state.keys_down.insert(note);
                let _ = velocity;
            }
            if let MidiEvent::NoteOff { note, .. } = event {
                self.state.keys_down.remove(&note);
            }
        }

        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Key {
                    key,
                    pressed,
                    repeat: false,
                    ..
                } = event
                {
                    if let Some(note) = keyboard_note(*key) {
                        if *pressed {
                            self.note_on(note, 0.9);
                        } else {
                            self.note_off(note);
                        }
                    }
                }
            }
        });

        egui::CentralPanel::default()
            .frame(egui::Frame {
                fill: reelsynth_ui_theme::Tokens::default().bg,
                ..Default::default()
            })
            .show(ctx, |ui| {
                let midi = ShellMidiDevices {
                    names: &self.midi_devices.names,
                    selected: self.midi_selected,
                };
                let config = ShellConfig {
                    show_wt_editor: true,
                    show_osc_column: true,
                    show_mod_matrix: true,
                    show_fx_rack: true,
                };

                let preview_patch = patch_from_state(&self.state, &self.current_patch);
                let now_secs = ui.input(|i| i.time);
                let is_playing =
                    self.scope.is_playing() || !self.state.keys_down.is_empty();
                let live_snapshot = if is_playing {
                    Some(self.scope.snapshot())
                } else {
                    None
                };
                let bank_for_osc: &dyn Fn(usize) -> usize = &|_| 0;

                let actions = if let Some(audio) = &self.audio {
                    if let Ok(mut bank) = audio.bank().write() {
                        let banks = [(*bank).clone()];
                        let scope_ctx = ScopeStripContext {
                            banks: &banks,
                            bank_for_osc,
                            live: live_snapshot.as_ref(),
                            is_playing,
                            now_secs,
                            state: &mut self.scope_strip_state,
                        };
                        let osc_ctx = OscStripContext {
                            banks: &banks,
                            bank_for_osc,
                            now_secs,
                            state: &mut self.osc_strip_state,
                        };
                        draw_shell(
                            ui,
                            ui.max_rect(),
                            &mut self.state,
                            Some(&mut *bank),
                            &preview_patch,
                            &midi,
                            &config,
                            Some(scope_ctx),
                            Some(osc_ctx),
                        )
                    } else {
                        let scope_ctx = ScopeStripContext {
                            banks: &[],
                            bank_for_osc,
                            live: live_snapshot.as_ref(),
                            is_playing,
                            now_secs,
                            state: &mut self.scope_strip_state,
                        };
                        let osc_ctx = OscStripContext {
                            banks: &[],
                            bank_for_osc,
                            now_secs,
                            state: &mut self.osc_strip_state,
                        };
                        draw_shell(
                            ui,
                            ui.max_rect(),
                            &mut self.state,
                            None,
                            &preview_patch,
                            &midi,
                            &config,
                            Some(scope_ctx),
                            Some(osc_ctx),
                        )
                    }
                } else {
                    let scope_ctx = ScopeStripContext {
                        banks: &[],
                        bank_for_osc,
                        live: None,
                        is_playing: false,
                        now_secs,
                        state: &mut self.scope_strip_state,
                    };
                    let osc_ctx = OscStripContext {
                        banks: &[],
                        bank_for_osc,
                        now_secs,
                        state: &mut self.osc_strip_state,
                    };
                    draw_shell(
                        ui,
                        ui.max_rect(),
                        &mut self.state,
                        None,
                        &preview_patch,
                        &midi,
                        &config,
                        Some(scope_ctx),
                        Some(osc_ctx),
                    )
                };

                if let Some(n) = actions.note_on {
                    self.note_on(n, 0.9);
                }
                if let Some(n) = actions.note_off {
                    self.note_off(n);
                }
                if actions.params_changed {
                    self.sync_params();
                }
                if actions.frame_edited {
                    self.sync_bank_edit();
                }
                if actions.open_preset {
                    self.open_preset();
                }
                if actions.save_preset {
                    self.save_preset();
                }
                if actions.import_wt_file {
                    self.import_wt_file();
                }
                if actions.save_wt_file {
                    self.save_wt_file();
                }
                if let Some(id) = actions.import_factory_wt {
                    self.import_factory_wt(&id);
                }
                if actions.import_vital_wt {
                    self.import_vital_wt();
                }
                if actions.import_wav_folder {
                    self.import_wav_folder();
                }
                if actions.import_serum_fxp {
                    self.import_serum_fxp();
                }
                if let Some(idx) = actions.midi_device_selected {
                    if idx != self.midi_selected {
                        self.connect_midi(idx);
                    }
                }
            });

        if self.audio.is_some() {
            ctx.request_repaint();
        }
    }
}
