#!/usr/bin/env python3
"""Holdout R_blend eval for hybrid champions (multi search seed) + N2N gate."""
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
from baselines.n2n_seam import embedded as n2n_embedded  # noqa: E402


def set_seed(seed: int, device: torch.device) -> None:
    torch.manual_seed(seed)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(seed)


@torch.no_grad()
def score_r_blend(
    ideal: torch.Tensor,
    eng: torch.Tensor,
    healed: torch.Tensor,
    *,
    alpha: float,
) -> float:
    return float(og.residual_score_blend(ideal, eng, healed, alpha=alpha).mean().item())


def refit_hybrid(
    summary: dict[str, Any],
    search_seed: int,
    device: torch.device,
) -> tuple[og.ArchConfig, og.SeamCell]:
    arch = summary["champ_arch"]
    hp = summary.get("champ_hp") or {}
    cfg = og.ArchConfig.from_dict(arch)
    cell = og.SeamCell(cfg).to(device)
    set_seed(search_seed, device)
    fit_steps = int(hp.get("fit_steps", 64))
    batch = int(hp.get("batch", 64))
    lr = float(hp.get("lr", 3e-3))
    adv = float(hp.get("adv_coef", 0.0))
    og.fit_cell(cell, cfg.ops, device, steps=fit_steps, batch=batch, lr=lr, adv_coef=adv)
    cell.eval()
    return cfg, cell


def eval_holdout_seeds(
    fn,
    *,
    holdout_start: int,
    holdout_count: int,
    batch: int,
    device: torch.device,
    alpha: float,
) -> dict[str, Any]:
    vals: list[float] = []
    per_seed: dict[str, float] = {}
    for k in range(holdout_count):
        seed = holdout_start + k
        set_seed(seed, device)
        ideal, eng = og.make_batch(batch, og.N, device)
        out = fn(eng)
        r = score_r_blend(ideal, eng, out, alpha=alpha)
        vals.append(r)
        per_seed[str(seed)] = r
    t = torch.tensor(vals, dtype=torch.float64)
    return {
        "holdout_seeds": [holdout_start + k for k in range(holdout_count)],
        "R_values": vals,
        "R_mean": float(t.mean().item()),
        "R_std": float(t.std(unbiased=False).item()) if len(vals) > 1 else 0.0,
        "per_seed": per_seed,
        "alpha": alpha,
        "batch": batch,
    }


def load_summary(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--config", type=Path, default=ROOT / "brand/artifacts/phase2_experiments/config.json")
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--out-dir", type=Path, default=ROOT / "brand/artifacts/phase2_experiments")
    ap.add_argument("--alpha", type=float, default=og.BLEND_ALPHA)
    args = ap.parse_args()
    cfg_doc = json.loads(args.config.read_text(encoding="utf-8"))
    device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")
    args.out_dir.mkdir(parents=True, exist_ok=True)

    holdout_start = int(cfg_doc["holdout_eval_seed_start"])
    holdout_count = int(cfg_doc["holdout_eval_seed_count"])
    batch = int(cfg_doc.get("score_batch", 64))
    reuse = cfg_doc.get("reuse_search_summary") or {}

    rows: list[dict[str, Any]] = []
    detail: dict[str, Any] = {
        "schema": "denoiseopt.phase2_holdout_eval.v1",
        "device": str(device),
        "alpha": args.alpha,
        "holdout_start": holdout_start,
        "holdout_count": holdout_count,
        "search_runs": {},
        "n2n_embedded": {},
    }

    # N2N gate (embedded weights — same ckpt as product / Rust)
    n2n = n2n_embedded(device)

    def n2n_fn(eng: torch.Tensor) -> torch.Tensor:
        return n2n(eng)

    n2n_h = eval_holdout_seeds(
        n2n_fn,
        holdout_start=holdout_start,
        holdout_count=holdout_count,
        batch=batch,
        device=device,
        alpha=args.alpha,
    )
    detail["n2n_embedded"] = n2n_h
    rows.append(
        {
            "method": "Noise2Noise_embedded",
            "search_seed": "",
            "holdout_R_mean": n2n_h["R_mean"],
            "holdout_R_std": n2n_h["R_std"],
            "beats_n2n_gate": "",
        }
    )

    gate = float(cfg_doc.get("n2n_gate_reference", {}).get("gate_value", 0.975))

    for search_seed in cfg_doc["search_seeds"]:
        seed_s = str(search_seed)
        run_dir = args.out_dir / f"search_{search_seed}" / "hybrid_lstm"
        summary_path = run_dir / "summary.json"
        if not summary_path.is_file() and seed_s in reuse:
            summary_path = ROOT / reuse[seed_s]
        if not summary_path.is_file():
            detail["search_runs"][seed_s] = {"error": f"missing summary: {summary_path}"}
            rows.append(
                {
                    "method": "hybrid_GA_PPO",
                    "search_seed": search_seed,
                    "holdout_R_mean": None,
                    "holdout_R_std": None,
                    "beats_n2n_gate": "missing_run",
                }
            )
            continue
        summary = load_summary(summary_path)
        cfg_h, cell = refit_hybrid(summary, int(search_seed), device)

        def ours_fn(eng: torch.Tensor, _cell=cell, _cfg=cfg_h) -> torch.Tensor:
            return og.apply_ops(eng, _cell, _cfg.ops)

        h = eval_holdout_seeds(
            ours_fn,
            holdout_start=holdout_start,
            holdout_count=holdout_count,
            batch=batch,
            device=device,
            alpha=args.alpha,
        )
        beats = h["R_mean"] > gate
        detail["search_runs"][seed_s] = {
            "summary_path": str(summary_path),
            "search_champ_r_train_geom": summary.get("champ_raw") or summary.get("champ_r"),
            "holdout": h,
            "beats_n2n_gate_mean": beats,
            "delta_mean_vs_n2n": h["R_mean"] - n2n_h["R_mean"],
        }
        rows.append(
            {
                "method": "hybrid_GA_PPO",
                "search_seed": search_seed,
                "holdout_R_mean": h["R_mean"],
                "holdout_R_std": h["R_std"],
                "beats_n2n_gate": beats,
            }
        )

    # Aggregate across search seeds (honest mean of per-seed holdout means)
    hybrid_means = [
        float(detail["search_runs"][str(s)]["holdout"]["R_mean"])
        for s in cfg_doc["search_seeds"]
        if str(s) in detail["search_runs"] and "holdout" in detail["search_runs"][str(s)]
    ]
    if hybrid_means:
        ht = torch.tensor(hybrid_means, dtype=torch.float64)
        detail["hybrid_search_seed_aggregate"] = {
            "holdout_R_mean_of_search_means": float(ht.mean().item()),
            "holdout_R_std_across_search_seeds": float(ht.std(unbiased=False).item())
            if len(hybrid_means) > 1
            else 0.0,
            "per_search_seed_holdout_means": hybrid_means,
            "beats_n2n_gate_aggregate": float(ht.mean().item()) > gate,
        }

    detail["n2n_gate"] = gate
    detail["rows"] = rows

    json_path = args.out_dir / "holdout_eval.json"
    json_path.write_text(json.dumps(detail, indent=2), encoding="utf-8")
    csv_path = args.out_dir / "holdout_eval.csv"
    with csv_path.open("w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(
            f,
            fieldnames=["method", "search_seed", "holdout_R_mean", "holdout_R_std", "beats_n2n_gate"],
        )
        w.writeheader()
        for row in rows:
            w.writerow(row)

    print(json.dumps(detail.get("hybrid_search_seed_aggregate", {}), indent=2))
    print(f"wrote {json_path} and {csv_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
