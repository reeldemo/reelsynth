# ReelSynth interop matrix (export v1)

Canonical export always starts from `.reelwt` + `.reelpreset`. Foreign formats are **targets**, not sources.

## v1 export targets

| Export target | Wavetable | Patch params | Mod matrix | FX | Loss notes |
|---------------|-----------|--------------|------------|-----|------------|
| `.reelwt` + `.reelpreset` | Full | Full | Full (16 slots) | N/A | Native canonical |
| Vital `.vitaltable` | Frames + name | Not embedded | Not embedded | No | JSON `{name, samples[][]}` only |
| WAV folder | One file per frame | N/A | N/A | N/A | `frame_NNN.wav`, 16-bit PCM mono |
| Serum `.fxp` WT subset | RSWT embedded blob | WT position, cutoff, ADSR | 4 slots max | No | See [SERUM_FXP.md](SERUM_FXP.md) |
| Ableton Wavetable map v2 | Multi-cycle WAV + `wav_frames/` | 5 param IDs + aliases | 4 macro hints | No | One-drag sprite; OSC/Send applies params; see WORKFLOW |
| SFZ | Rendered sample WAV | Filter opcodes subset | Dropped | No | One region per export |
| MIDI `.mid` | N/A | N/A | N/A | N/A | Type 0, single note default |
| Audio WAV stem | N/A | N/A | N/A | Post-synth | 24-bit PCM offline render |
| `reelpack/` bundle | All above + manifest | Session metadata in sidecar | In canonical preset | Optional | See [FORMAT.md](FORMAT.md#export) |

## Universal floor

Every `reelpack/` export emits **MIDI + 24-bit audio WAV** even when synth-specific targets fail. Failures are recorded in `export_report.json` — no silent downgrade.

## Round-trip expectations

| Pair | Frames | Params |
|------|--------|--------|
| Vital import → export → import | RMSE < 1e-5 | N/A |
| WAV folder import → export → import | Frame count preserved | N/A |
| Serum RSWT export → import | Exact float match | Partial (4 mod slots) |

## v2 (documented only)

- Surge `.wt`, Vital full `.vital` preset, CLAP/VST3 preset blob, FL Studio `.fst`
- Full Serum mod matrix and FX chain
- Ableton Live API wavetable **sprite** load (still unavailable; bridge uses multi-cycle drag + OSC params; VST3 is the seamless path)

## `export_report.json`

Each export writes or merges a report listing **dropped** and **non-mapped** parameters:

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

Reelpack merges child reports under `children` and aggregates `dropped` at the root.
