# QA matrix

Pitch × parameter coverage. Tests: [`tests/qa_matrix.rs`](../../tests/qa_matrix.rs), modules under [`tests/qa/`](../../tests/qa/).

## Pitch tiers

| Tier | Range | Use |
|------|-------|-----|
| **Piano full** | MIDI 21–108 (88 keys) | Factory Lead golden contract; nightly `#[ignore]` sweep |
| **Smoke trio** | MIDI 33, 69, 105 | Every PR — low A, A4, high G |
| **MIDI all** | MIDI 0–127 | Exhaustive note-number coverage (nightly / manual) |
| **Custom Hz** | `FREQ_CUSTOM` in `pitch_grid.rs` | Direct-frequency renders; stability-only above ~18 kHz |

Frequency for MIDI notes uses equal temperament: `440 × 2^((n−69)/12)` ([`note_to_freq`](../../src/engine/midi.rs)).

### Custom Hz values

| Hz | Role |
|----|------|
| 27.5 | A0 (piano low) |
| 55.0 | A1 |
| 110.0 | A2 |
| 220.0 | A3 |
| 440.0 | A4 reference |
| 880.0 | A5 |
| 1760.0 | A6 |
| 8000.0 | Upper partial range |
| 18000.0 | Near Nyquist @ 44.1 kHz — assert finite + bounded peak only |

## Parameter inventory

### Oscillator / wavetable

- WT position, morph A/B/amount, warp
- Wave quant (8/16/32/64/smooth), wave slot, wave slot fine
- Unison, detune, stereo spread, sub, noise
- FM source, ratio, index

### Envelopes

- Amp env (attack, decay, sustain, release)
- Filter env (attack, decay, sustain, release)

### Filters

- Filter 1 / 2: type, cutoff, resonance, key tracking, drive

### Modulation

- LFO1/2 rate, depth, shape, targets
- Mod matrix slots (velocity, filt_env, LFO, macros, MPE)
- Macros 1–8

### FX chain (per slot)

| Type | Key params |
|------|------------|
| Chorus | mix, rate, depth |
| Delay | mix, time_ms, feedback |
| Reverb | mix, size, damping |
| Distortion | drive, mix |
| Compressor | threshold, ratio, mix |

### Factory Lead contract

[`Patch::factory_lead()`](../../src/patch/factory.rs) is the default launch preset:

- `saw_morph` bank, frame 108 / slot 7 (“Lead”)
- Fast amp + filter envelopes, dual key-tracked filters
- LFO1 → WT pos, Vel → level, filt_env → cutoff
- Chorus + delay on, reverb bypassed

Golden WAV: `tests/fixtures/audio/factory_lead.wav` (regenerate via `cargo run --bin gen_qa_goldens`).

## Assertion rules

Implemented in [`tests/qa/invariants.rs`](../../tests/qa/invariants.rs):

| Check | Rule | Notes |
|-------|------|-------|
| `assert_render_finite` | Every sample is finite (no NaN/Inf) | All tiers |
| `assert_peak_bounded` | `peak(buf) ≤ 1.0` | Soft clip / headroom |
| `assert_rms_above_epsilon` | `rms(buf) > 1e-5` | Audible output at velocity 1.0 |
| `assert_smoke_render` | All three above | Default smoke helper |

High-frequency custom Hz (≥ 18 kHz): finite + peak bound only; RMS check may be relaxed in dedicated tests.

Additional helpers in [`tests/qa/helpers.rs`](../../tests/qa/helpers.rs): RMS range, spectral centroid, golden WAV comparison, stereo width.

## CI tiers

| Tier | Command | Coverage |
|------|---------|----------|
| **PR (default)** | `cargo test -p reelsynth` | Smoke trio × Factory Lead; amp/filt env + each FX type at 3 pitches; existing Q&A modules |
| **Nightly / manual** | `cargo test -p reelsynth -- --ignored` | Full piano 21–108 Factory Lead sweep (88 renders) |

Regenerate goldens after DSP or preset changes:

```bash
cargo run --bin gen_qa_goldens
```

## Module map

| File | Role |
|------|------|
| `pitch_grid.rs` | `PIANO_FULL`, `MIDI_ALL`, `FREQ_CUSTOM`, `SMOKE_PITCHES` |
| `invariants.rs` | Render assertion helpers |
| `matrix_factory_lead.rs` | Factory Lead × smoke + ignored piano sweep |
| `sweep_smoke.rs` | Amp env, filt env, each FX at smoke pitches |
| `helpers.rs` | Shared render + analysis utilities |
