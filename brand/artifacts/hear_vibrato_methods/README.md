# Hear vibrato method comparison

Side-by-side **looped vibrato** WAVs for DenoiseOpt methods (wavetable meta + optional transfer).

## Playback

- Sample rate: **44100 Hz**, mono PCM16
- Base pitch: **440.0 Hz (A4)** with sinusoidal vibrato **5.0 Hz**, depth **±3.0%** (same family as `scripts/bench_vibrato_spectrogram.py`)
- Duration: **3.0 s**
- Factory Lead FX post: **on (chorus+delay approx)**
- Holdout / eval seed: **20260719**; refit seed: **1902771841**

## How to compare

1. Open `meta/` clips for the same tile (e.g. `01_tile46_*`).
2. Play **nobake → dualcosine → ours_hybrid** (then other meta arms if present).
3. Listen for wrap/seam clicks under pitch motion; Ours should be smoother.
4. Optional: `transfer/<dataset>/` for engine / DualCosine / Ours on non-WT domains.

Any audio player works (VLC, foobar, Windows Media Player). For A/B, loop or scrub the same region.

## Meta samples

| # | Tile | Files |
|---|------|-------|
| 1 | 46 (paper heal) | `01_tile46_nobake.wav`, `01_tile46_dualcosine.wav`, `01_tile46_ours_hybrid.wav`, `01_tile46_random.wav`, `01_tile46_cmaes.wav`, `01_tile46_reinforce.wav`, `01_tile46_aging_evo.wav`, `01_tile46_tpe.wav` |
| 2 | 21 | `02_tile21_nobake.wav`, `02_tile21_dualcosine.wav`, `02_tile21_ours_hybrid.wav`, `02_tile21_random.wav`, `02_tile21_cmaes.wav`, `02_tile21_reinforce.wav`, `02_tile21_aging_evo.wav`, `02_tile21_tpe.wav` |
| 3 | 4 | `03_tile4_nobake.wav`, `03_tile4_dualcosine.wav`, `03_tile4_ours_hybrid.wav`, `03_tile4_random.wav`, `03_tile4_cmaes.wav`, `03_tile4_reinforce.wav`, `03_tile4_aging_evo.wav`, `03_tile4_tpe.wav` |
| 4 | 49 | `04_tile49_nobake.wav`, `04_tile49_dualcosine.wav`, `04_tile49_ours_hybrid.wav`, `04_tile49_random.wav`, `04_tile49_cmaes.wav`, `04_tile49_reinforce.wav`, `04_tile49_aging_evo.wav`, `04_tile49_tpe.wav` |
| 5 | 48 | `05_tile48_nobake.wav`, `05_tile48_dualcosine.wav`, `05_tile48_ours_hybrid.wav`, `05_tile48_random.wav`, `05_tile48_cmaes.wav`, `05_tile48_reinforce.wav`, `05_tile48_aging_evo.wav`, `05_tile48_tpe.wav` |

## Transfer domains

- `cwru_bearings/` — 6 WAVs (tiles [4, 35])
- `mfpt_bearings/` — 6 WAVs (tiles [38, 59])
- `mitbih_ecg/` — 6 WAVs (tiles [37, 29])
- `ptbxl_ecg/` — 6 WAVs (tiles [22, 31])
- `synth_cnc_g01/` — 6 WAVs (tiles [13, 17])
- `synth_pmu_cycle/` — 6 WAVs (tiles [24, 49])

## Rebuild

```bash
.venv_gpu/Scripts/python.exe scripts/export_hear_vibrato_methods.py --device cpu
```

Skip other meta arms / transfer:

```bash
.venv_gpu/Scripts/python.exe scripts/export_hear_vibrato_methods.py --device cpu --no-meta-arms --no-transfer
```

See `manifest.json` for absolute paths and cycle R scores.
