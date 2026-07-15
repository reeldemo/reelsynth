//! ReelSynth standalone application state.

use crate::app_settings::{AppSettings, GraphicsBackend, KeyboardLayoutSetting};
use crate::audio_commands::AudioCmd;
use crate::audio_host::AudioHandle;
use crate::keyboard_layout::{detect_layout, keyboard_note, qwer_index, ComputerLayout};
use crate::midi_host::{MidiDevices, MidiInputHandle};
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use reelsynth::import::{import_serum_fxp, import_vital, import_wav_folder};
use reelsynth::{
    load_preset, note_in_scale, resolve_diatonic_chord, scale_degree_to_midi, snap_note,
    resolve_bank_for_preset, ArpEngine, ArpEvent, Envelope, MidiEvent, Patch,
    PerformanceLayout, PerformanceSettings, ScaleBehavior, ScopeMonitor, WavetableBank,
};
use reelsynth_ui::{
    compose_to_patch_sequence, draw_shell, effect_slots_to_patch, factory_bank, factory_label,
    fm_source_from_index, filter_type_from_mode, lfo_shape_from_index, mod_slots_to_patch,
    osc_type_from_index, patch_from_state, sync_state_from_patch, warp_mode_from_index,
    OscStripContext, OscStripPreviewState, ShellConfig, ShellMidiDevices, ShellMode, UiState,
    ScopeStripContext, ScopeStripState, PianoRollTool,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone, Copy, Debug)]
enum PerformanceKey {
    Note(u8),
    ScaleDegree(usize),
    ChordDegree(usize),
    Freq(f32),
}

#[derive(Default)]
struct PerformanceInput {
    next_token: u64,
    token_notes: HashMap<u64, Vec<u8>>,
}

#[derive(Default)]
struct ArpLive {
    engine: ArpEngine,
    last_time: Option<f64>,
}

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
    performance: PerformanceInput,
    arp: ArpLive,
    pending_record_sync: bool,
    pending_record_notes: HashMap<u8, PendingRecordNote>,
    app_settings: AppSettings,
    keyboard_layout: ComputerLayout,
    last_midi_poll_secs: f64,
}

#[derive(Clone, Debug)]
struct PendingRecordNote {
    pitch: u8,
    start_beats: f32,
    velocity: f32,
}

