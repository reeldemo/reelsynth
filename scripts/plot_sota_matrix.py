#!/usr/bin/env python3
"""Colorblind-safe SOTA heatmap (methods x families for mean R)."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np

# IBM / ColorBrewer-inspired sequential (blue→yellow, colorblind-safe)
CMAP = "cividis"

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--json",
        type=Path,
        default=ROOT / "brand/artifacts/sota_matrix.json",
    )
    ap.add_argument(
        "--out",
        type=Path,
        default=ROOT / "brand/artifacts/figures/fig_sota_heatmap.png",
    )
    args = ap.parse_args()
    payload = json.loads(args.json.read_text(encoding="utf-8"))
    families = payload["families"]
    # Keep a readable method subset for the heatmap
    keep = [
        "neural_favorite",
        "mlp_on_R",
        "cnn_unet_on_R",
        "seam_fir3",
        "dual_cosine",
        "classic_quadratic",
        "hann_blend",
        "identity",
        "no_bake",
    ]
    name_map = {
        "neural_favorite": "favorite",
        "mlp_on_R": "MLP-on-R",
        "cnn_unet_on_R": "CNN/UNet-on-R",
        "seam_fir3": "seam FIR3",
        "dual_cosine": "DualCosine",
        "classic_quadratic": "quadratic",
        "hann_blend": "hann",
        "identity": "no-bake",
        "no_bake": "no-bake",
    }
    # Aggregate R per (method, family) across seeds
    mat = np.zeros((len(keep), len(families)))
    by_name = {r["name"]: r for r in payload["results_multifamily"]}
    for i, m in enumerate(keep):
        rows = by_name[m]["per_waveform"]
        for j, fam in enumerate(families):
            vals = [r["residual_R"] for r in rows if r["family"] == fam]
            mat[i, j] = float(np.mean(vals)) if vals else np.nan

    fig, ax = plt.subplots(figsize=(9.5, 4.2))
    im = ax.imshow(mat, aspect="auto", cmap=CMAP, vmin=0.35, vmax=1.0)
    ax.set_xticks(range(len(families)))
    ax.set_xticklabels(families, rotation=35, ha="right", fontsize=8)
    ax.set_yticks(range(len(keep)))
    ax.set_yticklabels([name_map[m] for m in keep], fontsize=9)
    for i in range(mat.shape[0]):
        for j in range(mat.shape[1]):
            v = mat[i, j]
            if np.isnan(v):
                continue
            ax.text(
                j,
                i,
                f"{v:.2f}",
                ha="center",
                va="center",
                fontsize=7,
                color="white" if v < 0.7 else "black",
            )
    cbar = fig.colorbar(im, ax=ax, fraction=0.046, pad=0.04)
    cbar.set_label("prolonged residual $R$ (1=best)", fontsize=9)
    ax.set_title(
        "SOTA matrix: mean $R$ over 2 seeds × 10 generative families (batch 64)",
        fontsize=10,
    )
    fig.tight_layout()
    args.out.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(args.out, dpi=200)
    # also copy-friendly name
    print("wrote", args.out)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
