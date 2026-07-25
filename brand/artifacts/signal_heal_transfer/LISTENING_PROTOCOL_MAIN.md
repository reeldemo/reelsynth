# Formal listening protocol (main Results) — signal-heal transfer

**Status:** Protocol + WAV paths only. **No MOS / MUSHRA scores invented.**

## Purpose

Provide audible A/B evidence for wrap/seam transfer claims in the DenoiseOpt meta paper main body (v9). Listeners compare cracked engine vs classical DualCosine vs Ours (hybrid GA–PPO) on the same cycle, looped so the wrap click is obvious.

## Conditions (A/B — forced choice or preference)

| Condition | Label | Source |
|-----------|-------|--------|
| A | Engine (cracked wrap) | Domain engine period, tiled as wavetable |
| B | DualCosine classical | `dual_cosine_blend` on engine |
| C | Ours hybrid | Refit FitCell from domain `hybrid_lstm` champ |

Play **A → B → C** (or randomized order with labels hidden). Ask: which has the least wrap/seam artifact while preserving content? Do **not** collect clinical diagnostic judgments for ECG.

## Playback parameters

- Sample rate: **44100 Hz** mono 16-bit
- Pitch: **110 Hz** (low enough that wrap clicks are salient)
- Duration: **2.5 s** looped period playback
- Peak normalize to ~−1 dBFS (0.89 linear)

## WAV paths (reelsynth)

Root: `brand/artifacts/signal_heal_transfer/hear_pack/`

Manifest: `brand/artifacts/signal_heal_transfer/hear_pack/manifest.json`

Per dataset (after `scripts/export_signal_heal_hear_pack.py`):

```
hear_pack/<dataset>/01_tileK_engine.wav
hear_pack/<dataset>/01_tileK_dualcosine.wav
hear_pack/<dataset>/01_tileK_ours.wav
…
```

Expected dataset folders when champions exist:

- `cwru_bearings/`
- `mitbih_ecg/`
- `ptbxl_ecg/` (if ran)
- `synth_cnc_g01/` (synthetic CNC proxy — label as such)
- `synth_pmu_cycle/` (synthetic power proxy — label as such)

## Wavetable hear pack (main musical Results)

Unrelated but linked for paper authors: musical meta-compare hear samples live at

`brand/artifacts/meta_approach_compare/hear_samples/`

(rebuild: `scripts/export_meta_hear_samples.py --approach hybrid_lstm`).

## Rebuild

```bash
.venv_gpu/Scripts/python scripts/export_signal_heal_hear_pack.py
```

## Reporting rule for paper

Cite paths + protocol. Report listener preference counts only if a human study was actually run. Until then: “listening assets prepared; formal MOS not collected.”
