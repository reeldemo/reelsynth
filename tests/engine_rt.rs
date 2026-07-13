//! Golden tests: offline render vs block-based realtime engine.

use approx::relative_eq;
use reelsynth::engine::SynthEngine;
use reelsynth::patch::{Envelope, Patch};
use reelsynth::wavetable::WavetableBank;

fn rms(samples: &[f32]) -> f32 {
    let mean = samples.iter().map(|s| s * s).sum::<f32>() / samples.len().max(1) as f32;
    mean.sqrt()
}

#[test]
fn offline_vs_realtime_rms_match_single_note() {
    let bank = WavetableBank::factory_saw_morph();
    let patch = Patch::default_mono();
    let sample_rate = 44100u32;
    let freq = 220.0f32;
    let duration = 0.5f32;

    let engine = SynthEngine::new(bank.clone(), patch.clone(), sample_rate);
    let offline = engine.render_offline(freq, duration);

    let num_samples = offline.len();
    let release_samples = (patch.envelope.release * sample_rate as f32) as usize;
    let tail_release = release_samples
        .min(num_samples.saturating_sub(1))
        .min(num_samples / 2)
        .max(1);
    let gate_samples = num_samples.saturating_sub(tail_release);

    let mut engine = SynthEngine::new(bank, patch, sample_rate);
    engine.note_on(0, 57, 1.0); // A3 ≈ 220 Hz

    let mut rt = vec![0.0f32; gate_samples];
    engine.process(&mut rt);

    engine.note_off(0, 57);
    let mut tail = vec![0.0f32; tail_release];
    engine.process(&mut tail);
    rt.extend(tail);

    let offline_rms = rms(&offline);
    let rt_rms = rms(&rt);
    assert!(
        relative_eq!(offline_rms, rt_rms, max_relative = 0.02),
        "offline_rms={offline_rms} rt_rms={rt_rms}"
    );
}

#[test]
fn engine_s3_param_setters() {
    let bank = WavetableBank::factory_saw_morph();
    let mut patch = Patch::default_mono();
    patch.ensure_oscillators(3);
    let mut engine = SynthEngine::new(bank, patch, 44100);

    engine.set_envelope(Envelope {
        attack: 0.05,
        decay: 0.1,
        sustain: 0.4,
        release: 0.2,
    });
    assert_eq!(engine.patch().envelope.attack, 0.05);

    engine.set_lfo_rate(3.5);
    engine.set_lfo_depth(0.25);
    assert_eq!(engine.patch().lfo.rate, 3.5);
    assert_eq!(engine.patch().lfo.depth, 0.25);

    engine.set_osc_level(1, 0.5);
    engine.set_osc_detune(2, 100.0);
    engine.set_sub_level(0.3);
    engine.set_noise_level(0.1);
    assert_eq!(engine.patch().oscillators[1].level, 0.5);
    assert_eq!(engine.patch().oscillators[2].detune, 100.0);
    assert_eq!(engine.patch().sub_level, 0.3);
    assert_eq!(engine.patch().noise_level, 0.1);
}

#[test]
fn polyphony_mixed_notes() {
    let bank = WavetableBank::factory_sine();
    let patch = Patch::default_mono();
    let mut engine = SynthEngine::new(bank, patch, 44100);

    engine.note_on(0, 60, 1.0);
    engine.note_on(0, 64, 0.8);
    engine.note_on(0, 67, 0.8);

    let mut block = vec![0.0f32; 2048];
    engine.process(&mut block);

    let peak = block.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
    assert!(peak > 0.01, "poly peak was {peak}");
}
