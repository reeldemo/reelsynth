//! CPAL audio stream and engine handle.

use super::audio_commands::{drain_commands, AudioCmd};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::Sender;
use reelsynth::{Patch, ScopeMonitor, SequenceProject, SynthEngine, TransportState, WavetableBank};
use std::sync::{Arc, RwLock};

pub struct AudioHandle {
    tx: Sender<AudioCmd>,
    _stream: cpal::Stream,
    bank: Arc<RwLock<WavetableBank>>,
    transport: Arc<RwLock<TransportState>>,
    sequence: Arc<RwLock<SequenceProject>>,
    scope: ScopeMonitor,
}

impl AudioHandle {
    pub fn send(&self, cmd: AudioCmd) {
        let _ = self.tx.send(cmd);
    }

    pub fn bank(&self) -> Arc<RwLock<WavetableBank>> {
        Arc::clone(&self.bank)
    }

    pub fn transport(&self) -> Arc<RwLock<TransportState>> {
        Arc::clone(&self.transport)
    }

    pub fn sequence(&self) -> Arc<RwLock<SequenceProject>> {
        Arc::clone(&self.sequence)
    }

    pub fn scope(&self) -> ScopeMonitor {
        self.scope.clone()
    }
}

pub fn start_audio(sample_rate: u32) -> Result<AudioHandle, String> {
    let bank = WavetableBank::factory_saw_morph();
    let patch = Patch::factory_lead();
    let bank_shared = Arc::new(RwLock::new(bank.clone()));
    let transport_shared = Arc::new(RwLock::new(TransportState::new(patch.sequence.bpm)));
    let sequence_shared = Arc::new(RwLock::new(patch.sequence.clone()));
    let mut engine = SynthEngine::new(bank, patch, sample_rate);

    let (tx, rx) = crossbeam_channel::unbounded::<AudioCmd>();
    let bank_for_audio = Arc::clone(&bank_shared);
    let transport_for_audio = Arc::clone(&transport_shared);
    let sequence_for_audio = Arc::clone(&sequence_shared);

    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no audio output device".to_string())?;
    let config = device
        .default_output_config()
        .map_err(|e| e.to_string())?;
    let sr = config.sample_rate().0;
    if sr != sample_rate {
        engine = SynthEngine::new(WavetableBank::factory_saw_morph(), Patch::factory_lead(), sr);
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
                        drain_commands(
                            &mut engine,
                            &rx,
                            &bank_for_audio,
                            &transport_for_audio,
                        );
                        engine.process_stereo(data);
                        if let Ok(mut t) = transport_for_audio.write() {
                            *t = engine.transport().clone();
                        }
                        if let Ok(mut s) = sequence_for_audio.write() {
                            *s = engine.patch().sequence.clone();
                        }
                    },
                    err_fn,
                    None,
                )
            } else {
                device.build_output_stream(
                    &config.into(),
                    move |data: &mut [f32], _| {
                        drain_commands(
                            &mut engine,
                            &rx,
                            &bank_for_audio,
                            &transport_for_audio,
                        );
                        engine.process(data);
                        if let Ok(mut t) = transport_for_audio.write() {
                            *t = engine.transport().clone();
                        }
                        if let Ok(mut s) = sequence_for_audio.write() {
                            *s = engine.patch().sequence.clone();
                        }
                    },
                    err_fn,
                    None,
                )
            }
        }
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config.into(),
            move |data: &mut [i16], _| {
                drain_commands(
                    &mut engine,
                    &rx,
                    &bank_for_audio,
                    &transport_for_audio,
                );
                let mut buf = vec![0.0f32; data.len()];
                engine.process(&mut buf);
                if let Ok(mut t) = transport_for_audio.write() {
                    *t = engine.transport().clone();
                }
                if let Ok(mut s) = sequence_for_audio.write() {
                    *s = engine.patch().sequence.clone();
                }
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
        transport: transport_shared,
        sequence: sequence_shared,
        scope: scope_monitor,
    })
}

