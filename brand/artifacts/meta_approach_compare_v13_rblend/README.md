# v13 D1 — matched-5k multi-seed re-search under R_blend

## Reboot / crash rule

**Checkpoints are the source of truth.** Safe to reboot anytime.

After login / reboot, either:

```powershell
cd C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\launch_v13_multiseed_search.ps1
```

or install auto-resume once:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File scripts\install_v13_d1_autostart.ps1
```

That puts Startup shortcuts for the launcher + watchdog. Both are **resume-only** (never `--force-fresh`).

## Status

```powershell
powershell -File scripts\status_v13_d1.ps1
```

## Layout

```
<meta_approach_compare_v13_rblend>/
  launcher.log
  watchdog.log
  <seed>/
    run.log
    hybrid_lstm/
      checkpoint.json   # resume every 25 iters
      history.jsonl
      champ_cell.pt
      summary.json      # written when 5000 done
    random/ ...
```

## Seeds / approaches

- Seeds: `1902771841`, `2026072701`, `2026072702`
- Order: `hybrid_lstm`, `random`, `cmaes`, `tpe`, `aging_evo`, `reinforce`
- Iters: 5000 under locked `R_blend` / `J`

## When all complete

```powershell
.\.venv_gpu\Scripts\python.exe scripts\finish_v13_d1_when_ready.py
```

## Do not

- Do not pass `--force-fresh` (wipes progress).
- Do not start a second launcher while one is already writing this tree (launcher self-exits if busy).
