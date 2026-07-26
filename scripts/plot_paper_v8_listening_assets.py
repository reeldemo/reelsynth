#!/usr/bin/env python3
"""Paper v8 listening / diversity panels from existing artifacts (W2 / AC-3.2, AC-3.3 gallery).

Builds:
  1) fig_hear_samples_panel — waveform strips from meta_approach_compare/hear_samples WAVs
  2) fig_wt_diversity_gallery — compact gallery from ReelSynth export + AKWF OA cycles

No new NAS. Does not modify/wipe meta_approach_compare/ (read-only).

Writes under brand/artifacts/paper_v8_listening/ and copies to
denoise-opt-meta paper v9/v10 figures folders.
"""
from __future__ import annotations

import argparse
import json
import shutil
import sys
import wave
from pathlib import Path

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
HEAR_DIR = ROOT / "brand" / "artifacts" / "meta_approach_compare" / "hear_samples"
EXPORT_JSON = ROOT / "brand" / "artifacts" / "real_wt_cycles" / "reelsynth_export_cycles.json"
AKWF_DIR = ROOT / "brand" / "artifacts" / "real_wt_cycles" / "oa_akwf"
OUT_DIR = ROOT / "brand" / "artifacts" / "paper_v8_listening"
META_PAPER = ROOT.parent / "denoise-opt-meta" / "paper"
V8_FIG = (
    META_PAPER
    / "Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9"
    / "figures"
)
V10_FIG = (
    META_PAPER
    / "Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v10"
    / "figures"
)

C_ENGINE = "#D55E00"
C_DUAL = "#0072B2"
C_OURS = "#009E73"


def read_wav_mono(path: Path) -> tuple[np.ndarray, int]:
    with wave.open(str(path), "rb") as wf:
        sr = wf.getframerate()
        n = wf.getnframes()
        raw = wf.readframes(n)
        sampwidth = wf.getsampwidth()
    if sampwidth == 2:
        x = np.frombuffer(raw, dtype=np.int16).astype(np.float64) / 32768.0
    else:
        raise ValueError(f"unsupported sampwidth {sampwidth} in {path}")
    return x, sr


def load_akwf_cycle(path: Path, L: int = 256) -> np.ndarray:
    x, _sr = read_wav_mono(path)
    if len(x) == L:
        y = x.astype(np.float64)
    else:
        # Linear resample to L
        t_old = np.linspace(0.0, 1.0, num=len(x), endpoint=False)
        t_new = np.linspace(0.0, 1.0, num=L, endpoint=False)
        y = np.interp(t_new, t_old, x.astype(np.float64))
    peak = float(np.max(np.abs(y))) if y.size else 0.0
    if peak > 1e-12:
        y = y / peak
    return y


def copy_to_paper(src: Path, *paper_dirs: Path) -> Path:
    dst = src
    for paper_dir in paper_dirs:
        if paper_dir is None:
            continue
        paper_dir.mkdir(parents=True, exist_ok=True)
        dst = paper_dir / src.name
        shutil.copy2(src, dst)
    return dst


