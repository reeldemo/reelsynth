# v13 D1 matched-5k outer-loop re-search (`R_blend` + `J`)

**Status: COMPLETE** (2026-07-31)

Three search seeds × six approaches × 5000 iters, sequential resume-only.

| Seed | Hybrid champ `R_blend` | Notes |
|------|------------------------|-------|
| 1902771841 | **0.9766** | beats N2N gate (~0.9750) |
| 2026072701 | 0.9730 | |
| 2026072702 | 0.9748 | |

Mean±std hybrid: **0.9748 ± 0.0018**. Aging second (0.9635 ± 0.0049).

## Commands

```powershell
# status
powershell -File scripts\status_v13_d1.ps1

# resume (noop if done)
powershell -File scripts\launch_v13_multiseed_search.ps1

# re-aggregate paper numbers
.\.venv_gpu\Scripts\python.exe scripts\finish_v13_d1_when_ready.py
.\.venv_gpu\Scripts\python.exe scripts\analyze_v13_param_fluctuation.py
```

## Paper

Numbers live in `denoise-opt-meta` v13 Table `tab:meta-approaches` and `figures/multiseed_summary.json`.

Do **not** `--force-fresh` this tree.
