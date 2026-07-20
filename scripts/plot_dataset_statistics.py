#!/usr/bin/env python3
"""Distribution plots for the frozen canonical evaluation corpus (paper holdout).

Histograms of ideal RMS, engine residual RMS, wrap jump, and identity residual R
on a large draw (default n=4096, seed 20260719) from make_batch.
"""
from __future__ import annotations

import argparse
import json
import shutil
import sys
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402

META_FIG = Path(
    r"C:\Users\Julian\Documents\Programming\github\reeldemo\denoise-opt-meta\paper\v7\figures"
)

CANONICAL_EVAL_SEED = 20_260_719
DEFAULT_N = 4096

# Okabe-Ito + print-friendly
PANEL_COLORS = ["#0072B2", "#009E73", "#E69F00", "#CC79A7"]


def set_seed(seed: int, device: torch.device) -> None:
    torch.manual_seed(seed)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(seed)


@torch.no_grad()
def collect_arrays(n: int, seed: int, device: torch.device) -> dict:
    set_seed(seed, device)
    ideal, eng = og.make_batch(n, og.N, device)
    resid = eng - ideal
    residual_rms = resid.pow(2).mean(dim=1).sqrt().cpu()
    ideal_rms = ideal.pow(2).mean(dim=1).sqrt().cpu()
    wrap_jump = (eng[:, 0] - eng[:, -1]).abs().cpu()
    r_identity = og.residual_score(ideal, eng).cpu()
    return {
        "ideal_rms": ideal_rms.numpy(),
        "engine_residual_rms": residual_rms.numpy(),
        "wrap_jump": wrap_jump.numpy(),
        "identity_R": r_identity.numpy(),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=DEFAULT_N)
    ap.add_argument("--seed", type=int, default=CANONICAL_EVAL_SEED)
    ap.add_argument("--out-dir", type=Path, default=None)
    ap.add_argument("--dpi", type=int, default=220)
    ap.add_argument("--also-meta-v7", action="store_true", default=True)
    args = ap.parse_args()

    try:
        import matplotlib

        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        import numpy as np
    except ImportError:
        print("ERROR: matplotlib and numpy required", flush=True)
        return 2

    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    data = collect_arrays(args.n, args.seed, device)

    out_dir = args.out_dir or (ROOT / "brand" / "artifacts" / "figures" / "dataset_stats")
    out_dir.mkdir(parents=True, exist_ok=True)

    panels = [
        ("ideal_rms", r"Ideal RMS per cycle", "Amplitude"),
        ("engine_residual_rms", r"Engine residual RMS ($|x-y|$)", "RMS"),
        ("wrap_jump", r"Wrap jump $|x_0 - x_{L-1}|$", "Magnitude"),
        ("identity_R", r"No-bake prolonged $R$ (passthrough)", r"$R$"),
    ]

    fig, axes = plt.subplots(2, 2, figsize=(7.2, 5.4), dpi=args.dpi)
    fig.suptitle(
        f"Evaluation corpus distributions ($n={args.n:,}$, seed {args.seed})",
        fontsize=11,
    )

    for ax, (key, title, xlab), color in zip(axes.flat, panels, PANEL_COLORS):
        vals = data[key]
        ax.hist(vals, bins=48, color=color, alpha=0.85, edgecolor="white", linewidth=0.3)
        p50 = float(np.median(vals))
        mean = float(np.mean(vals))
        ax.axvline(mean, color="#000000", linestyle="--", linewidth=1.0, label=f"mean {mean:.3f}")
        ax.axvline(p50, color="#D55E00", linestyle=":", linewidth=1.0, label=f"median {p50:.3f}")
        ax.set_title(title, fontsize=10)
        ax.set_xlabel(xlab, fontsize=9)
        ax.set_ylabel("Count", fontsize=9)
        ax.legend(loc="upper right", fontsize=7, frameon=False)

    fig.tight_layout(rect=[0, 0, 1, 0.96])
    png = out_dir / "fig_dataset_distributions.png"
    fig.savefig(png, dpi=args.dpi, bbox_inches="tight")
    plt.close(fig)

    # Cliff hardness: top deciles of wrap jump
    wj = data["wrap_jump"]
    p90 = float(np.percentile(wj, 90))
    p75 = float(np.percentile(wj, 75))
    hard_mask = wj >= p90
    summary = {
        "n_samples": args.n,
        "seed": args.seed,
        "cycle_length_N": int(og.N),
        "prolong_tiles": int(og.PROLONG),
        "seam_width": int(og.SEAM_W),
        "wrap_jump_p75": p75,
        "wrap_jump_p90": p90,
        "hard_top10pct_n": int(hard_mask.sum()),
        "hard_top10pct_identity_R_mean": float(data["identity_R"][hard_mask].mean()),
        "figure": png.name,
    }
    (out_dir / "dataset_distributions.json").write_text(
        json.dumps(summary, indent=2), encoding="utf-8"
    )

    if args.also_meta_v7:
        META_FIG.mkdir(parents=True, exist_ok=True)
        shutil.copy2(png, META_FIG / png.name)
        shutil.copy2(out_dir / "dataset_distributions.json", META_FIG / "dataset_distributions.json")
        print(f"also copied to {META_FIG}", flush=True)

    print(f"wrote {png}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
