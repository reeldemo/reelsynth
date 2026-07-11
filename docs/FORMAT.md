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
  "envelope": {"attack": 0.01, "decay": 0.2, "sustain": 0.6, "release": 0.4},
  "lfo": {"rate": 0.5, "depth": 0.2, "target": "wt_position"},
  "mod_matrix": [
    {"source": "lfo1", "target": "osc1_position", "amount": 0.35}
  ]
}
```

Sidecar `.reelpreset` may reference a sibling `.reelwt` by `wavetable_id` or embed `wavetable_path`.

## Import mapping gaps (v1)

| Source | Imported | Not imported (v2) |
|--------|----------|-------------------|
| Vital `.vitaltable` | frames, name | full mod matrix, FX |
| WAV folder | sorted cycles → frames | metadata |
| Serum `.fxp` | WT osc frames (subset) | full mod routing |
