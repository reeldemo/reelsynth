#!/usr/bin/env python3
"""Reproduce Table 13 / transfer-main grouped bar chart from results_table.json.

Reads prolonged residual R (and nested author-method R) from the frozen
signal_heal_transfer results table. Writes PNG/PDF under
brand/artifacts/signal_heal_transfer/figures/ and optionally mirrors into the
paper v11 figures/ directory.

CPU-only. No invented scores — JSON only.
"""
from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_JSON = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "results_table.json"
DEFAULT_OUT = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "figures" / "fig_signal_heal_transfer"
PAPER_V11_FIGS = Path(
    r"C:\Users\Julian\Documents\Programming\github\reeldemo\denoise-opt-meta\paper"
    r"\Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v11"
    r"\figures"
)

# Board order matches Table 13 columns.
BOARDS: list[tuple[str, str]] = [
    ("cwru_bearings", "CWRU"),
    ("mfpt_bearings", "MFPT"),
    ("paderborn_kat", "Paderborn"),
    ("mitbih_ecg", "MIT-BIH"),
    ("ptbxl_ecg", "PTB-XL"),
    ("synth_cnc_g01", "synth CNC"),
    ("synth_pmu_cycle", "synth PMU"),
]

# Okabe–Ito + neutrals (match other paper comparison figures).
# Order matches Table 13 rows.
METHODS: list[tuple[str, str, str]] = [
    ("ours_hybrid_lstm", "Ours", "#D55E00"),
    ("n2n_domain_trained", "SeamN2N", "#009E73"),
    ("no_bake", "no-bake", "#999999"),
    ("endpoint_pin_mean", "endpoint-pin", "#E69F00"),
    ("seam_fir3", "seam_fir3", "#56B4E9"),
    ("dual_cosine", "DualCosine", "#0072B2"),
    ("beatdiff", "BeatDiff (OOD)", "#000000"),
    ("cycle_gan_ecg", "Cycle-GAN (OOD)", "#CC79A7"),
    ("paderborn_alfirdausi_backbone_wrap", "Al Firdausi wrap", "#F0E442"),
]


def _mpl():
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    plt.rcParams.update(
        {
            "font.size": 9,
            "axes.titlesize": 10,
            "axes.labelsize": 9,
            "legend.fontsize": 7.5,
            "figure.dpi": 160,
            "savefig.dpi": 220,
            "axes.grid": True,
            "grid.alpha": 0.28,
            "axes.axisbelow": True,
            "pdf.fonttype": 42,
            "ps.fonttype": 42,
        }
    )
    return plt


def _scalar_r(cell: Any) -> float | None:
    """Extract prolonged R from a table cell (float or nested dict with R)."""
    if isinstance(cell, (int, float)):
        return float(cell)
    if isinstance(cell, dict):
        if cell.get("wrap_R") is None and "R" not in cell and "holdout_accuracy" in cell:
            # Classifier-only row — not a wrap-R bar.
            return None
        if "R" in cell and cell["R"] is not None:
            return float(cell["R"])
        if "wrap_R" in cell and cell["wrap_R"] is not None:
            return float(cell["wrap_R"])
    return None


def extract_matrix(table: dict[str, Any]) -> tuple[list[str], list[str], list[list[float | None]]]:
    board_labels = [lab for _, lab in BOARDS]
    method_labels = [lab for _, lab, _ in METHODS]
    mat: list[list[float | None]] = []
    for key, _lab, _c in METHODS:
        row: list[float | None] = []
        for board_key, _blab in BOARDS:
            board = table.get(board_key, {})
            row.append(_scalar_r(board.get(key)))
        mat.append(row)
    return board_labels, method_labels, mat