def plot_hear_panel(hear_dir: Path, out_dir: Path, *paper_dirs: Path) -> dict:
    man_path = hear_dir / "manifest.json"
    if not man_path.is_file():
        raise SystemExit(f"missing hear manifest: {man_path}")
    man = json.loads(man_path.read_text(encoding="utf-8"))
    samples = man["samples"]
    n = len(samples)
    fig, axes = plt.subplots(n, 3, figsize=(10.5, 1.35 * n), sharex=True)
    if n == 1:
        axes = np.array([axes])
    col_titles = ["no-bake", "DualCosine", "Ours healed"]
    keys = ["nobake", "dualcosine", "ours_healed"]
    colors = [C_ENGINE, C_DUAL, C_OURS]
    for row, entry in enumerate(samples):
        for col, (key, title, color) in enumerate(zip(keys, col_titles, colors)):
            ax = axes[row, col]
            path = hear_dir / entry["files"][key]
            audio, sr = read_wav_mono(path)
            # Show first ~25 ms so wrap clicks are visible as dense structure
            n_show = min(len(audio), int(sr * 0.025))
            t = np.arange(n_show) / sr * 1000.0
            ax.plot(t, audio[:n_show], color=color, lw=0.7)
            ax.set_ylim(-1.05, 1.05)
            ax.grid(True, alpha=0.2)
            if row == 0:
                ax.set_title(title, fontsize=9)
            if col == 0:
                ax.set_ylabel(f"tile {entry['tile_index']}\n$R$={entry['R']['ours_hybrid']:.3f}", fontsize=7.5)
            if row == n - 1:
                ax.set_xlabel("ms")
            # Tiny R annotations
            r_key = {"nobake": "no_bake", "dualcosine": "dual_cosine", "ours_healed": "ours_hybrid"}[key]
            ax.text(
                0.98,
                0.08,
                f"R={entry['R'][r_key]:.3f}",
                transform=ax.transAxes,
                ha="right",
                va="bottom",
                fontsize=6.5,
                color="#222222",
            )
    fig.suptitle(
        "Audible wrap-seam demos (fixed A4 playback) — downloadable WAVs under "
        "reelsynth/brand/artifacts/meta_approach_compare/hear_samples/",
        fontsize=9.5,
        y=1.01,
    )
    fig.tight_layout()
    png = out_dir / "fig_hear_samples_panel.png"
    pdf = out_dir / "fig_hear_samples_panel.pdf"
    fig.savefig(png, dpi=200, bbox_inches="tight")
    fig.savefig(pdf, bbox_inches="tight")
    plt.close(fig)
    copy_to_paper(png, *paper_dirs)
    copy_to_paper(pdf, *paper_dirs)
    meta = {
        "schema": "denoiseopt.hear_samples_panel.v1",
        "hear_dir": str(hear_dir.resolve()),
        "manifest": str(man_path.resolve()),
        "n_samples": n,
        "sample_rate": man.get("sample_rate"),
        "freq_hz": man.get("freq_hz"),
        "duration_s": man.get("duration_s"),
        "eval_seed": man.get("eval_seed"),
        "search_seed": man.get("search_seed"),
        "tiles": [s["tile_index"] for s in samples],
        "rebuild": "scripts/export_meta_hear_samples.py --approach hybrid_lstm",
        "png": str(png.resolve()),
        "pdf": str(pdf.resolve()),
        "note": "Paper panel only; not a formal listening / A/B study.",
    }
    jp = out_dir / "fig_hear_samples_panel.json"
    jp.write_text(json.dumps(meta, indent=2), encoding="utf-8")
    copy_to_paper(jp, *paper_dirs)
    return meta


