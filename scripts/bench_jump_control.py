#!/usr/bin/env python3
"""Jump-aware endpoint-pin control vs prolonged-R methods (Phase F3.1)."""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
import metrics_snr_sdr as msm  # noqa: E402
from baselines.endpoint_pin import endpoint_pin  # noqa: E402
from baselines.poly_seam_fitter import fit_poly_seam  # noqa: E402

HOLDOUT_SEED = 20260719
V11 = (
    ROOT.parent
    / "denoise-opt-meta"
    / "paper"
    / "Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v11"
    / "figures"
)


@torch.no_grad()
def score(ideal, eng, fn) -> dict:
    out = fn(eng)
    r = og.residual_score_blend(ideal, eng, out)
    sec = msm.secondary_metrics(
        ideal, out, periods=int(og.PROLONG), seam_w=og.SEAM_W, eng=eng, alpha=og.BLEND_ALPHA
    )
    return {
        "R_mean": float(r.mean().item()),
        "wrap_jump_mean": float(sec["wrap_jump_mean"]),
        "edge_rmse_mean": float(sec["edge_rmse_mean"]),
        "click_energy_mean": float(sec["click_energy_mean"]),
        "snr_db_mean": float(sec["snr_db_mean"]),
        "primary_metric": "r_blend",
        "blend_alpha": og.BLEND_ALPHA,
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--n-tiles", type=int, default=512)
    args = ap.parse_args()
    device = torch.device(args.device)
    torch.manual_seed(HOLDOUT_SEED)
    ideal, eng = og.make_batch(args.n_tiles, og.N, device)

    # Also hard-cliff subset
    jumps = msm.wrap_jump_abs(eng)
    p90 = float(torch.quantile(jumps, 0.90).item())
    mask = jumps >= p90
    ideal_h, eng_h = ideal[mask], eng[mask]

    methods = {
        "no_bake": lambda x: x,
        "dual_cosine": og.dual_cosine_blend,
        "endpoint_pin_mean": lambda x: endpoint_pin(x, seam_w=og.SEAM_W, mode="mean"),
        "endpoint_pin_zero": lambda x: endpoint_pin(x, seam_w=og.SEAM_W, mode="zero"),
        "poly_seam_d3": lambda x: fit_poly_seam(x, degree=3, seam_w=og.SEAM_W),
    }

    blob = {
        "meta": {
            "holdout_seed": HOLDOUT_SEED,
            "n_tiles": args.n_tiles,
            "p90_wrap_jump": p90,
            "n_hard": int(mask.sum().item()),
            "note": "Endpoint-pin lowers wrap-jump; R_blend often drops vs no-bake/favorite.",
            "primary_metric": "r_blend",
            "blend_alpha": og.BLEND_ALPHA,
            "nomenclature": {"no_bake": "passthrough unrepaired engine; legacy key identity"},
        },
        "all": {},
        "top10_wrap": {},
    }
    for name, fn in methods.items():
        blob["all"][name] = score(ideal, eng, fn)
        blob["top10_wrap"][name] = score(ideal_h, eng_h, fn)
        print(
            f"{name}: all R_blend={blob['all'][name]['R_mean']:.4f} "
            f"jump={blob['all'][name]['wrap_jump_mean']:.4f} | "
            f"hard R_blend={blob['top10_wrap'][name]['R_mean']:.4f} "
            f"jump={blob['top10_wrap'][name]['wrap_jump_mean']:.4f}"
        )
    # Legacy alias
    for section in ("all", "top10_wrap"):
        if "no_bake" in blob[section]:
            blob[section]["identity"] = blob[section]["no_bake"]

    V11.mkdir(parents=True, exist_ok=True)
    path = V11 / "jump_control.json"
    path.write_text(json.dumps(blob, indent=2), encoding="utf-8")
    print(f"wrote {path}")
    local = ROOT / "brand" / "artifacts" / "jump_control.json"
    local.write_text(json.dumps(blob, indent=2), encoding="utf-8")
    print(f"wrote {local}")


if __name__ == "__main__":
    main()
