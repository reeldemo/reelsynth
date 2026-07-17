//! CPAL audio stream and engine handle.

use super::audio_commands::{drain_commands, AudioCmd};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, SupportedStreamConfig};
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
    device_name: String,
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

    pub fn device_name(&self) -> &str {
        &self.device_name
    }
}

fn sync_transport_and_sequence(
    engine: &SynthEngine,
    transport_for_audio: &Arc<RwLock<TransportState>>,
    sequence_for_audio: &Arc<RwLock<SequenceProject>>,
) {
    if let Ok(mut t) = transport_for_audio.write() {
        let mut snap = engine.transport().clone();
        if snap.recording {
            let step = engine.patch().sequence.quantize.division.beats_per_step();
            snap.live_recorded = engine.sequencer().recorder.snapshot(step);
        } else {
            snap.live_recorded.clear();
        }
        *t = snap;
    }
    if let Ok(mut s) = sequence_for_audio.write() {
        *s = engine.patch().sequence.clone();
    }
}

fn render_f32(engine: &mut SynthEngine, stereo: bool, out: &mut [f32]) {
    if stereo {
        engine.process_stereo(out);
    } else {
        engine.process(out);
    }
}

fn build_stream<T>(
    device: &cpal::Device,
    config: SupportedStreamConfig,
    stereo: bool,
    mut engine: SynthEngine,
    rx: crossbeam_channel::Receiver<AudioCmd>,
    bank_for_audio: Arc<RwLock<WavetableBank>>,
    transport_for_audio: Arc<RwLock<TransportState>>,
    sequence_for_audio: Arc<RwLock<SequenceProject>>,
) -> Result<cpal::Stream, String>
where
    T: Sample + cpal::SizedSample,
    f32: cpal::FromSample<T>,
    T: cpal::FromSample<f32>,
{
    let err_fn = |e| eprintln!("audio stream error: {e}");
    let mut scratch = Vec::new();
    device
        .build_output_stream(
            &config.into(),
            move |data: &mut [T], _| {
                drain_commands(
                    &mut engine,
                    &rx,
                    &bank_for_audio,
                    &transport_for_audio,
                );
                scratch.resize(data.len(), 0.0);
                render_f32(&mut engine, stereo, &mut scratch);
                for (out, sample) in data.iter_mut().zip(scratch.iter()) {
                    *out = T::from_sample(*sample);
                }
                sync_transport_and_sequence(
                    &engine,
                    &transport_for_audio,
                    &sequence_for_audio,
                );
            },
            err_fn,
            None,
        )
        .map_err(|e| e.to_string())
}

/// Resolve an output device by preferred name, falling back to the host default.
fn resolve_output_device(
    host: &cpal::Host,
    preferred: Option<&str>,
) -> Result<(cpal::Device, String), String> {
    if let Some(want) = preferred {
        if let Ok(devices) = host.output_devices() {
            for device in devices {
                if let Ok(name) = device.name() {
                    if name == want {
                        return Ok((device, name));
                    }
                }
            }
        }
    }
    let device = host
        .default_output_device()
        .ok_or_else(|| "no audio output device".to_string())?;
    let name = device
        .name()
        .unwrap_or_else(|_| "Default".to_string());
    Ok((device, name))
}

pub fn start_audio_on_device(
    sample_rate: u32,
    preferred_device: Option<&str>,
    bank: Option<WavetableBank>,
    patch: Option<Patch>,
) -> Result<AudioHandle, String> {
    let bank = bank.unwrap_or_else(WavetableBank::factory_saw_morph);
    let patch = patch.unwrap_or_else(Patch::factory_lead);
    let bank_shared = Arc::new(RwLock::new(bank.clone()));
    let transport_shared = Arc::new(RwLock::new(TransportState::new(patch.sequence.bpm)));
    let sequence_shared = Arc::new(RwLock::new(patch.sequence.clone()));

    let (tx, rx) = crossbeam_channel::unbounded::<AudioCmd>();
    let bank_for_audio = Arc::clone(&bank_shared);
    let transport_for_audio = Arc::clone(&transport_shared);
    let sequence_for_audio = Arc::clone(&sequence_shared);

    let host = cpal::default_host();
    let (device, device_name) = resolve_output_device(&host, preferred_device)?;
    let config = device
        .default_output_config()
        .map_err(|e| e.to_string())?;
    let sr = config.sample_rate().0;
    let engine_sr = if sr != 0 { sr } else { sample_rate };
    let engine = SynthEngine::new(bank, patch, engine_sr);
    let scope_monitor = engine.scope_monitor().clone();
    let stereo = config.channels() >= 2;
    let sample_format = config.sample_format();

    let stream = match sample_format {
        SampleFormat::F32 => build_stream::<f32>(
            &device,
            config,
            stereo,
            engine,
            rx,
            bank_for_audio,
            transport_for_audio,
            sequence_for_audio,
        )?,
        SampleFormat::I16 => build_stream::<i16>(
            &device,
            config,
            stereo,
            engine,
            rx,
            bank_for_audio,
            transport_for_audio,
            sequence_for_audio,
        )?,
        SampleFormat::I32 => build_stream::<i32>(
            &device,
            config,
            stereo,
            engine,
            rx,
            bank_for_audio,
            transport_for_audio,
            sequence_for_audio,
        )?,
        SampleFormat::U8 => build_stream::<u8>(
            &device,
            config,
            stereo,
            engine,
            rx,
            bank_for_audio,
            transport_for_audio,
            sequence_for_audio,
        )?,
        SampleFormat::U16 => build_stream::<u16>(
            &device,
            config,
            stereo,
            engine,
            rx,
            bank_for_audio,
            transport_for_audio,
            sequence_for_audio,
        )?,
        other => return Err(format!("unsupported sample format: {other:?}")),
    };

    stream.play().map_err(|e| e.to_string())?;

    Ok(AudioHandle {
        tx,
        _stream: stream,
        bank: bank_shared,
        transport: transport_shared,
        sequence: sequence_shared,
        scope: scope_monitor,
        device_name,
    })
}
