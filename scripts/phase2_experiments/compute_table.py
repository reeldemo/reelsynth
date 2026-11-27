#!/usr/bin/env python3
"""Build compute-matched comparison table (evaluations + GPU-hours) from logs and runs."""
from __future__ import annotations

import argparse
import csv
import json
import sys
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]


def _read_json(path: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    return json.loads(path.read_text(encoding="utf-8"))


def _count_history_iters(history: Path) -> int | None:
    if not history.is_file():
        return None
    n = 0
    with history.open(encoding="utf-8") as f:
        for line in f:
            if line.strip():
                n += 1
    return n


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--config", type=Path, default=ROOT / "brand/artifacts/phase2_experiments/config.json")
    ap.add_argument("--out-dir", type=Path, default=ROOT / "brand/artifacts/phase2_experiments")
    args = ap.parse_args()
    cfg_doc = json.loads(args.config.read_text(encoding="utf-8"))
    args.out_dir.mkdir(parents=True, exist_ok=True)

    rows: list[dict[str, Any]] = []

    # Noise2Noise: single training run (documented defaults)
    n2n_meta = _read_json(ROOT / "brand/artifacts/n2n_seam_baselines/n2n_baseline.json")
    n2n_steps = 20000
    n2n_wall_h: float | None = None
    if n2n_meta:
        modes = n2n_meta.get("modes") or {}
        cc = modes.get("n2n_corrupt_corrupt") or {}
        n2n_steps = int(cc.get("steps", n2n_steps))
        sec = cc.get("elapsed_sec")
        if sec is not None:
            n2n_wall_h = float(sec) / 3600.0
    rows.append(
        {
            "method": "Noise2Noise_corrupt_corrupt",
            "search_or_train_seeds": "train_424242",
            "arch_evaluations": n2n_steps,
            "gpu_hours": n2n_wall_h if n2n_wall_h is not None else "",
            "notes": "One-shot train; each step is one forward+backward batch (default steps=20000). "
            "GPU-hours from n2n_baseline.json when present, else train once to log.",
        }
    )

    # Overnight hybrid (single-seed reference from published freeze)
    overnight = _read_json(ROOT / "brand/artifacts/overnight_gpu_rl_arch_latest.json")
    if overnight:
        rows.append(
            {
                "method": "Overnight_hybrid_GA_PPO",
                "search_or_train_seeds": str(overnight.get("seed", 1902771841)),
                "arch_evaluations": int(overnight.get("iter", 0)),
                "gpu_hours": float(overnight.get("elapsed_sec", 0)) / 3600.0,
                "notes": f"gpu={overnight.get('gpu')}; branch-best freeze ~6.9k iters",
            }
        )

    reuse = cfg_doc.get("reuse_search_summary") or {}
    hybrid_iters = int(cfg_doc["hybrid"]["iters"])

    for search_seed in cfg_doc["search_seeds"]:
        seed_s = str(search_seed)
        run_dir = args.out_dir / f"search_{search_seed}" / "hybrid_lstm"
        summary_path = run_dir / "summary.json"
        if not summary_path.is_file() and seed_s in reuse:
            summary_path = ROOT / reuse[seed_s]
        summary = _read_json(summary_path) if summary_path.is_file() else None
        hist_path = run_dir / "history.jsonl"
        if summary and summary.get("history_path"):
            hp = Path(str(summary["history_path"]))
            if hp.is_file():
                hist_path = hp
        iters = (
            int(summary["iters_done"])
            if summary and summary.get("iters_done")
            else _count_history_iters(hist_path) or hybrid_iters
        )
        wall_h = float(summary["wall_h"]) if summary and summary.get("wall_h") is not None else ""
        status = "completed" if summary else "not_run"
        rows.append(
            {
                "method": "Meta_hybrid_GA_PPO_5k",
                "search_or_train_seeds": search_seed,
                "arch_evaluations": iters,
                "gpu_hours": wall_h,
                "notes": f"FitCell-to-plateau protocol; status={status}; summary={summary_path}",
            }
        )

    out = {
        "schema": "denoiseopt.phase2_compute.v1",
        "rows": rows,
        "honesty": "arch_evaluations = outer-loop trials (each includes FitCell). N2N row counts training steps.",
    }
    json_path = args.out_dir / "compute_budget.json"
    json_path.write_text(json.dumps(out, indent=2), encoding="utf-8")
    csv_path = args.out_dir / "compute_budget.csv"
    with csv_path.open("w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(
            f,
            fieldnames=["method", "search_or_train_seeds", "arch_evaluations", "gpu_hours", "notes"],
        )
        w.writeheader()
        for row in rows:
            w.writerow(row)

    print(json.dumps(rows, indent=2))
    print(f"wrote {json_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
