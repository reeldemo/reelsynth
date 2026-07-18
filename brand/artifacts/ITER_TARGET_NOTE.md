# Iteration target note (hybrid RL+GA+depth+MoE)

**Date:** 2026-07-18  
**Run context:** `gpu-rl-arch-*` overnight CUDA search (`PPO+GA+PBT+NAS+depth+MoE`)

## Why not 1M

Live rate from `history.jsonl` / `overnight_gpu_rl_arch_latest.json` is ~**0.33–0.34 it/s** (not the ~13 it/s of lighter configs). At that pace:

| Budget | Iters reachable |
|--------|-----------------|
| 240 h  | ~**290–296k** |
| 1M @ 0.34 it/s | ~**817–830 h** (~34 days) |

So **1M does not fit** `--max-hours 240` for this hybrid + deep/MoE search.

## Paper-facing target

**250000** iterations (`--iters 250000 --max-hours 240`).

- Prefer 250k when rate ≥ 0.25 it/s (observed ~0.34).
- Would use 200k only if rate < 0.25.
- Slack vs 240 h: ETA ≈ **200–210 h** at 0.34 it/s (~30–40 h headroom).

## Finisher wait

`wait_1m_then_finish.ps1` and `overnight_1m_durable_finisher.ps1` complete when `iter >= 250000` from `overnight_gpu_rl_arch_latest.json` / history (or DONE flag), not 1M. Training loop also stops at 250k via `--iters`.

## ETA (from retarget restart)

Wall-clock to 250k ≈ **203 h** (~8.5 days) at measured rate; capped by `--max-hours 240`.

