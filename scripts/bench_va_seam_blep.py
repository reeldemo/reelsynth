#!/usr/bin/env python3
"""Score BLIT/BLEP, PolyBLEP, BLAMP seam repairs on frozen residual protocol.

Holdout seed 20260719. Canonical holdout + cliff strata + 20-wave multifamily.
Writes va_seam_blep.json under paper/v7/figures and brand/artifacts.
"""
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
import bench_sota_matrix as bsm  # noqa: E402
import metrics_snr_sdr as msm  # noqa: E402
from baselines.va_seam_blep import blit_blep_seam, blamp_seam, polyblep_seam  # noqa: E402

HOLDOUT_SEED = 20260719
V7 = ROOT.parent / "denoise-opt-meta" / "paper" / "v7" / "figures"
LOCAL = ROOT / "brand" / "artifacts" / "va_seam_blep.json"


def set_seed(seed: int, device: torch.device) -> None:
    torch.manual_seed(seed)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(seed)


@torch.no_grad()
def score_pair(ideal: torch.Tensor, out: torch.Tensor) -> dict:
    r = og.residual_score(ideal, out)
    sec = msm.secondary_metrics(ideal, out, periods=int(og.PROLONG), seam_w=og.SEAM_W)
    return {
        "R_mean": float(r.mean().item()),
        "R_std": float(r.std(unbiased=False).item()),
        **sec,
    }


@torch.no_grad()
def per_tile_metrics(ideal: torch.Tensor, out: torch.Tensor) -> dict[str, torch.Tensor]:
    r = og.residual_score(ideal, out)
    snr = msm.tiled_snr_db(ideal, out, periods=int(og.PROLONG))
    sdr = msm.tiled_sdr_db(ideal, out, periods=int(og.PROLONG))
    jump = msm.wrap_jump_abs(out)
    ermse = msm.edge_rmse(ideal, out, seam_w=og.SEAM_W)
    click = msm.click_energy(out, periods=4)
    return {
        "R": r,
        "snr_db": snr,
        "sdr_db": sdr,
        "wrap_jump": jump,
        "edge_rmse": ermse,
        "click_energy": click,
    }


def summarize(metrics: dict[str, torch.Tensor], mask: torch.Tensor) -> dict:
    n = int(mask.sum().item())
    out = {"n": n}
    for key, tens in metrics.items():
        sel = tens[mask]
        out[f"{key}_mean"] = float(sel.mean().item()) if n else float("nan")
        out[f"{key}_std"] = float(sel.std(unbiased=False).item()) if n else float("nan")
    return out


