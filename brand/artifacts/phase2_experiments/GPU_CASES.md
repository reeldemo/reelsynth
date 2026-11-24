# GPU command & cost estimate (case suite)

**No GPU run is performed as part of pre-registration.** Use this when champions exist at 5k plateau for all three search seeds.

## Command

```bash
# From repo root, CUDA host (e.g. Vast.ai RTX 3090, PyTorch 2.6+cu124)
pip install torch==2.6.0 --index-url https://download.pytorch.org/whl/cu124
pip install numpy matplotlib

# Ensure plateau summaries (one-time, ~17 GPU·h if 2026072701/02 not trained yet):
python scripts/phase2_experiments/run_all.py --device cuda --skip-search
# or full search:
# python scripts/phase2_experiments/run_all.py --device cuda

# Targeted case matrix (eval only — no re-search per case):
python scripts/phase2_experiments/run_all.py --device cuda --cases
```

## Runtime estimate (RTX 3090 class, eval-only `--cases`)

| Component | Count | ~Time |
|-----------|------:|------:|
| Case IDs | 20 | |
| Search seeds (refit+eval) | 3 | |
| Holdout eval seeds | 5 | |
| Forward passes per (case, s, h) | hybrid + N2N | ~2× batch 64 |

Empirical CPU smoke (1 case/family, batch 12, 1 holdout seed): **< 15 s**.  
Scaled to production: 20 × 3 × 5 × 2 forwards ≈ **600** batch evaluations → **~3–8 GPU·min** (dominated by FitCell refit: 3 refits once per search seed, cached per `run_cases.py`).

| Scenario | GPU·h (order of magnitude) |
|----------|---------------------------|
| `--cases` only (champions present) | **0.1–0.2** |
| Full 5k search for 2 plateau seeds + `--cases` | **~17–18** (search) + **0.2** (cases) |

## Vast.ai ballpark (2026 list prices, RTX 3090 ~$0.20–0.35/hr)

| Work | Cost |
|------|------|
| Case eval only | **< $0.10** |
| Full phase-2 (2× 5k search + cases) | **~$3.50–6.50** |

Report actual wall time and `$` from the provider invoice; do not substitute estimates for measured logs.
