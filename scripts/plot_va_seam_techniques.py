#!/usr/bin/env python3
"""Multi-panel figure: classical VA seam techniques on one cracked sine+cliff tile.

Same holdout seed / tile as fig_intro_sine_problem (seed 20260719, tile 46).
Writes paper/v7/figures/fig_va_seam_techniques.{png,json}.
"""
from __future__ import annotations

import json
import sys
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np
import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
from baselines.va_seam_blep import blit_blep_seam, blamp_seam, polyblep_seam  # noqa: E402

EVAL_SEED = 20260719
TILE_FALLBACK = 46  # matches fig_intro_sine_problem.json
PERIODS = 3
V11_FIG = (
    ROOT.parent
    / "denoise-opt-meta"
    / "paper"
    / "Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v11"
    / "figures"
)
OUT_PNG = V11_FIG / "fig_va_seam_techniques.png"
OUT_JSON = V11_FIG / "fig_va_seam_techniques.json"

# Wong colorblind-safe palette + B&W linestyles
C_IDEAL = "#000000"
C_ENGINE = "#D55E00"
C_BLIT = "#0072B2"
C_POLY = "#009E73"
C_BLAMP = "#CC79A7"
C_DUAL = "#56B4E9"


def prolong(cycle: torch.Tensor, periods: int = PERIODS) -> np.ndarray:
    return og.prolong_tile(cycle.unsqueeze(0), periods=periods)[0].cpu().numpy()


def pick_tile(eng: torch.Tensor) -> int:
    """Prefer max |wrap| tile (same rule as intro figure)."""
    wrap = (eng[:, 0] - eng[:, -1]).abs()
    idx = int(wrap.argmax().item())
    return idx


