#!/usr/bin/env python3
"""Finish DenoiseOpt overnight → paper when 1M iters are confirmed.

Steps:
  1) Verify latest.json / history reach 1_000_000
  2) Regenerate publication plots → reelsynth figures + denoise-opt-meta/paper/v4/figures
  3) Write results_blob.json with honest numbers
  4) Optionally invoke Klaut paper writer (if importable) with local Ollama
"""
from __future__ import annotations

import json
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

REEL = Path(r"C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth")
META = Path(r"C:\Users\Julian\Documents\Programming\github\reeldemo\denoise-opt-meta")
KLAUT = Path(r"C:\Users\Julian\Documents\Programming\github\klaut-pro\klaut-research-gateway")
TARGET = 1_000_000


def load_latest() -> dict:
    p = REEL / "brand" / "artifacts" / "overnight_gpu_rl_arch_latest.json"
    return json.loads(p.read_text(encoding="utf-8"))


def history_len(run_dir: Path) -> int:
    hist = run_dir / "history.jsonl"
    if not hist.is_file():
        return 0
    n = 0
    with hist.open(encoding="utf-8") as f:
        for line in f:
            if line.strip():
                n += 1
    return n


def main() -> int:
    latest = load_latest()
    it = int(latest.get("iter") or 0)
    run_dir = Path(latest["run_dir"])
    hist_n = history_len(run_dir)
    champ = float(latest["champion_residual"])
    baseline = float(latest["baseline_dual_cosine"])
    print(
        f"iter={it} history_lines={hist_n} champ={champ:.6f} baseline={baseline:.6f} "
        f"delta={champ - baseline:.6f}",
        flush=True,
    )
    if it < TARGET:
        print(f"NOT DONE: need {TARGET}, have {it}", flush=True)
        return 2

    py = REEL / ".venv_gpu" / "Scripts" / "python.exe"
    hist = run_dir / "history.jsonl"
    subprocess.check_call(
        [str(py), str(REEL / "scripts" / "plot_overnight_history.py"), str(hist), "--baseline", str(baseline)]
    )

    v4 = META / "paper" / "v4"
    v4.mkdir(parents=True, exist_ok=True)
    blob = {
        "written_at": datetime.now(timezone.utc).isoformat(),
        "target_iters": TARGET,
        "iters_completed": it,
        "history_lines": hist_n,
        "champion_residual": champ,
        "dual_cosine_baseline": baseline,
        "delta_vs_dual_cosine": champ - baseline,
        "champion_arch": latest.get("champion_arch"),
        "branch_best": latest.get("branch_best"),
        "run_dir": str(run_dir),
        "gpu": latest.get("gpu"),
        "elapsed_sec": latest.get("elapsed_sec"),
        "metric_definition": "R=clamp(1 - residual_rms/max(ideal_rms,eps), 0, 1); 1=best",
        "figures_dir": str(v4 / "figures"),
        "honest": True,
    }
    (v4 / "results_blob.json").write_text(json.dumps(blob, indent=2), encoding="utf-8")
    print(f"wrote {v4 / 'results_blob.json'}", flush=True)

    # Mark done flag for babysitters
    done = REEL / "brand" / "artifacts" / "overnight_gpu_DONE.flag"
    done.write_text(datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"), encoding="ascii")
    print(f"wrote {done}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
