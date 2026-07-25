# REPRO — signal-heal transfer (main-paper numbers)

## Environment

- Repo: `reelsynth`
- Python: `.venv_gpu/Scripts/python.exe` (CUDA preferred)
- Period length: `L=256`
- Metric: DenoiseOpt prolonged residual \(R\) (`overnight_gpu_rl_arch.residual_score`)
- Method under test: **hybrid_lstm** only (hybrid GA–PPO)
- Seeds: search/construction `1902771841`; holdout `20260719`

## Commands

```bash
# 1) Ensure raw data (CWRU Zenodo + MIT-BIH + PTB-XL subset; MFPT/Paderborn probed)
.venv_gpu/Scripts/python scripts/download_signal_heal_data.py

# 2) Build bundles + run classical board + hybrid search
.venv_gpu/Scripts/python scripts/bench_signal_heal_transfer.py --iters 250 --merge-existing

# New datasets only (keep prior CWRU/MIT-BIH rows):
.venv_gpu/Scripts/python scripts/bench_signal_heal_transfer.py --iters 250 --merge-existing \
  --datasets ptbxl_ecg,synth_cnc_g01,synth_pmu_cycle

# 3) Hear pack (A/B WAVs; no MOS)
.venv_gpu/Scripts/python scripts/export_signal_heal_hear_pack.py
```

## Outputs

| Path | Role |
|------|------|
| `brand/artifacts/signal_heal_transfer/results_table.json` | Main table + champ R |
| `brand/artifacts/signal_heal_transfer/fig_signal_heal_transfer.{png,pdf}` | Bar figure |
| `brand/artifacts/signal_heal_transfer/<dataset>/hybrid_lstm/summary.json` | Per-domain champ |
| `brand/artifacts/signal_heal_transfer/DEEP_SOTA_NOT_EXECUTED.json` | Honesty: deep baselines not run |
| `brand/artifacts/signal_heal_transfer/LISTENING_PROTOCOL_MAIN.md` | A/B protocol + WAV paths |
| `brand/artifacts/signal_heal_transfer/hear_pack/` | Exported WAVs |
| `brand/artifacts/signal_heal_transfer/cache/skipped_optional.json` | Skip reasons |

## Paper mirror

Figures/JSON copied into:

`../denoise-opt-meta/paper/Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9/figures/`

## Honesty

- Synthetic CNC/PMU rows are **proxies** when KIT / IEEE DataPort are login-walled.
- Deep SOTA (Cycle-GAN, BeatDiff, …) **not executed** — see `DEEP_SOTA_NOT_EXECUTED.json`.
- No invented MOS.
