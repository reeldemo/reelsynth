#!/usr/bin/env python3
"""Render fig_intro_sine_problem.png under locked R_blend labels.

  .venv_gpu/Scripts/python.exe scripts/plot_intro_sine_problem.py
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import matplotlib.pyplot as plt
import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
import bench_inference_same_score as bib  # noqa: E402

EVAL_SEED = 20_260_719
TILE = 46
PERIODS = 3


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--device", default="cpu")
    ap.add_argument(
        "--champ",
        type=Path,
        default=ROOT / "brand/artifacts/meta_approach_compare_v10/hybrid_lstm/champ_cell.pt",
    )
    ap.add_argument(
        "--holdout",
        type=Path,
        default=ROOT / "brand/artifacts/canonical_eval_dataset/holdout_batch.pt",
    )
    ap.add_argument(
        "--out-png",
        type=Path,
        default=ROOT.parent
        / "denoise-opt-meta/paper/Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v11/figures/fig_intro_sine_problem.png",
    )
    ap.add_argument(
        "--out-json",
        type=Path,
        default=ROOT.parent
        / "denoise-opt-meta/paper/Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v11/figures/fig_intro_sine_problem.json",
    )
    args = ap.parse_args()
    device = torch.device(args.device)

    blob = torch.load(args.holdout, map_location="cpu", weights_only=False)
    ideal_b, eng_b = blob["ideal"].float(), blob["engine"].float()
    ideal = ideal_b[TILE].unsqueeze(0).to(device)
    eng = eng_b[TILE].unsqueeze(0).to(device)
    wrap = float((eng[0, 0] - eng[0, -1]).abs().item())

    cfg, cell, r_saved, _ = bib.load_fitted(args.champ, device)
    with torch.no_grad():
        out_ours = og.apply_ops(eng, cell, cfg.ops)
        out_dc = og.dual_cosine_blend(eng)
        scores = {
            "R_engine": float(og.residual_score_blend(ideal, eng, eng).mean()),
            "R_dual_cosine": float(og.residual_score_blend(ideal, eng, out_dc).mean()),
            "R_neural_favorite_tile": float(
                og.residual_score_blend(ideal, eng, out_ours).mean()
            ),
        }
        torch.manual_seed(EVAL_SEED)
        ideal64, eng64 = og.make_batch(64, og.N, device)
        out_b = og.apply_ops(eng64, cell, cfg.ops)
        batch_mean = float(og.residual_score_blend(ideal64, eng64, out_b).mean())

    # Prolong for display
    def prolong(x: torch.Tensor) -> torch.Tensor:
        return og.prolong_tile(x, periods=PERIODS)[0].detach().cpu().numpy()

    ideal_p = prolong(ideal)
    eng_p = prolong(eng)
    dc_p = prolong(out_dc)
    ours_p = prolong(out_ours)
    n = ideal_p.shape[0]
    xs = list(range(n))
    seam0 = og.N - 1

    fig, axes = plt.subplots(2, 3, figsize=(12.5, 6.2), dpi=140)
    fig.suptitle(
        f"Sine-wrap modulation edge case | holdout seed={EVAL_SEED}, tile={TILE}, "
        f"|wrap|={wrap:.3f} | favorite=v10_hybrid_lstm_champ | "
        f"tile $R_{{\\mathrm{{blend}}}}$={scores['R_neural_favorite_tile']:.4f}, "
        f"batch mean={batch_mean:.4f}",
        fontsize=9,
    )

    axes[0, 0].plot(xs, ideal_p, color="black", lw=1.1)
    axes[0, 0].set_title("(a) Ideal (no cliff)")
    axes[0, 1].plot(xs, eng_p, color="crimson", lw=1.0)
    axes[0, 1].set_title(f"(b) No-bake $R_{{\\mathrm{{blend}}}}$={scores['R_engine']:.4f}")
    axes[0, 2].plot(xs, dc_p, color="royalblue", lw=1.0)
    axes[0, 2].set_title(
        f"(c) Dual Cosine $R_{{\\mathrm{{blend}}}}$={scores['R_dual_cosine']:.4f}"
    )
    axes[1, 0].plot(xs, ours_p, color="seagreen", lw=1.0)
    axes[1, 0].set_title(
        f"(d) Ours $R_{{\\mathrm{{blend}}}}$={scores['R_neural_favorite_tile']:.4f}"
    )
    axes[1, 1].plot(xs, eng_p - ideal_p, color="crimson", lw=0.9, label="engine − ideal")
    axes[1, 1].plot(xs, ours_p - ideal_p, color="seagreen", lw=0.9, label="ours − ideal")
    axes[1, 1].set_title("(e) Residuals")
    axes[1, 1].legend(fontsize=7, loc="upper right")

    lo, hi = seam0 - 25, seam0 + 25
    axes[1, 2].plot(range(lo, hi), ideal_p[lo:hi], color="black", lw=1.2, label="ideal")
    axes[1, 2].plot(range(lo, hi), eng_p[lo:hi], color="crimson", lw=1.0, label="engine")
    axes[1, 2].plot(range(lo, hi), dc_p[lo:hi], color="royalblue", lw=1.0, label="DualCosine")
    axes[1, 2].plot(range(lo, hi), ours_p[lo:hi], color="seagreen", lw=1.0, label="ours")
    axes[1, 2].set_title("(f) Seam zoom (first wrap)")
    axes[1, 2].legend(fontsize=6, loc="best")

    for ax in axes.ravel():
        ax.grid(True, alpha=0.25)
        ax.set_xlim(0, n - 1)
    fig.tight_layout(rect=(0, 0, 1, 0.93))
    args.out_png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(args.out_png)
    fig.savefig(args.out_png.with_suffix(".pdf"))
    plt.close(fig)

    meta = {
        "eval_seed": EVAL_SEED,
        "tile_index": TILE,
        "wrap_abs": wrap,
        "primary_metric": "r_blend",
        "blend_alpha": float(og.BLEND_ALPHA),
        **scores,
        "R_neural_favorite_batch_mean": batch_mean,
        "R_neural_favorite_saved": float(r_saved),
        "favorite_path": str(args.champ),
        "favorite_tag": "v10_hybrid_lstm_champ",
        "periods_shown": PERIODS,
        "cycle_length": int(og.N),
        "pick_note": "band_match_max_wrap",
        "note": "Printed scores are R_blend (alpha=0.7); same tile as prior intro figure.",
    }
    args.out_json.write_text(json.dumps(meta, indent=2), encoding="utf-8")
    canon = ROOT / "brand/artifacts/canonical_eval_dataset/fig_intro_sine_problem.json"
    canon.write_text(json.dumps(meta, indent=2), encoding="utf-8")
    print(f"Wrote {args.out_png}")
    print(f"Wrote {args.out_json}")
    print(json.dumps(meta, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
