#!/usr/bin/env python3
"""Professional figures for the DenoiseOpt arXiv-style paper."""

from __future__ import annotations

import json
from pathlib import Path

import matplotlib.pyplot as plt
import matplotlib.patheffects as pe
import numpy as np

ART = Path(__file__).resolve().parent
BG = "#0b0f14"
PANEL = "#121820"
TEXT = "#e8eef4"
MUTED = "#8b9aab"
ACCENT = "#3d9ecb"
ACCENT2 = "#5ec8a0"
WARN = "#e0a15c"
BAD = "#d46a6a"
GRID = "#1e2a36"


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
            "axes.titlesize": 14,
            "axes.labelsize": 12,
        }
    )


def fig_meta_pareto():
    meta = json.loads((ART / "denoise_opt_meta_search.json").read_text())
    trials = meta["trials"]
    d = np.array([t["val"]["denoise"] for t in trials])
    s = np.array([t["val"]["shape"] for t in trials])
    q = np.array([t["val"]["quality"] for t in trials])
    names = [t["name"] for t in trials]
    champ = meta["champion"]["name"]

    style()
    fig, ax = plt.subplots(figsize=(10, 7), dpi=200)
    sc = ax.scatter(d, s, c=q, cmap="viridis", s=70, zorder=3, edgecolors=BG, linewidths=0.6)
    for i, n in enumerate(names):
        if n != "grid" or q[i] >= np.percentile(q, 85):
            ax.annotate(
                n if n != "grid" else "",
                (d[i], s[i]),
                textcoords="offset points",
                xytext=(6, 4),
                fontsize=8,
                color=MUTED,
            )
    # champion
    ci = next(i for i, t in enumerate(trials) if t.get("meta_score") == meta["champion"].get("meta_score"))
    ax.scatter([d[ci]], [s[ci]], s=220, facecolors=ACCENT2, edgecolors=TEXT, linewidths=1.5, zorder=5)
    ax.annotate(
        f"champion · {champ}",
        (d[ci], s[ci]),
        xytext=(-40, -28),
        textcoords="offset points",
        color=ACCENT2,
        fontsize=11,
        fontweight="bold",
        arrowprops=dict(arrowstyle="-|>", color=ACCENT2, lw=1.3),
        path_effects=[pe.withStroke(linewidth=3, foreground=BG)],
    )

    # baselines
    for b in meta["baselines"]:
        if b["algo"] in {"classic", "dual_cosine", "denoise_opt"}:
            ax.scatter([b["denoise"]], [b["shape"]], marker="D", s=90, color=WARN, zorder=4)
            ax.annotate(
                b["algo"].replace("_", " "),
                (b["denoise"], b["shape"]),
                xytext=(8, -10),
                textcoords="offset points",
                fontsize=9,
                color=WARN,
            )

    ax.set_xlabel(r"Denoise gain  $\mathcal{D} = \mathrm{clamp}\!\left(\frac{C(r)-C(y)}{C(r)}\right)$")
    ax.set_ylabel(r"Shape retention  $\mathcal{S} = 1 - \mathrm{MAE}_{\mathrm{mid}}/\mathrm{RMS}$")
    ax.set_title("Meta-learning Pareto: denoise vs shape (validation)")
    ax.grid(True, alpha=0.85)
    cb = fig.colorbar(sc, ax=ax, fraction=0.046, pad=0.03)
    cb.set_label(r"quality  $Q=\frac{1}{2}(\mathcal{D}+\mathcal{S})$")
    fig.subplots_adjust(left=0.12, right=0.88, top=0.9, bottom=0.14)
    out = ART / "fig_meta_pareto.png"
    fig.savefig(out, dpi=200, facecolor=BG)
    plt.close(fig)
    print("wrote", out)


def fig_family_radar():
    bench = json.loads((ART / "denoise_opt_bench_100k_fit.json").read_text())
    fams = bench["per_family"]
    labels = [f["family"].replace("_", "\n") for f in fams]
    dens = [f["denoise"] for f in fams]
    shps = [f["shape"] for f in fams]
    qs = [f["quality"] for f in fams]

    style()
    fig, axes = plt.subplots(1, 2, figsize=(14, 5.5), dpi=200)

    ax = axes[0]
    x = np.arange(len(labels))
    w = 0.38
    ax.bar(x - w / 2, dens, w, label=r"$\mathcal{D}$", color=ACCENT)
    ax.bar(x + w / 2, shps, w, label=r"$\mathcal{S}$", color=ACCENT2)
    ax.set_xticks(x)
    ax.set_xticklabels(labels, fontsize=8)
    ax.set_ylim(0, 1.05)
    ax.set_ylabel("Score")
    ax.set_title("Per-family denoise / shape on 100k bench")
    ax.legend(facecolor=PANEL, edgecolor=GRID, labelcolor=TEXT)
    ax.grid(True, axis="y", alpha=0.8)

    ax = axes[1]
    colors = [ACCENT2 if q > 0.8 else (ACCENT if q > 0.74 else WARN) for q in qs]
    ax.barh(range(len(labels)), qs, color=colors)
    ax.set_yticks(range(len(labels)))
    ax.set_yticklabels([f["family"].replace("_", " ") for f in fams], fontsize=9)
    ax.set_xlim(0.65, 0.95)
    ax.set_xlabel(r"Quality $Q$")
    ax.set_title("Family ranking (higher is better)")
    ax.grid(True, axis="x", alpha=0.8)
    ax.invert_yaxis()

    fig.tight_layout()
    out = ART / "fig_family_breakdown.png"
    fig.savefig(out, dpi=200, facecolor=BG)
    plt.close(fig)
    print("wrote", out)


