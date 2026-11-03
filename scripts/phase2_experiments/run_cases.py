#!/usr/bin/env python3
"""Pre-registered targeted case suite: hybrid vs N2N (see PREREGISTRATION.md)."""
from __future__ import annotations

import argparse
import csv
import json
import sys
from pathlib import Path
from typing import Any, Callable

import torch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
import metrics_snr_sdr as msm  # noqa: E402
import overnight_gpu_rl_arch as og  # noqa: E402
from baselines.n2n_seam import embedded as n2n_embedded  # noqa: E402
from phase2_experiments._seeds import (  # noqa: E402
    format_mean_std,
    load_config,
    mean_std,
    require_min_search_seeds,
    resolve_search_seeds,
)
from phase2_experiments.case_generators import CaseSpec, all_cases, make_case_batch, smoke_cases  # noqa: E402
from phase2_experiments.holdout_eval import load_summary, refit_hybrid  # noqa: E402

JUMP_HALF_W = 8
ALPHA = 0.7


def summary_path_for_seed(cfg: dict, out_dir: Path, search_seed: int) -> Path | None:
    seed_s = str(search_seed)
    run_dir = out_dir / f"search_{search_seed}" / "hybrid_lstm" / "summary.json"
    if run_dir.is_file():
        return run_dir
    reuse = (cfg.get("reuse_search_summary") or {}).get(seed_s)
    if reuse:
        p = ROOT / reuse
        if p.is_file():
            return p
    return None


@torch.no_grad()
def score_batch(
    ideal: torch.Tensor,
    eng: torch.Tensor,
    jump_idx: torch.Tensor,
    heal_fn: Callable[[torch.Tensor], torch.Tensor],
) -> dict[str, float]:
    out = heal_fn(eng)
    r_blend = float(og.residual_score_blend(ideal, eng, out, alpha=ALPHA).mean().item())
    r_jump = float(msm.jump_local_r(ideal, out, jump_idx, half_w=JUMP_HALF_W).mean().item())
    sec = msm.secondary_metrics(
        ideal, out, periods=int(og.PROLONG), seam_w=og.SEAM_W, eng=eng, alpha=ALPHA
    )
    return {
        "R_blend": r_blend,
        "R_jump": r_jump,
        "snr_db_mean": sec.get("snr_db_mean", float("nan")),
        "sdr_db_mean": sec.get("sdr_db_mean", float("nan")),
        "edge_rmse_mean": sec.get("edge_rmse_mean", float("nan")),
    }


def preregistered_win(deltas: list[float], per_holdout: list[float]) -> dict[str, Any]:
    if not deltas:
        return {"mu_delta": None, "sigma_delta": None, "win_blend": False}
    mu, sig = mean_std(deltas)
    hold_ok = all(x > 0 for x in per_holdout)
    return {
        "mu_delta": mu,
        "sigma_delta": sig,
        "min_holdout_mean_delta": min(per_holdout) if per_holdout else None,
        "win_directional": bool(mu > sig and hold_ok),
        "holdout_all_positive": hold_ok,
    }


