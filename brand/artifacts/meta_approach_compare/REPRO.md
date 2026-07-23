# Meta-approach 5k compare — reproducibility

- Suite completed (UTC): `2026-07-23T02:02:58Z` (`STATUS.json` `all_complete: true`)
- Aggregate rebuilt: `meta_approach_compare.json` + TeX table + learning curves + bar chart
- Search seed: `1902771841`
- Holdout / heal-viz seed: `20260719` (tile prefer `46`)
- Iters: `5000` per approach
- Approaches (code → manuscript): random→Random NAS, cmaes→Cont. CMA-ES, reinforce→Arch REINFORCE, aging_evo→Aging evolution, tpe→TPE Bayes NAS, hybrid_lstm→Ours (hybrid GA–PPO)
- Vocab: LSTM + xLSTM in `BLOCKS` (LSTM?/xLSTM? columns = champion graph membership)
- Reward modes (hybrid/REINFORCE/PBT): abs_r, vs_dualcosine, vs_nobake, neglog_gap
- Winner @5k (absolute champ $R$): **Ours (hybrid GA–PPO)** $R\approx 0.99146$, $\Delta R\approx +0.17488$ vs DualCosine $\approx 0.81658$, wall $\approx 8.46$\,h
- Device (suite): `NVIDIA GeForce RTX 3090` / torch `2.6.0+cu124`

## Artifacts (paper/v7/figures)

| File | Role |
|------|------|
| `meta_approach_compare.json` | Publishable aggregate |
| `meta_approaches_table.tex` | Table~\ref{tab:meta-approaches} |
| `fig_meta_approach_compare.png` | Learning curves |
| `meta_approach_bars.png` / `.pdf` | Bar chart (champ $R$ + $\Delta R$) |
| `fig_meta_heal_samples.png` / `.pdf` / `.json` | Healed wrap-seam eval |
| `hear_samples/` | Audible WAV demos (5 holdout tiles × nobake / DualCosine / Ours) |

## Rebuild figures without re-running search

From reelsynth root (GPU venv):

```bash
.venv_gpu/Scripts/python.exe scripts/bench_meta_approaches_5k.py --aggregate-only
.venv_gpu/Scripts/python.exe scripts/plot_meta_heal_samples.py --approach hybrid_lstm
.venv_gpu/Scripts/python.exe scripts/export_meta_hear_samples.py --approach hybrid_lstm
```

## Relaunch clean publishable run (destructive)

```bash
python scripts/launch_meta_approach_compare.py --iters 5000 --force --fresh
```

Poll:

```bash
python scripts/meta_approach_status.py
```

## Honest limits

- Heal figure refits FitCell from champion arch+HP; suite checkpoints do not persist fitted `state_dict`.
- DualCosine $\Delta R$ is a reporting / PPO-centering gap only; objective is maximize absolute $R$ toward the ideal sibling.
- Rank learned methods vs the full classical board (no-bake, FIR, DualCosine, \ldots), not DualCosine alone.