def fig_algo_baselines():
    meta = json.loads((ART / "denoise_opt_meta_search.json").read_text())
    base = [b for b in meta["baselines"] if b["algo"] != "ensemble_v3"]  # hide shape-broken for clarity
    # include champion as bar
    champ = meta["champion"]
    rows = base + [
        {
            "algo": "meta champion",
            "denoise": champ["val"]["denoise"],
            "shape": champ["val"]["shape"],
            "quality": champ["val"]["quality"],
        }
    ]
    labels = [r["algo"].replace("_", " ") for r in rows]
    d = [r["denoise"] for r in rows]
    s = [r["shape"] for r in rows]
    q = [r["quality"] for r in rows]

    style()
    fig, ax = plt.subplots(figsize=(11, 6), dpi=200)
    x = np.arange(len(labels))
    w = 0.25
    ax.bar(x - w, d, w, label=r"$\mathcal{D}$", color=ACCENT)
    ax.bar(x, s, w, label=r"$\mathcal{S}$", color=ACCENT2)
    ax.bar(x + w, q, w, label=r"$Q$", color=WARN)
    ax.set_xticks(x)
    ax.set_xticklabels(labels, rotation=15, ha="right")
    ax.set_ylim(0.5, 1.05)
    ax.set_ylabel("Score")
    ax.set_title("Algorithm comparison on held-out validation (N=1500)")
    ax.legend(facecolor=PANEL, edgecolor=GRID, labelcolor=TEXT, ncol=3)
    ax.grid(True, axis="y", alpha=0.85)
    # loss functional annotation
    ax.text(
        0.02,
        0.05,
        r"$L=(1-\mathcal{D})+\lambda(1-\mathcal{S})$   ·   seam-local inference $\mathcal{O}(N)$",
        transform=ax.transAxes,
        fontsize=10,
        color=MUTED,
    )
    fig.tight_layout()
    out = ART / "fig_algo_baselines.png"
    fig.savefig(out, dpi=200, facecolor=BG)
    plt.close(fig)
    print("wrote", out)


def fig_crackle_reduction_curve():
    # From 12-iter artifact reduction history if present, else synthesize from meta
    hist_path = ART / "artifact_reduction_history.json"
    style()
    fig, ax = plt.subplots(figsize=(11, 6), dpi=200)
    if hist_path.exists():
        hist = json.loads(hist_path.read_text())
        xs = [r["iteration"] for r in hist["iterations"]]
        ys = [r["mean_artifact"] for r in hist["iterations"]]
        run = [r["running_best_artifact"] for r in hist["iterations"]]
        ax.plot(xs, ys, "-o", color=ACCENT, label="candidate $C$")
        ax.plot(xs, run, "-", color=ACCENT2, lw=2.5, label="running best")
        ax.set_xticks(xs)
        ax.set_xticklabels([r["algo"].replace("_", "\n") for r in hist["iterations"]], fontsize=8)
        ax.set_title("Bake algorithm search: mean crackle functional $C(\\cdot)$")
    ax.set_xlabel("Algorithm iteration")
    ax.set_ylabel(r"$C(x)=2\,\mathrm{wrap}+\mathrm{max\_step}+0.35\,\mathrm{hf}$")
    ax.grid(True, alpha=0.85)
    ax.legend(facecolor=PANEL, edgecolor=GRID, labelcolor=TEXT)
    fig.tight_layout()
    out = ART / "fig_crackle_curve.png"
    fig.savefig(out, dpi=200, facecolor=BG)
    plt.close(fig)
    print("wrote", out)


def fig_pipeline():
    """Schematic of DenoiseOpt pipeline as a clean block diagram."""
    style()
    fig, ax = plt.subplots(figsize=(12, 4.2), dpi=200)
    ax.set_xlim(0, 12)
    ax.set_ylim(0, 4)
    ax.axis("off")
    ax.set_facecolor(BG)
    fig.patch.set_facecolor(BG)

    boxes = [
        (0.3, 1.4, "Input\ncycle $r$"),
        (2.3, 1.4, "Seam\nlocalize"),
        (4.3, 1.4, "Dual-end\nfade $\\theta$"),
        (6.3, 1.4, "Polish\n+ pin"),
        (8.3, 1.4, "Mid-cycle\ncopy"),
        (10.2, 1.4, "Output\n$y$"),
    ]
    for x, y, t in boxes:
        ax.add_patch(
            plt.Rectangle((x, y), 1.6, 1.4, facecolor=PANEL, edgecolor=ACCENT, linewidth=1.5, zorder=2)
        )
        ax.text(x + 0.8, y + 0.7, t, ha="center", va="center", fontsize=10, color=TEXT, zorder=3)
    for i in range(len(boxes) - 1):
        x0 = boxes[i][0] + 1.6
        x1 = boxes[i + 1][0]
        ax.annotate(
            "",
            xy=(x1, 2.1),
            xytext=(x0, 2.1),
            arrowprops=dict(arrowstyle="-|>", color=ACCENT2, lw=1.6),
        )
    ax.text(
        6,
        3.5,
        r"DenoiseOpt inference  ·  $\theta^\star$ frozen  ·  $L=(1-\mathcal{D})+\lambda(1-\mathcal{S})$",
        ha="center",
        fontsize=13,
        color=TEXT,
        fontweight="bold",
    )
    ax.text(
        6,
        0.55,
        "Shape invariant: samples outside fade zones equal $r$ (exact).",
        ha="center",
        fontsize=10,
        color=MUTED,
    )
    out = ART / "fig_pipeline.png"
    fig.savefig(out, dpi=200, facecolor=BG)
    plt.close(fig)
    print("wrote", out)


def main():
    fig_pipeline()
    fig_meta_pareto()
    fig_family_radar()
    fig_algo_baselines()
    fig_crackle_reduction_curve()


if __name__ == "__main__":
    main()
