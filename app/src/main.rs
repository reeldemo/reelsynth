//! ReelSynth S1 performance UI — matches `brand/mockups/s1-performance.html`.

mod midi_input;

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui;
use midi_input::{MidiDevices, MidiInputHandle};
use reelsynth::{import::{import_serum_fxp, import_vital, import_wav_folder}, load_preset, resolve_bank_for_preset, Envelope, Macro, ModSlot, Patch, ScopeMonitor, SynthEngine, WavetableBank};
use reelsynth::engine::MidiEvent;
use reelsynth_ui::{draw_s1, factory_bank, factory_label, fm_algorithm_index, fm_source_from_index, fm_source_index, fx_slots_from_effects, fx_slots_to_effects, mod_routes_from_slots, mod_routes_to_slots, osc_type_from_index, osc_type_index, warp_mode_from_index, warp_mode_index, APP_HEIGHT_FULL, S1MidiDevices, S1ShellConfig, S1State, ScopeStripContext, ScopeStripState};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

enum AudioCmd {
  Midi(MidiEvent),
    SetWtPosition(f32),
    SetFilterCutoff(f32),
    SetFilterResonance(f32),
    SetFilterType(String),
    SetFilterKeyTracking(f32),
    SetEnvelope(Envelope),
    SetFilterEnvelope(Envelope),
    SetLfo { rate: f32, depth: f32, shape: String },
    SetLfo2 { rate: f32, depth: f32, shape: String },
    SetMacros(Vec<Macro>),
    SetOsc {
        index: usize,
        level: f32,
        detune: f32,
        unison: u32,
        position: f32,
        pan: f32,
        osc_type: String,
        pulse_width: f32,
        morph_a: f32,
        morph_b: f32,
        morph_amount: f32,
        warp_mode: String,
        warp_amount: f32,
        fm_source: String,
        fm_ratio: f32,
        fm_index: f32,
    },
    SetOscFm {
        index: usize,
        fm_source: String,
        fm_ratio: f32,
        fm_index: f32,
    },
    SetFilterDrive(f32),
    SetFilter2 {
        cutoff: f32,
        resonance: f32,
        filter_type: String,
        drive: f32,
    },
    SetUnisonStereoSpread(f32),
    SetSubLevel(f32),
    SetNoiseLevel(f32),
    SetModMatrix(Vec<ModSlot>),
    SetEffects(Vec<reelsynth::EffectSlot>),
    LoadPreset {
        patch: Patch,
        bank: WavetableBank,
    },
    UpdateBank(WavetableBank),
}

struct AudioHandle {
    tx: Sender<AudioCmd>,
    _stream: cpal::Stream,
    bank: Arc<RwLock<WavetableBank>>,
    scope: ScopeMonitor,
}

impl AudioHandle {
    fn send(&self, cmd: AudioCmd) {
        let _ = self.tx.send(cmd);
    }

    fn bank(&self) -> Arc<RwLock<WavetableBank>> {
        Arc::clone(&self.bank)
    }

    fn scope(&self) -> ScopeMonitor {
        self.scope.clone()
    }
}

fn start_audio(sample_rate: u32) -> Result<AudioHandle, String> {
    let bank = WavetableBank::factory_saw_morph();
    let patch = Patch::default_mono();
    let bank_shared = Arc::new(RwLock::new(bank.clone()));
    let mut engine = SynthEngine::new(bank, patch, sample_rate);

    let (tx, rx) = crossbeam_channel::unbounded::<AudioCmd>();
    let bank_for_audio = Arc::clone(&bank_shared);

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no audio output device".to_string())?;
    let config = device
        .default_output_config()
        .map_err(|e| e.to_string())?;
    let sr = config.sample_rate().0;
    if sr != sample_rate {
        engine = SynthEngine::new(WavetableBank::factory_saw_morph(), Patch::default_mono(), sr);
    }
    let scope_monitor = engine.scope_monitor().clone();

    let mut engine = engine;
    let err_fn = |e| eprintln!("audio stream error: {e}");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let channels = config.channels() as usize;
            if channels >= 2 {
                device.build_output_stream(
                    &config.into(),
                    move |data: &mut [f32], _| {
                        drain_commands(&mut engine, &rx, &bank_for_audio);
                        engine.process_stereo(data);
                    },
                    err_fn,
                    None,
                )
            } else {
                device.build_output_stream(
                    &config.into(),
                    move |data: &mut [f32], _| {
                        drain_commands(&mut engine, &rx, &bank_for_audio);
                        engine.process(data);
                    },
                    err_fn,
                    None,
                )
            }
        }
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config.into(),
            move |data: &mut [i16], _| {
                drain_commands(&mut engine, &rx, &bank_for_audio);
                let mut buf = vec![0.0f32; data.len()];
                engine.process(&mut buf);
                for (out, sample) in data.iter_mut().zip(buf.iter()) {
                    *out = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                }
            },
            err_fn,
            None,
        ),
        other => return Err(format!("unsupported sample format: {other:?}")),
    }
    .map_err(|e| e.to_string())?;

    stream.play().map_err(|e| e.to_string())?;

    Ok(AudioHandle {
        tx,
        _stream: stream,
        bank: bank_shared,
        scope: scope_monitor,
    })
}

