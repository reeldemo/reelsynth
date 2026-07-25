#!/usr/bin/env python3
"""Paper figures: wrap-seam heal overlays for the meta-approach 5k Ours champion.

Loads champion arch/HP from meta_approach_compare (default: hybrid_lstm / Ours),
refits FitCell (search seed 1902771841), scores absolute R on holdout seed 20260719,
and plots prolonged waveforms + seam zoom + spectrograms vs ideal / no-bake / DualCosine.

Writes:
  denoise-opt-meta/paper/v7/figures/fig_meta_heal_samples.{png,pdf,json}
  brand/artifacts/meta_approach_compare/fig_meta_heal_samples.png (mirror)
"""
from __future__ import annotations

import argparse
import json
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

EVAL_SEED = 20260719
SEARCH_SEED = 1902771841
TILE_FALLBACK = 46
PERIODS = 3
V7_FIG = ROOT.parent / "denoise-opt-meta" / "paper" / "v7" / "figures"
META_DIR = ROOT / "brand" / "artifacts" / "meta_approach_compare"

C_IDEAL = "#000000"
C_ENGINE = "#D55E00"
C_DUAL = "#0072B2"
C_OURS = "#009E73"
C_FIR = "#CC79A7"


def prolong(cycle: torch.Tensor, periods: int = PERIODS) -> np.ndarray:
    return og.prolong_tile(cycle.unsqueeze(0), periods=periods)[0].detach().cpu().numpy()


def seam_fir3(frames: torch.Tensor) -> torch.Tensor:
    """Lightweight classical FIR bake (same family as board rows)."""
    k = torch.tensor([0.25, 0.5, 0.25], device=frames.device, dtype=frames.dtype).view(1, 1, 3)
    x = frames.unsqueeze(1)
    y = torch.nn.functional.conv1d(x, k, padding=1).squeeze(1)
    w = og.SEAM_W
    out = frames.clone()
    out[:, :w] = y[:, :w]
    out[:, -w:] = y[:, -w:]
    return out


def load_holdout(device: torch.device) -> tuple[torch.Tensor, torch.Tensor, str]:
    holdout = ROOT / "brand" / "artifacts" / "canonical_eval_dataset" / "holdout_batch.pt"
    if holdout.is_file():
        blob = torch.load(holdout, map_location="cpu", weights_only=False)
        return blob["ideal"].to(device).float(), blob["engine"].to(device).float(), "holdout_batch.pt"
    torch.manual_seed(EVAL_SEED)
    ideal, eng = og.make_batch(64, og.N, device)
    return ideal, eng, "make_batch_seed_20260719"


def pick_tile(eng: torch.Tensor) -> tuple[int, str]:
    wrap = (eng[:, 0] - eng[:, -1]).abs()
    idx = int(wrap.argmax().item())
    note = "max_wrap"
    if TILE_FALLBACK < eng.shape[0] and float(wrap[TILE_FALLBACK]) >= 0.9 * float(wrap[idx]):
        idx = TILE_FALLBACK
        note = "prefer_intro_tile46"
    return idx, note


def spectrogram_db(y: np.ndarray, n_fft: int = 64) -> np.ndarray:
    """Simple |STFT| in dB for short tiled period display."""
    win = np.hanning(n_fft).astype(np.float64)
    hop = n_fft // 4
    if len(y) < n_fft:
        y = np.pad(y, (0, n_fft - len(y)))
    frames = []
    for i in range(0, len(y) - n_fft + 1, hop):
        frames.append(np.fft.rfft(y[i : i + n_fft] * win))
    mag = np.abs(np.stack(frames, axis=1)) + 1e-8
    return 20.0 * np.log10(mag)


def refit_champ(
    summary: dict,
    device: torch.device,
) -> tuple[og.ArchConfig, og.SeamCell, float, dict]:
    arch = summary["champ_arch"]
    hp = summary.get("champ_hp") or {}
    cfg = og.ArchConfig.from_dict(arch)
    cell = og.SeamCell(cfg).to(device)
    torch.manual_seed(SEARCH_SEED)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(SEARCH_SEED)
    fit_steps = int(hp.get("fit_steps", 64))
    batch = int(hp.get("batch", 64))
    lr = float(hp.get("lr", 3e-3))
    adv = float(hp.get("adv_coef", 0.0))
    train_r, converged = og.fit_cell(
        cell, cfg.ops, device, steps=fit_steps, batch=batch, lr=lr, adv_coef=adv
    )
    cell.eval()
    return cfg, cell, float(train_r), {
        "fit_steps": fit_steps,
        "batch": batch,
        "lr": lr,
        "adv_coef": adv,
        "converged": bool(converged),
        "train_r_last": float(train_r),
    }