@torch.no_grad()
def main() -> None:
    device = torch.device("cpu")
    holdout = ROOT / "brand" / "artifacts" / "canonical_eval_dataset" / "holdout_batch.pt"
    if holdout.is_file():
        blob = torch.load(holdout, map_location="cpu", weights_only=False)
        ideal_b = blob["ideal"]
        eng_b = blob["engine"]
        pick_note = "holdout_batch.pt_max_wrap"
    else:
        torch.manual_seed(EVAL_SEED)
        ideal_b, eng_b = og.make_batch(64, og.N, device)
        pick_note = "make_batch_max_wrap"

    idx = pick_tile(eng_b)
    # Keep intro tile if still high-wrap (documented continuity).
    wrap_all = (eng_b[:, 0] - eng_b[:, -1]).abs()
    if TILE_FALLBACK < eng_b.shape[0] and float(wrap_all[TILE_FALLBACK]) >= 0.9 * float(wrap_all[idx]):
        idx = TILE_FALLBACK
        pick_note = pick_note + "_prefer_intro_tile46"

    ideal = ideal_b[idx].float()
    eng = eng_b[idx].float()
    wrap_abs = float((eng[0] - eng[-1]).abs().item())

    blit = blit_blep_seam(eng.unsqueeze(0), seam_w=og.SEAM_W)[0]
    poly = polyblep_seam(eng.unsqueeze(0), seam_w=og.SEAM_W)[0]
    blamp = blamp_seam(eng.unsqueeze(0), seam_w=og.SEAM_W)[0]
    dual = og.dual_cosine_blend(eng.unsqueeze(0))[0]

    scores = {
        "engine": float(
            og.residual_score_blend(ideal.unsqueeze(0), eng.unsqueeze(0), eng.unsqueeze(0)).item()
        ),
        "blit_blep": float(
            og.residual_score_blend(ideal.unsqueeze(0), eng.unsqueeze(0), blit.unsqueeze(0)).item()
        ),
        "polyblep": float(
            og.residual_score_blend(ideal.unsqueeze(0), eng.unsqueeze(0), poly.unsqueeze(0)).item()
        ),
        "blamp": float(
            og.residual_score_blend(ideal.unsqueeze(0), eng.unsqueeze(0), blamp.unsqueeze(0)).item()
        ),
        "dual_cosine": float(
            og.residual_score_blend(ideal.unsqueeze(0), eng.unsqueeze(0), dual.unsqueeze(0)).item()
        ),
    }
    primary_metric = "r_blend"

    L = int(ideal.numel())
    x = np.arange(L * PERIODS)
    y_ideal = prolong(ideal)
    y_eng = prolong(eng)
    y_blit = prolong(blit)
    y_poly = prolong(poly)
    y_blamp = prolong(blamp)
    y_dual = prolong(dual)

    # Seam neighborhood for annotation (first wrap)
    seam_lo = L - 24
    seam_hi = L + 24

    fig, axes = plt.subplots(2, 3, figsize=(11.2, 5.8), sharey=True)
    fig.suptitle(
        "Classical seam-repair methods on a high-wrap wavetable example",
        fontsize=11,
    )

    panels = [
        (axes[0, 0], "(a) Ideal reference", y_ideal, C_IDEAL, "-", None, None),
        (axes[0, 1], f"(b) Unrepaired signal  R={scores['engine']:.4f}", y_eng, C_ENGINE, "-", y_ideal, "value discontinuity"),
        (axes[0, 2], f"(c) BLIT/BLEP repair  R={scores['blit_blep']:.4f}", y_blit, C_BLIT, "--", y_ideal, "step correction"),
        (axes[1, 0], f"(d) PolyBLEP repair  R={scores['polyblep']:.4f}", y_poly, C_POLY, "-.", y_ideal, "polynomial correction"),
        (axes[1, 1], f"(e) BLAMP repair  R={scores['blamp']:.4f}", y_blamp, C_BLAMP, ":", y_ideal, "slope correction"),
        (axes[1, 2], f"(f) DualCosine repair  R={scores['dual_cosine']:.4f}", y_dual, C_DUAL, (0, (3, 1, 1, 1)), y_ideal, "end-window fade"),
    ]

    for ax, title, y, color, ls, overlay, tag in panels:
        if overlay is not None:
            ax.plot(x, overlay, color=C_IDEAL, lw=0.9, alpha=0.45, label="ideal")
        ax.plot(x, y, color=color, lw=1.35, ls=ls, label="signal")
        # Mark wrap seams
        for k in range(1, PERIODS):
            ax.axvline(k * L - 0.5, color="#888888", lw=0.7, ls=":", alpha=0.7)
        # Shade first seam region
        ax.axvspan(seam_lo, seam_hi, color="#F0E442", alpha=0.18, zorder=0)
        if tag:
            ax.text(
                0.02,
                0.04,
                tag,
                transform=ax.transAxes,
                fontsize=8,
                va="bottom",
                ha="left",
                bbox=dict(boxstyle="round,pad=0.2", fc="white", ec="#666666", alpha=0.9),
            )
        ax.set_title(title, fontsize=9.5)
        ax.set_xlim(0, L * PERIODS - 1)
        ax.grid(True, alpha=0.25)
        ax.set_xlabel("sample (tiled)")
        if ax in (axes[0, 0], axes[1, 0]):
            ax.set_ylabel("amplitude")

    # Legend strip
    from matplotlib.lines import Line2D

    handles = [
        Line2D([0], [0], color=C_IDEAL, lw=1.4, label="ideal"),
        Line2D([0], [0], color=C_ENGINE, lw=1.4, label="engine cliff"),
        Line2D([0], [0], color=C_BLIT, lw=1.4, ls="--", label="BLIT/BLEP (step)"),
        Line2D([0], [0], color=C_POLY, lw=1.4, ls="-.", label="PolyBLEP (poly step)"),
        Line2D([0], [0], color=C_BLAMP, lw=1.4, ls=":", label="BLAMP (slope)"),
        Line2D([0], [0], color=C_DUAL, lw=1.4, ls=(0, (3, 1, 1, 1)), label="DualCosine (fade)"),
    ]
    fig.legend(handles=handles, loc="lower center", ncol=6, fontsize=8, frameon=False)
    fig.tight_layout(rect=[0, 0.06, 1, 0.94])
    V11_FIG.mkdir(parents=True, exist_ok=True)
    fig.savefig(OUT_PNG, dpi=180, bbox_inches="tight")
    plt.close(fig)

    meta = {
        "eval_seed": EVAL_SEED,
        "tile_index": idx,
        "wrap_abs": wrap_abs,
        "pick_note": pick_note,
        "cycle_length": L,
        "periods_shown": PERIODS,
        "seam_w": int(og.SEAM_W),
        "primary_metric": "r_blend",
        "blend_alpha": og.BLEND_ALPHA,
        "R_tile": scores,
        "canonical_batch_means_from_va_seam_blep_json": {
            "polyblep": 0.9288,
            "blit_blep": 0.8680,
            "blamp": 0.9718,
            "dual_cosine": 0.8249,
        },
        "operator_notes": {
            "blit_blep": "raised-cosine BLEP-family step residual on wrap value jump (Stilson/Smith lineage)",
            "polyblep": "quadratic polyBLEP residual (Nam et al. / osc::va::poly_blep)",
            "blamp": "polyBLAMP residual on wrap slope jump (Esqueda et al.)",
            "dual_cosine": "raised-cosine end fades (baseline bake)",
        },
        "png": str(OUT_PNG),
    }
    OUT_JSON.write_text(json.dumps(meta, indent=2), encoding="utf-8")
    print(json.dumps(meta, indent=2))
    print(f"wrote {OUT_PNG}")


if __name__ == "__main__":
    main()
