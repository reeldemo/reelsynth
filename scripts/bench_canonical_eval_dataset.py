#!/usr/bin/env python3
"""Freeze the canonical DenoiseOpt eval corpus and score classical + AI methods on it.

Corpus generator matches overnight_gpu_rl_arch.make_batch (sine+cliff wrap cycles).
This is the paper-facing holdout: fixed seeds, persisted metrics + method scores.
"""
from __future__ import annotations

import argparse
import json
import math
import sys
import time
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
import bench_classical_vs_ai as cav  # noqa: E402
import bench_inference_same_score as bib  # noqa: E402

# Paper-facing holdout seed (distinct from overnight search DEFAULT_SEED).
CANONICAL_EVAL_SEED = 20_260_719
# Larger draw for distribution metrics (same generative process).
METRICS_N_SAMPLES = 4096
# Matched-bench batch size (same as classical_vs_ai / inference benches).
SCORE_BATCH = 64
# Extra seeds for mean ± std on the score batch.
MULTI_SEEDS = 5


def set_seed(seed: int, device: torch.device) -> None:
    torch.manual_seed(seed)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(seed)


def make_frozen_batch(
    n_samples: int, seed: int, device: torch.device
) -> tuple[torch.Tensor, torch.Tensor]:
    set_seed(seed, device)
    return og.make_batch(n_samples, og.N, device)


@torch.no_grad()
def dataset_metrics(ideal: torch.Tensor, eng: torch.Tensor) -> dict:
    """Summarize frozen corpus geometry (CPU-friendly stats)."""
    resid = eng - ideal
    residual_rms = resid.pow(2).mean(dim=1).sqrt()
    ideal_rms = ideal.pow(2).mean(dim=1).sqrt()
    wrap_jump = (eng[:, 0] - eng[:, -1]).abs()
    # Prolonged engine residual R (identity bake) for hardness baseline.
    r_identity = og.residual_score(ideal, eng)
    return {
        "n_samples": int(ideal.shape[0]),
        "cycle_length_N": int(ideal.shape[1]),
        "prolong_tiles": int(og.PROLONG),
        "seam_width": int(og.SEAM_W),
        "ideal_rms": {
            "mean": float(ideal_rms.mean().item()),
            "std": float(ideal_rms.std(unbiased=False).item()),
            "min": float(ideal_rms.min().item()),
            "max": float(ideal_rms.max().item()),
            "p50": float(ideal_rms.median().item()),
        },
        "engine_residual_rms": {
            "mean": float(residual_rms.mean().item()),
            "std": float(residual_rms.std(unbiased=False).item()),
            "min": float(residual_rms.min().item()),
            "max": float(residual_rms.max().item()),
            "p50": float(residual_rms.median().item()),
        },
        "wrap_discontinuity_abs": {
            "mean": float(wrap_jump.mean().item()),
            "std": float(wrap_jump.std(unbiased=False).item()),
            "min": float(wrap_jump.min().item()),
            "max": float(wrap_jump.max().item()),
            "p50": float(wrap_jump.median().item()),
        },
        "identity_residual_R": {
            "mean": float(r_identity.mean().item()),
            "std": float(r_identity.std(unbiased=False).item()),
            "min": float(r_identity.min().item()),
            "max": float(r_identity.max().item()),
        },
        "family_labels": None,
        "note": (
            "Single synthetic family: make_batch sine+cliff (not Rust sound_bench families). "
            "No train/eval split of labeled clean audio: overnight search draws i.i.d. "
            "batches from the same generator; this freeze is the paper holdout for method tables."
        ),
    }


@torch.no_grad()
def score_on_batch(
    fn, ideal: torch.Tensor, eng: torch.Tensor, *, warmup: int = 5, repeats: int = 50
) -> dict:
    device = eng.device
    for _ in range(warmup):
        out = fn(eng)
        _ = og.residual_score(ideal, out).mean()
    if device.type == "cuda":
        torch.cuda.synchronize()
    out = fn(eng)
    r = float(og.residual_score(ideal, out).mean().item())
    if device.type == "cuda":
        torch.cuda.synchronize()
    t0 = time.perf_counter()
    for _ in range(repeats):
        out = fn(eng)
        _ = og.residual_score(ideal, out).mean()
    if device.type == "cuda":
        torch.cuda.synchronize()
    ms = 1000.0 * (time.perf_counter() - t0) / repeats
    batch = eng.shape[0]
    return {"residual": r, "ms_per_batch": ms, "ms_per_sample": ms / batch}


