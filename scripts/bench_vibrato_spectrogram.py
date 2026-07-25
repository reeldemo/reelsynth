#!/usr/bin/env python3
"""Vibrato / dynamic-pitch spectrogram eval for DenoiseOpt paper v8 (W2 / AC-3.1).

Renders prolonged cracked (no-bake) vs DualCosine vs Ours under slow vibrato
(modulated wavetable playback), emits spectrogram + difference figures, and
reports mean prolonged R on the cycle board plus modulated-playback residual.

Writes:
  brand/artifacts/vibrato_spectrogram/
  denoise-opt-meta/paper/Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9/figures/fig_vibrato_spectrogram.{png,pdf,json}

Does NOT touch brand/artifacts/meta_approach_compare/ contents (read-only champ).
"""
from __future__ import annotations

import argparse
import json
import math
import shutil
import sys
import wave
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np
import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
from plot_meta_heal_samples import (  # noqa: E402
    EVAL_SEED,
    META_DIR,
    SEARCH_SEED,
    load_holdout,
    pick_tile,
    refit_champ,
    score_batch,
    spectrogram_db,
)

OUT_DIR = ROOT / "brand" / "artifacts" / "vibrato_spectrogram"
V8_FIG = ROOT.parent / "denoise-opt-meta" / "paper" / "Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9" / "figures"

C_ENGINE = "#D55E00"
C_DUAL = "#0072B2"
C_OURS = "#009E73"
C_IDEAL = "#000000"

SR = 44100
BASE_FREQ_HZ = 220.0
VIBRATO_RATE_HZ = 5.0
VIBRATO_DEPTH = 0.03  # ±3% pitch
DURATION_S = 2.0
N_FFT = 1024
HOP = 256


def write_wav_mono(path: Path, samples: np.ndarray, sr: int = SR) -> None:
    x = np.asarray(samples, dtype=np.float64)
    peak = float(np.max(np.abs(x))) if x.size else 0.0
    if peak > 1e-12:
        x = x / peak * 0.89
    pcm = np.clip(x * 32767.0, -32768, 32767).astype(np.int16)
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as wf:
        wf.setnchannels(1)
        wf.setsampwidth(2)
        wf.setframerate(sr)
        wf.writeframes(pcm.tobytes())


def render_vibrato(
    cycle: np.ndarray,
    *,
    sr: int,
    base_hz: float,
    duration_s: float,
    vib_rate_hz: float,
    vib_depth: float,
) -> np.ndarray:
    """Linear-interpolated wavetable playback with sinusoidal pitch vibrato."""
    table = np.asarray(cycle, dtype=np.float64).reshape(-1)
    n = len(table)
    n_out = int(round(sr * duration_s))
    t = np.arange(n_out, dtype=np.float64) / float(sr)
    freq = base_hz * (1.0 + vib_depth * np.sin(2.0 * math.pi * vib_rate_hz * t))
    phase_inc = (n * freq) / float(sr)
    phase = np.cumsum(phase_inc) % n
    idx = np.floor(phase).astype(np.int64)
    frac = phase - idx
    a = table[idx]
    b = table[(idx + 1) % n]
    return a + frac * (b - a)


def modulated_residual_r(ideal_audio: np.ndarray, out_audio: np.ndarray) -> float:
    """Playback-domain analogue of prolonged R under identical vibrato envelope."""
    resid = out_audio - ideal_audio
    residual_rms = float(np.sqrt(np.mean(resid * resid)))
    ideal_rms = float(np.sqrt(np.mean(ideal_audio * ideal_audio)))
    if ideal_rms < 1e-12:
        return 0.0
    return float(np.clip(1.0 - residual_rms / ideal_rms, 0.0, 1.0))


