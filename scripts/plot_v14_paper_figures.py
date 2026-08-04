#!/usr/bin/env python3
"""Regenerate v14 paper plots from the FitCell-to-plateau hybrid champion.

Writes into denoise-opt-meta v14 figures/:
  - fig_search_learning_curve.{png,pdf}  (short FitCell vs plateau)
  - fig_v14_converge_learning_curve.{png,pdf}
  - champ_residual_vs_iter.png (plateau run)
  - fig_meta_heal_samples.{png,pdf} from v14 champ_cell.pt
"""
from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402

V14_FIG = (
    ROOT.parent
    / "denoise-opt-meta"
    / "paper"
    / "Unsupervised_Wrap-Discontinuity_Repair_in_Wavetable_Synthesis_via_Hybrid_GA-PPO_Meta-Search_v14"
    / "figures"
)
COMPARE_V10 = ROOT / "brand" / "artifacts" / "meta_approach_compare_v10"
CONVERGE = (
    ROOT
    / "brand"
    / "artifacts"
    / "meta_approach_compare_v14_converge"
    / "1902771841"
    / "hybrid_lstm"
)

C_IDEAL = "#000000"
C_ENGINE = "#D55E00"
C_DUAL = "#0072B2"
C_OURS = "#009E73"
C_N2N = "#009E73"
C_J = "#0072B2"
C_RAW = "#56B4E9"


