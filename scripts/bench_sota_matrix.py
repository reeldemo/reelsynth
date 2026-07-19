#!/usr/bin/env python3
"""Build a SOTA matrix JSON: methods x (R, SNR, SDR, wrap-jump, latency, params).

Phase 3a starter: reuses frozen canonical holdout + classical scorers from
bench_classical_vs_ai.CLASSICAL. Optional neural favorite via --favorite-pt.
Does not invent PESQ/STOI. Multi-family expansion is a follow-up.
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
import bench_canonical_eval_dataset as bced  # noqa: E402
import metrics_snr_sdr as msm  # noqa: E402


@torch.no_grad()
def score_method(
    name: str,
    kind: str,
    fn,
    ideal: torch.Tensor,
    eng: torch.Tensor,
    *,
    n_params: int = 0,
    warmup: int = 3,
    repeats: int = 20,
) -> dict:
    device = eng.device
    for _ in range(warmup):
        _ = fn(eng)
    if device.type == "cuda":
        torch.cuda.synchronize()
    t0 = time.perf_counter()
    out = None
    for _ in range(repeats):
        out = fn(eng)
    if device.type == "cuda":
        torch.cuda.synchronize()
    dt_ms = (time.perf_counter() - t0) * 1000.0 / repeats
    assert out is not None
    r = og.residual_score(ideal, out)
    sec = msm.secondary_metrics(ideal, out, periods=int(og.PROLONG))
    dc_r = None
    return {
        "name": name,
        "kind": kind,
        "residual_R": float(r.mean().item()),
        "ms_per_batch": float(dt_ms),
        "n_params": int(n_params),
        **sec,
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--seed", type=int, default=bced.CANONICAL_EVAL_SEED)
    ap.add_argument("--batch", type=int, default=bced.SCORE_BATCH)
    ap.add_argument(
        "--out",
        type=Path,
        default=ROOT / "brand" / "artifacts" / "sota_matrix.json",
    )
    args = ap.parse_args()
    device = torch.device(args.device)
    ideal, eng = bced.make_frozen_batch(args.batch, args.seed, device)

    methods: list[dict] = []
    for name, fn, kind in cav.CLASSICAL:
        methods.append(score_method(name, kind, fn, ideal, eng, n_params=0))

    dual = next(m for m in methods if m["name"] == "dual_cosine")
    for m in methods:
        m["delta_R_vs_dual_cosine"] = float(m["residual_R"] - dual["residual_R"])

    payload = {
        "protocol": "EVAL_PROTOCOL v1 / Phase 3a starter",
        "device": str(device),
        "canonical_eval_seed": int(args.seed),
        "score_batch": int(args.batch),
        "cycle_length": int(og.N),
        "prolong_tiles": int(og.PROLONG),
        "note": (
            "SNR/SDR are tiled vs procedural ideal. No PESQ/STOI. "
            "Single sine+cliff family until multi-family export lands. "
            "Neural favorite row deferred to --favorite wiring / method_scores merge."
        ),
        "results": methods,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(f"Wrote {args.out} ({len(methods)} methods)")
    for m in methods:
        print(
            f"{m['name']:28} R={m['residual_R']:.4f}  "
            f"SNR={m['snr_db_mean']:.2f}dB  SDR={m['sdr_db_mean']:.2f}dB  "
            f"jump={m['wrap_jump_mean']:.3f}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
