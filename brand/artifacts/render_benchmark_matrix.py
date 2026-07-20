#!/usr/bin/env python3
"""Benchmark matrix + paper figures for residual-objective meta-learning."""

from __future__ import annotations

import json
from collections import Counter
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np

ART = Path(__file__).resolve().parent
BG, PANEL, TEXT, MUTED = "#0b0f14", "#121820", "#e8eef4", "#8b9aab"
ACCENT, ACCENT2, WARN, GRID = "#3d9ecb", "#5ec8a0", "#e0a15c", "#1e2a36"
RES = "#c77dff"


def style():
    plt.rcParams.update(
        {
            "font.family": "DejaVu Sans",
            "figure.facecolor": BG,
            "axes.facecolor": PANEL,
            "text.color": TEXT,
            "axes.labelcolor": TEXT,
            "axes.titlecolor": TEXT,
            "xtick.color": MUTED,
            "ytick.color": MUTED,
            "axes.edgecolor": GRID,
            "grid.color": GRID,
        }
    )


def fig_benchmark_matrix():
    meta = json.loads((ART / "denoise_opt_meta_1500.json").read_text())
    rows = meta["benchmark_matrix_5"]
    labels, R, D, S = [], [], [], []
    for r in rows:
        name = r["algo"].replace("naive_dual_cosine", "Naive\nDualCosine").replace(
            "meta_top", "Meta\nTop "
        )
        labels.append(name)
        R.append(r["residual"])
        D.append(r["denoise"])
        S.append(r["shape"])

    style()
    fig, axes = plt.subplots(1, 2, figsize=(13, 5.8), dpi=220)

    ax = axes[0]
    x = np.arange(len(labels))
    w = 0.25
    ax.bar(x - w, R, w, color=RES, label=r"residual (primary)")
    ax.bar(x, D, w, color=ACCENT, label=r"$\mathcal{D}$ denoise")
    ax.bar(x + w, S, w, color=ACCENT2, label=r"$\mathcal{S}$ shape")
    ax.set_xticks(x)
    ax.set_xticklabels(labels, fontsize=9)
    ax.set_ylim(0.55, 1.02)
    ax.set_ylabel("Score")
    ax.set_title("Five-algorithm matrix — residual primary (val $N{=}2000$)")
    ax.legend(facecolor=PANEL, edgecolor=GRID, labelcolor=TEXT, fontsize=9)
    ax.grid(True, axis="y", alpha=0.85)
    ax.text(
        0.02,
        0.04,
        r"outer: residual  ·  inner: $L=(1-\mathcal{D})+\lambda(1-\mathcal{S})$  ·  1500 trials",
        transform=ax.transAxes,
        fontsize=8,
        color=MUTED,
    )

    ax = axes[1]
    M = np.array([R, D, S])
    im = ax.imshow(M, aspect="auto", cmap="viridis", vmin=0.55, vmax=1.0)
    ax.set_yticks([0, 1, 2])
    ax.set_yticklabels([r"residual", r"$\mathcal{D}$", r"$\mathcal{S}$"])
    ax.set_xticks(range(len(labels)))
    ax.set_xticklabels(labels, fontsize=9)
    ax.set_title("Metric × algorithm heatmap")
    for i in range(3):
        for j in range(len(labels)):
            ax.text(
                j,
                i,
                f"{M[i, j]:.3f}",
                ha="center",
                va="center",
                color="white" if M[i, j] < 0.85 else "#0b0f14",
                fontsize=9,
                fontweight="bold",
            )
    cb = fig.colorbar(im, ax=ax, fraction=0.046, pad=0.04)
    cb.set_label("score")

    fig.tight_layout()
    out = ART / "fig_benchmark_matrix.png"
    fig.savefig(out, dpi=220, facecolor=BG)
    plt.close(fig)
    print("wrote", out)


def fig_prior_hist():
    meta = json.loads((ART / "denoise_opt_meta_1500.json").read_text())
    top = meta.get("pareto_top20_fast") or []
    c = Counter(t.get("prior", "?") for t in top)
    for t in meta["top4"]:
        c[t.get("prior", "?")] += 3

    style()
    fig, ax = plt.subplots(figsize=(9, 5), dpi=200)
    keys = list(c.keys())
    vals = [c[k] for k in keys]
    ax.barh(keys, vals, color=ACCENT)
    ax.set_xlabel("Weighted presence in top-20 / top-4")
    ax.set_title("Prior families among elite residual-ranked trials")
    ax.grid(True, axis="x", alpha=0.8)
    fig.tight_layout()
    out = ART / "fig_prior_families.png"
    fig.savefig(out, dpi=200, facecolor=BG)
    plt.close(fig)
    print("wrote", out)


def fig_pareto_1500():
    meta = json.loads((ART / "denoise_opt_meta_1500.json").read_text())
    pts = meta.get("pareto_top20_fast") or []
    res = [p["val_fast"]["residual"] for p in pts]
    loss = [p["val_fast"]["loss"] for p in pts]
    style()
    fig, ax = plt.subplots(figsize=(8.5, 6.5), dpi=200)
    ax.scatter(loss, res, c=range(len(res)), cmap="plasma", s=80, edgecolors=BG)
    for i, p in enumerate(pts[:5]):
        ax.annotate(
            p.get("prior", ""),
            (loss[i], res[i]),
            textcoords="offset points",
            xytext=(6, 4),
            fontsize=8,
            color=MUTED,
        )
    for r in meta["benchmark_matrix_5"]:
        if r["kind"] == "meta":
            L = (1.0 - r["denoise"]) + (1.0 - r["shape"])
            ax.scatter(
                [L],
                [r["residual"]],
                s=160,
                facecolors=ACCENT2,
                edgecolors=TEXT,
                zorder=5,
            )
    naive = meta["benchmark_matrix_5"][0]
    Ln = (1.0 - naive["denoise"]) + (1.0 - naive["shape"])
    ax.scatter(
        [Ln],
        [naive["residual"]],
        s=160,
        marker="D",
        facecolors=WARN,
        edgecolors=TEXT,
        zorder=5,
        label="naive DualCosine",
    )
    ax.set_xlabel(r"Unsupervised loss proxy $(1-\mathcal{D})+(1-\mathcal{S})$")
    ax.set_ylabel(r"Residual score (1 = best)")
    ax.set_title("Elite trials — residual vs loss (1500-run meta)")
    ax.grid(True, alpha=0.85)
    ax.legend(facecolor=PANEL, edgecolor=GRID, labelcolor=TEXT)
    fig.tight_layout()
    out = ART / "fig_pareto_1500.png"
    fig.savefig(out, dpi=200, facecolor=BG)
    plt.close(fig)
    print("wrote", out)


if __name__ == "__main__":
    fig_benchmark_matrix()
    fig_prior_hist()
    fig_pareto_1500()
