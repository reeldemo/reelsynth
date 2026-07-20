# Iteration target note (hybrid RL+GA+depth+MoE)

**Date:** 2026-07-18  
**Run context:** `gpu-rl-arch-*` overnight CUDA search (`PPO+GA+PBT+NAS+depth+MoE`)

## Why not 1M

Live rate from `history.jsonl` / `overnight_gpu_rl_arch_latest.json` varies by config. At hybrid + deep/MoE pace (~0.3–1.3 it/s depending on arch):

| Budget | Iters reachable (approx) |
|--------|--------------------------|
| 240 h @ 0.34 it/s | ~**290–296k** |
| 240 h @ 1.3 it/s | ~**1.1M** |
| 1M @ 0.34 it/s | ~**817–830 h** (~34 days) |

**1M is not the paper target** for this hybrid + deep/MoE search; watchdog/babysit must not relaunch `complex_arch` / `--iters 1000000`.

## Paper-facing target

**500000** iterations (`--iters 500000 --max-hours 240`).

- Matches live training worker and detached launcher defaults.
- Prefer 500k when rate supports finishing inside 240 h; otherwise retarget honestly in-run.
- Dense history: `--history-every 1`.

## Finisher wait

`wait_1m_then_finish.ps1`, `overnight_1m_durable_finisher.ps1`, babysit, and watchdog JobArgs complete / restart at **500000** (or DONE flag after target), not 250k or 1M.

## Warm-start

`--seed-fitted path/to/*_fitted.json` (or `.pt`) seeds `pop[0]` arch/hp and loads cell weights when present. Prior champ: `gpu-rl-arch-20260718T175603Z` **R≈0.9905** (`champion_iter_000795_fitted.json`).