def plot_wt_gallery(export_json: Path, akwf_dir: Path, out_dir: Path, *paper_dirs: Path) -> dict:
    blob = json.loads(export_json.read_text(encoding="utf-8"))
    cycles = blob["cycles"]
    manifest = blob["manifest"]
    # Prefer one dry mid-morph frame per bank, then fill up to 7 with other dry morphs.
    by_bank: dict[str, list[int]] = {}
    for i, m in enumerate(manifest):
        by_bank.setdefault(m["bank"], []).append(i)
    chosen: list[int] = []
    for _bank, idxs in by_bank.items():
        dry = [i for i in idxs if "_dry" in str(manifest[i].get("id", ""))]
        pool = dry or idxs
        best = min(pool, key=lambda i: abs(float(manifest[i].get("morph_frac", 0.5)) - 0.5))
        chosen.append(best)
    if len(chosen) < 7:
        used = set(chosen)
        candidates = [
            i
            for i, m in enumerate(manifest)
            if i not in used and "_fx_" not in str(m.get("id", ""))
        ]
        candidates.sort(key=lambda i: float(manifest[i].get("morph_frac", 0.5)))
        need = 7 - len(chosen)
        if candidates and need > 0:
            picks = np.linspace(0, len(candidates) - 1, num=need, dtype=int)
            for pi in picks:
                idx = int(candidates[int(pi)])
                if idx not in used:
                    chosen.append(idx)
                    used.add(idx)
                if len(chosen) >= 7:
                    break
    chosen = chosen[:7]

    akwf_files = sorted(akwf_dir.glob("AKWF_*.wav"))[:7] if akwf_dir.is_dir() else []

    n_rows = 2
    n_cols = max(len(chosen), len(akwf_files), 1)
    fig, axes = plt.subplots(n_rows, n_cols, figsize=(1.45 * n_cols, 3.35), sharey=True)
    if n_cols == 1:
        axes = np.array([[axes[0]], [axes[1]]])

    for col in range(n_cols):
        ax = axes[0, col]
        if col < len(chosen):
            i = chosen[col]
            y = np.asarray(cycles[i], dtype=np.float64)
            ax.plot(y, color="#333333", lw=0.9)
            ax.set_title(manifest[i]["bank"].replace("_", " "), fontsize=7.5)
            ax.set_xlim(0, len(y) - 1)
        else:
            ax.axis("off")
        if col == 0:
            ax.set_ylabel("ReelSynth export", fontsize=8)

        ax2 = axes[1, col]
        if col < len(akwf_files):
            y2 = load_akwf_cycle(akwf_files[col])
            ax2.plot(y2, color="#555555", lw=0.9)
            ax2.set_title(akwf_files[col].stem, fontsize=7)
            ax2.set_xlim(0, len(y2) - 1)
        else:
            ax2.axis("off")
        if col == 0:
            ax2.set_ylabel("AKWF OA", fontsize=8)
        ax2.set_xlabel("sample", fontsize=7)

    for ax in axes.ravel():
        if ax.has_data():
            ax.grid(True, alpha=0.2)
            ax.set_ylim(-1.15, 1.15)

    fig.suptitle(
        "Compact wavetable diversity gallery (existing exports only — no new NAS)",
        fontsize=10,
        y=1.02,
    )
    fig.tight_layout()
    png = out_dir / "fig_wt_diversity_gallery.png"
    pdf = out_dir / "fig_wt_diversity_gallery.pdf"
    fig.savefig(png, dpi=200, bbox_inches="tight")
    fig.savefig(pdf, bbox_inches="tight")
    plt.close(fig)
    copy_to_paper(png, *paper_dirs)
    copy_to_paper(pdf, *paper_dirs)

    meta = {
        "schema": "denoiseopt.wt_diversity_gallery.v1",
        "export_json": str(export_json.resolve()),
        "export_indices": chosen,
        "export_ids": [manifest[i]["id"] for i in chosen],
        "export_banks": [manifest[i]["bank"] for i in chosen],
        "akwf_files": [p.name for p in akwf_files],
        "akwf_dir": str(akwf_dir.resolve()) if akwf_dir.is_dir() else None,
        "matrix_fold": "paper/Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v10/figures/real_wt_matrix.json",
        "png": str(png.resolve()),
        "pdf": str(pdf.resolve()),
        "note": "Gallery only; scores remain in real_wt_matrix.json. No new meta search.",
    }
    jp = out_dir / "fig_wt_diversity_gallery.json"
    jp.write_text(json.dumps(meta, indent=2), encoding="utf-8")
    copy_to_paper(jp, *paper_dirs)
    return meta


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--hear-dir", type=Path, default=HEAR_DIR)
    ap.add_argument("--export-json", type=Path, default=EXPORT_JSON)
    ap.add_argument("--akwf-dir", type=Path, default=AKWF_DIR)
    ap.add_argument("--out-dir", type=Path, default=OUT_DIR)
    ap.add_argument("--paper-fig-dir", type=Path, default=V8_FIG)
    ap.add_argument("--paper-fig-dir-v10", type=Path, default=V10_FIG)
    args = ap.parse_args()

    out_resolved = args.out_dir.resolve()
    forbidden = (ROOT / "brand" / "artifacts" / "meta_approach_compare").resolve()
    if out_resolved == forbidden or forbidden in out_resolved.parents:
        raise SystemExit(f"refusing --out-dir under meta_approach_compare/: {out_resolved}")

    paper_dirs = [d for d in (args.paper_fig_dir, args.paper_fig_dir_v10) if d is not None]
    args.out_dir.mkdir(parents=True, exist_ok=True)
    hear_meta = plot_hear_panel(args.hear_dir, args.out_dir, *paper_dirs)
    gallery_meta = plot_wt_gallery(args.export_json, args.akwf_dir, args.out_dir, *paper_dirs)

    summary = {
        "hear_panel": hear_meta,
        "wt_gallery": gallery_meta,
    }
    (args.out_dir / "REPRO.md").write_text(
        "# Paper v8 listening / diversity panels\n\n"
        "Rebuild:\n\n"
        "```bash\n"
        ".venv_gpu/Scripts/python.exe scripts/plot_paper_v8_listening_assets.py\n"
        "```\n\n"
        "Hear WAVs (pre-existing):\n"
        "`brand/artifacts/meta_approach_compare/hear_samples/`\n"
        "(rebuild via `scripts/export_meta_hear_samples.py`).\n",
        encoding="utf-8",
    )
    print(json.dumps({"hear_png": hear_meta["png"], "gallery_png": gallery_meta["png"]}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