def load_history(path: Path) -> list[dict]:
    rows: list[dict] = []
    with path.open(encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rows.append(json.loads(line))
    return rows


def series(rows: list[dict], key: str) -> tuple[np.ndarray, np.ndarray]:
    xs = np.array([int(r["iter"]) for r in rows], dtype=np.int32)
    ys = np.array([float(r[key]) for r in rows], dtype=np.float64)
    return xs, ys


def first_above(xs: np.ndarray, ys: np.ndarray, thr: float) -> int | None:
    hit = np.where(ys > thr)[0]
    return int(xs[int(hit[0])]) if hit.size else None


def load_v14_cell(device: torch.device) -> tuple[og.ArchConfig, og.SeamCell, dict]:
    blob = torch.load(CONVERGE / "champ_cell.pt", map_location=device, weights_only=False)
    cfg = og.ArchConfig.from_dict(blob["architecture"])
    cell = og.SeamCell(cfg).to(device)
    cell.load_state_dict(blob["cell_state_dict"])
    cell.eval()
    return cfg, cell, blob


def prolong(cycle: torch.Tensor, periods: int = 3) -> np.ndarray:
    return og.prolong_tile(cycle.unsqueeze(0), periods=periods)[0].detach().cpu().numpy()


@torch.no_grad()
def score_batch(ideal: torch.Tensor, out: torch.Tensor) -> float:
    return float(og.residual_score(ideal, out).mean().item())


def plot_learning_curves(out_dir: Path) -> dict:
    v10 = load_history(COMPARE_V10 / "hybrid_lstm" / "history.jsonl")
    v14 = load_history(CONVERGE / "history.jsonl")

    vx, vr = series(v10, "champ_raw")
    dc_v10 = float(v10[0].get("baseline_dual_cosine", 0.504))
    n2n = float(v10[0].get("n2n_gate_r", v14[0].get("n2n_gate_r", 0.975)))

    cx, cr = series(v14, "champ_raw")
    dc_v14 = float(v14[0].get("baseline_dual_cosine", 0.504))
    n2n_v14 = float(v14[0].get("n2n_gate_r", n2n))

    fig, axes = plt.subplots(1, 2, figsize=(7.2, 3.15), constrained_layout=True)

    ax = axes[0]
    ax.plot(vx, vr, color=C_RAW, lw=1.6, label=r"Champ $R_{\mathrm{blend}}$")
    ax.axhline(dc_v10, color="#999999", ls="--", lw=1.2, label=f"Dual Cosine ({dc_v10:.3f})")
    ax.axhline(n2n, color=C_N2N, ls=":", lw=1.3, label=f"N2N gate ({n2n:.3f})")
    ax.set_title("Short FitCell hybrid ($1{,}200$ evals)")
    ax.set_xlabel("Outer-loop evaluation")
    ax.set_ylabel(r"Champion $R_{\mathrm{blend}}$")
    ax.set_xlim(1, int(vx[-1]))
    ax.grid(True, alpha=0.25)
    ax.legend(loc="lower right", fontsize=7)

    ax = axes[1]
    ax.plot(cx, cr, color=C_RAW, lw=1.6, label=r"Champ $R_{\mathrm{blend}}$")
    ax.axhline(dc_v14, color="#999999", ls="--", lw=1.2, label=f"Dual Cosine ({dc_v14:.3f})")
    ax.axhline(n2n_v14, color=C_N2N, ls=":", lw=1.3, label=f"N2N gate ({n2n_v14:.3f})")
    gate_it = first_above(cx, cr, n2n_v14)
    if gate_it is not None:
        ax.axvline(gate_it, color=C_N2N, ls=":", lw=0.9, alpha=0.7)
        ax.text(
            0.02,
            0.98,
            rf"$R_{{\mathrm{{blend}}}}$$>$N2N @ {gate_it}; final {cr[-1]:.4f}",
            transform=ax.transAxes,
            va="top",
            fontsize=7,
        )
    ax.set_title("FitCell-to-plateau hybrid ($750$ evals)")
    ax.set_xlabel("Outer-loop evaluation")
    ax.set_ylabel(r"Champion $R_{\mathrm{blend}}$")
    ax.set_xlim(1, int(cx[-1]))
    ax.grid(True, alpha=0.25)
    ax.legend(loc="lower right", fontsize=7)

    stem = out_dir / "fig_search_learning_curve"
    fig.savefig(stem.with_suffix(".png"), dpi=220)
    fig.savefig(stem.with_suffix(".pdf"))
    plt.close(fig)

    # Dedicated single-panel for the plateau section
    fig, ax = plt.subplots(figsize=(3.4, 2.6), constrained_layout=True)
    ax.plot(cx, cr, color=C_RAW, lw=1.7, label=r"Champ $R_{\mathrm{blend}}$")
    ax.axhline(dc_v14, color="#999999", ls="--", lw=1.2, label=f"Dual Cosine ({dc_v14:.3f})")
    ax.axhline(n2n_v14, color=C_N2N, ls=":", lw=1.4, label=f"N2N gate ({n2n_v14:.3f})")
    if gate_it is not None:
        ax.axvline(gate_it, color=C_N2N, ls=":", lw=0.9, alpha=0.75)
    ax.set_xlabel("Outer-loop evaluation")
    ax.set_ylabel(r"Champion $R_{\mathrm{blend}}$")
    ax.set_title(r"FitCell-to-plateau")
    ax.set_xlim(1, int(cx[-1]))
    ax.grid(True, alpha=0.25)
    ax.legend(loc="lower right", fontsize=7)
    stem2 = out_dir / "fig_v14_converge_learning_curve"
    fig.savefig(stem2.with_suffix(".png"), dpi=220)
    fig.savefig(stem2.with_suffix(".pdf"))
    plt.close(fig)

    # Appendix champ residual (raw R_blend)
    fig, ax = plt.subplots(figsize=(3.4, 2.6), constrained_layout=True)
    ax.plot(cx, cr, color="#000000", lw=1.7, marker="o", markevery=max(1, len(cx) // 25), markersize=3.5)
    ax.axhline(dc_v14, color="#999999", ls="--", lw=1.3, label=f"Dual Cosine ({dc_v14:.3f})")
    ax.axhline(n2n_v14, color=C_N2N, ls=":", lw=1.3, label=f"N2N gate ({n2n_v14:.3f})")
    ax.set_xlabel("Outer-loop evaluation")
    ax.set_ylabel(r"Champion $R_{\mathrm{blend}}$")
    ax.set_title("FitCell-to-plateau champion residual")
    ax.set_xlim(1, int(cx[-1]))
    ax.grid(True, alpha=0.25)
    ax.legend(loc="lower right", fontsize=7)
    fig.savefig(out_dir / "champ_residual_vs_iter.png", dpi=220)
    plt.close(fig)

    summary = {
        "short_fitcell_final_r_blend": float(vr[-1]),
        "plateau_final_r_blend": float(cr[-1]),
        "n2n_gate": n2n_v14,
        "plateau_first_above_n2n_iter": gate_it,
        "seed": 1902771841,
    }
    (out_dir / "fig_search_learning_curve_summary.json").write_text(
        json.dumps(summary, indent=2), encoding="utf-8"
    )
    (out_dir / "fig_v14_converge_learning_curve_summary.json").write_text(
        json.dumps(summary, indent=2), encoding="utf-8"
    )
    return summary


def plot_heal(out_dir: Path, device: torch.device) -> dict:
    cfg, cell, blob = load_v14_cell(device)
    holdout = ROOT / "brand" / "artifacts" / "canonical_eval_dataset" / "holdout_batch.pt"
    if holdout.is_file():
        packed = torch.load(holdout, map_location="cpu", weights_only=False)
        ideal_b = packed["ideal"].to(device).float()
        eng_b = packed["engine"].to(device).float()
        hold_note = "holdout_batch.pt"
    else:
        torch.manual_seed(20260719)
        ideal_b, eng_b = og.make_batch(64, og.N, device)
        hold_note = "make_batch"

    wrap = (eng_b[:, 0] - eng_b[:, -1]).abs()
    idx = 46 if eng_b.shape[0] > 46 else int(wrap.argmax().item())
    ideal = ideal_b[idx : idx + 1]
    eng = eng_b[idx : idx + 1]
    dual = og.dual_cosine_blend(eng)
    with torch.no_grad():
        ours = og.apply_ops(eng, cell, cfg.ops)

    scores = {
        "no_bake": score_batch(ideal, eng),
        "dual_cosine": score_batch(ideal, dual),
        "ours": score_batch(ideal, ours),
        "champ_r_blend_ckpt": float(blob.get("r_blend", blob.get("champ_raw", 0.0))),
    }

    periods = 3
    L = int(ideal.shape[-1])
    x = np.arange(L * periods)
    y_ideal = prolong(ideal[0], periods)
    y_eng = prolong(eng[0], periods)
    y_dual = prolong(dual[0], periods)
    y_ours = prolong(ours[0], periods)
    wrap_abs = float((eng[0, 0] - eng[0, -1]).abs().item())

    fig = plt.figure(figsize=(7.2, 5.6), constrained_layout=True)
    gs = fig.add_gridspec(2, 2, height_ratios=[1.15, 1.0])
    ax = fig.add_subplot(gs[0, :])
    ax.plot(x, y_ideal, color=C_IDEAL, lw=1.15, label=r"ideal sibling $r^\star$")
    ax.plot(x, y_eng, color=C_ENGINE, lw=1.1, ls="--", label=f"no-bake $R$={scores['no_bake']:.3f}")
    ax.plot(x, y_dual, color=C_DUAL, lw=1.15, ls="-.", label=f"Dual Cosine $R$={scores['dual_cosine']:.3f}")
    ax.plot(x, y_ours, color=C_OURS, lw=1.4, label=f"Ours (plateau) $R$={scores['ours']:.3f}")
    for k in range(1, periods):
        ax.axvline(k * L - 0.5, color="#888888", lw=0.7, ls=":", alpha=0.7)
    ax.axvspan(L - 24, L + 24, color="#F0E442", alpha=0.15, zorder=0)
    ax.set_title(f"Holdout wrap-seam heal (plateau champ, tile={idx}, $|wrap|={wrap_abs:.3f}$)")
    ax.set_xlabel("sample (tiled)")
    ax.set_ylabel("amplitude")
    ax.legend(loc="upper right", fontsize=7, ncol=2)
    ax.grid(True, alpha=0.2)

    axz = fig.add_subplot(gs[1, 0])
    lo, hi = L - 40, L + 40
    axz.plot(x[lo:hi], y_ideal[lo:hi], color=C_IDEAL, lw=1.2)
    axz.plot(x[lo:hi], y_eng[lo:hi], color=C_ENGINE, lw=1.1, ls="--")
    axz.plot(x[lo:hi], y_dual[lo:hi], color=C_DUAL, lw=1.15, ls="-.")
    axz.plot(x[lo:hi], y_ours[lo:hi], color=C_OURS, lw=1.4)
    axz.axvline(L - 0.5, color="#888888", lw=0.8, ls=":")
    axz.set_title("Seam zoom")
    axz.set_xlabel("sample")
    axz.grid(True, alpha=0.2)

    axb = fig.add_subplot(gs[1, 1])
    labels = ["no-bake", "Dual Cosine", "Ours"]
    vals = [scores["no_bake"], scores["dual_cosine"], scores["ours"]]
    colors = [C_ENGINE, C_DUAL, C_OURS]
    axb.bar(labels, vals, color=colors, width=0.65)
    axb.set_ylim(0.0, 1.05)
    axb.set_ylabel(r"Absolute $R$ (tile)")
    axb.set_title("Tile residual")
    axb.grid(True, axis="y", alpha=0.25)

    stem = out_dir / "fig_meta_heal_samples"
    fig.savefig(stem.with_suffix(".png"), dpi=220)
    fig.savefig(stem.with_suffix(".pdf"))
    plt.close(fig)

    meta = {
        "schema": "denoiseopt.meta_heal_samples.v14_plateau",
        "holdout": hold_note,
        "tile": idx,
        "scores": scores,
        "champ_ops": list(cfg.ops),
        "source_champ": str(CONVERGE / "champ_cell.pt"),
    }
    (out_dir / "fig_meta_heal_samples.json").write_text(json.dumps(meta, indent=2), encoding="utf-8")
    return meta


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out-dir", type=Path, default=V14_FIG)
    ap.add_argument("--device", type=str, default="cpu")
    ap.add_argument("--skip-heal", action="store_true")
    args = ap.parse_args()
    out_dir: Path = args.out_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    summary = plot_learning_curves(out_dir)
    print("learning curves:", json.dumps(summary, indent=2))

    if not args.skip_heal:
        device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")
        heal = plot_heal(out_dir, device)
        print("heal:", json.dumps(heal, indent=2))

    # Mirror summary JSON into paper results blob for the converge table
    paper_json = out_dir / "v14_hybrid_converge_results.json"
    summ_path = CONVERGE / "summary.json"
    champ_j = None
    if summ_path.is_file():
        champ_j = float(json.loads(summ_path.read_text(encoding="utf-8")).get("champ_j", float("nan")))
    paper_json.write_text(
        json.dumps(
            {
                "schema": "denoiseopt.v14_hybrid_converge.v1",
                "seed": 1902771841,
                "iters": 750,
                "champ_r_blend": summary["plateau_final_r_blend"],
                "champ_j": champ_j,
                "n2n_gate_r": summary["n2n_gate"],
                "beats_n2n": summary["plateau_final_r_blend"] > summary["n2n_gate"],
                "first_above_n2n_iter": summary["plateau_first_above_n2n_iter"],
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    print(f"wrote figures under {out_dir}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
