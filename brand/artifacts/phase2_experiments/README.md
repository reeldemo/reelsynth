# Phase 2 experiments (DenoiseOpt review B3 / M3 / M5)

## Julian rule (≥3 search seeds)

Every experiment run uses **at least three search seeds** by default (`1902771841`, `2026072701`, `2026072702`).  
Holdout results are reported as **mean ± std** across those search seeds (each seed: 5 holdout eval seeds `20260719`…`20260723`).

## Production (GPU plateau for seeds 2026072701 / 2026072702)

```bash
python scripts/phase2_experiments/run_all.py --device cuda
```

- **~8.5 GPU·h per search seed** (5k outer iters, RTX 3090 class) for each plateau seed not yet in `search_<seed>/hybrid_lstm/summary.json`.
- Seed `1902771841` reuses `meta_approach_compare` unless `--force-fresh-search`.
- **Do not fake** missing GPU runs; holdout report lists missing seeds and the command above.

Eval-only (no search):

```bash
python scripts/phase2_experiments/run_all.py --skip-search --device cuda
```

## CI / CPU smoke

```bash
python scripts/phase2_experiments/run_all.py --device cpu --smoke
```

Tiny hybrid (`smoke_hybrid` in `config.json`) trains plateau seeds on CPU for plumbing only; production claims still require CUDA 5k runs.

Workflow: `.github/workflows/ci.yml` on **ubuntu-latest** (`workflow_dispatch`, push/PR to `phase2-experiments`).  
Job `python-phase2` runs `run_all.py --device cpu --smoke` (3 seeds, tiny budget, mean ± std report).

## Outputs

| File | Content |
|------|---------|
| `holdout_eval.csv` | Per search seed: holdout `R_blend` mean ± std (α=0.7) |
| `alpha_sensitivity.csv` | α ∈ {0.5…0.9} holdout means |
| `compute_budget.csv` | Arch evaluations + GPU-hours from logs |
| `RUN_STATUS.json` | Orchestrator step status |

Config: `config.json`.