impl ReelSynthApp {
    pub fn new(
        audio: Option<Arc<AudioHandle>>,
        midi_devices: MidiDevices,
        midi_event_tx: Sender<MidiEvent>,
        midi_event_rx: Receiver<MidiEvent>,
        app_settings: AppSettings,
    ) -> Self {
        let keyboard_layout = match app_settings.keyboard_layout {
            KeyboardLayoutSetting::Auto => detect_layout(),
            KeyboardLayoutSetting::Qwerty => ComputerLayout::Qwerty,
            KeyboardLayoutSetting::Azerty => ComputerLayout::Azerty,
            KeyboardLayoutSetting::Qwertz => ComputerLayout::Qwertz,
        };
        let status = if audio.is_some() {
            format!(
                "Audio OK — keys: {} · click or MIDI",
                keyboard_layout.label()
            )
        } else {
            format!("No audio — UI only · keys: {}", keyboard_layout.label())
        };
        let midi_handle = MidiInputHandle::disconnected();
        let mut state = UiState {
            status,
            ..UiState::default()
        };
        let mut current_patch = Patch::factory_lead();
        sync_state_from_patch(&mut state, &current_patch);
        current_patch.mod_matrix = mod_slots_to_patch(&state.mod_routes);
        current_patch.effects = effect_slots_to_patch(&state.fx_slots);
        let scope = audio.as_ref().map(|a| a.scope()).unwrap_or_default();
        if let Some(a) = &audio {
            let seq = compose_to_patch_sequence(&state.compose);
            a.send(AudioCmd::SetSequence(seq));
        }
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
            performance: PerformanceInput::default(),
            arp: ArpLive::default(),
            pending_record_sync: false,
            pending_record_notes: HashMap::new(),
            app_settings,
            keyboard_layout,
            last_midi_poll_secs: 0.0,
        }
    }

    fn effective_keyboard_layout(&self) -> ComputerLayout {
        match self.app_settings.keyboard_layout {
            KeyboardLayoutSetting::Auto => self.keyboard_layout,
            KeyboardLayoutSetting::Qwerty => ComputerLayout::Qwerty,
            KeyboardLayoutSetting::Azerty => ComputerLayout::Azerty,
            KeyboardLayoutSetting::Qwertz => ComputerLayout::Qwertz,
        }
    }

    fn poll_midi_autoconnect(&mut self, now_secs: f64) {
        if now_secs - self.last_midi_poll_secs < 2.0 {
            return;
        }
        self.last_midi_poll_secs = now_secs;
        let changed = self.midi_devices.refresh();
        if !self.app_settings.auto_midi_keyboard {
            return;
        }
        if let Some(idx) = self.midi_devices.keyboard_like_index() {
            if self.midi_selected != idx || changed {
                self.midi_selected = idx;
                self.midi_handle = MidiInputHandle::connect(
                    &self.midi_devices,
                    idx,
                    self.midi_event_tx.clone(),
                )
                .unwrap_or_else(|_| MidiInputHandle::disconnected());
                let name = self
                    .midi_devices
                    .names
                    .get(idx)
                    .cloned()
                    .unwrap_or_else(|| "MIDI".into());
                self.state.midi_device = name.clone();
                self.state.status = format!("MIDI: {name}");
            }
        } else if self.midi_selected != 0 && changed {
            self.midi_selected = 0;
            self.midi_handle = MidiInputHandle::disconnected();
            self.state.midi_device = "None".into();
        }
    }

    fn draw_settings_window(&mut self, ctx: &egui::Context) {
        egui::Window::new("Settings")
            .collapsible(true)
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.label("Graphics");
                let mut backend_idx = self.app_settings.graphics_backend.index();
                egui::ComboBox::from_label("Backend")
                    .selected_text(self.app_settings.graphics_backend.label())
                    .show_ui(ui, |ui| {
                        for (i, label) in ["Auto", "GPU (WGPU)", "OpenGL (Glow)"].iter().enumerate() {
                            if ui.selectable_label(backend_idx == i, *label).clicked() {
                                backend_idx = i;
                                self.app_settings.graphics_backend = GraphicsBackend::from_index(i);
                                self.app_settings.pending_backend_restart = true;
                                self.app_settings.save();
                            }
                        }
                    });
                if ui.checkbox(&mut self.app_settings.gpu_waveforms, "GPU waveforms").changed() {
                    self.app_settings.save();
                }
                if self.app_settings.pending_backend_restart {
                    ui.colored_label(
                        egui::Color32::from_rgb(0xde, 0xa0, 0x4a),
                        "Restart required for graphics backend change",
                    );
                }
                ui.separator();
                ui.label("Input");
                if ui
                    .checkbox(
                        &mut self.app_settings.auto_midi_keyboard,
                        "Auto-connect MIDI keyboard",
                    )
                    .changed()
                {
                    self.app_settings.save();
                }
                let mut layout_idx = self.app_settings.keyboard_layout.index();
                egui::ComboBox::from_label("Keyboard layout")
                    .selected_text(self.app_settings.keyboard_layout.label())
                    .show_ui(ui, |ui| {
                        for (i, label) in ["Auto", "QWERTY", "AZERTY", "QWERTZ"].iter().enumerate() {
                            if ui.selectable_label(layout_idx == i, *label).clicked() {
                                layout_idx = i;
                                self.app_settings.keyboard_layout =
                                    KeyboardLayoutSetting::from_index(i);
                                self.app_settings.save();
                            }
                        }
                    });
                ui.label(format!(
                    "Detected: {}",
                    self.effective_keyboard_layout().label()
                ));
            });
    }

    fn compose_is_recording(&self) -> bool {
        self.state.shell_mode == ShellMode::Compose
            && self.state.compose.transport.recording
            && self.state.compose.armed_track().is_some()
    }

    fn record_clip_target(&mut self) -> Option<(usize, usize)> {
        let ti = self.state.compose.armed_track()?;
        let playhead = self.state.compose.transport.playhead_beats;
        let step = self.state.compose.snap_division.beats_per_step();

        let ci = if let Some(ci) = self.state.compose.selected_clip {
            if ci < self.state.compose.project.tracks[ti].clips.len() {
                Some(ci)
            } else {
                None
            }
        } else {
            None
        };

        let ci = ci.unwrap_or_else(|| {
            let snapped = self.state.compose.snap_beats(playhead);
            let len = (self.state.compose.project.loop_region.end_beats - snapped).max(step);
            let clip = reelsynth_ui::Clip::new(snapped, len);
            self.state.compose.project.tracks[ti].clips.push(clip);
            let idx = self.state.compose.project.tracks[ti].clips.len() - 1;
            self.state.compose.selected_track = ti;
            self.state.compose.selected_clip = Some(idx);
            idx
        });
        Some((ti, ci))
    }

    fn finalize_record_note(&mut self, pending: PendingRecordNote, end_beats: f32) {
        let Some((ti, ci)) = self.record_clip_target() else {
            return;
        };
        let step = self.state.compose.snap_division.beats_per_step();
        let snapped_start = self.state.compose.snap_beats(pending.start_beats);
        let mut duration = (end_beats - pending.start_beats).max(step);
        if self.state.compose.snap_enabled {
            duration = self.state.compose.snap_beats(duration).max(step);
        }
        let clip_len = self.state.compose.project.tracks[ti].clips[ci].length_beats;
        let start = snapped_start.clamp(0.0, clip_len);
        duration = duration.min(clip_len - start).max(step);

        use reelsynth_ui::MidiNote;
        self.state.compose.project.tracks[ti].clips[ci].notes.push(MidiNote {
            pitch: pending.pitch,
            start_beats: start,
            duration_beats: duration,
            velocity: pending.velocity,
        });
        self.state.status = format!(
            "Recorded {} @ beat {:.2} ({:.2} beats)",
            pending.pitch, start, duration
        );
        if let Some(a) = &self.audio {
            let seq = compose_to_patch_sequence(&self.state.compose);
            a.send(AudioCmd::SetSequence(seq.clone()));
            self.current_patch.sequence = seq;
        }
    }

    fn record_note_on(&mut self, note: u8, velocity: f32) {
        let playhead = self.state.compose.transport.playhead_beats;
        self.pending_record_notes.insert(
            note,
            PendingRecordNote {
                pitch: note,
                start_beats: playhead,
                velocity: velocity.clamp(0.01, 1.0),
            },
        );
    }

    fn record_note_off(&mut self, note: u8) {
        if let Some(pending) = self.pending_record_notes.remove(&note) {
            let end = self.state.compose.transport.playhead_beats;
            self.finalize_record_note(pending, end);
        }
    }

    fn handle_compose_note_on(&mut self, note: u8, velocity: f32) {
        if self.compose_is_recording() {
            self.record_note_on(note, velocity);
            if self.audio.is_some() {
                self.engine_note_on(note, velocity);
            }
            return;
        }
        if self.state.compose.piano_roll_focused
            && self.state.compose.piano_roll_tool == PianoRollTool::Pencil
        {
            self.engine_note_on(note, velocity * 0.65);
            return;
        }
        self.performance_note_on(PerformanceKey::Note(note), velocity);
    }

    fn handle_compose_note_off(&mut self, note: u8) {
        if self.compose_is_recording() {
            self.record_note_off(note);
            if self.audio.is_some() {
                self.engine_note_off(note);
            }
            return;
        }
        self.performance_note_off(PerformanceKey::Note(note));
    }

    fn handle_live_note_on(&mut self, note: u8, velocity: f32) {
        if self.state.shell_mode == ShellMode::Compose {
            self.handle_compose_note_on(note, velocity);
        } else {
            self.performance_note_on(PerformanceKey::Note(note), velocity);
        }
    }

    fn handle_live_note_off(&mut self, note: u8) {
        if self.state.shell_mode == ShellMode::Compose {
            self.handle_compose_note_off(note);
        } else {
            self.performance_note_off(PerformanceKey::Note(note));
        }
    }

    fn engine_note_on(&mut self, note: u8, velocity: f32) {
        if self.state.keys_down.insert(note) {
            if let Some(a) = &self.audio {
                a.send(AudioCmd::Midi(MidiEvent::note_on(0, note, velocity)));
            }
        }
    }

    fn engine_note_on_freq(&mut self, note: u8, freq: f32, velocity: f32) {
        if self.state.keys_down.insert(note) {
            if let Some(a) = &self.audio {
                a.send(AudioCmd::NoteOnFreq {
                    channel: 0,
                    note,
                    freq,
                    velocity,
                });
            }
        }
    }

    fn engine_note_off(&mut self, note: u8) {
        if self.state.keys_down.remove(&note) {
            if let Some(a) = &self.audio {
                a.send(AudioCmd::Midi(MidiEvent::note_off(0, note)));
            }
        }
    }

    fn transform_piano_note(&self, raw: u8, settings: &PerformanceSettings) -> u8 {
        if settings.scale.is_chromatic() {
            return raw;
        }
        match settings.layout {
            PerformanceLayout::Piano | PerformanceLayout::Scale => {
                snap_note(raw, settings.root, settings.scale)
            }
            PerformanceLayout::Chords => raw,
        }
    }

    fn current_bpm(&self) -> f32 {
        if self.state.shell_mode == ShellMode::Compose {
            self.state.compose.project.bpm.max(1.0)
        } else {
            self.current_patch
                .sequence
                .bpm
                .max(1.0)
        }
    }

    fn tick_arp(&mut self, now: f64) {
        let settings = self.state.performance.to_settings();
        if !settings.arp.enabled {
            self.arp.last_time = Some(now);
            return;
        }

        let dt_secs = self
            .arp
            .last_time
            .map(|t| (now - t).max(0.0) as f32)
            .unwrap_or(0.0);
        self.arp.last_time = Some(now);
        if dt_secs <= 0.0 {
            return;
        }

        let bpm = self.current_bpm();
        let dt_beats = dt_secs * bpm / 60.0;
        let events = self
            .arp
            .engine
            .tick(dt_beats, &settings.arp, &settings);
        for event in events {
            self.dispatch_arp_event(event);
        }
    }

    fn dispatch_arp_event(&mut self, event: ArpEvent) {
        match event {
            ArpEvent::NoteOn { note, velocity } => {
                if self.compose_is_recording() {
                    self.record_note_on(note, velocity);
                }
                self.engine_note_on(note, velocity);
            }
            ArpEvent::NoteOff { note } => {
                if self.compose_is_recording() {
                    self.record_note_off(note);
                }
                self.engine_note_off(note);
            }
        }
    }

    fn release_arp_notes(&mut self) {
        for note in self.arp.engine.pending_note_offs() {
            self.dispatch_arp_event(ArpEvent::NoteOff { note });
        }
    }

    fn performance_note_on_direct(&mut self, key: PerformanceKey, velocity: f32) {
        let settings = self.state.performance.to_settings();
        match key {
            PerformanceKey::Note(raw) => {
                let note = self.transform_piano_note(raw, &settings);
                if settings.scale_behavior == ScaleBehavior::Filter
                    && !settings.scale.is_chromatic()
                    && !note_in_scale(note, settings.root, settings.scale)
                {
                    return;
                }
                self.engine_note_on(note, velocity);
            }
            PerformanceKey::ScaleDegree(deg) => {
                let note = scale_degree_to_midi(
                    settings.root,
                    settings.scale,
                    deg,
                    settings.base_octave,
                );
                self.engine_note_on(note, velocity);
            }
            PerformanceKey::ChordDegree(deg) => {
                let notes = resolve_diatonic_chord(
                    settings.root,
                    settings.scale,
                    deg,
                    settings.base_octave,
                    settings.chord_set,
                    settings.voicing,
                );
                let token = self.performance.next_token;
                self.performance.next_token += 1;
                for note in &notes {
                    self.engine_note_on(*note, velocity);
                }
                self.performance.token_notes.insert(token, notes);
                self.state.active_chord_token = Some(token);
            }
            PerformanceKey::Freq(freq) => {
                let note = freq_to_midi_note(freq);
                self.engine_note_on_freq(note, freq, velocity);
            }
        }
    }

    fn performance_note_off_direct(&mut self, key: PerformanceKey) {
        let settings = self.state.performance.to_settings();
        match key {
            PerformanceKey::Note(raw) => {
                let note = self.transform_piano_note(raw, &settings);
                self.engine_note_off(note);
            }
            PerformanceKey::ScaleDegree(deg) => {
                let note = scale_degree_to_midi(
                    settings.root,
                    settings.scale,
                    deg,
                    settings.base_octave,
                );
                self.engine_note_off(note);
            }
            PerformanceKey::ChordDegree(_) => {
                if let Some(token) = self.state.active_chord_token.take() {
                    self.release_chord_token(token);
                }
            }
            PerformanceKey::Freq(freq) => {
                let note = freq_to_midi_note(freq);
                self.engine_note_off(note);
            }
        }
    }

    fn performance_note_on(&mut self, key: PerformanceKey, velocity: f32) {
        let settings = self.state.performance.to_settings();
        if !settings.arp.enabled {
            self.performance_note_on_direct(key, velocity);
            return;
        }

        let arp = settings.arp.clone();
        match key {
            PerformanceKey::Note(raw) => {
                let note = self.transform_piano_note(raw, &settings);
                if settings.scale_behavior == ScaleBehavior::Filter
                    && !settings.scale.is_chromatic()
                    && !note_in_scale(note, settings.root, settings.scale)
                {
                    return;
                }
                self.arp.engine.note_on(note, velocity, &arp, &settings);
            }
            PerformanceKey::ScaleDegree(deg) => {
                let note = scale_degree_to_midi(
                    settings.root,
                    settings.scale,
                    deg,
                    settings.base_octave,
                );
                self.arp.engine.note_on(note, velocity, &arp, &settings);
            }
            PerformanceKey::ChordDegree(deg) => {
                let notes = resolve_diatonic_chord(
                    settings.root,
                    settings.scale,
                    deg,
                    settings.base_octave,
                    settings.chord_set,
                    settings.voicing,
                );
                self.arp
                    .engine
                    .set_chord_pool(notes, velocity, &arp, &settings);
                self.state.active_chord_token = None;
            }
            PerformanceKey::Freq(freq) => {
                let note = freq_to_midi_note(freq);
                self.arp.engine.note_on(note, velocity, &arp, &settings);
            }
        }
    }

    fn performance_note_off(&mut self, key: PerformanceKey) {
        let settings = self.state.performance.to_settings();
        if !settings.arp.enabled {
            self.performance_note_off_direct(key);
            return;
        }

        let arp = settings.arp.clone();
        match key {
            PerformanceKey::Note(raw) => {
                let note = self.transform_piano_note(raw, &settings);
                self.arp.engine.note_off(note, &arp, &settings);
            }
            PerformanceKey::ScaleDegree(deg) => {
                let note = scale_degree_to_midi(
                    settings.root,
                    settings.scale,
                    deg,
                    settings.base_octave,
                );
                self.arp.engine.note_off(note, &arp, &settings);
            }
            PerformanceKey::ChordDegree(_) => {
                self.state.active_chord_token = None;
                self.arp.engine.all_notes_off(&arp);
            }
            PerformanceKey::Freq(freq) => {
                let note = freq_to_midi_note(freq);
                self.arp.engine.note_off(note, &arp, &settings);
            }
        }

        if self.arp.engine.pool_is_empty() {
            self.release_arp_notes();
        }
    }

    fn release_chord_token(&mut self, token: u64) {
        if let Some(notes) = self.performance.token_notes.remove(&token) {
            for note in notes {
                self.engine_note_off(note);
            }
        }
    }

    fn release_chord_degree(&mut self, degree: usize) {
        if let Some(token) = self.state.active_chord_token.take() {
            self.release_chord_token(token);
            return;
        }
        let settings = self.state.performance.to_settings();
        let notes = resolve_diatonic_chord(
            settings.root,
            settings.scale,
            degree,
            settings.base_octave,
            settings.chord_set,
            settings.voicing,
        );
        for note in notes {
            self.engine_note_off(note);
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

    fn poll_compose_transport(&mut self) {
        if self.state.shell_mode != ShellMode::Compose {
            return;
        }
        let Some(audio) = &self.audio else {
            return;
        };
        if let Ok(t) = audio.transport().read() {
            self.state.compose.transport.playing = t.playing;
            self.state.compose.transport.recording = t.recording;
            self.state.compose.transport.playhead_beats = t.playhead_beats;
            self.state.compose.transport.loop_enabled = t.loop_enabled;
            self.state.compose.project.loop_region.start_beats = t.loop_start;
            self.state.compose.project.loop_region.end_beats = t.loop_end;
            self.state.compose.project.loop_region.enabled = t.loop_enabled;
            self.state.compose.project.bpm = t.bpm;
            self.state.compose.live_record_overlay = if t.recording {
                t.live_recorded.clone()
            } else {
                Vec::new()
            };
        }
    }

    fn sync_compose_sequence_from_engine(&mut self) {
        let Some(audio) = &self.audio else {
            return;
        };
        let sequence = audio.sequence();
        let Ok(seq) = sequence.read() else {
            return;
        };
        let selected_track = self.state.compose.selected_track;
        let selected_clip = self.state.compose.selected_clip;
        let selected_notes = self.state.compose.selected_notes.clone();
        let snap_division = self.state.compose.snap_division;
        self.state.compose.project = seq.clone();
        self.state.compose.snap_division = snap_division;
        self.state.compose.selected_track = selected_track.min(seq.tracks.len().saturating_sub(1));
        self.state.compose.selected_clip = selected_clip.filter(|ci| {
            self.state
                .compose
                .project
                .tracks
                .get(self.state.compose.selected_track)
                .is_some_and(|t| *ci < t.clips.len())
        });
        self.state.compose.selected_notes = selected_notes;
        self.current_patch.sequence = seq.clone();
    }

    fn handle_compose_actions(&mut self, actions: &reelsynth_ui::ShellActions, was_recording: bool) {
        let Some(audio) = &self.audio else {
            return;
        };

        if actions.transport_play {
            audio.send(AudioCmd::TransportPlay);
        }
        if actions.transport_stop {
            audio.send(AudioCmd::TransportStop);
            if was_recording {
                self.pending_record_sync = true;
            }
        }
        if actions.transport_record {
            let track = self.state.compose.armed_track();
            audio.send(AudioCmd::TransportRecord { track });
        }
        if let Some(beats) = actions.transport_seek {
            audio.send(AudioCmd::SeekPlayhead(beats));
        }
        if let Some(_scene_idx) = actions.scene_launch {
            let slots = self.state.compose.active_scene_slots.clone();
            audio.send(AudioCmd::LaunchScene { slots });
            self.state.compose.transport.playing = true;
            self.state.compose.transport.playhead_beats = 0.0;
        }
        if actions.sequence_changed {
            let seq = compose_to_patch_sequence(&self.state.compose);
            audio.send(AudioCmd::SetBpm(seq.bpm));
            audio.send(AudioCmd::SetSequence(seq.clone()));
            self.current_patch.sequence = seq;
            for (ti, track) in self.state.compose.project.tracks.iter().enumerate() {
                audio.send(AudioCmd::SetRecordArm {
                    track: ti,
                    armed: track.arm,
                });
            }
        }
    }
}

fn freq_to_midi_note(freq: f32) -> u8 {
    if freq <= 0.0 {
        return 60;
    }
    let midi = 69.0 + 12.0 * (freq / 440.0).log2();
    midi.round().clamp(0.0, 127.0) as u8
}

fn keyboard_performance_key(
    key: egui::Key,
    layout: PerformanceLayout,
    computer_layout: ComputerLayout,
) -> Option<PerformanceKey> {
    match layout {
        PerformanceLayout::Piano => {
            keyboard_note(key, computer_layout).map(PerformanceKey::Note)
        }
        PerformanceLayout::Scale => {
            qwer_index(key, computer_layout).map(PerformanceKey::ScaleDegree)
        }
        PerformanceLayout::Chords => qwer_index(key, computer_layout).and_then(|i| {
            if i < 7 {
                Some(PerformanceKey::ChordDegree(i))
            } else {
                None
            }
        }),
    }
}

impl eframe::App for ReelSynthApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        while let Ok(event) = self.midi_event_rx.try_recv() {
            let event = match event {
                MidiEvent::NoteOn {
                    channel,
                    note,
                    velocity,
                } if self.state.scale_lock_midi => {
                    let settings = self.state.performance.to_settings();
                    let snapped = snap_note(note, settings.root, settings.scale);
                    MidiEvent::note_on(channel, snapped, velocity)
                }
                other => other,
            };

            if self.compose_is_recording() {
                match event {
                    MidiEvent::NoteOn { note, velocity, .. } => {
                        self.handle_compose_note_on(note, velocity);
                    }
                    MidiEvent::NoteOff { note, .. } => {
                        self.handle_compose_note_off(note);
                    }
                    other => {
                        if let Some(a) = &self.audio {
                            a.send(AudioCmd::Midi(other));
                        }
                    }
                }
            } else if let Some(a) = &self.audio {
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

        let layout = self
            .state
            .performance
            .to_settings()
            .layout;
        let computer_layout = self.effective_keyboard_layout();
        ctx.input(|i| {
            for event in &i.events {
                if let egui::Event::Key {
                    key,
                    pressed,
                    repeat: false,
                    ..
                } = event
                {
                    if let Some(perf_key) =
                        keyboard_performance_key(*key, layout, computer_layout)
                    {
                        if *pressed {
                            match perf_key {
                                PerformanceKey::Note(n) => self.handle_live_note_on(n, 0.9),
                                other => self.performance_note_on(other, 0.9),
                            }
                        } else {
                            match perf_key {
                                PerformanceKey::Note(n) => self.handle_live_note_off(n),
                                other => self.performance_note_off(other),
                            }
                        }
                    }
                }
            }
        });

        let now_secs = ctx.input(|i| i.time);
        self.poll_midi_autoconnect(now_secs);
        self.draw_settings_window(ctx);

        self.poll_compose_transport();
        if self.pending_record_sync && !self.state.compose.transport.recording {
            self.sync_compose_sequence_from_engine();
            self.pending_record_sync = false;
        }

        let now_secs = ctx.input(|i| i.time);
        self.tick_arp(now_secs);

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

                let was_recording = self.state.compose.transport.recording;

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
                    self.handle_live_note_on(n, 0.9);
                }
                if let Some(n) = actions.note_off {
                    self.handle_live_note_off(n);
                }
                if let Some(deg) = actions.chord_degree_on {
                    self.performance_note_on(PerformanceKey::ChordDegree(deg), 0.9);
                }
                if let Some(deg) = actions.chord_degree_off {
                    self.release_chord_degree(deg);
                    self.state.active_chord_degree = None;
                }
                if let Some((freq, vel)) = actions.note_on_freq {
                    self.performance_note_on(PerformanceKey::Freq(freq), vel);
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
                if let Some(ref id) = actions.import_factory_wt {
                    self.import_factory_wt(id);
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
                self.handle_compose_actions(&actions, was_recording);
            });

        if self.audio.is_some() {
            ctx.request_repaint();
        }
    }
}