def method_fns():
    return {
        "no_bake": lambda x: x,
        "dual_cosine": og.dual_cosine_blend,
        "seam_fir3": cav.seam_fir3,
        "blit_blep": lambda x: blit_blep_seam(x, seam_w=og.SEAM_W),
        "polyblep": lambda x: polyblep_seam(x, seam_w=og.SEAM_W),
        "blamp": lambda x: blamp_seam(x, seam_w=og.SEAM_W),
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--batch", type=int, default=64)
    ap.add_argument("--n-tiles", type=int, default=4096)
    ap.add_argument("--skip-multifamily", action="store_true")
    args = ap.parse_args()
    device = torch.device(args.device)
    methods = method_fns()

    # --- canonical holdout ---
    set_seed(HOLDOUT_SEED, device)
    ideal, eng = og.make_batch(args.batch, og.N, device)
    canonical = {}
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
        canonical[name] = row
        print(
            f"canonical {name}: R={row['R_mean']:.4f} "
            f"jump={row['wrap_jump_mean']:.4f} edge={row['edge_rmse_mean']:.4f} ms={ms:.2f}"
        )

    # --- cliff strata ---
    set_seed(HOLDOUT_SEED, device)
    ideal_c, eng_c = og.make_batch(args.n_tiles, og.N, device)
    engine_jump = msm.wrap_jump_abs(eng_c)
    p75 = float(torch.quantile(engine_jump, 0.75).item())
    p90 = float(torch.quantile(engine_jump, 0.90).item())
    masks = {
        "all": torch.ones(args.n_tiles, dtype=torch.bool, device=device),
        "top25_wrap": engine_jump >= p75,
        "top10_wrap": engine_jump >= p90,
    }
    strata: dict = {}
    for name, fn in methods.items():
        out = fn(eng_c)
        metrics = per_tile_metrics(ideal_c, out)
        strata[name] = {k: summarize(metrics, m) for k, m in masks.items()}
        print(
            f"cliff {name}: all R={strata[name]['all']['R_mean']:.4f} "
            f"top10 R={strata[name]['top10_wrap']['R_mean']:.4f} "
            f"top10 edge={strata[name]['top10_wrap']['edge_rmse_mean']:.4f}"
        )

    # --- multifamily (20-wave) ---
    multifamily = {}
    if not args.skip_multifamily:
        wave_specs = [(fam, seed) for seed in bsm.WAVEFORM_SEEDS for fam in bsm.FAMILIES]
        per_method_r: dict[str, list[float]] = {k: [] for k in methods}
        per_method_snr: dict[str, list[float]] = {k: [] for k in methods}
        per_method_sdr: dict[str, list[float]] = {k: [] for k in methods}
        per_method_jump: dict[str, list[float]] = {k: [] for k in methods}
        for fam, seed in wave_specs:
            ideal_f, eng_f = bsm.make_family_batch(fam, args.batch, og.N, device, seed=seed)
            for name, fn in methods.items():
                out = fn(eng_f)
                row = score_pair(ideal_f, out)
                per_method_r[name].append(row["R_mean"])
                per_method_snr[name].append(row["snr_db_mean"])
                per_method_sdr[name].append(row["sdr_db_mean"])
                per_method_jump[name].append(row["wrap_jump_mean"])
        for name in methods:
            r = torch.tensor(per_method_r[name])
            snr = torch.tensor(per_method_snr[name])
            sdr = torch.tensor(per_method_sdr[name])
            jump = torch.tensor(per_method_jump[name])
            dc_r = torch.tensor(per_method_r["dual_cosine"])
            multifamily[name] = {
                "R_mean": float(r.mean().item()),
                "R_std": float(r.std(unbiased=False).item()),
                "snr_db_mean": float(snr.mean().item()),
                "snr_db_std": float(snr.std(unbiased=False).item()),
                "sdr_db_mean": float(sdr.mean().item()),
                "sdr_db_std": float(sdr.std(unbiased=False).item()),
                "wrap_jump_mean": float(jump.mean().item()),
                "wrap_jump_std": float(jump.std(unbiased=False).item()),
                "delta_R_vs_dual_cosine": float((r - dc_r).mean().item()),
                "n_waveforms": len(wave_specs),
            }
            print(
                f"multi {name}: R={multifamily[name]['R_mean']:.4f} "
                f"dR_dc={multifamily[name]['delta_R_vs_dual_cosine']:+.4f}"
            )

    blob = {
        "meta": {
            "holdout_seed": HOLDOUT_SEED,
            "batch": args.batch,
            "n_tiles_cliff": args.n_tiles,
            "L": int(og.N),
            "SEAM_W": int(og.SEAM_W),
            "PROLONG": int(og.PROLONG),
            "wrap_jump_p75": p75,
            "wrap_jump_p90": p90,
            "implementation": (
                "cycle-local seam residual bake; polyblep matches osc::va::poly_blep; "
                "blit_blep raised-cosine BLEP-family; blamp polyBLAMP on slope jump"
            ),
            "citations": ["stilson1996", "nam2009polyblep", "esqueda2016blamp"],
            "code": "reelsynth/scripts/baselines/va_seam_blep.py",
            "nomenclature": {"no_bake": "passthrough unrepaired engine; legacy key identity"},
        },
        "canonical_holdout": canonical,
        "cliff_strata": strata,
        "multifamily": multifamily,
    }
    if "no_bake" in canonical:
        canonical["identity"] = canonical["no_bake"]
    if "no_bake" in strata:
        strata["identity"] = strata["no_bake"]
    if multifamily and "no_bake" in multifamily:
        multifamily["identity"] = multifamily["no_bake"]

    V7.mkdir(parents=True, exist_ok=True)
    out_v7 = V7 / "va_seam_blep.json"
    out_v7.write_text(json.dumps(blob, indent=2), encoding="utf-8")
    print(f"wrote {out_v7}")
    LOCAL.parent.mkdir(parents=True, exist_ok=True)
    LOCAL.write_text(json.dumps(blob, indent=2), encoding="utf-8")
    print(f"wrote {LOCAL}")


if __name__ == "__main__":
    main()
