#!/usr/bin/env python3
"""Score polynomial seam fitter on holdout + cliff strata + real-wt (Phase F2)."""
from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
import bench_classical_vs_ai as cav  # noqa: E402
import metrics_snr_sdr as msm  # noqa: E402
from baselines.poly_seam_fitter import fit_poly_seam  # noqa: E402

HOLDOUT_SEED = 20260719
V6 = ROOT.parent / "denoise-opt-meta" / "paper" / "v6" / "figures"


@torch.no_grad()
def score_pair(ideal: torch.Tensor, out: torch.Tensor) -> dict:
    r = og.residual_score(ideal, out)
    sec = msm.secondary_metrics(ideal, out, periods=int(og.PROLONG), seam_w=og.SEAM_W)
    return {
        "R_mean": float(r.mean().item()),
        "R_std": float(r.std(unbiased=False).item()),
        **sec,
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--batch", type=int, default=64)
    args = ap.parse_args()
    device = torch.device(args.device)

    torch.manual_seed(HOLDOUT_SEED)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(HOLDOUT_SEED)
    ideal, eng = og.make_batch(args.batch, og.N, device)

    methods = {
        "no_bake": lambda x: x,
        "dual_cosine": og.dual_cosine_blend,
        "seam_fir3": cav.seam_fir3,
        "poly_seam_d3": lambda x: fit_poly_seam(x, degree=3, seam_w=og.SEAM_W),
        "poly_seam_d1": lambda x: fit_poly_seam(x, degree=1, seam_w=og.SEAM_W),
    }

    rows = {}
    for name, fn in methods.items():
        if device.type == "cuda":
            torch.cuda.synchronize()
        t0 = time.perf_counter()
        out = fn(eng)
        if device.type == "cuda":
            torch.cuda.synchronize()
        ms = (time.perf_counter() - t0) * 1000.0
        row = score_pair(ideal, out)
        row["ms_batch"] = float(ms)
        rows[name] = row
        print(f"{name}: R={row['R_mean']:.4f} jump={row['wrap_jump_mean']:.4f} ms={ms:.2f}")

    blob = {
        "meta": {
            "holdout_seed": HOLDOUT_SEED,
            "batch": args.batch,
            "L": int(og.N),
            "SEAM_W": int(og.SEAM_W),
            "ssm_status": "deferred",
            "ssm_reason": "timeboxed; LSTM remains seq ceiling (train seed 424243)",
            "nomenclature": {"no_bake": "passthrough unrepaired engine; legacy key identity"},
        },
        "canonical_holdout": rows,
    }
    if "no_bake" in rows:
        rows["identity"] = rows["no_bake"]
    V6.mkdir(parents=True, exist_ok=True)
    out_path = V6 / "poly_baseline.json"
    out_path.write_text(json.dumps(blob, indent=2), encoding="utf-8")
    print(f"wrote {out_path}")
    local = ROOT / "brand" / "artifacts" / "poly_baseline.json"
    local.parent.mkdir(parents=True, exist_ok=True)
    local.write_text(json.dumps(blob, indent=2), encoding="utf-8")


if __name__ == "__main__":
    main()