fn drain_commands(
    engine: &mut SynthEngine,
    rx: &Receiver<AudioCmd>,
    bank_shared: &Arc<RwLock<WavetableBank>>,
) {
    loop {
        match rx.try_recv() {
            Ok(AudioCmd::Midi(event)) => engine.handle_event(event),
            Ok(AudioCmd::SetWtPosition(p)) => engine.set_wt_position(p),
            Ok(AudioCmd::SetFilterCutoff(c)) => engine.set_filter_cutoff(c),
            Ok(AudioCmd::SetFilterResonance(r)) => engine.set_filter_resonance(r),
            Ok(AudioCmd::SetFilterType(t)) => engine.set_filter_type(&t),
            Ok(AudioCmd::SetFilterKeyTracking(kt)) => engine.set_filter_key_tracking(kt),
            Ok(AudioCmd::SetEnvelope(e)) => engine.set_envelope(e),
            Ok(AudioCmd::SetFilterEnvelope(e)) => engine.set_filter_envelope(e),
            Ok(AudioCmd::SetLfo { rate, depth, shape }) => {
                engine.set_lfo_rate(rate);
                engine.set_lfo_depth(depth);
                engine.set_lfo_shape(&shape);
            }
            Ok(AudioCmd::SetLfo2 { rate, depth, shape }) => {
                engine.set_lfo2_rate(rate);
                engine.set_lfo2_depth(depth);
                engine.set_lfo2_shape(&shape);
            }
            Ok(AudioCmd::SetMacros(macros)) => engine.set_macros(macros),
            Ok(AudioCmd::SetOsc {
                index,
                level,
                detune,
                unison,
                position,
                pan,
                osc_type,
                pulse_width,
                morph_a,
                morph_b,
                morph_amount,
                warp_mode,
                warp_amount,
                fm_source,
                fm_ratio,
                fm_index,
            }) => {
                engine.set_osc_level(index, level);
                engine.set_osc_detune(index, detune);
                engine.set_osc_unison(index, unison);
                engine.set_osc_position(index, position);
                engine.set_osc_pan(index, pan);
                engine.set_osc_type(index, &osc_type);
                engine.set_osc_pulse_width(index, pulse_width);
                engine.set_osc_morph(index, morph_a, morph_b, morph_amount);
                engine.set_osc_warp(index, &warp_mode, warp_amount);
                engine.set_osc_fm(index, &fm_source, fm_ratio, fm_index);
            }
            Ok(AudioCmd::SetOscFm {
                index,
                fm_source,
                fm_ratio,
                fm_index,
            }) => engine.set_osc_fm(index, &fm_source, fm_ratio, fm_index),
            Ok(AudioCmd::SetFilterDrive(d)) => engine.set_filter_drive(d),
            Ok(AudioCmd::SetFilter2 {
                cutoff,
                resonance,
                filter_type,
                drive,
            }) => {
                engine.set_filter2_cutoff(cutoff);
                engine.set_filter2_resonance(resonance);
                engine.set_filter2_type(&filter_type);
                engine.set_filter2_drive(drive);
            }
            Ok(AudioCmd::SetUnisonStereoSpread(s)) => engine.set_unison_stereo_spread(s),
            Ok(AudioCmd::SetSubLevel(l)) => engine.set_sub_level(l),
            Ok(AudioCmd::SetNoiseLevel(l)) => engine.set_noise_level(l),
            Ok(AudioCmd::SetModMatrix(slots)) => engine.set_mod_matrix(slots),
            Ok(AudioCmd::SetEffects(effects)) => engine.set_effects(effects),
            Ok(AudioCmd::LoadPreset { patch, bank }) => {
                engine.load_preset(bank.clone(), patch);
                if let Ok(mut g) = bank_shared.write() {
                    *g = engine.bank().clone();
                }
            }
            Ok(AudioCmd::UpdateBank(bank)) => {
                let patch = engine.patch().clone();
                engine.load_preset(bank.clone(), patch);
                if let Ok(mut g) = bank_shared.write() {
                    *g = bank;
                }
            }
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

fn resolve_bank(path: &Path, preset: &Patch) -> Result<WavetableBank, String> {
    resolve_bank_for_preset(path, preset).or_else(|_| match preset.wavetable_id.as_deref() {
        Some("saw_morph") => Ok(WavetableBank::factory_saw_morph()),
        Some(id) => Err(format!("could not resolve wavetable for id {id}")),
        None => Ok(WavetableBank::factory_saw_morph()),
    })
}

fn lfo_shape_from_index(idx: usize) -> &'static str {
    match idx {
        1 => "tri",
        2 => "saw",
        3 => "sh",
        _ => "sine",
    }
}

fn lfo_shape_index(shape: &str) -> usize {
    match shape.to_ascii_lowercase().as_str() {
        "tri" | "triangle" => 1,
        "saw" => 2,
        "sh" | "s&h" => 3,
        _ => 0,
    }
}

fn sync_state_from_patch(state: &mut S1State, patch: &Patch) {
    state.preset_name = patch.name.clone();
    state.preset_category = preset_category_label(patch);
    state.wt_bank_name = patch
        .wavetable_id
        .as_deref()
        .and_then(factory_label)
        .map(str::to_string)
        .unwrap_or_else(|| {
            patch
                .wavetable_id
                .as_deref()
                .unwrap_or("wavetable")
                .replace('_', " ")
        });
    state.wt_position = patch
        .oscillators
        .first()
        .map(|o| o.position)
        .unwrap_or(0.0);
    for i in 0..3 {
        if let Some(osc) = patch.oscillators.get(i) {
            state.osc_level[i] = osc.level;
            state.osc_pan[i] = osc.pan;
            state.osc_coarse[i] = osc.detune;
            state.osc_unison[i] = osc.unison;
            state.osc_position[i] = osc.position;
            state.osc_type[i] = osc_type_index(&osc.osc_type);
            state.osc_pulse_width[i] = osc.pulse_width;
            state.osc_warp_mode[i] = warp_mode_index(&osc.warp_mode);
            state.osc_warp_amount[i] = osc.warp_amount;
            state.osc_fm_source[i] = fm_source_index(&osc.fm_source);
            state.osc_fm_algorithm[i] = fm_algorithm_index(&osc.fm_source);
            state.osc_fm_ratio[i] = osc.fm_ratio;
            state.osc_fm_index[i] = osc.fm_index;
        }
    }
    state.unison_stereo_spread = patch.unison_stereo_spread;
    state.filter_drive = patch.filter.drive;
    state.filter2_cutoff = patch.filter2.cutoff;
    state.filter2_resonance = patch.filter2.resonance;
    state.filter2_mode = filter_mode_from_type(&patch.filter2.filter_type);
    state.filter2_drive = patch.filter2.drive;
    for i in 0..3 {
        if let Some(osc) = patch.oscillators.get(i) {
            state.osc_morph_a[i] = osc.morph_a;
            state.osc_morph_b[i] = osc.morph_b;
            state.osc_morph_amount[i] = osc.morph_amount;
        }
    }
    let idx = state.osc_tab.min(2);
    state.wt_morph_a = state.osc_morph_a[idx];
    state.wt_morph_b = state.osc_morph_b[idx];
    state.wt_morph_amount = state.osc_morph_amount[idx];
    state.sub_level = patch.sub_level;
    state.noise_level = patch.noise_level;
    state.filter_cutoff = patch.filter.cutoff;
    state.filter_resonance = patch.filter.resonance;
    state.filter_key_tracking = patch.filter.key_tracking;
    state.filter_mode = filter_mode_from_type(&patch.filter.filter_type);
    state.env_attack = patch.envelope.attack;
    state.env_decay = patch.envelope.decay;
    state.env_sustain = patch.envelope.sustain;
    state.env_release = patch.envelope.release;
    state.filt_env_attack = patch.filter_envelope.attack;
    state.filt_env_decay = patch.filter_envelope.decay;
    state.filt_env_sustain = patch.filter_envelope.sustain;
    state.filt_env_release = patch.filter_envelope.release;
    state.lfo_rate = patch.lfo.rate;
    state.lfo_depth = patch.lfo.depth;
    state.lfo_shape = lfo_shape_index(&patch.lfo.shape);
    state.lfo2_rate = patch.lfo2.rate;
    state.lfo2_depth = patch.lfo2.depth;
    state.lfo2_shape = lfo_shape_index(&patch.lfo2.shape);
    for (i, mac) in patch.macros.iter().enumerate().take(4) {
        state.macro_values[i] = mac.value;
    }
    state.mod_routes = mod_routes_from_slots(&patch.mod_matrix);
    state.mod_route_total = state.mod_routes.len().max(24);
    state.fx_slots = fx_slots_from_effects(&patch.effects);
}

fn filter_mode_from_type(filter_type: &str) -> usize {
    match filter_type.to_ascii_lowercase().as_str() {
        "highpass" | "hp" => 1,
        "bandpass" | "bp" => 2,
        "notch" => 3,
        _ => 0,
    }
}

fn filter_type_from_mode(mode: usize) -> &'static str {
    match mode {
        1 => "highpass",
        2 => "bandpass",
        3 => "notch",
        _ => "lowpass",
    }
}

fn preset_category_label(patch: &Patch) -> String {
    let wt = patch
        .wavetable_id
        .as_deref()
        .unwrap_or("wavetable")
        .replace('_', " ");
    format!("Preset · Wavetable · {wt}")
}

fn patch_from_state(state: &S1State, base: &Patch) -> Patch {
    let mut patch = base.clone();
    patch.name = state.preset_name.clone();
    patch.ensure_oscillators(3);
    for i in 0..3 {
        if let Some(osc) = patch.oscillators.get_mut(i) {
            osc.level = state.osc_level[i];
            osc.pan = state.osc_pan[i];
            osc.detune = state.osc_coarse[i];
            osc.unison = state.osc_unison[i];
            osc.position = state.osc_position[i];
            osc.osc_type = osc_type_from_index(state.osc_type[i]).into();
            osc.pulse_width = state.osc_pulse_width[i];
            osc.warp_mode = warp_mode_from_index(state.osc_warp_mode[i]).into();
            osc.warp_amount = state.osc_warp_amount[i];
            osc.morph_a = state.osc_morph_a[i];
            osc.morph_b = state.osc_morph_b[i];
            osc.morph_amount = state.osc_morph_amount[i];
            osc.fm_source = fm_source_from_index(state.osc_fm_source[i]).into();
            osc.fm_ratio = state.osc_fm_ratio[i];
            osc.fm_index = state.osc_fm_index[i];
            if state.osc_morph_amount[i] > 0.0 {
                osc.position = state.osc_morph_a[i]
                    + (state.osc_morph_b[i] - state.osc_morph_a[i]) * state.osc_morph_amount[i];
            }
        }
    }
    patch.filter.cutoff = state.filter_cutoff;
    patch.filter.resonance = state.filter_resonance;
    patch.filter.key_tracking = state.filter_key_tracking;
    patch.filter.drive = state.filter_drive;
    patch.filter.filter_type = filter_type_from_mode(state.filter_mode).into();
    patch.filter2.cutoff = state.filter2_cutoff;
    patch.filter2.resonance = state.filter2_resonance;
    patch.filter2.drive = state.filter2_drive;
    patch.filter2.filter_type = filter_type_from_mode(state.filter2_mode).into();
    patch.unison_stereo_spread = state.unison_stereo_spread;
    patch.envelope = Envelope {
        attack: state.env_attack,
        decay: state.env_decay,
        sustain: state.env_sustain,
        release: state.env_release,
    };
    patch.filter_envelope = Envelope {
        attack: state.filt_env_attack,
        decay: state.filt_env_decay,
        sustain: state.filt_env_sustain,
        release: state.filt_env_release,
    };
    patch.lfo.rate = state.lfo_rate;
    patch.lfo.depth = state.lfo_depth;
    patch.lfo.shape = lfo_shape_from_index(state.lfo_shape).into();
    patch.lfo2.rate = state.lfo2_rate;
    patch.lfo2.depth = state.lfo2_depth;
    patch.lfo2.shape = lfo_shape_from_index(state.lfo2_shape).into();
    for (i, mac) in patch.macros.iter_mut().enumerate().take(4) {
        mac.value = state.macro_values[i];
    }
    patch.sub_level = state.sub_level;
    patch.noise_level = state.noise_level;
    patch.mod_matrix = mod_routes_to_slots(&state.mod_routes);
    patch.effects = fx_slots_to_effects(&state.fx_slots);
    patch
}

fn main() -> eframe::Result<()> {
    let audio = match start_audio(44100) {
        Ok(a) => Some(Arc::new(a)),
        Err(e) => {
            eprintln!("audio init failed: {e}");
            None
        }
    };

    let midi_devices = MidiDevices::enumerate();
    let (midi_event_tx, midi_event_rx) = crossbeam_channel::unbounded::<MidiEvent>();

    eframe::run_native(
        "ReelSynth",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1280.0, APP_HEIGHT_FULL])
                .with_min_inner_size([1024.0, 640.0])
                .with_title("ReelSynth"),
            ..Default::default()
        },
        Box::new(move |cc| {
            reelsynth_ui_theme::apply(&cc.egui_ctx);
            Ok(Box::new(ReelSynthApp::new(
                audio.clone(),
                midi_devices,
                midi_event_tx,
                midi_event_rx,
            )))
        }),
    )
}

