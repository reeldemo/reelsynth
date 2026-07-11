//! Minimal playable ReelSynth — S1 preview for manual testing.

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use eframe::egui;
use reelsynth::engine::note_to_freq;
use reelsynth::patch::Patch;
use reelsynth::SynthEngine;
use reelsynth::WavetableBank;
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
}

impl AudioHandle {
    fn send(&self, cmd: AudioCmd) {
        let _ = self.tx.send(cmd);
    }
}

fn start_audio(sample_rate: u32) -> Result<AudioHandle, String> {
    let bank = WavetableBank::factory_saw_morph();
    let patch = Patch::default_mono();
    let mut engine = SynthEngine::new(bank, patch, sample_rate);

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
                .with_inner_size([720.0, 520.0])
                .with_title("ReelSynth"),
            ..Default::default()
        },
        Box::new(move |_cc| {
            Ok(Box::new(ReelSynthApp::new(audio.clone())))
        }),
    )
}

struct ReelSynthApp {
    themed: bool,
    audio: Option<Arc<AudioHandle>>,
    wt_position: f32,
    filter_cutoff: f32,
    filter_resonance: f32,
    keys_down: std::collections::HashSet<u8>,
    status: String,
}

impl ReelSynthApp {
    fn new(audio: Option<Arc<AudioHandle>>) -> Self {
        let status = if audio.is_some() {
            "Audio OK — click keys or use QWERTY row (Z–M)".into()
        } else {
            "No audio — UI only".into()
        };
        Self {
            themed: false,
            audio,
            wt_position: 0.0,
            filter_cutoff: 1200.0,
            filter_resonance: 0.3,
            keys_down: std::collections::HashSet::new(),
            status,
        }
    }

    fn note_on(&mut self, note: u8) {
        if self.keys_down.insert(note) {
            if let Some(a) = &self.audio {
                a.send(AudioCmd::NoteOn(note, 0.9));
            }
        }
    }

    fn note_off(&mut self, note: u8) {
        if self.keys_down.remove(&note) {
            if let Some(a) = &self.audio {
                a.send(AudioCmd::NoteOff(note));
            }
        }
    }

    fn sync_params(&mut self) {
        if let Some(a) = &self.audio {
            a.send(AudioCmd::SetWtPosition(self.wt_position));
            a.send(AudioCmd::SetFilterCutoff(self.filter_cutoff));
            a.send(AudioCmd::SetFilterResonance(self.filter_resonance));
        }
    }
}

/// Computer keyboard → MIDI (one octave from C3).
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
        if !self.themed {
            reelsynth_ui_theme::apply(ctx);
            self.themed = true;
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
                            self.note_on(note);
                        } else {
                            self.note_off(note);
                        }
                    }
                }
            }
        });

        egui::TopBottomPanel::top("header").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("ReelSynth");
                ui.separator();
                ui.label(&self.status);
            });
        });

        egui::SidePanel::right("controls").min_width(220.0).show(ctx, |ui| {
            ui.heading("Controls");
            ui.add_space(8.0);
            if ui
                .add(egui::Slider::new(&mut self.wt_position, 0.0..=255.0).text("WT position"))
                .changed()
            {
                self.sync_params();
            }
            if ui
                .add(
                    egui::Slider::new(&mut self.filter_cutoff, 40.0..=12000.0)
                        .logarithmic(true)
                        .text("Cutoff"),
                )
                .changed()
            {
                self.sync_params();
            }
            if ui
                .add(
                    egui::Slider::new(&mut self.filter_resonance, 0.0..=0.95).text("Resonance"),
                )
                .changed()
            {
                self.sync_params();
            }
            ui.add_space(12.0);
            ui.label("Keyboard: Z S X D C V G B H N J M");
            ui.label(format!("Middle C = 48, freq ≈ {:.1} Hz", note_to_freq(48)));
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Piano");
            ui.add_space(8.0);
            let notes: &[(u8, &str)] = &[
                (48, "C"),
                (50, "D"),
                (52, "E"),
                (53, "F"),
                (55, "G"),
                (57, "A"),
                (59, "B"),
                (60, "C"),
            ];
            ui.horizontal_wrapped(|ui| {
                for (note, label) in notes {
                    let down = self.keys_down.contains(note);
                    let btn = egui::Button::new(format!("{label}\n{note}"))
                        .min_size(egui::vec2(56.0, 72.0))
                        .fill(if down {
                            ctx.style().visuals.widgets.active.bg_fill
                        } else {
                            ctx.style().visuals.widgets.inactive.bg_fill
                        });
                    let resp = ui.add(btn);
                    if resp.clicked() {
                        if down {
                            self.note_off(*note);
                        } else {
                            self.note_on(*note);
                        }
                    }
                }
            });
            ui.add_space(16.0);
            ui.label("Click keys to toggle notes, or hold QWERTY keys.");
        });

        if self.audio.is_some() {
            ctx.request_repaint();
        }
    }
}
