# Design: FitCell train-to-convergence + latency-regularized meta-search (v14)

**Date:** 2026-07-31  
**Status:** implemented (2026-07-31) — Approach 1 + HP search + safe parallelization + regular reporting  
**Artifact tree:** `brand/artifacts/meta_approach_compare_v14_converge/` (never wipe v13)

## Problem

Matched outer-loop search used FitCell with a hard cap of ~16–48 steps. Early-stop existed but was too tight/short, so heavy arches (including `n2n_unet`) never finished training. Frozen Noise2Noise got a full dedicated train — unfair. GPU sat ~30–40% util while dual-seed parallel previously leaked VRAM.

## Goals

1. **Every** candidate (all arches / mixed graphs) trains **until plateau** (or a high hard cap).
2. Keep **latency in \(J\)** so slow models are regularized: \(J = R_{\mathrm{blend}} - \lambda \cdot \mathrm{latency\_norm}\).
3. **Search hyperparameters** for fit + \(\lambda\) + LR/batch (not only architecture).
4. **Report regularly** (fit steps used, converged?, champ, GPU).
5. **Use more of the GPU without dual-seed VRAM leaks** — prefer larger fit batches + optional guarded 2-way FitCell, never uncontrolled multi-seed on one GPU.

## Non-goals

- Replacing frozen N2N training recipe with FitCell.
- Re-running / wiping `meta_approach_compare_v13_rblend/`.
- Dual full seeds on one GPU by default.

## FitCell convergence protocol

```
fit_max_steps   = 1024          # hard safety cap (searched in {512,768,1024,1536,2048})
fit_patience    = 20            # consecutive stagnant checks (searched in {10,15,20,30})
fit_rel_eps     = 1e-5          # relative |ΔR|/|R| (searched in {1e-4, 3e-5, 1e-5, 3e-6})
fit_check_every = 1             # check plateau every N steps
```

- Stop when plateau hits **or** `fit_max_steps`.
- Return `(last_r, converged: bool, steps_used: int)`.
- Log `fit_steps_used`, `fit_converged`, `fit_max_steps` on every trial history row.

## Objective / latency

- Keep \(J = R - \lambda \cdot \mathrm{latency\_norm}\) with measured forward ms.
- Make \(\lambda\) searchable (default 0.02; grid e.g. `{0.01, 0.02, 0.05, 0.1}`).
- Prefer higher \(\lambda\) when choosing among near-tied \(R\).

## Hyperparameter search space (co-tuned online)

| Knob | Default | Search set |
|------|---------|------------|
| `fit_max_steps` | 1024 | 512, 768, 1024, 1536, 2048 |
| `fit_patience` | 20 | 10, 15, 20, 30 |
| `fit_rel_eps` | 1e-5 | 1e-4, 3e-5, 1e-5, 3e-6 |
| `lambda_latency` | 0.02 | 0.01, 0.02, 0.05, 0.1 |
| `lr` | existing | keep current PBT range |
| `batch` (FitCell) | 48 → **96** default | 48, 64, 96, 128 (VRAM-guarded) |

Outer iterations default **750** per approach (vs 5000) because each eval is ~20–40× more FitCell work.

## Safe parallelization (no dual-seed leak)

1. **Primary:** increase FitCell `batch` so a single trial saturates more SMs (target GPU util ↑ without second process).
2. **Optional micro-parallel (`--fit-parallel 1|2`):** at most **2** concurrent FitCells **only if** free VRAM ≥ `min_free_mib` (default 4096). After each fit: `del` temps + `torch.cuda.empty_cache()` + synchronize.
3. **Never** launch two full seed runners on one GPU by default (v13 parallel launcher stays deprecated).
4. Prefetch next `ArchConfig` on CPU while GPU fits (cheap; no extra VRAM).

## Reporting

- History JSONL: add fit convergence fields every trial.
- Every `--ckpt-every` (default 10): print + append status line with champ \(R\), \(J\), mean `fit_steps_used`, converged fraction, GPU mem.
- `scripts/status_v14_converge.ps1` mirrors v13 status.
- Periodic heartbeat file `STATUS.json` for dashboard/watchdog.

## Launch / artifacts

```
scripts/launch_v14_converge_search.ps1
  → brand/artifacts/meta_approach_compare_v14_converge/<seed>/
```

Seeds: same three as D1 (`1902771841`, `2026072701`, `2026072702`), sequential seeds, resume-only.

## Acceptance

- FitCell returns `steps_used` and `converged`; majority of MLP/`tf_split` trials converge before cap.
- `n2n_unet` trials routinely use ≫48 steps when improving.
- History + status expose convergence + HP.
- No dual-seed default; optional `--fit-parallel 2` aborts/falls back to 1 if VRAM low.
- Watchdog / launcher reboot-safe like v13.
