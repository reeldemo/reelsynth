#!/usr/bin/env python3
"""Plot hybrid vs Random champion curves for the v11 peer-review response.

Panel A: matched 5k meta_approach_compare under superseded prolonged R.
Panel B: v10.1 hybrid under locked R_blend (Random under R_blend not available).

Writes PNG/PDF into the v11 paper figures/ folder.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

import matplotlib.pyplot as plt
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
COMPARE = ROOT / "brand" / "artifacts" / "meta_approach_compare"
COMPARE_V10 = ROOT / "brand" / "artifacts" / "meta_approach_compare_v10"
PAPER_FIG = Path(
    r"C:\Users\Julian\Documents\Programming\github\reeldemo\denoise-opt-meta"
    r"\paper\Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v11"
    r"\figures"
)


def load_history(path: Path) -> list[dict]:
    rows: list[dict] = []
    with path.open(encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rows.append(json.loads(line))
    return rows


def champ_series(rows: list[dict], key: str = "champ") -> tuple[np.ndarray, np.ndarray]:
    xs = np.array([int(r["iter"]) for r in rows], dtype=np.int32)
    ys = np.array([float(r[key]) for r in rows], dtype=np.float64)
    return xs, ys


def first_surpass(xs: np.ndarray, ys: np.ndarray, threshold: float) -> int | None:
    hit = np.where(ys > threshold)[0]
    if hit.size == 0:
        return None
    return int(xs[int(hit[0])])


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--out-dir", type=Path, default=PAPER_FIG)
    args = ap.parse_args()
    out_dir: Path = args.out_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    hybrid = load_history(COMPARE / "hybrid_lstm" / "history.jsonl")
    random = load_history(COMPARE / "random" / "history.jsonl")
    hx, hy = champ_series(hybrid, "champ")
    rx, ry = champ_series(random, "champ")
    dc = float(hybrid[0].get("baseline_dual_cosine", 0.81658))

    v10 = load_history(COMPARE_V10 / "hybrid_lstm" / "history.jsonl")
    vx, vy = champ_series(v10, "champ")  # J-scored champ under R_blend search
    vx_raw, vy_raw = champ_series(v10, "champ_raw")  # raw R_blend
    dc_v10 = float(v10[0].get("baseline_dual_cosine", 0.504))
    n2n_gate = float(v10[0].get("n2n_gate_r", 0.9750))

    sur_h = first_surpass(hx, hy, dc)
    sur_r = first_surpass(rx, ry, dc)
    sur_v10 = first_surpass(vx_raw, vy_raw, dc_v10)

    fig, axes = plt.subplots(1, 2, figsize=(7.2, 3.2), constrained_layout=True)

    ax = axes[0]
    ax.plot(hx, hy, color="#0072B2", lw=1.6, label="Hybrid GA–PPO")
    ax.plot(rx, ry, color="#E69F00", lw=1.4, label="Random NAS")
    ax.axhline(dc, color="#999999", ls="--", lw=1.2, label=f"Dual Cosine ({dc:.3f})")
    ax.set_xlabel("Outer-loop evaluation")
    ax.set_ylabel("Champion prolonged $R$")
    ax.set_title("Matched 5k (superseded metric)")
    ax.set_xlim(1, max(int(hx[-1]), int(rx[-1])))
    ax.grid(True, alpha=0.25)
    ax.legend(loc="lower right", fontsize=8)
    note = []
    if sur_h is not None:
        note.append(f"hybrid$>$DC @ {sur_h}")
    if sur_r is not None:
        note.append(f"random$>$DC @ {sur_r}")
    if note:
        ax.text(0.02, 0.98, "; ".join(note), transform=ax.transAxes, va="top", fontsize=7)

    ax = axes[1]
    ax.plot(vx, vy, color="#0072B2", lw=1.6, label="Hybrid champ $J$")
    ax.plot(vx_raw, vy_raw, color="#56B4E9", lw=1.2, label="Hybrid champ $R_{\\mathrm{blend}}$")
    ax.axhline(dc_v10, color="#999999", ls="--", lw=1.2, label=f"Dual Cosine ({dc_v10:.3f})")
    ax.axhline(n2n_gate, color="#009E73", ls=":", lw=1.3, label=f"N2N gate ({n2n_gate:.3f})")
    ax.set_xlabel("Outer-loop evaluation")
    ax.set_ylabel("Champion score")
    ax.set_title("v10.1 hybrid ($R_{\\mathrm{blend}}$ / $J$)")
    ax.set_xlim(1, int(vx[-1]))
    ax.grid(True, alpha=0.25)
    ax.legend(loc="lower right", fontsize=7)
    if sur_v10 is not None:
        ax.text(
            0.02,
            0.98,
            f"$R_{{\\mathrm{{blend}}}}$$>$DC @ {sur_v10}; Random under $R_{{\\mathrm{{blend}}}}$ deferred",
            transform=ax.transAxes,
            va="top",
            fontsize=7,
        )

    stem = out_dir / "fig_search_learning_curve"
    fig.savefig(stem.with_suffix(".png"), dpi=220)
    fig.savefig(stem.with_suffix(".pdf"))
    plt.close(fig)

    summary = {
        "panel_a_metric": "superseded_prolonged_R",
        "panel_a_seed": 1902771841,
        "panel_a_budget": 5000,
        "panel_a_hybrid_final_champ": float(hy[-1]),
        "panel_a_random_final_champ": float(ry[-1]),
        "panel_a_dual_cosine": dc,
        "panel_a_hybrid_first_surpass_dc_iter": sur_h,
        "panel_a_random_first_surpass_dc_iter": sur_r,
        "panel_b_metric": "r_blend_and_J",
        "panel_b_seed": 1902771841,
        "panel_b_budget": 1200,
        "panel_b_hybrid_final_champ_j": float(vy[-1]),
        "panel_b_hybrid_final_champ_raw_r_blend": float(vy_raw[-1]),
        "panel_b_dual_cosine_r_blend": dc_v10,
        "panel_b_n2n_gate_r_blend": n2n_gate,
        "panel_b_hybrid_first_surpass_dc_iter": sur_v10,
        "random_under_r_blend": "deferred",
    }
    (out_dir / "fig_search_learning_curve_summary.json").write_text(
        json.dumps(summary, indent=2), encoding="utf-8"
    )
    print(f"wrote {stem.with_suffix('.png')}")
    print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