def stft_db(y: np.ndarray, n_fft: int = N_FFT, hop: int = HOP) -> np.ndarray:
    win = np.hanning(n_fft).astype(np.float64)
    if len(y) < n_fft:
        y = np.pad(y, (0, n_fft - len(y)))
    frames = []
    for i in range(0, len(y) - n_fft + 1, hop):
        frames.append(np.fft.rfft(y[i : i + n_fft] * win))
    mag = np.abs(np.stack(frames, axis=1)) + 1e-10
    return 20.0 * np.log10(mag)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--approach", type=str, default="hybrid_lstm")
    ap.add_argument("--device", type=str, default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--meta-dir", type=Path, default=META_DIR)
    ap.add_argument("--out-dir", type=Path, default=OUT_DIR)
    ap.add_argument("--paper-fig-dir", type=Path, default=V8_FIG)
    ap.add_argument("--sr", type=int, default=SR)
    ap.add_argument("--base-hz", type=float, default=BASE_FREQ_HZ)
    ap.add_argument("--vib-rate", type=float, default=VIBRATO_RATE_HZ)
    ap.add_argument("--vib-depth", type=float, default=VIBRATO_DEPTH)
    ap.add_argument("--duration", type=float, default=DURATION_S)
    ap.add_argument("--tile", type=int, default=-1, help="Holdout tile index (-1 = pick_tile)")
    args = ap.parse_args()

    out_resolved = args.out_dir.resolve()
    forbidden = (ROOT / "brand" / "artifacts" / "meta_approach_compare").resolve()
    if out_resolved == forbidden or forbidden in out_resolved.parents:
        raise SystemExit(
            f"refusing --out-dir under meta_approach_compare/: {out_resolved}"
        )

    device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")
    summary_path = args.meta_dir / args.approach / "summary.json"
    if not summary_path.is_file():
        raise SystemExit(f"missing champion summary: {summary_path}")
    summary = json.loads(summary_path.read_text(encoding="utf-8"))

    print(f"refitting champion from {summary_path} on {device} …")
    cfg, cell, train_r, fit_meta = refit_champ(summary, device)
    ideal_b, eng_b, hold_note = load_holdout(device)

    if args.tile >= 0:
        idx = int(args.tile)
        pick_note = f"cli_tile_{idx}"
    else:
        idx, pick_note = pick_tile(eng_b)

    ideal = ideal_b[idx : idx + 1]
    eng = eng_b[idx : idx + 1]
    dual = og.dual_cosine_blend(eng)
    with torch.no_grad():
        ours = og.apply_ops(eng, cell, cfg.ops)

    scores_cycle = {
        "no_bake": score_batch(ideal, eng),
        "dual_cosine": score_batch(ideal, dual),
        "ours_hybrid": score_batch(ideal, ours),
    }

    # Holdout batch means under cycle R (unchanged by vibrato; reported for context)
    dual_b = og.dual_cosine_blend(eng_b)
    with torch.no_grad():
        ours_b = og.apply_ops(eng_b, cell, cfg.ops)
    scores_holdout_cycle = {
        "no_bake": score_batch(ideal_b, eng_b),
        "dual_cosine": score_batch(ideal_b, dual_b),
        "ours_hybrid": score_batch(ideal_b, ours_b),
        "n": int(ideal_b.shape[0]),
    }

    cycles = {
        "ideal": ideal[0].detach().cpu().numpy(),
        "nobake": eng[0].detach().cpu().numpy(),
        "dualcosine": dual[0].detach().cpu().numpy(),
        "ours": ours[0].detach().cpu().numpy(),
    }
    vib_kw = dict(
        sr=args.sr,
        base_hz=args.base_hz,
        duration_s=args.duration,
        vib_rate_hz=args.vib_rate,
        vib_depth=args.vib_depth,
    )
    audio = {k: render_vibrato(v, **vib_kw) for k, v in cycles.items()}
    scores_mod = {
        "no_bake": modulated_residual_r(audio["ideal"], audio["nobake"]),
        "dual_cosine": modulated_residual_r(audio["ideal"], audio["dualcosine"]),
        "ours_hybrid": modulated_residual_r(audio["ideal"], audio["ours"]),
    }

    # Mean modulated R across a few high-wrap holdout tiles
    wrap = (eng_b[:, 0] - eng_b[:, -1]).abs().detach().cpu().numpy()
    top_idx = list(np.argsort(-wrap)[:8])
    if idx not in top_idx:
        top_idx = [idx] + top_idx[:7]
    mod_acc = {"no_bake": [], "dual_cosine": [], "ours_hybrid": []}
    with torch.no_grad():
        for ti in top_idx:
            id_t = ideal_b[ti : ti + 1]
            en_t = eng_b[ti : ti + 1]
            du_t = og.dual_cosine_blend(en_t)
            ou_t = og.apply_ops(en_t, cell, cfg.ops)
            id_a = render_vibrato(id_t[0].detach().cpu().numpy(), **vib_kw)
            mod_acc["no_bake"].append(
                modulated_residual_r(id_a, render_vibrato(en_t[0].detach().cpu().numpy(), **vib_kw))
            )
            mod_acc["dual_cosine"].append(
                modulated_residual_r(id_a, render_vibrato(du_t[0].detach().cpu().numpy(), **vib_kw))
            )
            mod_acc["ours_hybrid"].append(
                modulated_residual_r(id_a, render_vibrato(ou_t[0].detach().cpu().numpy(), **vib_kw))
            )
    scores_mod_mean = {k: float(np.mean(v)) for k, v in mod_acc.items()}
    scores_mod_std = {k: float(np.std(v)) for k, v in mod_acc.items()}

    args.out_dir.mkdir(parents=True, exist_ok=True)
    wav_names = {}
    for key, arr in (("nobake", audio["nobake"]), ("dualcosine", audio["dualcosine"]), ("ours", audio["ours"]), ("ideal", audio["ideal"])):
        name = f"tile{idx}_vibrato_{key}.wav"
        write_wav_mono(args.out_dir / name, arr, sr=args.sr)
        wav_names[key] = name

    # --- Figure ---
    sp = {k: stft_db(audio[k]) for k in ("nobake", "dualcosine", "ours", "ideal")}
    vmin = min(sp[k].min() for k in ("nobake", "dualcosine", "ours"))
    vmax = max(sp[k].max() for k in ("nobake", "dualcosine", "ours"))
    diff_ours = sp["ours"] - sp["nobake"]
    diff_dual = sp["dualcosine"] - sp["nobake"]
    dlim = float(np.percentile(np.abs(np.concatenate([diff_ours.ravel(), diff_dual.ravel()])), 99))
    dlim = max(dlim, 1.0)

    fig = plt.figure(figsize=(11.2, 8.4))
    gs = fig.add_gridspec(3, 3, height_ratios=[1.0, 1.0, 0.85], hspace=0.42, wspace=0.28)

    titles = [
        ("(a) no-bake (cracked)", "nobake", C_ENGINE),
        ("(b) DualCosine", "dualcosine", C_DUAL),
        ("(c) Ours (hybrid GA–PPO)", "ours", C_OURS),
    ]
    for col, (title, key, _c) in enumerate(titles):
        ax = fig.add_subplot(gs[0, col])
        im = ax.imshow(sp[key], origin="lower", aspect="auto", cmap="magma", vmin=vmin, vmax=vmax)
        ax.set_title(title, fontsize=9)
        ax.set_xlabel("frame")
        if col == 0:
            ax.set_ylabel("freq bin")
        fig.colorbar(im, ax=ax, fraction=0.046, pad=0.03, label="dB")

    ax_d0 = fig.add_subplot(gs[1, 0])
    ax_d1 = fig.add_subplot(gs[1, 1])
    ax_wave = fig.add_subplot(gs[1, 2])
    im0 = ax_d0.imshow(diff_dual, origin="lower", aspect="auto", cmap="coolwarm", vmin=-dlim, vmax=dlim)
    im1 = ax_d1.imshow(diff_ours, origin="lower", aspect="auto", cmap="coolwarm", vmin=-dlim, vmax=dlim)
    ax_d0.set_title("(d) DualCosine − no-bake (dB)", fontsize=9)
    ax_d1.set_title("(e) Ours − no-bake (dB)", fontsize=9)
    for ax, im in ((ax_d0, im0), (ax_d1, im1)):
        ax.set_xlabel("frame")
        ax.set_ylabel("freq bin")
        fig.colorbar(im, ax=ax, fraction=0.046, pad=0.03)

    # Short waveform excerpt (~4 vibrato periods worth of samples at start)
    n_show = min(len(audio["ideal"]), int(args.sr * 0.08))
    t = np.arange(n_show) / args.sr * 1000.0
    ax_wave.plot(t, audio["ideal"][:n_show], color=C_IDEAL, lw=0.9, label="ideal")
    ax_wave.plot(t, audio["nobake"][:n_show], color=C_ENGINE, lw=0.85, ls="--", label="no-bake")
    ax_wave.plot(t, audio["ours"][:n_show], color=C_OURS, lw=1.0, label="Ours")
    ax_wave.set_title("(f) Waveform excerpt (ms)", fontsize=9)
    ax_wave.set_xlabel("time (ms)")
    ax_wave.set_ylabel("amp")
    ax_wave.legend(fontsize=7, frameon=False, loc="upper right")
    ax_wave.grid(True, alpha=0.25)

    ax_bar = fig.add_subplot(gs[2, :])
    labels = ["no-bake", "DualCosine", "Ours"]
    cycle_vals = [scores_cycle["no_bake"], scores_cycle["dual_cosine"], scores_cycle["ours_hybrid"]]
    mod_vals = [scores_mod["no_bake"], scores_mod["dual_cosine"], scores_mod["ours_hybrid"]]
    mod_mean = [scores_mod_mean["no_bake"], scores_mod_mean["dual_cosine"], scores_mod_mean["ours_hybrid"]]
    x = np.arange(3)
    w = 0.25
    b0 = ax_bar.bar(x - w, cycle_vals, width=w, color=[C_ENGINE, C_DUAL, C_OURS], edgecolor="#333", label="cycle $R$ (tile)")
    b1 = ax_bar.bar(x, mod_vals, width=w, color=[C_ENGINE, C_DUAL, C_OURS], alpha=0.55, edgecolor="#333", hatch="//", label="modulated $R$ (tile)")
    b2 = ax_bar.bar(x + w, mod_mean, width=w, color=[C_ENGINE, C_DUAL, C_OURS], alpha=0.35, edgecolor="#333", hatch="\\\\", label=f"modulated $R$ mean (n={len(top_idx)})")
    ax_bar.set_xticks(x)
    ax_bar.set_xticklabels(labels)
    ax_bar.set_ylabel("$R$ (higher better)")
    ax_bar.set_ylim(min(cycle_vals + mod_vals + mod_mean) - 0.05, 1.02)
    ax_bar.legend(fontsize=8, frameon=False, ncol=3, loc="lower right")
    ax_bar.grid(True, axis="y", alpha=0.28)
    ax_bar.set_title(
        f"(g) Absolute $R$ | vibrato rate={args.vib_rate} Hz, depth=±{100 * args.vib_depth:.1f}%, "
        f"base={args.base_hz} Hz | tile={idx}",
        fontsize=9,
    )
    for bars in (b0, b1, b2):
        for bar in bars:
            h = bar.get_height()
            ax_bar.text(bar.get_x() + bar.get_width() / 2, h + 0.004, f"{h:.3f}", ha="center", va="bottom", fontsize=6.5)

    wrap_abs = float((eng[0, 0] - eng[0, -1]).abs().item())
    fig.suptitle(
        "DenoiseOpt under slow vibrato playback | "
        f"holdout seed={EVAL_SEED}, tile={idx}, |wrap|={wrap_abs:.3f} | "
        f"search seed={SEARCH_SEED}",
        fontsize=10,
        y=0.995,
    )

    png = args.out_dir / "fig_vibrato_spectrogram.png"
    pdf = args.out_dir / "fig_vibrato_spectrogram.pdf"
    fig.savefig(png, dpi=200, bbox_inches="tight")
    fig.savefig(pdf, bbox_inches="tight")
    plt.close(fig)

    args.paper_fig_dir.mkdir(parents=True, exist_ok=True)
    for src in (png, pdf):
        shutil.copy2(src, args.paper_fig_dir / src.name)

    meta = {
        "schema": "denoiseopt.vibrato_spectrogram.v1",
        "approach_code": args.approach,
        "approach_display": "Ours (hybrid GA–PPO)",
        "eval_seed": EVAL_SEED,
        "search_seed": SEARCH_SEED,
        "tile_index": idx,
        "pick_note": pick_note,
        "wrap_abs": wrap_abs,
        "holdout_source": hold_note,
        "playback": {
            "sample_rate": args.sr,
            "base_hz": args.base_hz,
            "vibrato_rate_hz": args.vib_rate,
            "vibrato_depth": args.vib_depth,
            "duration_s": args.duration,
            "n_fft": N_FFT,
            "hop": HOP,
        },
        "R_cycle_tile": scores_cycle,
        "R_cycle_holdout_mean": scores_holdout_cycle,
        "R_modulated_tile": scores_mod,
        "R_modulated_mean_top_wrap": scores_mod_mean,
        "R_modulated_std_top_wrap": scores_mod_std,
        "modulated_tile_indices": [int(i) for i in top_idx],
        "refit": fit_meta,
        "train_r_last": train_r,
        "champ_arch": cfg.to_dict(),
        "wav_files": wav_names,
        "png": str(png.resolve()),
        "pdf": str(pdf.resolve()),
        "paper_copies": [
            str((args.paper_fig_dir / "fig_vibrato_spectrogram.png").resolve()),
            str((args.paper_fig_dir / "fig_vibrato_spectrogram.pdf").resolve()),
        ],
        "note": (
            "Cycle R is the standard prolonged residual on L=256 periods. "
            "Modulated R applies the same clamp(1 - rms(out-ideal)/rms(ideal)) formula "
            "to vibrato-rendered audio under a shared pitch envelope. "
            "No formal listening study; spectrogram + downloadable WAVs only."
        ),
    }
    json_path = args.out_dir / "results.json"
    json_path.write_text(json.dumps(meta, indent=2), encoding="utf-8")
    shutil.copy2(json_path, args.paper_fig_dir / "fig_vibrato_spectrogram.json")

    readme = f"""# Vibrato / dynamic-pitch spectrogram (paper v8 W2)

Slow vibrato playback of cracked / DualCosine / Ours on holdout tile **{idx}**.

- Sample rate: **{args.sr} Hz**
- Base pitch: **{args.base_hz} Hz**
- Vibrato: **{args.vib_rate} Hz** rate, **±{100 * args.vib_depth:.1f}%** depth
- Duration: **{args.duration} s**
- Seeds: eval **{EVAL_SEED}**, search/refit **{SEARCH_SEED}**

Figures: `fig_vibrato_spectrogram.{{png,pdf}}` (mirrored to `paper/Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9/figures/`).

Rebuild:

```bash
.venv_gpu/Scripts/python.exe scripts/bench_vibrato_spectrogram.py --approach hybrid_lstm
```
"""
    (args.out_dir / "README.md").write_text(readme, encoding="utf-8")

    # Short cycle spectrogram panel reuse of plot_meta helper (sanity)
    _ = spectrogram_db  # imported for API parity / future short-tile panels

    print(json.dumps({k: meta[k] for k in ("R_cycle_tile", "R_modulated_tile", "R_modulated_mean_top_wrap", "png")}, indent=2))
    print(f"wrote {png}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
