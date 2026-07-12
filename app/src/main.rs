//! ReelSynth S1 performance UI — matches `brand/mockups/s1-performance.html`.

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui;
use reelsynth::patch::Patch;
use reelsynth::SynthEngine;
use reelsynth::WavetableBank;
use reelsynth_ui::{draw_s1, S1State};
use std::sync::Arc;

enum AudioCmd {
    NoteOn(u8, f32),
    NoteOff(u8),
    SetWtPosition(f32),
    SetFilterCutoff(f32),
    SetFilterResonance(f32),
}

struct AudioHandle {
    tx: Sender<AudioCmd>,
    _stream: cpal::Stream,
    bank: WavetableBank,
}

impl AudioHandle {
    fn send(&self, cmd: AudioCmd) {
        let _ = self.tx.send(cmd);
    }
}

fn start_audio(sample_rate: u32) -> Result<AudioHandle, String> {
    let bank = WavetableBank::factory_saw_morph();
    let patch = Patch::default_mono();
    let mut engine = SynthEngine::new(bank.clone(), patch, sample_rate);

    let (tx, rx) = crossbeam_channel::unbounded::<AudioCmd>();

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

    let mut engine = engine;
    let err_fn = |e| eprintln!("audio stream error: {e}");

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config.into(),
            move |data: &mut [f32], _| {
                drain_commands(&mut engine, &rx);
                engine.process(data);
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config.into(),
            move |data: &mut [i16], _| {
                drain_commands(&mut engine, &rx);
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
        bank,
    })
}

fn drain_commands(engine: &mut SynthEngine, rx: &Receiver<AudioCmd>) {
    loop {
        match rx.try_recv() {
            Ok(AudioCmd::NoteOn(n, v)) => engine.note_on(n, v),
            Ok(AudioCmd::NoteOff(n)) => engine.note_off(n),
            Ok(AudioCmd::SetWtPosition(p)) => engine.set_wt_position(p),
            Ok(AudioCmd::SetFilterCutoff(c)) => engine.set_filter_cutoff(c),
            Ok(AudioCmd::SetFilterResonance(r)) => engine.set_filter_resonance(r),
            Err(TryRecvError::Empty) => break,
            Err(TryRecvError::Disconnected) => break,
        }
    }
}

fn main() -> eframe::Result<()> {
    let audio = match start_audio(44100) {
        Ok(a) => Some(Arc::new(a)),
        Err(e) => {
            eprintln!("audio init failed: {e}");
            None
        }
    };

    eframe::run_native(
        "ReelSynth",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([1280.0, 720.0])
                .with_min_inner_size([1024.0, 640.0])
                .with_title("ReelSynth"),
            ..Default::default()
        },
        Box::new(move |cc| {
            reelsynth_ui_theme::apply(&cc.egui_ctx);
            Ok(Box::new(ReelSynthApp::new(audio.clone())))
        }),
    )
}

struct ReelSynthApp {
    audio: Option<Arc<AudioHandle>>,
    state: S1State,
}

impl ReelSynthApp {
    fn new(audio: Option<Arc<AudioHandle>>) -> Self {
        let status = if audio.is_some() {
            "Audio OK — click keys or use QWERTY row (Z–M)".into()
        } else {
            "No audio — UI only".into()
        };
        Self {
            audio,
            state: S1State {
                status,
                ..S1State::default()
            },
        }
    }

    fn note_on(&mut self, note: u8) {
        if self.state.keys_down.insert(note) {
            if let Some(a) = &self.audio {
                a.send(AudioCmd::NoteOn(note, 0.9));
            }
        }
    }

    fn note_off(&mut self, note: u8) {
        if self.state.keys_down.remove(&note) {
            if let Some(a) = &self.audio {
                a.send(AudioCmd::NoteOff(note));
            }
        }
    }

    fn sync_params(&mut self) {
        if let Some(a) = &self.audio {
            a.send(AudioCmd::SetWtPosition(self.state.wt_position));
            a.send(AudioCmd::SetFilterCutoff(self.state.filter_cutoff));
            a.send(AudioCmd::SetFilterResonance(self.state.filter_resonance));
        }
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
                            self.note_on(note);
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
                let bank = self.audio.as_ref().map(|a| &a.bank);
                let actions = draw_s1(ui, ui.max_rect(), &mut self.state, bank);

                if let Some(n) = actions.note_on {
                    self.note_on(n);
                }
                if let Some(n) = actions.note_off {
                    self.note_off(n);
                }
                if actions.params_changed {
                    self.sync_params();
                }
                if actions.open_preset {
                    self.state.status = "Open preset — stub".into();
                }
                if actions.save_preset {
                    self.state.status = "Save preset — stub".into();
                }
            });

        if self.audio.is_some() {
            ctx.request_repaint();
        }
    }
}
