#!/usr/bin/env python3
"""Publication figures: dataset corpus inventory + algorithm comparison bars.

CPU-only. Reads frozen paper/artifact JSON (no GPU search). Writes PNG/PDF +
inventory JSON under brand/artifacts/figures/ and mirrors into paper v9 figures/.
"""
from __future__ import annotations

import argparse
import json
import math
import shutil
from pathlib import Path
from typing import Any

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
ART = ROOT / "brand" / "artifacts"
OUT_DS = ART / "figures" / "dataset_stats"
OUT_CMP = ART / "figures" / "comparison_plots"
PAPER_V9 = Path(
    r"C:\Users\Julian\Documents\Programming\github\reeldemo\denoise-opt-meta\paper"
    r"\Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9"
    r"\figures"
)

# Okabe–Ito
C_OURS = "#D55E00"
C_BLUE = "#0072B2"
C_GREEN = "#009E73"
C_GRAY = "#999999"
C_SKY = "#56B4E9"
C_YELLOW = "#E69F00"
C_PURPLE = "#CC79A7"


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
            "grid.alpha": 0.28,
            "axes.axisbelow": True,
        }
    )
    return plt


def _load(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def _save_fig(fig, plt, stem: Path) -> None:
    stem.parent.mkdir(parents=True, exist_ok=True)
    png = stem.with_suffix(".png")
    pdf = stem.with_suffix(".pdf")
    fig.savefig(png, bbox_inches="tight")
    fig.savefig(pdf, bbox_inches="tight")
    plt.close(fig)
    print(f"wrote {png.name} + pdf", flush=True)


def _copy_to_paper(paths: list[Path]) -> None:
    PAPER_V9.mkdir(parents=True, exist_ok=True)
    for p in paths:
        if p.exists():
            shutil.copy2(p, PAPER_V9 / p.name)


def _raw_hours_cwru() -> float | None:
    try:
        import scipy.io as sio
    except ImportError:
        return None
    raw = ART / "signal_heal_transfer" / "raw" / "cwru"
    if not raw.is_dir():
        return None
    secs = 0.0
    for m in raw.glob("*.mat"):
        mat = sio.loadmat(str(m))
        de = None
        for k, v in mat.items():
            if k.endswith("DE_time") or k.endswith("_DE_time"):
                de = np.asarray(v).ravel()
                break
        if de is None:
            continue
        secs += float(de.size) / 12000.0
    return secs / 3600.0


def _raw_hours_mitbih() -> float | None:
    raw = ART / "signal_heal_transfer" / "raw" / "mitdb"
    dats = list(raw.glob("*.dat")) if raw.is_dir() else []
    if not dats:
        return None
    # MIT-BIH: 360 Hz, 2 channels, 2 bytes/sample => 4 bytes per time step
    secs = sum(f.stat().st_size // 4 for f in dats) / 360.0
    return secs / 3600.0


def _raw_hours_ptbxl() -> float | None:
    raw = ART / "signal_heal_transfer" / "raw" / "ptbxl"
    if not raw.is_dir():
        return None
    hrs = list(raw.rglob("*_hr.dat"))
    if not hrs:
        return None
    # WFDB 16-bit, 12 leads => 24 bytes/sample @ 500 Hz
    samples = sum(f.stat().st_size for f in hrs) / 24.0
    return samples / 500.0 / 3600.0


def _raw_hours_mfpt() -> float | None:
    try:
        import scipy.io as sio
    except ImportError:
        return None
    raw = ART / "signal_heal_transfer" / "raw" / "mfpt"
    mats = [p for p in raw.glob("*.mat")] if raw.is_dir() else []
    if not mats:
        return None
    secs = 0.0
    for m in mats:
        mat = sio.loadmat(str(m))
        best = 0
        fs = 48828.0  # MFPT common rate; overwritten if present
        for k, v in mat.items():
            if k.startswith("__"):
                continue
            arr = np.asarray(v)
            if arr.ndim == 1 and arr.size > best:
                best = int(arr.size)
            if arr.ndim == 2 and max(arr.shape) > best:
                best = int(max(arr.shape))
            if str(k).lower() in {"sr", "fs", "samplingrate", "sampling_rate"}:
                try:
                    fs = float(np.asarray(v).ravel()[0])
                except Exception:
                    pass
        if best > 0:
            secs += best / max(fs, 1.0)
    return secs / 3600.0 if secs > 0 else None


def build_inventory() -> dict[str, Any]:
    cache = ART / "signal_heal_transfer" / "cache"
    transfer_domains: list[dict[str, Any]] = []
    for name in [
        "cwru_bearings",
        "mfpt_bearings",
        "mitbih_ecg",
        "ptbxl_ecg",
        "synth_cnc_g01",
        "synth_pmu_cycle",
    ]:
        meta_path = cache / f"{name}_meta.json"
        if not meta_path.exists():
            continue
        meta = _load(meta_path)
        entry = {
            "name": name,
            "n_periods": int(meta.get("n", 0)),
            "period_l": int(meta.get("period_l", 256)),
            "fs_hz": meta.get("fs_hz"),
            "domain": meta.get("domain"),
            "files": meta.get("files") or meta.get("records"),
            "n_source_files": (
                len(meta.get("files") or meta.get("records") or [])
                if (meta.get("files") or meta.get("records"))
                else meta.get("n_records_files")
            ),
            "subset": meta.get("subset"),
            "citation": meta.get("citation"),
            "wrap": meta.get("wrap"),
            "label": meta.get("label"),
        }
        transfer_domains.append(entry)

    hours = {
        "cwru_bearings_raw_hours": _raw_hours_cwru(),
        "mfpt_bearings_raw_hours_approx": _raw_hours_mfpt(),
        "mitbih_ecg_raw_hours": _raw_hours_mitbih(),
        "ptbxl_hr_raw_hours": _raw_hours_ptbxl(),
        "note": (
            "Hours are raw source duration where files exist on disk, not tiled eval "
            "period count. Synth CNC/PMU have no raw hours (procedural)."
        ),
    }

    ds_json = OUT_DS / "dataset_distributions.json"
    dist = _load(ds_json) if ds_json.exists() else {}
    metrics_path = PAPER_V9 / "dataset_metrics.json"
    metrics = _load(metrics_path) if metrics_path.exists() else {}

    real_wt = _load(ART / "real_wt_cycles" / "real_wt_matrix.json")
    meta_wt = real_wt.get("meta", {})
    export_json = ART / "real_wt_cycles" / "reelsynth_export_cycles.json"
    export_blob = _load(export_json) if export_json.is_file() else {}
    oa_dir = ART / "real_wt_cycles" / "oa_akwf"
    oa_wav_n = len(list(oa_dir.glob("*.wav"))) if oa_dir.is_dir() else 0
    factory_n = int(
        export_blob.get("n_cycles")
        or meta_wt.get("reelsynth_export_n")
        or real_wt.get("reelsynth_export_primary", {}).get("n_cycles", 25)
    )
    akwf_n = int(
        real_wt.get("oa_akwf_secondary", {}).get("n_cycles")
        or meta_wt.get("oa_n")
        or oa_wav_n
        or 24
    )
    # Prefer on-disk OA count when larger than stale scored matrix
    if oa_wav_n > akwf_n:
        akwf_n = oa_wav_n

    inv: dict[str, Any] = {
        "schema": "denoiseopt.dataset_inventory.v1",
        "primary_procedural": {
            "generator": "overnight_gpu_rl_arch.make_batch",
            "family": "sine_cliff",
            "L": 256,
            "prolong_tiles_N": 16,
            "seam_width": 8,
            "holdout_seed": 20260719,
            "search_seed": 1902771841,
            "metrics_draw_n": int(dist.get("n_samples") or 4096),
            "score_batch": 64,
            "multi_seed_classical": 5,
            "train_vs_holdout": (
                "Search draws fresh i.i.d. batches; frozen holdout tensors are never "
                "used for training. N2N/seq baselines use train seeds 424242/424243."
            ),
            "distributions_summary": dist,
            "metrics_summary": metrics.get("metrics") or metrics,
        },
        "multi_family_sota": {
            "n_waveforms": 20,
            "n_families": 10,
            "families": [
                "sine_cliff",
                "harmonic_fft",
                "am_fm",
                "nonlinear",
                "combo",
                "triple_mix",
                "extreme_overlay",
                "open_wrap_bias",
                "soft_noise",
                "wide_seam",
            ],
            "seeds": [20260719, 20260720],
            "batch_per_waveform": 64,
            "total_cycles_scored": 20 * 64,
            "rust_sound_bench_secondary": {
                "n_pairs": 20,
                "note": "10 families x 2 seeds, L=256",
            },
        },
        "wavetable_native": {
            "reelsynth_factory_export_n": factory_n,
            "reelsynth_factory_dry_n": int(export_blob.get("n_dry") or 0) or None,
            "reelsynth_factory_fx_n": int(export_blob.get("n_fx") or 0) or None,
            "akwf_oa_n": akwf_n,
            "procedural_standin_tertiary_n": int(
                real_wt.get("procedural_standin", {}).get("n_cycles", 24)
            ),
            "L": 256,
            "protocol_seed": 20260719,
        },
        "meta_approach_compare": {
            "outer_iters": 5000,
            "approaches": ["random", "cmaes", "reinforce", "aging_evo", "tpe", "hybrid_lstm"],
            "batch": 48,
            "seed": 1902771841,
            "artifact_dir": "brand/artifacts/meta_approach_compare/",
        },
        "transfer_domains": transfer_domains,
        "raw_source_hours": hours,
        "totals": {
            "procedural_metrics_cycles": int(dist.get("n_samples") or 4096),
            "procedural_score_batch": 64,
            "multi_family_cycles": 20 * 64,
            "factory_export_cycles": factory_n,
            "akwf_cycles": akwf_n,
            "transfer_periods_sum": int(
                sum(int(d.get("n_periods") or 0) for d in transfer_domains)
            ),
        },
        "plot4_note": (
            "fig_dataset_size_overview shows comparable ~1280 boards only "
            "(multi-family / Factory+FX / AKWF / transfer). Holdout-4096 and "
            "score-batch-64 live under totals but are omitted from that bar chart."
        ),
    }
    return inv


def plot_dataset_size_overview(inv: dict[str, Any], out_dir: Path) -> list[Path]:
    plt = _mpl()
    written: list[Path] = []

    # Panel A (paper Fig 4): comparable boards only at ~1280 height
    labels = [
        "Multi-family\n(20×64)",
        "Factory+FX\nexport",
        "AKWF\nOA",
        "Transfer\nperiods",
    ]
    vals = [
        inv["totals"]["multi_family_cycles"],
        inv["totals"]["factory_export_cycles"],
        inv["totals"]["akwf_cycles"],
        inv["totals"]["transfer_periods_sum"],
    ]
    fig, ax = plt.subplots(figsize=(6.4, 3.6))
    colors = [C_GREEN, C_YELLOW, C_PURPLE, C_OURS]
    bars = ax.bar(labels, vals, color=colors, edgecolor="white", linewidth=0.4)
    ax.set_ylabel("Cycles / periods")
    ax.set_title("Comparable evaluation corpora (~1280)")
    for b, v in zip(bars, vals):
        ax.text(
            b.get_x() + b.get_width() / 2,
            b.get_height(),
            f"{v:,}",
            ha="center",
            va="bottom",
            fontsize=8,
        )
    ax.set_ylim(0, max(vals) * 1.18 if vals else 1.0)
    fig.tight_layout()
    stem = out_dir / "fig_dataset_size_overview"
    _save_fig(fig, plt, stem)
    written.extend([stem.with_suffix(".png"), stem.with_suffix(".pdf")])

    # Supplemental: holdout metrics draw + score batch (not Plot 4 main board)
    fig, ax = plt.subplots(figsize=(4.8, 3.2))
    supp_labels = ["Holdout\nmetrics", "Score\nbatch"]
    supp_vals = [
        inv["totals"]["procedural_metrics_cycles"],
        inv["totals"]["procedural_score_batch"],
    ]
    bars = ax.bar(supp_labels, supp_vals, color=[C_BLUE, C_SKY], edgecolor="white", linewidth=0.4)
    ax.set_ylabel("Cycles")
    ax.set_title("Procedural holdout / score batch (excluded from Fig 4)")
    for b, v in zip(bars, supp_vals):
        ax.text(
            b.get_x() + b.get_width() / 2,
            b.get_height(),
            f"{v:,}",
            ha="center",
            va="bottom",
            fontsize=8,
        )
    ax.set_ylim(0, max(supp_vals) * 1.18)
    fig.tight_layout()
    stem = out_dir / "fig_dataset_procedural_holdout_sizes"
    _save_fig(fig, plt, stem)
    written.extend([stem.with_suffix(".png"), stem.with_suffix(".pdf")])

    # Panel B: transfer domain period counts
    domains = inv["transfer_domains"]
    if domains:
        fig, ax = plt.subplots(figsize=(7.2, 3.6))
        names = [d["name"].replace("_", "\n") for d in domains]
        ns = [d["n_periods"] for d in domains]
        fs = [d.get("fs_hz") or float("nan") for d in domains]
        bars = ax.bar(names, ns, color=C_BLUE, edgecolor="white", linewidth=0.4)
        ax.set_ylabel("Eval periods (L=256)")
        ax.set_title("Sci/eng transfer domain sizes (tiled periods)")
        for b, n, f in zip(bars, ns, fs):
            label = f"n={n}"
            if f == f:  # not NaN
                label += f"\nfs={int(f)} Hz"
            ax.text(
                b.get_x() + b.get_width() / 2,
                b.get_height(),
                label,
                ha="center",
                va="bottom",
                fontsize=7,
            )
        ax.set_ylim(0, max(ns) * 1.28)
        fig.tight_layout()
        stem = out_dir / "fig_dataset_transfer_sizes"
        _save_fig(fig, plt, stem)
        written.extend([stem.with_suffix(".png"), stem.with_suffix(".pdf")])

    # Panel C: multi-family counts
    fams = inv["multi_family_sota"]["families"]
    fig, ax = plt.subplots(figsize=(7.2, 3.4))
    ax.barh(
        list(reversed(fams)),
        [64 * 2] * len(fams),
        color=C_GREEN,
        edgecolor="white",
        linewidth=0.4,
    )
    ax.set_xlabel("Cycles (2 seeds x batch 64)")
    ax.set_title("Multi-family SOTA matrix: cycles per family")
    ax.set_xlim(0, 160)
    fig.tight_layout()
    stem = out_dir / "fig_dataset_family_counts"
    _save_fig(fig, plt, stem)
    written.extend([stem.with_suffix(".png"), stem.with_suffix(".pdf")])

    return written


def plot_meta_bars(out_dir: Path) -> list[Path]:
    plt = _mpl()
    meta = _load(PAPER_V9 / "meta_approach_compare.json")
    order = ["random", "cmaes", "reinforce", "aging_evo", "tpe", "hybrid_lstm"]
    labels = {
        "random": "Random\nNAS",
        "cmaes": "CMA-ES",
        "reinforce": "REINFORCE",
        "aging_evo": "Aging\nevo",
        "tpe": "TPE",
        "hybrid_lstm": "Ours\n(hybrid)",
    }
    rs = [float(meta["approaches"][k]["champ_r"]) for k in order]
    dual = float(meta.get("baseline_dual_cosine", 0.8166))
    colors = [C_SKY, C_BLUE, C_GREEN, C_YELLOW, C_PURPLE, C_OURS]
    fig, ax = plt.subplots(figsize=(6.4, 3.6))
    xs = np.arange(len(order))
    bars = ax.bar(xs, rs, color=colors, edgecolor="white", linewidth=0.4)
    ax.axhline(dual, color=C_GRAY, ls="--", lw=1.2, label=f"DualCosine {dual:.3f}")
    ax.set_xticks(xs)
    ax.set_xticklabels([labels[k] for k in order])
    ax.set_ylabel("Champion prolonged $R$")
    ax.set_title("Matched 5k outer-loop meta-approach compare")
    ax.set_ylim(0.80, 1.005)
    for b, v in zip(bars, rs):
        ax.text(
            b.get_x() + b.get_width() / 2,
            b.get_height() + 0.001,
            f"{v:.3f}",
            ha="center",
            va="bottom",
            fontsize=7,
        )
    ax.legend(loc="lower right", frameon=False, fontsize=8)
    fig.tight_layout()
    stem = out_dir / "meta_approach_bars"
    _save_fig(fig, plt, stem)
    # also write learning curves if history exists
    written = [stem.with_suffix(".png"), stem.with_suffix(".pdf")]

    fig, ax = plt.subplots(figsize=(6.8, 3.8))
    hist_root = ART / "meta_approach_compare"
    for k, col in zip(order, colors):
        hp = hist_root / k / "history.jsonl"
        if not hp.exists():
            continue
        xs_i: list[int] = []
        ys: list[float] = []
        best = -math.inf
        for line in hp.read_text(encoding="utf-8").splitlines():
            if not line.strip():
                continue
            row = json.loads(line)
            it = int(row.get("iter") or row.get("i") or row.get("step") or len(xs_i))
            r = row.get("champ") or row.get("champ_r") or row.get("best_r") or row.get("r")
            if r is None:
                continue
            best = max(best, float(r))
            xs_i.append(it)
            ys.append(best)
        if xs_i:
            ax.plot(xs_i, ys, color=col, lw=1.4, label=labels[k].replace("\n", " "))
    ax.axhline(dual, color=C_GRAY, ls="--", lw=1.0)
    ax.set_xlabel("Outer iteration")
    ax.set_ylabel("Best champion $R$ so far")
    ax.set_title("Meta-approach learning curves (5k gate)")
    ax.set_xlim(0, 5000)
    ax.set_ylim(0.80, 1.005)
    ax.legend(loc="lower right", fontsize=7, ncol=2, frameon=False)
    fig.tight_layout()
    stem2 = out_dir / "fig_meta_approach_compare"
    _save_fig(fig, plt, stem2)
    written.extend([stem2.with_suffix(".png"), stem2.with_suffix(".pdf")])
    return written


def plot_transfer_bars(out_dir: Path) -> list[Path]:
    plt = _mpl()
    # Prefer paper results_table (includes MFPT)
    table_path = PAPER_V9 / "results_table.json"
    blob = _load(table_path)
    table: dict[str, dict[str, float]] = blob["table"]
    prefer_methods = [
        "ours_hybrid_lstm",
        "no_bake",
        "endpoint_pin_mean",
        "seam_fir3",
        "dual_cosine",
    ]
    method_labels = {
        "ours_hybrid_lstm": "Ours",
        "no_bake": "no-bake",
        "endpoint_pin_mean": "endpoint-pin",
        "seam_fir3": "seam_fir3",
        "dual_cosine": "DualCosine",
    }
    ds_order = [
        "cwru_bearings",
        "mfpt_bearings",
        "mitbih_ecg",
        "ptbxl_ecg",
        "synth_cnc_g01",
        "synth_pmu_cycle",
    ]
    datasets = [d for d in ds_order if d in table]
    methods = [m for m in prefer_methods if any(m in table[d] for d in datasets)]
    x = np.arange(len(datasets))
    width = 0.15
    fig, ax = plt.subplots(figsize=(8.2, 3.8))
    palette = [C_OURS, C_GRAY, C_YELLOW, C_GREEN, C_BLUE]
    for i, m in enumerate(methods):
        vals = [float(table[d].get(m, float("nan"))) for d in datasets]
        ax.bar(
            x + (i - (len(methods) - 1) / 2) * width,
            vals,
            width,
            label=method_labels.get(m, m),
            color=palette[i % len(palette)],
            edgecolor="white",
            linewidth=0.3,
        )
    ax.set_xticks(x)
    ax.set_xticklabels(
        [
            d.replace("_bearings", "")
            .replace("_ecg", "")
            .replace("synth_", "synth ")
            .replace("_", " ")
            for d in datasets
        ],
        fontsize=8,
    )
    ax.set_ylabel("Holdout-refit prolonged $R$")
    ax.set_title("Sci/eng wrap-heal transfer (classical board)")
    ax.set_ylim(0.0, 1.05)
    ax.legend(ncol=3, fontsize=7, frameon=False, loc="lower right")
    fig.tight_layout()
    stem = out_dir / "fig_signal_heal_transfer"
    _save_fig(fig, plt, stem)
    return [stem.with_suffix(".png"), stem.with_suffix(".pdf")]


def plot_classical_and_sota(out_dir: Path) -> list[Path]:
    plt = _mpl()
    written: list[Path] = []
    sota = _load(PAPER_V9 / "sota_matrix.json")
    rows = sota["canonical_holdout"]
    # bar: method R
    names = []
    vals = []
    for row in rows:
        r = row.get("R_mean") or row.get("residual_R") or row.get("R")
        if r is None:
            continue
        names.append(str(row.get("method") or row.get("name")))
        vals.append(float(r))
    # highlight ours / dual
    colors = []
    for n in names:
        if "neural" in n or "favorite" in n:
            colors.append(C_OURS)
        elif "dual" in n or n == "cosine_fade":
            colors.append(C_BLUE)
        elif n == "identity":
            colors.append(C_GRAY)
        else:
            colors.append(C_SKY)
    fig, ax = plt.subplots(figsize=(7.4, 3.8))
    xs = np.arange(len(names))
    ax.bar(xs, vals, color=colors, edgecolor="white", linewidth=0.3)
    ax.set_xticks(xs)
    ax.set_xticklabels(names, rotation=55, ha="right", fontsize=7)
    ax.set_ylabel("Prolonged $R$")
    ax.set_title("Canonical holdout: classical vs AI methods")
    ax.set_ylim(0.35, 1.02)
    fig.tight_layout()
    stem = out_dir / "fig_classical_vs_ai_bars"
    _save_fig(fig, plt, stem)
    written.extend([stem.with_suffix(".png"), stem.with_suffix(".pdf")])

    # multifamily mean R heatmap-like bars for top methods if present
    stats = sota.get("stats") or {}
    multifam = None
    if isinstance(stats, dict) and "by_method" in stats:
        multifam = stats["by_method"]
    elif isinstance(sota.get("results_multifamily"), dict):
        multifam = sota["results_multifamily"]

    # Build method x mean R from results_multifamily
    method_means: dict[str, list[float]] = {}
    rm = sota.get("results_multifamily")
    if isinstance(rm, list):
        for wave in rm:
            # Either per-method aggregate rows, or per-waveform score maps
            if "residual_R_mean" in wave and ("name" in wave or "method" in wave):
                m = str(wave.get("name") or wave.get("method"))
                method_means[m] = [float(wave["residual_R_mean"])]
                continue
            methods = wave.get("methods") or wave.get("scores") or {}
            if isinstance(methods, dict):
                for m, sc in methods.items():
                    if isinstance(sc, dict):
                        r = (
                            sc.get("R_mean")
                            or sc.get("residual_R")
                            or sc.get("R")
                        )
                    else:
                        r = sc
                    if r is None:
                        continue
                    method_means.setdefault(m, []).append(float(r))
    elif isinstance(rm, dict):
        for m, sc in rm.items():
            if isinstance(sc, dict) and "R_mean" in sc:
                method_means[m] = [float(sc["R_mean"])]

    if method_means:
        items = sorted(
            ((m, float(np.mean(vs))) for m, vs in method_means.items()),
            key=lambda kv: kv[1],
            reverse=True,
        )[:14]
        fig, ax = plt.subplots(figsize=(7.4, 3.8))
        labs = [m for m, _ in items]
        vs = [v for _, v in items]
        cols = [C_OURS if ("neural" in m or "hybrid" in m or "favorite" in m) else C_SKY for m in labs]
        ax.barh(list(reversed(labs)), list(reversed(vs)), color=list(reversed(cols)))
        ax.set_xlabel("Mean prolonged $R$ (20-waveform)")
        ax.set_title("SOTA matrix summary (multi-family mean $R$)")
        ax.set_xlim(0.7, 1.01)
        fig.tight_layout()
        stem = out_dir / "fig_sota_method_bars"
        _save_fig(fig, plt, stem)
        written.extend([stem.with_suffix(".png"), stem.with_suffix(".pdf")])

    return written


def plot_hp_sensitivity(out_dir: Path) -> list[Path]:
    plt = _mpl()
    hp = _load(PAPER_V9 / "meta_hp_sensitivity.json")
    table = hp.get("table") or []
    if isinstance(table, dict):
        rows = [{"name": k, **v} for k, v in table.items()]
    else:
        rows = list(table)
    names = []
    vals = []
    for row in rows:
        name = str(
            row.get("name")
            or row.get("config")
            or row.get("arm")
            or row.get("label")
            or "?"
        )
        r = row.get("champ_r") or row.get("R") or row.get("champ_raw")
        if r is None:
            continue
        names.append(name)
        vals.append(float(r))
    if not names:
        return []
    default = float(hp.get("default_champ_raw") or np.median(vals))
    fig, ax = plt.subplots(figsize=(7.2, 3.6))
    xs = np.arange(len(names))
    cols = [C_OURS if abs(v - default) < 1e-6 else C_SKY for v in vals]
    ax.bar(xs, vals, color=cols, edgecolor="white", linewidth=0.3)
    ax.axhline(default, color=C_GRAY, ls="--", lw=1.0, label=f"default {default:.4f}")
    ax.set_xticks(xs)
    ax.set_xticklabels(names, rotation=50, ha="right", fontsize=7)
    ax.set_ylabel("Champion $R$")
    ax.set_title("HP sensitivity (matched budget arms)")
    ax.set_ylim(min(vals) - 0.01, max(vals) + 0.008)
    ax.legend(frameon=False, fontsize=8)
    fig.tight_layout()
    stem = out_dir / "fig_meta_hp_sensitivity"
    _save_fig(fig, plt, stem)
    return [stem.with_suffix(".png"), stem.with_suffix(".pdf")]


def plot_n2n_bars(out_dir: Path) -> list[Path]:
    plt = _mpl()
    n2n = _load(PAPER_V9 / "n2n_baseline.json")
    seq = _load(PAPER_V9 / "seq_baseline.json")
    sota = _load(PAPER_V9 / "sota_matrix.json")
    favorite = None
    dual = None
    identity = None
    for row in sota["canonical_holdout"]:
        m = row.get("method") or row.get("name")
        raw = row.get("R_mean") or row.get("residual_R") or row.get("R")
        if raw is None:
            continue
        r = float(raw)
        if m == "neural_favorite":
            favorite = r
        elif m == "dual_cosine":
            dual = r
        elif m == "identity":
            identity = r
    items = [
        ("no-bake", identity, C_GRAY),
        ("DualCosine", dual, C_BLUE),
        ("N2N corrupt", n2n["modes"]["n2n_corrupt_corrupt"]["eval"]["residual_R"], C_SKY),
        ("N2N sibling", n2n["modes"]["n2n_sibling_supervised"]["eval"]["residual_R"], C_GREEN),
        ("seq CNN1D", seq["models"]["seq_cnn1d"]["eval"]["residual_R"], C_YELLOW),
        ("seq LSTM", seq["models"]["seq_lstm"]["eval"]["residual_R"], C_PURPLE),
        ("Ours (favorite)", favorite, C_OURS),
    ]
    items = [(a, float(b), c) for a, b, c in items if b is not None]
    fig, ax = plt.subplots(figsize=(6.8, 3.6))
    xs = np.arange(len(items))
    bars = ax.bar(xs, [v for _, v, _ in items], color=[c for _, _, c in items], edgecolor="white")
    ax.set_xticks(xs)
    ax.set_xticklabels([a for a, _, _ in items], rotation=25, ha="right", fontsize=8)
    ax.set_ylabel("Canonical holdout prolonged $R$")
    ax.set_title("N2N / seq baselines vs Ours (same generator)")
    ax.set_ylim(0.80, 1.005)
    for b, (_, v, _) in zip(bars, items):
        ax.text(
            b.get_x() + b.get_width() / 2,
            b.get_height() + 0.001,
            f"{v:.3f}",
            ha="center",
            va="bottom",
            fontsize=7,
        )
    fig.tight_layout()
    stem = out_dir / "fig_n2n_vs_ours_bars"
    _save_fig(fig, plt, stem)
    return [stem.with_suffix(".png"), stem.with_suffix(".pdf")]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--skip-distributions", action="store_true")
    args = ap.parse_args()

    OUT_DS.mkdir(parents=True, exist_ok=True)
    OUT_CMP.mkdir(parents=True, exist_ok=True)

    if not args.skip_distributions:
        # Refresh holdout histograms on CPU
        import os

        os.environ.setdefault("CUDA_VISIBLE_DEVICES", "")
        from plot_dataset_statistics import main as dist_main

        # Call module main with defaults pointing at OUT_DS; monkey via argv
        import sys

        sys.argv = [
            "plot_dataset_statistics.py",
            "--out-dir",
            str(OUT_DS),
            "--dpi",
            "220",
        ]
        # Disable v7 copy by patching: run collect inline instead for safety
        try:
            dist_main()
        except Exception as e:
            print(f"WARN: distribution refresh failed: {e}", flush=True)

    inv = build_inventory()
    inv_path = OUT_DS / "dataset_inventory.json"
    inv_path.write_text(json.dumps(inv, indent=2), encoding="utf-8")
    print(f"wrote {inv_path}", flush=True)

    written: list[Path] = [inv_path]
    written.extend(plot_dataset_size_overview(inv, OUT_DS))
    # keep legacy distributions copy to paper
    for name in ("fig_dataset_distributions.png", "dataset_distributions.json", "dataset_inventory.json"):
        p = OUT_DS / name
        if p.exists():
            written.append(p)

    written.extend(plot_meta_bars(OUT_CMP))
    written.extend(plot_transfer_bars(OUT_CMP))
    written.extend(plot_classical_and_sota(OUT_CMP))
    written.extend(plot_hp_sensitivity(OUT_CMP))
    written.extend(plot_n2n_bars(OUT_CMP))

    # Mirror key publishable figs into paper v9 + keep inventory in dataset_stats
    mirror_names = [
        "fig_dataset_distributions.png",
        "fig_dataset_size_overview.png",
        "fig_dataset_size_overview.pdf",
        "fig_dataset_procedural_holdout_sizes.png",
        "fig_dataset_procedural_holdout_sizes.pdf",
        "fig_dataset_transfer_sizes.png",
        "fig_dataset_transfer_sizes.pdf",
        "fig_dataset_family_counts.png",
        "fig_dataset_family_counts.pdf",
        "dataset_distributions.json",
        "dataset_inventory.json",
        "meta_approach_bars.png",
        "meta_approach_bars.pdf",
        "fig_meta_approach_compare.png",
        "fig_meta_approach_compare.pdf",
        "fig_signal_heal_transfer.png",
        "fig_signal_heal_transfer.pdf",
        "fig_classical_vs_ai_bars.png",
        "fig_classical_vs_ai_bars.pdf",
        "fig_sota_method_bars.png",
        "fig_sota_method_bars.pdf",
        "fig_meta_hp_sensitivity.png",
        "fig_meta_hp_sensitivity.pdf",
        "fig_n2n_vs_ours_bars.png",
        "fig_n2n_vs_ours_bars.pdf",
    ]
    paths: list[Path] = []
    for name in mirror_names:
        for base in (OUT_DS, OUT_CMP):
            p = base / name
            if p.exists():
                paths.append(p)
    _copy_to_paper(paths)
    # Also copy inventory JSON next to distributions in paper
    print(f"mirrored {len(paths)} files -> {PAPER_V9}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
