#!/usr/bin/env python3
"""Re-score ablate-* final champions under R_blend (weights frozen; no re-search).

  .venv_gpu/Scripts/python.exe scripts/rescore_ablate_rblend.py --device cuda
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
import bench_inference_same_score as bib  # noqa: E402

ABLATE = {
    "Full hybrid": "ablate-full-150it-1902771841",
    "GA-only": "ablate-ga-only-150it-1902771841",
    "GA+PPO": "ablate-ga-ppo-150it-1902771841",
    "PPO-only": "ablate-ppo-only-150it-1902771841",
}


@torch.no_grad()
def score_cell(cell, ops, device, batch: int = 64) -> float:
    ideal, eng = og.make_batch(batch, og.N, device)
    out = og.apply_ops(eng, cell, ops)
    return float(og.residual_score_blend(ideal, eng, out).mean().item())


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument(
        "--out",
        type=Path,
        default=ROOT / "brand/artifacts/ablate_rblend_rescore.json",
    )
    args = ap.parse_args()
    device = torch.device(args.device if torch.cuda.is_available() or args.device == "cpu" else "cpu")
    baseline = float(og.dual_cosine_baseline(device, batch=128))

    results = {}
    for label, run_id in ABLATE.items():
        pt = ROOT / "brand/artifacts/models" / run_id / "fitted" / "final_champion_fitted.pt"
        if not pt.exists():
            # fall back to newest fitted
            cands = sorted((pt.parent).glob("*_fitted.pt"), key=lambda p: p.stat().st_mtime)
            if not cands:
                print(f"skip {label}: no fitted pt")
                continue
            pt = cands[-1]
        cfg, cell, saved, _ = bib.load_fitted(pt, device)
        live = score_cell(cell, cfg.ops, device)
        results[label] = {
            "path": str(pt),
            "residual_saved_hint": float(saved),
            "r_blend_live": live,
            "delta_vs_dual_cosine": live - baseline,
            "n_params": int(sum(p.numel() for p in cell.parameters())),
            "arch": cfg.to_dict(),
        }
        print(f"{label}: R_blend={live:.5f} (saved hint {saved:.5f}) path={pt.name}")

    payload = {
        "primary_metric": "r_blend",
        "blend_alpha": float(og.BLEND_ALPHA),
        "baseline_dual_cosine": baseline,
        "note": (
            "Isolated 150-it ablate-* final champions re-scored under R_blend. "
            "Search freezes maximized prolonged R; branch-best freezes from the 5k hybrid "
            "history remain prolonged-only (no separate fitted weights)."
        ),
        "isolated_150it": results,
    }
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(f"Wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