def plot_transfer(
    table: dict[str, Any],
    out_stem: Path,
    *,
    title: str | None = None,
) -> list[Path]:
    plt = _mpl()
    board_labels, method_labels, mat = extract_matrix(table)
    n_boards = len(board_labels)
    n_methods = len(method_labels)
    x = __import__("numpy").arange(n_boards, dtype=float)
    width = min(0.11, 0.82 / max(n_methods, 1))

    fig, ax = plt.subplots(figsize=(11.2, 4.4))
    for i, ((key, lab, color), vals) in enumerate(zip(METHODS, mat)):
        xs = x + (i - (n_methods - 1) / 2.0) * width
        heights = [0.0 if v is None else v for v in vals]
        # Hide missing (---) bars by zero height + no edge; hatch OOD author probes.
        present = [v is not None for v in vals]
        hatch = "//" if key in ("beatdiff", "cycle_gan_ecg") else (
            ".." if key == "paderborn_alfirdausi_backbone_wrap" else None
        )
        bars = ax.bar(
            xs,
            heights,
            width * 0.92,
            label=lab,
            color=color,
            edgecolor="white" if hatch is None else "#333333",
            linewidth=0.35,
            hatch=hatch,
            zorder=3,
        )
        for bar, ok in zip(bars, present):
            if not ok:
                bar.set_height(0.0)
                bar.set_linewidth(0.0)
                bar.set_facecolor("none")
                bar.set_hatch(None)

    ax.set_xticks(x)
    ax.set_xticklabels(board_labels, fontsize=8)
    ax.set_ylabel(r"Prolonged residual $R$ (higher better)")
    ax.set_ylim(0.0, 1.05)
    ax.set_title(
        title
        or "Table 13 transfer board — prolonged $R$ (author OOD probes hatched)",
        fontsize=10,
    )
    ax.legend(
        ncol=5,
        frameon=False,
        loc="upper center",
        bbox_to_anchor=(0.5, -0.14),
        columnspacing=0.9,
        handlelength=1.4,
    )
    ax.axhline(0.0, color="k", lw=0.4)
    fig.tight_layout()
    out_stem.parent.mkdir(parents=True, exist_ok=True)
    png = out_stem.with_suffix(".png")
    pdf = out_stem.with_suffix(".pdf")
    fig.savefig(png, bbox_inches="tight")
    fig.savefig(pdf, bbox_inches="tight")
    plt.close(fig)
    return [png, pdf]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--json", type=Path, default=DEFAULT_JSON)
    ap.add_argument("--out", type=Path, default=DEFAULT_OUT, help="Output stem (no extension)")
    ap.add_argument(
        "--paper-figs",
        type=Path,
        default=PAPER_V11_FIGS,
        help="Also copy PNG/PDF (+ JSON snapshot) here; empty string to skip",
    )
    ap.add_argument("--no-paper-copy", action="store_true")
    args = ap.parse_args()

    blob = json.loads(args.json.read_text(encoding="utf-8"))
    table = blob["table"]
    written = plot_transfer(table, args.out)
    for p in written:
        print(f"wrote {p}")

    if not args.no_paper_copy and str(args.paper_figs):
        dest = Path(args.paper_figs)
        dest.mkdir(parents=True, exist_ok=True)
        for p in written:
            shutil.copy2(p, dest / p.name)
            print(f"copied -> {dest / p.name}")
        # Keep a frozen JSON snapshot beside the figure for the paper slug.
        snap = dest / "signal_heal_transfer_results_table.json"
        shutil.copy2(args.json, snap)
        print(f"copied -> {snap}")
        # Convenience copies at artifact root (legacy paths referenced by benches).
        art_root = ROOT / "brand" / "artifacts" / "signal_heal_transfer"
        for p in written:
            shutil.copy2(p, art_root / p.name)
            print(f"copied -> {art_root / p.name}")

    # Sanity: print Table-13-shaped matrix (4 decimals).
    _, method_labels, mat = extract_matrix(table)
    board_labs = [lab for _, lab in BOARDS]
    print("method," + ",".join(board_labs))
    for lab, row in zip(method_labels, mat):
        cells = ["---" if v is None else f"{v:.4f}" for v in row]
        print(lab + "," + ",".join(cells))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
