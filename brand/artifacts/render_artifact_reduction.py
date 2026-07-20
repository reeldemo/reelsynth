#!/usr/bin/env python3
"""Render high-quality 'Artifact Reduction' share graphic for X."""

from __future__ import annotations

import json
from pathlib import Path

import matplotlib.pyplot as plt
import matplotlib.patheffects as pe

DATA = Path(__file__).resolve().parent / "artifact_reduction_history.json"
OUT = Path(__file__).resolve().parent / "artifact_reduction.png"

BG = "#0a0a0a"
SURFACE = "#141416"
CARD = "#18181b"
TEXT = "#fafafa"
MUTED = "#a1a1aa"
ACCENT = "#2a6b8a"
ACCENT_SOFT = "#3d8fad"
GRID = "#27272a"
WIN = "#5ec8a0"
BAD = "#c45c5c"


def main() -> None:
    report = json.loads(DATA.read_text(encoding="utf-8"))
    rows = report["iterations"]
    xs = [r["iteration"] for r in rows]
    arts = [float(r["mean_artifact"]) for r in rows]
    running = [float(r["running_best_artifact"]) for r in rows]
    labels = [r["algo"].replace("_", " ") for r in rows]
    winner = report["winner"]
    pct = float(report["pct_reduced_vs_raw"])
    raw = float(report["raw_mean_artifact"])
    win_score = float(report["winner_artifact"])
    classic = float(next(r["mean_artifact"] for r in rows if r["algo"] == "classic"))

    plt.rcParams.update(
        {
            "font.family": "DejaVu Sans",
            "figure.facecolor": BG,
            "axes.facecolor": SURFACE,
            "text.color": TEXT,
            "axes.labelcolor": TEXT,
            "xtick.color": MUTED,
            "ytick.color": MUTED,
            "axes.edgecolor": GRID,
        }
    )

    fig = plt.figure(figsize=(16, 9), dpi=200)

    # Title block
    fig.text(
        0.055,
        0.94,
        "Artifact Reduction",
        fontsize=40,
        fontweight="bold",
        color=TEXT,
        va="top",
    )
    fig.text(
        0.055,
        0.875,
        "ReelSynth  ·  eliminate path  ·  12 periodize algorithms on harsh signal matrix",
        fontsize=14,
        color=MUTED,
        va="top",
    )

    # Main plot
    ax = fig.add_axes([0.055, 0.16, 0.68, 0.62])
    ymin = min(arts + running) * 0.78
    ymax = max(arts) * 1.16
    ax.set_xlim(0.35, max(xs) + 0.65)
    ax.set_ylim(ymin, ymax)
    ax.set_facecolor(SURFACE)
    ax.grid(True, color=GRID, linewidth=0.9, alpha=0.95)
    ax.set_axisbelow(True)
    for spine in ax.spines.values():
        spine.set_color(GRID)

    ax.plot(
        xs,
        running,
        color=ACCENT_SOFT,
        linewidth=3.2,
        solid_capstyle="round",
        zorder=3,
        label="Running best",
    )
    ax.fill_between(xs, ymin, running, color=ACCENT_SOFT, alpha=0.10, zorder=1)
    ax.plot(
        xs,
        arts,
        color=ACCENT,
        linewidth=2.4,
        marker="o",
        markersize=8.5,
        markerfacecolor=BG,
        markeredgewidth=2.0,
        markeredgecolor=ACCENT,
        zorder=4,
        label="Candidate",
    )

    win_i = next(i for i, r in enumerate(rows) if r["algo"] == winner)
    ax.scatter(
        [xs[win_i]],
        [arts[win_i]],
        s=280,
        facecolors=WIN,
        edgecolors=TEXT,
        linewidths=1.6,
        zorder=6,
    )
    # Callout left of winner so it clears the rising cross_detrend spike
    ax.annotate(
        f"best · iter {xs[win_i]}",
        xy=(xs[win_i], arts[win_i]),
        xytext=(xs[win_i] - 2.0, arts[win_i] + (ymax - ymin) * 0.28),
        fontsize=12,
        color=WIN,
        fontweight="bold",
        ha="center",
        arrowprops=dict(
            arrowstyle="-|>",
            color=WIN,
            lw=1.6,
            connectionstyle="arc3,rad=0.15",
        ),
        path_effects=[pe.withStroke(linewidth=4, foreground=BG)],
        zorder=7,
    )

    ax.set_xticks(xs)
    ax.set_xticklabels(labels, fontsize=9.5, color=MUTED, rotation=30, ha="right")
    ax.tick_params(axis="x", pad=3, length=0)
    ax.tick_params(axis="y", labelsize=11, pad=5)
    ax.set_ylabel("Mean artifact score   (lower is better)", fontsize=13, labelpad=10)
    ax.set_xlabel("Improvement iteration", fontsize=13, labelpad=8)
    leg = ax.legend(
        loc="upper left",
        frameon=True,
        fontsize=11,
        facecolor=CARD,
        edgecolor=GRID,
        labelcolor=TEXT,
        borderpad=0.7,
    )
    leg.get_frame().set_linewidth(1.0)

    # Stats panel as real axes (guarantees visible text)
    axc = fig.add_axes([0.76, 0.28, 0.20, 0.50])
    axc.set_facecolor(CARD)
    for spine in axc.spines.values():
        spine.set_color(GRID)
        spine.set_linewidth(1.3)
    axc.set_xticks([])
    axc.set_yticks([])
    axc.set_xlim(0, 1)
    axc.set_ylim(0, 1)

    axc.text(0.08, 0.92, "RESULT", fontsize=11, color=MUTED, fontweight="bold", va="top")
    axc.text(0.08, 0.78, f"−{pct:.0f}%", fontsize=42, color=WIN, fontweight="bold", va="top")
    axc.text(0.08, 0.62, "vs untreated raw", fontsize=12, color=MUTED, va="top")

    axc.text(0.08, 0.48, "untreated", fontsize=10, color=MUTED, va="top")
    axc.text(0.08, 0.40, f"{raw:.2f}", fontsize=20, color=BAD, fontweight="bold", va="top")

    axc.text(0.08, 0.28, "winner", fontsize=10, color=MUTED, va="top")
    axc.text(
        0.08,
        0.20,
        winner.replace("_", " "),
        fontsize=16,
        color=TEXT,
        fontweight="bold",
        va="top",
    )
    axc.text(0.08, 0.08, f"{win_score:.3f}", fontsize=20, color=WIN, fontweight="bold", va="top")

    fig.text(
        0.76,
        0.22,
        f"{raw:.2f}  →  {win_score:.3f}",
        fontsize=13,
        color=TEXT,
        fontweight="bold",
        va="center",
    )
    fig.text(
        0.76,
        0.175,
        f"classic {classic:.2f}  ·  dual cosine wins",
        fontsize=10,
        color=MUTED,
        va="center",
    )

    fig.text(
        0.055,
        0.045,
        "reelsynth  ·  MIT  ·  crackle = 0 eliminate  ·  dual-end raised-cosine periodize",
        fontsize=11,
        color=MUTED,
        ha="left",
        va="center",
    )
    fig.text(
        0.96,
        0.045,
        "share graphic",
        fontsize=10,
        color="#52525b",
        ha="right",
        va="center",
    )

    OUT.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(OUT, dpi=200, facecolor=BG, edgecolor="none")
    plt.close(fig)
    print(f"wrote {OUT} ({OUT.stat().st_size} bytes)")


if __name__ == "__main__":
    main()
