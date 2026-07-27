#!/usr/bin/env python3
"""Aggregate v13 multi-seed holdout + (partial) search stats for the paper."""
from __future__ import annotations

import json
import math
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
HOLDOUT = ROOT / "brand" / "artifacts" / "holdout_multiseed_v13"
SEARCH = ROOT / "brand" / "artifacts" / "meta_approach_compare_v13_rblend"
PAPER_FIG = (
    ROOT.parent
    / "denoise-opt-meta"
    / "paper"
    / "Unsupervised_Wrap-Discontinuity_Repair_in_Wavetable_Synthesis_via_Hybrid_GA-PPO_Meta-Search_v13"
    / "figures"
)
OUT = HOLDOUT / "multiseed_summary.json"


def mean_std(xs: list[float]) -> tuple[float, float]:
    if not xs:
        return float("nan"), float("nan")
    m = sum(xs) / len(xs)
    if len(xs) < 2:
        return m, 0.0
    var = sum((x - m) ** 2 for x in xs) / (len(xs) - 1)
    return m, math.sqrt(var)


def sign_test_p(deltas: list[float]) -> dict[str, Any]:
    """Two-sided exact binomial sign test on nonzero deltas."""
    pos = sum(1 for d in deltas if d > 0)
    neg = sum(1 for d in deltas if d < 0)
    n = pos + neg
    if n == 0:
        return {"n": 0, "pos": 0, "neg": 0, "p_two_sided": 1.0}
    # exact binomial under H0 p=0.5
    # P(|X-n/2| >= |pos-n/2|) = 2 * sum_{k=k*}^n Binom
    from math import comb

    k = max(pos, neg)
    tail = sum(comb(n, i) for i in range(k, n + 1)) / (2**n)
    p = min(1.0, 2.0 * tail)
    return {"n": n, "pos": pos, "neg": neg, "p_two_sided": p}


def load_holdout() -> dict[str, Any]:
    seeds = []
    hold_ours, hold_n2n, hold_dc = [], [], []
    mf_ours_means, mf_n2n_means, mf_wins, mf_ns = [], [], [], []
    per_board_deltas: dict[str, list[float]] = {}
    cliff_rows: list[dict] = []

    for seed_dir in sorted(HOLDOUT.glob("20*")):
        if not seed_dir.is_dir():
            continue
        seed = seed_dir.name
        n2n_path = seed_dir / "n2n_vs_ours" / "n2n_vs_ours.json"
        if not n2n_path.is_file():
            continue
        blob = json.loads(n2n_path.read_text(encoding="utf-8"))
        seeds.append(int(seed))
        h = blob["holdout"]
        hold_ours.append(float(h["ours"]["R_blend"]))
        hold_n2n.append(float(h["n2n"]["R_blend"]))
        hold_dc.append(float(h.get("dual_cosine_R_blend", h.get("dual_cosine", {}).get("R_blend", float("nan")))))
        mf = blob["multifamily_summary"]
        mf_ours_means.append(float(mf["ours_R_blend_mean"]))
        mf_n2n_means.append(float(mf["n2n_R_blend_mean"]))
        mf_wins.append(int(mf["ours_beats_n2n_count"]))
        mf_ns.append(int(mf["n_boards"]))
        for key, row in blob.get("multifamily", {}).items():
            per_board_deltas.setdefault(key, []).append(
                float(row["delta_R_blend_ours_minus_n2n"])
            )

        cliff_path = seed_dir / "cliff_strata.json"
        if cliff_path.is_file():
            c = json.loads(cliff_path.read_text(encoding="utf-8"))
            flat = c.get("by_stratum", {})
            cliff_rows.append(
                {
                    "seed": int(seed),
                    "all_ours": flat.get("all", {}).get("neural_favorite", {}).get("R_mean"),
                    "all_n2n": flat.get("all", {}).get("n2n_corrupt_corrupt", {}).get("R_mean"),
                    "all_dc": flat.get("all", {}).get("dual_cosine", {}).get("R_mean"),
                    "top10_ours": flat.get("top10_wrap", {}).get("neural_favorite", {}).get("R_mean"),
                    "top10_dc": flat.get("top10_wrap", {}).get("dual_cosine", {}).get("R_mean"),
                    "top10_nobake": flat.get("top10_wrap", {}).get("no_bake", {}).get("R_mean"),
                }
            )

    # Aggregate board-level mean deltas across seeds, then sign-test
    board_mean_deltas = []
    for key, ds in sorted(per_board_deltas.items()):
        board_mean_deltas.append(sum(ds) / len(ds))

    o_m, o_s = mean_std(hold_ours)
    n_m, n_s = mean_std(hold_n2n)
    d_m, d_s = mean_std(hold_dc)
    mfo_m, mfo_s = mean_std(mf_ours_means)
    mfn_m, mfn_s = mean_std(mf_n2n_means)

    return {
        "seeds": seeds,
        "holdout": {
            "ours_mean": o_m,
            "ours_std": o_s,
            "n2n_mean": n_m,
            "n2n_std": n_s,
            "dual_cosine_mean": d_m,
            "dual_cosine_std": d_s,
            "ours_beats_n2n_seeds": sum(1 for a, b in zip(hold_ours, hold_n2n) if a > b),
            "n_seeds": len(seeds),
            "per_seed": {
                "ours": hold_ours,
                "n2n": hold_n2n,
                "dual_cosine": hold_dc,
            },
        },
        "multifamily": {
            "ours_mean_of_means": mfo_m,
            "ours_std_of_means": mfo_s,
            "n2n_mean_of_means": mfn_m,
            "n2n_std_of_means": mfn_s,
            "win_counts_per_seed": mf_wins,
            "n_boards_per_seed": mf_ns,
            "mean_win_frac": (
                sum(w / n for w, n in zip(mf_wins, mf_ns)) / len(mf_wins) if mf_wins else float("nan")
            ),
            "sign_test_on_board_mean_deltas": sign_test_p(board_mean_deltas),
            "board_mean_deltas": board_mean_deltas,
        },
        "cliff": cliff_rows,
    }


def load_search() -> dict[str, Any]:
    approaches: dict[str, list[float]] = {}
    for seed_dir in sorted(SEARCH.glob("*")):
        if not seed_dir.is_dir() or not seed_dir.name.isdigit():
            continue
        for ap_dir in seed_dir.iterdir():
            if not ap_dir.is_dir():
                continue
            summary = ap_dir / "summary.json"
            if not summary.is_file():
                continue
            blob = json.loads(summary.read_text(encoding="utf-8"))
            name = blob.get("approach") or ap_dir.name
            # Prefer r_blend / champ_raw under locked metric
            val = blob.get("champ_raw")
            if val is None:
                val = blob.get("champ_r")
            if val is None:
                continue
            approaches.setdefault(name, []).append(float(val))
    table = {}
    for name, vals in sorted(approaches.items()):
        m, s = mean_std(vals)
        table[name] = {"n": len(vals), "mean": m, "std": s, "values": vals}
    return {"approaches": table, "root": str(SEARCH)}


def main() -> int:
    summary = {
        "schema": "denoiseopt.v13_multiseed_summary.v1",
        "holdout_root": str(HOLDOUT),
        "search_root": str(SEARCH),
        "holdout": load_holdout(),
        "search": load_search(),
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    if PAPER_FIG.is_dir():
        (PAPER_FIG / "multiseed_summary.json").write_text(
            json.dumps(summary, indent=2), encoding="utf-8"
        )
    print(json.dumps(summary, indent=2)[:4000])
    print(f"wrote {OUT}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