def load_neural_favorite(device: torch.device):
    fav_meta = json.loads(
        (ROOT / "brand/artifacts/inference_bench/inference_bench.json").read_text(encoding="utf-8")
    )
    fav = fav_meta["favorite"]
    cfg, cell, residual_saved, _ = bib.load_fitted(Path(fav["path"]), device)

    def neural_fn(eng):
        return og.apply_ops(eng, cell, cfg.ops)

    return neural_fn, {
        "tag": fav.get("tag"),
        "path": fav["path"],
        "residual_saved": residual_saved,
        "n_params": sum(p.numel() for p in cell.parameters()),
    }


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--eval-seed", type=int, default=CANONICAL_EVAL_SEED)
    ap.add_argument("--score-batch", type=int, default=SCORE_BATCH)
    ap.add_argument("--metrics-n", type=int, default=METRICS_N_SAMPLES)
    ap.add_argument("--multi-seeds", type=int, default=MULTI_SEEDS)
    ap.add_argument("--repeats", type=int, default=50)
    args = ap.parse_args()

    device = torch.device(args.device)
    out_dir = ROOT / "brand/artifacts/canonical_eval_dataset"
    out_dir.mkdir(parents=True, exist_ok=True)

    # --- Freeze metrics corpus + score holdout batch ---
    ideal_m, eng_m = make_frozen_batch(args.metrics_n, args.eval_seed, device)
    metrics = dataset_metrics(ideal_m.cpu(), eng_m.cpu())
    metrics["generator"] = {
        "fn": "overnight_gpu_rl_arch.make_batch",
        "procedure": (
            "Per sample: two-harmonic sine (freq~U(1,4), phase~U(0,2pi)), "
            "seam cliff amplitude ~±U(0.08,0.43) applied over SEAM_W samples at both ends, "
            "plus seam-boosted Gaussian noise (0.02 at ends, 0.15× mid)."
        ),
        "eval_seed": args.eval_seed,
        "overnight_search_seed": og.DEFAULT_SEED,
        "cycle_length": og.N,
        "prolong_tiles": og.PROLONG,
        "seam_width": og.SEAM_W,
        "score_batch": args.score_batch,
        "metrics_n_samples": args.metrics_n,
    }

    # Persist score-batch tensors for exact replay (canonical seed only).
    ideal_s, eng_s = make_frozen_batch(args.score_batch, args.eval_seed, device)
    torch.save(
        {
            "ideal": ideal_s.cpu(),
            "engine": eng_s.cpu(),
            "seed": args.eval_seed,
            "N": og.N,
            "SEAM_W": og.SEAM_W,
            "PROLONG": og.PROLONG,
        },
        out_dir / "holdout_batch.pt",
    )

    neural_fn, neural_meta = load_neural_favorite(device)
    methods = list(cav.CLASSICAL) + [("neural_favorite_meta", neural_fn, "ai")]

    # Primary scores on frozen holdout.
    primary_rows = []
    for name, fn, kind in methods:
        m = score_on_batch(fn, ideal_s, eng_s, repeats=args.repeats)
        row = {"name": name, "kind": kind, **m, "n_params": 0, "eval_seed": args.eval_seed}
        if kind == "ai":
            row.update(neural_meta)
        primary_rows.append(row)
        print(f"primary {name:28} R={m['residual']:.6f}  {m['ms_per_batch']:.3f} ms/batch")

    # Multi-seed mean ± std (fresh make_batch per seed, same size).
    multi = {name: [] for name, _, _ in methods}
    for k in range(args.multi_seeds):
        seed_k = args.eval_seed + k
        ideal_k, eng_k = make_frozen_batch(args.score_batch, seed_k, device)
        for name, fn, _kind in methods:
            m = score_on_batch(fn, ideal_k, eng_k, warmup=2, repeats=max(10, args.repeats // 5))
            multi[name].append(m["residual"])
            print(f"seed={seed_k} {name:28} R={m['residual']:.6f}")

    multi_summary = {}
    for name, vals in multi.items():
        t = torch.tensor(vals, dtype=torch.float64)
        multi_summary[name] = {
            "seeds": [args.eval_seed + k for k in range(args.multi_seeds)],
            "R_mean": float(t.mean().item()),
            "R_std": float(t.std(unbiased=False).item()) if len(vals) > 1 else 0.0,
            "R_values": vals,
        }
        for row in primary_rows:
            if row["name"] == name:
                row["R_mean_multiseed"] = multi_summary[name]["R_mean"]
                row["R_std_multiseed"] = multi_summary[name]["R_std"]

    classical = [r for r in primary_rows if r["kind"] == "non_ai"]
    best_classical = max(
        (r for r in classical if r["name"] != "identity"),
        key=lambda x: x["residual"],
    )
    dual = next(r for r in classical if r["name"] == "dual_cosine")
    ai = next(r for r in primary_rows if r["kind"] == "ai")

    payload = {
        "device": str(device),
        "canonical_eval_seed": args.eval_seed,
        "score_batch": args.score_batch,
        "metrics": metrics,
        "results": primary_rows,
        "multiseed": multi_summary,
        "summary": {
            "best_classical": best_classical["name"],
            "best_classical_R": best_classical["residual"],
            "dual_cosine_R": dual["residual"],
            "ai_favorite_R": ai["residual"],
            "delta_R_ai_minus_best_classical": ai["residual"] - best_classical["residual"],
            "delta_R_ai_minus_dual_cosine": ai["residual"] - dual["residual"],
            "ai_ms_per_batch": ai["ms_per_batch"],
            "best_classical_ms_per_batch": best_classical["ms_per_batch"],
            "dual_cosine_ms_per_batch": dual["ms_per_batch"],
        },
        "creation": {
            "script": "scripts/bench_canonical_eval_dataset.py",
            "make_batch": "scripts/overnight_gpu_rl_arch.py::make_batch",
            "residual": "R=clamp(1-residual_rms/max(ideal_rms,eps),0,1) after prolong_tile N=PROLONG",
        },
    }

    (out_dir / "dataset_metrics.json").write_text(
        json.dumps({"metrics": metrics, "generator": metrics["generator"]}, indent=2),
        encoding="utf-8",
    )
    (out_dir / "method_scores.json").write_text(json.dumps(payload, indent=2), encoding="utf-8")

    # Markdown table for paper drafting
    lines = [
        "| Method | kind | R (seed) | R mean±std | ms/batch |",
        "|---|---|---:|---:|---:|",
    ]
    for r in sorted(primary_rows, key=lambda x: -x["residual"]):
        lines.append(
            f"| {r['name']} | {r['kind']} | {r['residual']:.4f} | "
            f"{r['R_mean_multiseed']:.4f}±{r['R_std_multiseed']:.4f} | "
            f"{r['ms_per_batch']:.2f} |"
        )
    (out_dir / "method_scores_table.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    print("SUMMARY", json.dumps(payload["summary"], indent=2))
    print("wrote", out_dir)


if __name__ == "__main__":
    main()