@torch.no_grad()
def score_batch(ideal: torch.Tensor, out: torch.Tensor) -> float:
    return float(og.residual_score(ideal, out).mean().item())


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--approach", type=str, default="hybrid_lstm")
    ap.add_argument("--device", type=str, default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--meta-dir", type=Path, default=META_DIR)
    args = ap.parse_args()

    device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")
    summary_path = args.meta_dir / args.approach / "summary.json"
    if not summary_path.is_file():
        raise SystemExit(f"missing champion summary: {summary_path}")
    summary = json.loads(summary_path.read_text(encoding="utf-8"))

    cfg, cell, train_r, fit_meta = refit_champ(summary, device)
    ideal_b, eng_b, hold_note = load_holdout(device)
    idx, pick_note = pick_tile(eng_b)
    ideal = ideal_b[idx : idx + 1]
    eng = eng_b[idx : idx + 1]

    dual = og.dual_cosine_blend(eng)
    fir = seam_fir3(eng)
    ours = og.apply_ops(eng, cell, cfg.ops)

    # Tile scores (absolute R vs ideal sibling)
    scores_tile = {
        "no_bake": score_batch(ideal, eng),
        "dual_cosine": score_batch(ideal, dual),
        "seam_fir3": score_batch(ideal, fir),
        "ours_hybrid": score_batch(ideal, ours),
    }
    # Holdout batch means (honest board comparison on full holdout)
    dual_b = og.dual_cosine_blend(eng_b)
    fir_b = seam_fir3(eng_b)
    ours_b = og.apply_ops(eng_b, cell, cfg.ops)
    scores_holdout = {
        "no_bake": score_batch(ideal_b, eng_b),
        "dual_cosine": score_batch(ideal_b, dual_b),
        "seam_fir3": score_batch(ideal_b, fir_b),
        "ours_hybrid": score_batch(ideal_b, ours_b),
        "n": int(ideal_b.shape[0]),
    }

    L = int(ideal.shape[-1])
    x = np.arange(L * PERIODS)
    y_ideal = prolong(ideal[0])
    y_eng = prolong(eng[0])
    y_dual = prolong(dual[0])
    y_ours = prolong(ours[0])
    wrap_abs = float((eng[0, 0] - eng[0, -1]).abs().item())

    fig = plt.figure(figsize=(11.0, 7.2))
    gs = fig.add_gridspec(3, 2, height_ratios=[1.05, 0.85, 1.0], hspace=0.38, wspace=0.22)

    ax_full = fig.add_subplot(gs[0, :])
    ax_full.plot(x, y_ideal, color=C_IDEAL, lw=1.2, label="ideal sibling $r^*$")
    ax_full.plot(x, y_eng, color=C_ENGINE, lw=1.15, ls="--", label=f"no-bake $R$={scores_tile['no_bake']:.4f}")
    ax_full.plot(x, y_dual, color=C_DUAL, lw=1.2, ls="-.", label=f"DualCosine $R$={scores_tile['dual_cosine']:.4f}")
    ax_full.plot(x, y_ours, color=C_OURS, lw=1.45, label=f"Ours (hybrid GA–PPO) $R$={scores_tile['ours_hybrid']:.4f}")
    for k in range(1, PERIODS):
        ax_full.axvline(k * L - 0.5, color="#888888", lw=0.7, ls=":", alpha=0.75)
    ax_full.axvspan(L - 24, L + 24, color="#F0E442", alpha=0.16, zorder=0)
    ax_full.set_title(
        f"Healed wrap seam | holdout seed={EVAL_SEED}, tile={idx}, |wrap|={wrap_abs:.3f} | "
        f"search seed={SEARCH_SEED}, refit Ours champ",
        fontsize=10,
    )
    ax_full.set_xlabel("sample (tiled periods)")
    ax_full.set_ylabel("amplitude")
    ax_full.legend(loc="upper right", fontsize=8, frameon=False, ncol=2)
    ax_full.grid(True, alpha=0.25)
    ax_full.set_xlim(0, L * PERIODS - 1)

    # Seam zoom around first wrap
    ax_zoom = fig.add_subplot(gs[1, 0])
    lo, hi = L - 40, L + 40
    ax_zoom.plot(x[lo:hi], y_ideal[lo:hi], color=C_IDEAL, lw=1.3, label="ideal")
    ax_zoom.plot(x[lo:hi], y_eng[lo:hi], color=C_ENGINE, lw=1.2, ls="--", label="no-bake")
    ax_zoom.plot(x[lo:hi], y_dual[lo:hi], color=C_DUAL, lw=1.2, ls="-.", label="DualCosine")
    ax_zoom.plot(x[lo:hi], y_ours[lo:hi], color=C_OURS, lw=1.5, label="Ours")
    ax_zoom.axvline(L - 0.5, color="#666666", lw=0.9, ls=":")
    ax_zoom.set_title("(b) Seam zoom (first wrap)")
    ax_zoom.set_xlabel("sample")
    ax_zoom.set_ylabel("amplitude")
    ax_zoom.grid(True, alpha=0.25)
    ax_zoom.legend(fontsize=7, frameon=False)

    # Classical board bars on this tile
    ax_bar = fig.add_subplot(gs[1, 1])
    names = ["no-bake", "seam_fir3", "DualCosine", "Ours"]
    vals = [
        scores_tile["no_bake"],
        scores_tile["seam_fir3"],
        scores_tile["dual_cosine"],
        scores_tile["ours_hybrid"],
    ]
    colors = [C_ENGINE, C_FIR, C_DUAL, C_OURS]
    bars = ax_bar.bar(np.arange(4), vals, color=colors, edgecolor="#333333", width=0.7)
    for b, h in zip(bars, ["", "//", "\\\\", "xx"]):
        b.set_hatch(h)
    ax_bar.set_xticks(np.arange(4))
    ax_bar.set_xticklabels(names, fontsize=8)
    ax_bar.set_ylabel("Tile prolonged $R$")
    ax_bar.set_title("(c) Absolute $R$ on this tile (classical board)")
    ax_bar.set_ylim(min(vals) - 0.02, 1.0)
    ax_bar.grid(True, axis="y", alpha=0.28)
    for i, v in enumerate(vals):
        ax_bar.text(i, v + 0.003, f"{v:.3f}", ha="center", va="bottom", fontsize=7.5)

    # Spectrograms: cracked vs healed
    ax_sp0 = fig.add_subplot(gs[2, 0])
    ax_sp1 = fig.add_subplot(gs[2, 1])
    sp_eng = spectrogram_db(y_eng)
    sp_ours = spectrogram_db(y_ours)
    vmin = min(sp_eng.min(), sp_ours.min())
    vmax = max(sp_eng.max(), sp_ours.max())
    im0 = ax_sp0.imshow(sp_eng, origin="lower", aspect="auto", cmap="magma", vmin=vmin, vmax=vmax)
    im1 = ax_sp1.imshow(sp_ours, origin="lower", aspect="auto", cmap="magma", vmin=vmin, vmax=vmax)
    ax_sp0.set_title("(d) Spectrogram: no-bake (cracked)")
    ax_sp1.set_title("(e) Spectrogram: Ours healed")
    for ax in (ax_sp0, ax_sp1):
        ax.set_xlabel("frame")
        ax.set_ylabel("freq bin")
    fig.colorbar(im0, ax=ax_sp0, fraction=0.046, pad=0.02, label="dB")
    fig.colorbar(im1, ax=ax_sp1, fraction=0.046, pad=0.02, label="dB")

    fig.suptitle(
        "DenoiseOpt wrap heal | objective = max absolute $R$ toward ideal sibling "
        f"(holdout mean Ours $R$={scores_holdout['ours_hybrid']:.4f}, "
        f"DualCosine={scores_holdout['dual_cosine']:.4f})",
        fontsize=10,
        y=0.995,
    )

    V7_FIG.mkdir(parents=True, exist_ok=True)
    out_png = V7_FIG / "fig_meta_heal_samples.png"
    out_pdf = V7_FIG / "fig_meta_heal_samples.pdf"
    out_json = V7_FIG / "fig_meta_heal_samples.json"
    fig.savefig(out_png, dpi=200, bbox_inches="tight")
    fig.savefig(out_pdf, bbox_inches="tight")
    plt.close(fig)
    args.meta_dir.mkdir(parents=True, exist_ok=True)
    mirror = args.meta_dir / "fig_meta_heal_samples.png"
    import shutil

    shutil.copy2(out_png, mirror)

    meta = {
        "schema": "denoiseopt.meta_heal_samples.v1",
        "approach_code": args.approach,
        "approach_display": "Ours (hybrid GA–PPO)",
        "eval_seed": EVAL_SEED,
        "search_seed": SEARCH_SEED,
        "tile_index": idx,
        "wrap_abs": wrap_abs,
        "holdout_source": hold_note,
        "pick_note": pick_note,
        "champ_raw_from_summary": summary.get("champ_raw"),
        "delta_r_vs_dual_cosine_summary": summary.get("delta_r_vs_dual_cosine"),
        "refit": fit_meta,
        "train_r_last": train_r,
        "champ_arch": cfg.to_dict(),
        "R_tile": scores_tile,
        "R_holdout_mean": scores_holdout,
        "note": (
            "Cell weights are refit from champion arch+HP (meta suite does not persist "
            "fitted state_dict). Tile R is holdout geometry; summary champ_raw is search-time "
            "FitCell score. Rank by absolute R vs classical board; DualCosine ΔR is reporting only."
        ),
        "png": str(out_png),
        "pdf": str(out_pdf),
    }
    out_json.write_text(json.dumps(meta, indent=2), encoding="utf-8")
    print(json.dumps(meta, indent=2))
    print(f"wrote {out_png}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
