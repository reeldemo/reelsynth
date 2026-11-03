"""Print holdout aggregates after phase-2 eval steps."""
from __future__ import annotations

import json
from pathlib import Path
from typing import Any

from phase2_experiments._seeds import format_mean_std, mean_std


def print_holdout_report(holdout_json: Path, *, gate: float = 0.975) -> None:
    if not holdout_json.is_file():
        print(f"WARN: no holdout report at {holdout_json}")
        return
    doc = json.loads(holdout_json.read_text(encoding="utf-8"))
    alpha = doc.get("alpha", 0.7)
    n2n = doc.get("n2n_embedded") or {}
    n2n_mean = float(n2n.get("R_mean", float("nan")))
    n2n_std = float(n2n.get("R_std", float("nan")))

    print("\n=== Phase-2 holdout report (R_blend) ===", flush=True)
    print(
        f"N2N embedded — holdout mean ± std ({len(n2n.get('holdout_seeds', []))} eval seeds, α={alpha}): "
        f"{format_mean_std(n2n_mean, n2n_std)}",
        flush=True,
    )
    print(f"  per holdout seed: {n2n.get('per_seed', {})}", flush=True)

    per_search: list[float] = []
    missing: list[str] = []
    for seed_s, run in (doc.get("search_runs") or {}).items():
        if "holdout" in run:
            m = float(run["holdout"]["R_mean"])
            s = float(run["holdout"]["R_std"])
            per_search.append(m)
            print(
                f"Hybrid search seed {seed_s} — holdout mean ± std: {format_mean_std(m, s)}",
                flush=True,
            )
        elif "error" in run:
            missing.append(seed_s)
            print(f"Hybrid search seed {seed_s} — MISSING ({run['error']})", flush=True)

    agg = doc.get("hybrid_search_seed_aggregate") or {}
    if per_search:
        m, s = mean_std(per_search)
        print(
            f"\nAcross {len(per_search)} search seed(s) (mean of each seed's holdout mean): "
            f"{format_mean_std(m, s)}",
            flush=True,
        )
        if agg:
            print(
                f"  (json aggregate) beats_n2n_gate={agg.get('beats_n2n_gate_aggregate')} "
                f"gate={gate}",
                flush=True,
            )
    if missing:
        print(
            f"\nGPU plateau runs still required for search seed(s): {', '.join(missing)}. "
            "Command:\n"
            "  python scripts/phase2_experiments/run_all.py --device cuda\n"
            "(Do not fake; ~8.5 GPU·h per 5k seed on RTX 3090.)",
            flush=True,
        )
    print("=== end holdout report ===\n", flush=True)


def print_alpha_report(alpha_json: Path) -> None:
    if not alpha_json.is_file():
        return
    doc = json.loads(alpha_json.read_text(encoding="utf-8"))
    rows = doc.get("rows") or []
    row07 = next((r for r in rows if float(r.get("alpha", -1)) == 0.7), None)
    if row07:
        print(
            "α=0.7 holdout — hybrid "
            f"{format_mean_std(float(row07['hybrid_holdout_R_mean']), float(row07['hybrid_holdout_R_std']))} "
            f"vs N2N {format_mean_std(float(row07['n2n_holdout_R_mean']), float(row07['n2n_holdout_R_std']))}",
            flush=True,
        )