struct ReelSynthApp {
    audio: Option<Arc<AudioHandle>>,
    state: S1State,
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
}

impl ReelSynthApp {
    fn new(
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
        let mut state = S1State {
            status,
            ..S1State::default()
        };
        let mut current_patch = Patch::default_mono();
        sync_state_from_patch(&mut state, &current_patch);
        current_patch.mod_matrix = mod_routes_to_slots(&state.mod_routes);
        current_patch.effects = fx_slots_to_effects(&state.fx_slots);
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
            for i in 0..3 {
                a.send(AudioCmd::SetOsc {
                    index: i,
                    level: self.state.osc_level[i],
                    detune: self.state.osc_coarse[i],
                    unison: self.state.osc_unison[i],
                    position: self.state.osc_position[i],
                    pan: self.state.osc_pan[i],
                    osc_type: osc_type_from_index(self.state.osc_type[i]).into(),
                    pulse_width: self.state.osc_pulse_width[i],
                    morph_a: self.state.osc_morph_a[i],
                    morph_b: self.state.osc_morph_b[i],
                    morph_amount: self.state.osc_morph_amount[i],
                    warp_mode: warp_mode_from_index(self.state.osc_warp_mode[i]).into(),
                    warp_amount: self.state.osc_warp_amount[i],
                    fm_source: fm_source_from_index(self.state.osc_fm_source[i]).into(),
                    fm_ratio: self.state.osc_fm_ratio[i],
                    fm_index: self.state.osc_fm_index[i],
                });
                a.send(AudioCmd::SetOscFm {
                    index: i,
                    fm_source: fm_source_from_index(self.state.osc_fm_source[i]).into(),
                    fm_ratio: self.state.osc_fm_ratio[i],
                    fm_index: self.state.osc_fm_index[i],
                });
            }
            a.send(AudioCmd::SetSubLevel(self.state.sub_level));
            a.send(AudioCmd::SetNoiseLevel(self.state.noise_level));
            a.send(AudioCmd::SetModMatrix(mod_routes_to_slots(&self.state.mod_routes)));
            a.send(AudioCmd::SetEffects(fx_slots_to_effects(&self.state.fx_slots)));
        }
        self.current_patch = patch_from_state(&self.state, &self.current_patch);
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
                let midi = S1MidiDevices {
                    names: &self.midi_devices.names,
                    selected: self.midi_selected,
                };
                let config = S1ShellConfig {
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
                        draw_s1(
                            ui,
                            ui.max_rect(),
                            &mut self.state,
                            Some(&mut *bank),
                            &preview_patch,
                            &midi,
                            &config,
                            Some(scope_ctx),
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
                        draw_s1(
                            ui,
                            ui.max_rect(),
                            &mut self.state,
                            None,
                            &preview_patch,
                            &midi,
                            &config,
                            Some(scope_ctx),
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
                    draw_s1(
                        ui,
                        ui.max_rect(),
                        &mut self.state,
                        None,
                        &preview_patch,
                        &midi,
                        &config,
                        Some(scope_ctx),
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
