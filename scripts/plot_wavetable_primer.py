#!/usr/bin/env python3
"""Wavetable synthesis primer figure for the paper introduction.

Three panels: (a) one stored period, (b) tiled playback with wrap marks,
(c) zoom at a mismatched wrap join (seam / click).

Writes:
  brand/artifacts/paper_figures/fig_wavetable_primer.{png,pdf}
and copies PNG into the v11 paper figures/ directory.
"""
from __future__ import annotations

import shutil
from pathlib import Path

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
OUT_DIR = ROOT / "brand" / "artifacts" / "paper_figures"
PAPER_FIG = (
    ROOT.parent
    / "denoise-opt-meta"
    / "paper"
    / "Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v11"
    / "figures"
)

# Okabe–Ito (colorblind-safe) + B&W-friendly linestyles
C_SIGNAL = "#0072B2"  # blue
C_WRAP = "#D55E00"  # vermillion
C_ANNOT = "#000000"
C_CLIFF = "#E69F00"  # orange

L = 256
N_TILES = 4


def _mpl():
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    plt.rcParams.update(
        {
            "font.size": 9,
            "axes.titlesize": 10,
            "axes.labelsize": 9,
            "figure.dpi": 160,
            "savefig.dpi": 220,
            "axes.grid": True,
            "grid.alpha": 0.25,
            "axes.axisbelow": True,
        }
    )
    return plt


def make_period(n: int = L) -> np.ndarray:
    """Mild saw-like period with a few harmonics (not a pure sine)."""
    t = np.linspace(0.0, 1.0, n, endpoint=False)
    # Soft saw: fundamental + decaying odd/even harmonics
    x = (
        1.00 * np.sin(2 * np.pi * t)
        + 0.45 * np.sin(4 * np.pi * t)
        + 0.22 * np.sin(6 * np.pi * t)
        + 0.12 * np.sin(8 * np.pi * t)
        - 0.08 * np.cos(2 * np.pi * t)
    )
    x = x / np.max(np.abs(x)) * 0.85
    return x.astype(np.float64)


def make_mismatched(period: np.ndarray, cliff: float = 0.55) -> np.ndarray:
    """Force x[0] != x[L-1] with a small artificial endpoint cliff."""
    y = period.copy()
    # Raise the last few samples so the wrap has a visible step
    y[-1] = y[0] - cliff
    y[-2] = 0.65 * y[-2] + 0.35 * y[-1]
    y[-3] = 0.85 * y[-3] + 0.15 * y[-1]
    return y


