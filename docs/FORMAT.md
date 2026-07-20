# ReelSynth file formats

## `.reelwt` — wavetable bank (binary)

| Offset | Type | Field |
|--------|------|-------|
| 0 | `char[6]` | Magic `REELWT` |
| 6 | `u16` LE | Version (currently `1`) |
| 8 | `u32` LE | `num_frames` |
| 12 | `u32` LE | `frame_size` (samples per frame) |
| 16 | `f32[]` | Interleaved frames, length `num_frames * frame_size` |

Default factory banks use 256 frames × 2048 samples. All samples are float32 in −1…1.

## `.reelpreset` — patch (JSON, schema `reelsynth-preset-v1`)

```json
{
  "schema": "reelsynth-preset-v1",
  "name": "dark pluck",
  "wavetable_id": "saw_morph",
  "oscillators": [
    {"type": "wavetable", "level": 1.0, "position": 0.0, "detune": 0.0, "unison": 1}
  ],
  "filter": {"type": "lowpass", "cutoff": 1200, "resonance": 0.3},
  "filter2": {"type": "lowpass", "cutoff": 2400, "resonance": 0.25},
  "filters": [
    {"type": "lowpass", "cutoff": 1200, "resonance": 0.3},
    {"type": "highpass", "cutoff": 80, "resonance": 0.1}
  ],
  "envelope": {"attack": 0.01, "decay": 0.2, "sustain": 0.6, "release": 0.4},
  "lfo": {"rate": 0.5, "depth": 0.2, "target": "wt_position"},
  "mod_matrix": [
    {"source": "lfo1", "target": "osc1_position", "amount": 0.35}
  ]
}
```

Optional `filters` is the musical voice SVF chain (serial). When omitted, the engine uses legacy `filter` then `filter2`. An explicit `"filters": []` bypasses the voice filter. Header **Overtone** slots are session-only and are not stored here.

Sidecar `.reelpreset` may reference a sibling `.reelwt` by `wavetable_id` or embed `wavetable_path`.

## Import mapping gaps (v1)

| Source | Imported | Not imported (v2) |
|--------|----------|-------------------|
| Vital `.vitaltable` | frames, name | full mod matrix, FX |
| WAV folder | sorted cycles → frames | metadata |
| Serum `.fxp` | WT osc frames (subset) | full mod routing |

## Export (v1)

Export always from canonical `.reelwt` + `.reelpreset`. CLI:

```bash
cargo run --bin reelsynth-export -- vital table.reelwt -o table.vitaltable
cargo run --bin reelsynth-export -- reelpack patch.reelpreset -o out/ \
  --targets vital,wav,serum,ableton,sfz,midi,audio
```

Python (PyO3): `export_wavetable_py`, `export_preset_py`, `export_reelpack_py`.

### `reelpack/` layout

```
my_sound.reelpack/
  reelpack.json
  export_report.json
  canonical/patch.reelpreset
  canonical/table.reelwt
  synth/vital/table.vitaltable
  synth/wav_frames/frame_000.wav …
  synth/serum/patch_export.fxp
  synth/ableton/wavetable_map.json
  daw/midi/melody.mid
  daw/audio/melody.wav
  daw/sfz/patch.sfz + samples/
```

See [INTEROP.md](INTEROP.md) for the full loss matrix and [SERUM_FXP.md](SERUM_FXP.md) for Serum byte layout.
