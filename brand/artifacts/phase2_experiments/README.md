# Phase 2 experiments (DenoiseOpt review B3 / M3 / M5)

## Run (GPU required for new search seeds)

```bash
# Full pipeline: hybrid 5k search on seeds 2026072701/02 + holdout + α sweep + compute table
python scripts/phase2_experiments/run_all.py --device cuda

# Eval only (reuse meta_approach_compare summary for seed 1902771841)
python scripts/phase2_experiments/run_all.py --skip-search --device cuda
```

**Hardware:** NVIDIA CUDA GPU recommended (published 5k hybrid ≈ 8.5 GPU·h per search seed on RTX 3090; two new seeds ≈ 17 GPU·h). CPU can run holdout/α eval on an existing champion but not full 5k search in reasonable time.

## Outputs

| File | Content |
|------|---------|
| `holdout_eval.csv` | 5-seed holdout `R_blend` (α=0.7) vs N2N gate 0.975 |
| `alpha_sensitivity.csv` | α ∈ {0.5…0.9} holdout means |
| `compute_budget.csv` | Arch evaluations + GPU-hours from logs |
| `RUN_STATUS.json` | Orchestrator step status |

Config: `config.json`.
