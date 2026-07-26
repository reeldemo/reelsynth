# Interop (export v1)

Canonical state is always `.reelwt` + `.reelpreset`. Foreign formats are export targets, not sources of truth.

## Targets

| Target | Wavetable | Patch params | Mod matrix | FX | Notes |
|--------|-----------|--------------|------------|-----|-------|
| `.reelwt` + `.reelpreset` | Full | Full | Full (16 slots) | N/A | Native |
| Vital `.vitaltable` | Frames + name | No | No | No | JSON `{name, samples[][]}` |
| WAV folder | One file per frame | — | — | — | `frame_NNN.wav`, 16-bit mono |
| Serum `.fxp` WT subset | RSWT blob | WT pos, cutoff, ADSR | 4 slots max | No | [SERUM_FXP.md](SERUM_FXP.md) |
| Ableton map v2 | Multicycle WAV + frames | 5 params + aliases | 4 macro hints | No | One-drag sprite; OSC/Send for params |
| SFZ | Rendered sample | Filter opcodes subset | Dropped | No | One region |
| MIDI `.mid` | — | — | — | — | Type 0, single demo note |
| Audio WAV | — | — | — | Post-synth | 24-bit offline stem |
| `reelpack/` | Above + manifest | Sidecar metadata | In canonical preset | Optional | [FORMAT.md](FORMAT.md#export) |

## Floor

Every `reelpack/` still emits MIDI + 24-bit WAV even if a synth target fails. Failures go in `export_report.json` — no silent success.

## Round-trips we check

| Pair | Frames | Params |
|------|--------|--------|
| Vital import → export → import | RMSE < 1e-5 | N/A |
| WAV folder round-trip | Frame count kept | N/A |
| Serum RSWT export → import | Exact floats | Partial (4 mod slots) |

## v2 (documented, not shipped)

- Surge `.wt`, full Vital `.vital`, host preset blobs, FL `.fst`
- Full Serum mod matrix + FX
- Live API sprite load (still unavailable; bridge = multicycle drag + OSC; VST3 for in-Live play)

## `export_report.json`

```json
{
  "version": 1,
  "target": "serum",
  "success": true,
  "output_path": "synth/serum/patch_export.fxp",
  "dropped": [
    {"path": "mod_matrix[4]", "reason": "Serum v1 supports 4 mod slots; dropped lfo1→osc2_position"}
  ],
  "warnings": [],
  "errors": []
}
```

Reelpack nests child reports under `children` and rolls `dropped` up to the root.
