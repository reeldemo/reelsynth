# Serum `.fxp` (wavetable subset)

Export/import uses an embedded **`RSWT`** chunk in a minimal FXP wrapper. Not a full Xfer preset — frames plus a small scalar block for round-trip / WT handoff.

## FXP wrapper

| Offset | Size | Field |
|--------|------|-------|
| 0 | 4 | Magic `CcnK` |
| 4 | 4 | Payload size (u32 LE) |
| 8 | 4 | Content type `1` (preset) |
| 12 | 4 | Plugin ID `ReSy` |
| 16 | 4 | Preset index `0` |
| 20 | 4 | Preset name length (u32 LE) |
| 24 | N | Preset name (UTF-8) |
| 24+N | … | **RSWT payload** (below) |

## RSWT payload

| Offset | Size | Field |
|--------|------|-------|
| 0 | 4 | Tag `RSWT` |
| 4 | 4 | Table name length (u32 LE) |
| 8 | N | Table name (UTF-8) |
| 8+N | 4 | `num_frames` (u32 LE) |
| 12+N | 4 | `frame_size` (u32 LE) |
| 16+N | 4 | WT position (f32 LE) |
| 20+N | 4 | Filter cutoff Hz (f32 LE) |
| 24+N | 4 | Amp attack sec (f32 LE) |
| 28+N | 4 | Amp release sec (f32 LE) |
| 32+N | F×S×4 | Frame samples (f32 LE, interleaved frames) |

Where `F = num_frames`, `S = frame_size`.

## Import path

1. Scan for `RSWT` tag → structured parse (preferred, lossless frames).
2. Fallback: longest run of finite floats in ±1.5 range (legacy/heuristic).
3. If no floats: factory metallic placeholder (import never silent-fails).

## Export limitations (v1)

- Mod matrix: first **4** slots only; slots 4–15 listed in `export_report.json`.
- LFO depth, sub osc, noise: not embedded in RSWT.
- FX chain: not supported.

## Serum compatibility

Files produced are intended for ReelSynth round-trip and Reeldemo handoff. Loading in commercial Serum may require manual WT import from exported WAV frames until full FXP reverse engineering (v2).