def main() -> None:
    plt = _mpl()

    period = make_period(L)
    cracked = make_mismatched(period)
    tiled = np.tile(cracked, N_TILES)

    # Shared y limits for (a)/(b); (c) uses a tighter zoom window
    y_lo, y_hi = -1.15, 1.15

    fig, axes = plt.subplots(1, 3, figsize=(11.0, 3.15))
    fig.subplots_adjust(wspace=0.28, left=0.06, right=0.98, top=0.88, bottom=0.18)

    # --- (a) stored period ---
    ax = axes[0]
    n = np.arange(L)
    ax.plot(n, period, color=C_SIGNAL, lw=1.4, ls="-")
    ax.plot([0], [period[0]], "o", color=C_ANNOT, ms=4.5, zorder=5)
    ax.plot([L - 1], [period[L - 1]], "s", color=C_ANNOT, ms=4.5, zorder=5)
    ax.annotate(
        r"$0$",
        xy=(0, period[0]),
        xytext=(14, 18),
        textcoords="offset points",
        fontsize=8,
        color=C_ANNOT,
        arrowprops=dict(arrowstyle="-", color=C_ANNOT, lw=0.7),
    )
    ax.annotate(
        r"$L{-}1$",
        xy=(L - 1, period[L - 1]),
        xytext=(-36, 16),
        textcoords="offset points",
        fontsize=8,
        color=C_ANNOT,
        arrowprops=dict(arrowstyle="-", color=C_ANNOT, lw=0.7),
    )
    ax.set_xlim(-4, L + 4)
    ax.set_ylim(y_lo, y_hi)
    ax.set_xlabel("sample index")
    ax.set_ylabel("amplitude")
    ax.set_title("(a) Stored period $x[0..L{-}1]$")

    # --- (b) tiled playback ---
    ax = axes[1]
    x_t = np.arange(L * N_TILES)
    ax.plot(x_t, tiled, color=C_SIGNAL, lw=1.15, ls="-")
    for k in range(1, N_TILES):
        ax.axvline(k * L, color=C_WRAP, ls="--", lw=1.0, alpha=0.9)
    # Label wrap lines once
    ax.text(
        L + 6,
        y_hi - 0.12,
        "read pointer wraps",
        fontsize=7.5,
        color=C_WRAP,
        rotation=90,
        va="top",
        ha="left",
    )
    # "one period" bracket on first cycle
    bracket_y = y_lo + 0.12
    ax.annotate(
        "",
        xy=(0, bracket_y),
        xytext=(L, bracket_y),
        arrowprops=dict(arrowstyle="<->", color=C_ANNOT, lw=1.0),
    )
    ax.text(L / 2, bracket_y + 0.08, "one period", ha="center", va="bottom", fontsize=8)
    ax.set_xlim(-8, L * N_TILES + 8)
    ax.set_ylim(y_lo, y_hi)
    ax.set_xlabel("sample index (playback)")
    ax.set_ylabel("amplitude")
    ax.set_title("(b) Tiled playback (loop)")

    # --- (c) wrap zoom with mismatched seam ---
    ax = axes[2]
    # Zoom around first wrap at index L
    half = 28
    lo, hi = L - half, L + half
    x_z = np.arange(lo, hi)
    y_z = tiled[lo:hi]
    ax.plot(x_z, y_z, color=C_SIGNAL, lw=1.5, ls="-")
    ax.axvline(L, color=C_WRAP, ls="--", lw=1.15)
    # Mark the two samples at the join
    ax.plot([L - 1], [tiled[L - 1]], "s", color=C_CLIFF, ms=5, zorder=5)
    ax.plot([L], [tiled[L]], "o", color=C_CLIFF, ms=5, zorder=5)
    # Vertical stub highlighting the cliff
    ax.plot(
        [L - 1, L],
        [tiled[L - 1], tiled[L]],
        color=C_CLIFF,
        lw=1.8,
        ls=":",
        zorder=4,
    )
    ax.annotate(
        "wrap join / seam",
        xy=(L, 0.5 * (tiled[L - 1] + tiled[L])),
        xytext=(L + 8, tiled[L] + 0.35),
        fontsize=8,
        color=C_ANNOT,
        arrowprops=dict(arrowstyle="->", color=C_ANNOT, lw=0.8),
    )
    ax.annotate(
        "click",
        xy=(L, tiled[L]),
        xytext=(L - 22, tiled[L] - 0.55),
        fontsize=8,
        color=C_WRAP,
        fontweight="bold",
        arrowprops=dict(arrowstyle="->", color=C_WRAP, lw=0.9),
    )
    ax.set_xlim(lo, hi)
    # Slightly wider y so annotations fit; still readable vs (a)/(b)
    ax.set_ylim(min(y_z.min() - 0.25, -1.0), max(y_z.max() + 0.35, 1.0))
    ax.set_xlabel("sample index")
    ax.set_ylabel("amplitude")
    ax.set_title("(c) Mismatched wrap (zoom)")

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    png = OUT_DIR / "fig_wavetable_primer.png"
    pdf = OUT_DIR / "fig_wavetable_primer.pdf"
    fig.savefig(png, bbox_inches="tight")
    fig.savefig(pdf, bbox_inches="tight")
    plt.close(fig)
    print(f"wrote {png}")
    print(f"wrote {pdf}")

    PAPER_FIG.mkdir(parents=True, exist_ok=True)
    dest = PAPER_FIG / "fig_wavetable_primer.png"
    shutil.copy2(png, dest)
    print(f"copied -> {dest}")
    # Optional PDF next to paper figures
    shutil.copy2(pdf, PAPER_FIG / "fig_wavetable_primer.pdf")


if __name__ == "__main__":
    main()
