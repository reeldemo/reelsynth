#!/usr/bin/env python3
"""α sensitivity for R_blend on the 5-seed holdout (fixed champion, no retrain)."""
from __future__ import annotations

import argparse
import csv
import json
import sys
from pathlib import Path
from typing import Any

import torch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
from phase2_experiments.holdout_eval import (  # noqa: E402
    eval_holdout_seeds,
    load_summary,
    refit_hybrid,
    set_seed,
)
from baselines.n2n_seam import embedded as n2n_embedded  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--config", type=Path, default=ROOT / "brand/artifacts/phase2_experiments/config.json")
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--out-dir", type=Path, default=ROOT / "brand/artifacts/phase2_experiments")
    args = ap.parse_args()
    cfg_doc = json.loads(args.config.read_text(encoding="utf-8"))
    device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")
    args.out_dir.mkdir(parents=True, exist_ok=True)

    search_seed = int(cfg_doc.get("alpha_sweep_search_seed", 1902771841))
    alphas = [float(a) for a in cfg_doc["alpha_sweep"]]
    holdout_start = int(cfg_doc["holdout_eval_seed_start"])
    holdout_count = int(cfg_doc["holdout_eval_seed_count"])
    batch = int(cfg_doc.get("score_batch", 64))
    reuse = cfg_doc.get("reuse_search_summary") or {}

    run_dir = args.out_dir / f"search_{search_seed}" / "hybrid_lstm"
    summary_path = run_dir / "summary.json"
    if not summary_path.is_file() and str(search_seed) in reuse:
        summary_path = ROOT / reuse[str(search_seed)]
    if not summary_path.is_file():
        raise SystemExit(f"Missing champion summary for alpha sweep: {summary_path}")

    summary = load_summary(summary_path)
    cfg_h, cell = refit_hybrid(summary, search_seed, device)

    def ours_fn(eng: torch.Tensor) -> torch.Tensor:
        return og.apply_ops(eng, cell, cfg_h.ops)

    n2n = n2n_embedded(device)

    def n2n_fn(eng: torch.Tensor) -> torch.Tensor:
        return n2n(eng)

    rows: list[dict[str, Any]] = []
    for alpha in alphas:
        h_ours = eval_holdout_seeds(
            ours_fn,
            holdout_start=holdout_start,
            holdout_count=holdout_count,
            batch=batch,
            device=device,
            alpha=alpha,
        )
        h_n2n = eval_holdout_seeds(
            n2n_fn,
            holdout_start=holdout_start,
            holdout_count=holdout_count,
            batch=batch,
            device=device,
            alpha=alpha,
        )
        rows.append(
            {
                "alpha": alpha,
                "hybrid_holdout_R_mean": h_ours["R_mean"],
                "hybrid_holdout_R_std": h_ours["R_std"],
                "n2n_holdout_R_mean": h_n2n["R_mean"],
                "n2n_holdout_R_std": h_n2n["R_std"],
                "delta_hybrid_minus_n2n": h_ours["R_mean"] - h_n2n["R_mean"],
            }
        )

    out = {
        "schema": "denoiseopt.phase2_alpha_sweep.v1",
        "search_seed": search_seed,
        "summary_path": str(summary_path),
        "holdout_seeds": [holdout_start + k for k in range(holdout_count)],
        "note": "R_blend asymmetry: scores use R_seam(ideal,out) and R_body(eng,out); swapping ideal/eng changes α weights.",
        "rows": rows,
    }
    json_path = args.out_dir / "alpha_sensitivity.json"
    json_path.write_text(json.dumps(out, indent=2), encoding="utf-8")
    csv_path = args.out_dir / "alpha_sensitivity.csv"
    with csv_path.open("w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()))
        w.writeheader()
        w.writerow(rows[0])
        for row in rows[1:]:
            w.writerow(row)

    print(json.dumps(rows, indent=2))
    print(f"wrote {json_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