def run_suite(
    *,
    cases: list[CaseSpec],
    search_seeds: list[int],
    holdout_seeds: list[int],
    batch: int,
    device: torch.device,
    cfg: dict,
    out_dir: Path,
) -> dict[str, Any]:
    out_cases = out_dir / "cases"
    out_cases.mkdir(parents=True, exist_ok=True)
    n2n = n2n_embedded(device)

    def n2n_fn(eng: torch.Tensor) -> torch.Tensor:
        return n2n(eng)

    hybrids: dict[int, tuple[Any, Any]] = {}
    for s in search_seeds:
        sp = summary_path_for_seed(cfg, out_dir, s)
        if sp is None:
            raise SystemExit(f"Missing hybrid summary for search seed {s}")
        summary = load_summary(sp)
        hybrids[s] = refit_hybrid(summary, s, device)

    rows: list[dict[str, Any]] = []
    case_summaries: list[dict[str, Any]] = []

    for spec in cases:
        deltas_blend: list[float] = []
        deltas_jump: list[float] = []
        holdout_mean_blend: list[float] = []
        holdout_mean_jump: list[float] = []

        for h_seed in holdout_seeds:
            ideal, eng, jump_idx = make_case_batch(spec, batch, og.N, device, h_seed)
            n2n_m = score_batch(ideal, eng, jump_idx, n2n_fn)
            per_search_blend: list[float] = []
            per_search_jump: list[float] = []
            for s in search_seeds:
                cfg_h, cell = hybrids[s]

                def heal_fn(e: torch.Tensor, _cell=cell, _cfg=cfg_h) -> torch.Tensor:
                    return og.apply_ops(e, _cell, _cfg.ops)

                hm = score_batch(ideal, eng, jump_idx, heal_fn)
                d_b = hm["R_blend"] - n2n_m["R_blend"]
                d_j = hm["R_jump"] - n2n_m["R_jump"]
                deltas_blend.append(d_b)
                deltas_jump.append(d_j)
                per_search_blend.append(d_b)
                per_search_jump.append(d_j)
                rows.append(
                    {
                        "case_id": spec.case_id,
                        "family": spec.family,
                        "holdout_seed": h_seed,
                        "search_seed": s,
                        "hybrid_R_blend": hm["R_blend"],
                        "n2n_R_blend": n2n_m["R_blend"],
                        "delta_R_blend": d_b,
                        "hybrid_R_jump": hm["R_jump"],
                        "n2n_R_jump": n2n_m["R_jump"],
                        "delta_R_jump": d_j,
                        "hybrid_snr_db": hm["snr_db_mean"],
                        "n2n_snr_db": n2n_m["snr_db_mean"],
                    }
                )
            holdout_mean_blend.append(sum(per_search_blend) / len(per_search_blend))
            holdout_mean_jump.append(sum(per_search_jump) / len(per_search_jump))

        win_b = preregistered_win(deltas_blend, holdout_mean_blend)
        win_j = preregistered_win(deltas_jump, holdout_mean_jump)
        # Across search seeds: mean ± std of holdout-averaged hybrid R_blend
        hybrid_by_search = []
        for s in search_seeds:
            vals = [
                r["hybrid_R_blend"]
                for r in rows
                if r["case_id"] == spec.case_id and r["search_seed"] == s
            ]
            if vals:
                hybrid_by_search.append(sum(vals) / len(vals))
        hs_m, hs_s = mean_std(hybrid_by_search)
        ns = [
            r["n2n_R_blend"]
            for r in rows
            if r["case_id"] == spec.case_id
        ]
        n2n_m = sum(ns) / len(ns) if ns else float("nan")

        case_summaries.append(
            {
                "case_id": spec.case_id,
                "family": spec.family,
                "params": spec.params,
                "hybrid_R_blend_holdout_mean±std_across_search": {
                    "mean": hs_m,
                    "std": hs_s,
                },
                "n2n_R_blend_mean": n2n_m,
                "decision_R_blend": win_b,
                "decision_R_jump": win_j,
            }
        )
        print(
            f"CASE {spec.case_id:16} hybrid R_blend (search mean±std): {format_mean_std(hs_m, hs_s)} "
            f"| N2N {n2n_m:.5f} | Δ_pool blend μ={win_b['mu_delta']:.5f} σ={win_b['sigma_delta']:.5f} "
            f"win={win_b['win_directional']}",
            flush=True,
        )

    payload = {
        "schema": "denoiseopt.phase2_cases.v1",
        "preregistration": "brand/artifacts/phase2_experiments/PREREGISTRATION.md",
        "jump_half_w": JUMP_HALF_W,
        "alpha": ALPHA,
        "search_seeds": search_seeds,
        "holdout_eval_seeds": holdout_seeds,
        "batch": batch,
        "device": str(device),
        "cases": case_summaries,
        "rows": rows,
    }
    (out_cases / "cases_results.json").write_text(json.dumps(payload, indent=2), encoding="utf-8")
    csv_path = out_cases / "cases_summary.csv"
    with csv_path.open("w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(
            f,
            fieldnames=[
                "case_id",
                "family",
                "hybrid_R_blend_mean_across_search",
                "hybrid_R_blend_std_across_search",
                "n2n_R_blend_mean",
                "mu_delta_blend",
                "sigma_delta_blend",
                "win_directional_blend",
            ],
        )
        w.writeheader()
        for c in case_summaries:
            d = c["decision_R_blend"]
            h = c["hybrid_R_blend_holdout_mean±std_across_search"]
            w.writerow(
                {
                    "case_id": c["case_id"],
                    "family": c["family"],
                    "hybrid_R_blend_mean_across_search": h["mean"],
                    "hybrid_R_blend_std_across_search": h["std"],
                    "n2n_R_blend_mean": c["n2n_R_blend_mean"],
                    "mu_delta_blend": d.get("mu_delta"),
                    "sigma_delta_blend": d.get("sigma_delta"),
                    "win_directional_blend": d.get("win_directional"),
                }
            )
    print(f"wrote {out_cases / 'cases_results.json'}", flush=True)
    return payload


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--config", type=Path, default=ROOT / "brand/artifacts/phase2_experiments/config.json")
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--smoke", action="store_true")
    ap.add_argument("--out-dir", type=Path, default=ROOT / "brand/artifacts/phase2_experiments")
    args = ap.parse_args()
    cfg = load_config(args.config)
    device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")
    search_seeds = resolve_search_seeds(cfg, "")
    require_min_search_seeds(search_seeds, cfg=cfg)
    h0 = int(cfg["holdout_eval_seed_start"])
    hcount = int(cfg["holdout_eval_seed_count"])
    holdout_seeds = [h0 + k for k in range(hcount)]
    if args.smoke:
        holdout_seeds = [h0]
        batch = int(cfg.get("smoke_holdout_batch", 12))
        cases = smoke_cases()
    else:
        batch = int(cfg.get("score_batch", 64))
        cases = all_cases()

    print(
        f"cases suite: n_cases={len(cases)} search_seeds={search_seeds} "
        f"holdout_seeds={holdout_seeds} batch={batch} smoke={args.smoke}",
        flush=True,
    )
    run_suite(
        cases=cases,
        search_seeds=search_seeds,
        holdout_seeds=holdout_seeds,
        batch=batch,
        device=device,
        cfg=cfg,
        out_dir=args.out_dir,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
