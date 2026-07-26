#!/usr/bin/env python3
"""Latency microbench on signal-heal transfer domain bundles.

Times inference (forward + prolonged residual) for:
  - no_bake (identity)
  - DualCosine
  - Ours champ FitCell (domain hybrid_lstm champ_cell.pt + summary arch)
  - N2N (wavetable SeamN2N ckpt, zero-shot on L=256 domains; null if missing)

Warmup 20 / timed 100 by default. Writes
``brand/artifacts/signal_heal_transfer/latency_table.json`` and bar plots.

Prefer ``--device cpu`` while a live domain search holds the GPU.
"""
from __future__ import annotations

import argparse
import json
import math
import shutil
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import overnight_gpu_rl_arch as og  # noqa: E402
from baselines.n2n_seam import SeamN2N, n_params as n2n_n_params  # noqa: E402
from signal_heal.datasets import DomainBatcher, ensure_bundles  # noqa: E402

OUT = ROOT / "brand" / "artifacts" / "signal_heal_transfer"
META_PAPER = ROOT.parent / "denoise-opt-meta"
N2N_CKPT = ROOT / "brand" / "artifacts" / "n2n_seam_baselines" / "n2n_corrupt_corrupt.pt"

DOMAINS = [
    "cwru_bearings",
    "mfpt_bearings",
    "mitbih_ecg",
    "ptbxl_ecg",
    "synth_cnc_g01",
    "synth_pmu_cycle",
]

METHOD_ORDER = ["no_bake", "dual_cosine", "ours_hybrid_lstm", "n2n_corrupt_corrupt"]
METHOD_COLORS = {
    "no_bake": "#999999",
    "dual_cosine": "#0072B2",
    "ours_hybrid_lstm": "#D55E00",
    "n2n_corrupt_corrupt": "#009E73",
}
METHOD_LABELS = {
    "no_bake": "no-bake",
    "dual_cosine": "DualCosine",
    "ours_hybrid_lstm": "Ours (hybrid)",
    "n2n_corrupt_corrupt": "N2N (WT→domain)",
}


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def gpu_busy_hint() -> str | None:
    """Best-effort: another process holding CUDA memory → prefer CPU."""
    try:
        if not torch.cuda.is_available():
            return None
        free, total = torch.cuda.mem_get_info(0)
        used = total - free
        # Search jobs typically hold >1.5 GiB; leave headroom for us.
        if used > 1.5 * (1024**3):
            return f"cuda mem used≈{used / (1024**3):.1f} GiB; prefer CPU while search runs"
    except Exception:
        pass
    return None


def load_ours_cell(
    ds_dir: Path, device: torch.device
) -> tuple[og.SeamCell, og.ArchConfig, dict[str, Any]] | None:
    summary_path = ds_dir / "hybrid_lstm" / "summary.json"
    cell_path = ds_dir / "hybrid_lstm" / "champ_cell.pt"
    if not summary_path.is_file():
        return None
    summary = json.loads(summary_path.read_text(encoding="utf-8"))
    arch = summary.get("champ_arch")
    if not arch:
        return None
    cfg = og.ArchConfig.from_dict(arch)
    cell = og.SeamCell(cfg).to(device)
    if cell_path.is_file():
        sd = torch.load(cell_path, map_location=device, weights_only=False)
        cell.load_state_dict(sd, strict=False)
    else:
        # Arch-only: still time structure, but mark weights as random/uninit.
        summary = {**summary, "weights": "missing_champ_cell_pt"}
    cell.eval()
    return cell, cfg, summary


def load_n2n(device: torch.device) -> tuple[SeamN2N, dict[str, Any]] | None:
    if not N2N_CKPT.is_file():
        return None
    blob = torch.load(N2N_CKPT, map_location="cpu", weights_only=False)
    state = blob["state_dict"] if isinstance(blob, dict) and "state_dict" in blob else blob
    model = SeamN2N.from_state(state, device)
    meta = {
        "ckpt": str(N2N_CKPT),
        "source": "wavetable_n2n_corrupt_corrupt_zero_shot",
        "n_params": n2n_n_params(model),
    }
    return model, meta


@torch.no_grad()
def time_fn(
    fn: Callable[[torch.Tensor], torch.Tensor],
    ideal: torch.Tensor,
    eng: torch.Tensor,
    *,
    device: torch.device,
    warmup: int,
    repeats: int,
) -> dict[str, float]:
    batch = int(eng.shape[0])
    for _ in range(warmup):
        out = fn(eng)
        _ = og.residual_score(ideal, out).mean()
    if device.type == "cuda":
        torch.cuda.synchronize()
    t0 = time.perf_counter()
    r_last = 0.0
    for _ in range(repeats):
        out = fn(eng)
        r_last = float(og.residual_score(ideal, out).mean().item())
    if device.type == "cuda":
        torch.cuda.synchronize()
    ms = 1000.0 * (time.perf_counter() - t0) / max(repeats, 1)
    return {
        "ms_per_batch": ms,
        "ms_per_period": ms / batch,
        "residual_live": r_last,
    }


def plot_latency(table: dict[str, dict[str, Any]], out_png: Path, out_pdf: Path, batch: int) -> None:
    datasets = [d for d in DOMAINS if d in table]
    methods = [m for m in METHOD_ORDER if any(m in table[d] for d in datasets)]
    n_ds = max(len(datasets), 1)
    n_cols = 3 if n_ds >= 3 else n_ds
    n_rows = int(math.ceil(n_ds / n_cols))
    fig, axes = plt.subplots(
        n_rows,
        n_cols,
        figsize=(max(10.0, 3.5 * n_cols), max(6.2, 3.4 * n_rows)),
        squeeze=False,
    )
    flat = list(axes.ravel())
    for i, ds in enumerate(datasets):
        ax = flat[i]
        vals = []
        cols = []
        labels = []
        for m in methods:
            row = table[ds].get(m) or {}
            v = row.get("ms_per_batch")
            vals.append(float(v) if v is not None else float("nan"))
            cols.append(METHOD_COLORS.get(m, "#56B4E9"))
            labels.append(METHOD_LABELS.get(m, m))
        xs = range(len(methods))
        ax.bar(xs, vals, color=cols)
        ax.set_xticks(list(xs))
        ax.set_xticklabels(labels, rotation=40, ha="right", fontsize=8)
        ax.set_ylabel(f"ms / batch={batch}")
        ax.set_title(ds.replace("_", " "), fontsize=10)
        ax.grid(axis="y", alpha=0.3)
    for j in range(len(datasets), len(flat)):
        flat[j].axis("off")
    fig.suptitle(
        "Transfer-domain inference latency (device in JSON; warmup/timed fixed)",
        fontsize=12,
    )
    fig.tight_layout()
    out_png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out_png, dpi=180)
    fig.savefig(out_pdf)
    plt.close(fig)


def write_tex_snippet(path: Path, payload: dict[str, Any]) -> None:
    """Short Results snippet: latency table (ms/batch)."""
    table = payload.get("table") or {}
    device = payload.get("config", {}).get("device", "?")
    batch = payload.get("config", {}).get("batch", 64)
    lines = [
        r"% Auto-generated by scripts/bench_signal_heal_latency.py — do not hand-edit numbers.",
        r"\paragraph{Transfer inference latency.}",
        rf"Table~\ref{{tab:transfer-latency}} reports wall-clock inference on transfer holdout batches "
        rf"(device=\texttt{{{device}}}, batch={batch}, warmup=20, timed=100; "
        r"includes prolonged residual). "
        r"N2N uses the wavetable \texttt{n2n\_corrupt\_corrupt} checkpoint zero-shot when present; "
        r"domain-trained N2N was not run. "
        r"These timings fill the prior transfer gap (wavetable tables had latency; transfer had $R$ only).",
        "",
        r"\begin{table}[t]",
        r"  \centering",
        (
            r"  \caption{Transfer-domain inference latency (ms/batch; higher is slower). "
            + f"Batch={batch}; "
            + r"\texttt{---} = method unavailable.}"
        ),
        r"  \label{tab:transfer-latency}",
        r"  \setlength{\tabcolsep}{3.2pt}",
        r"  \begin{tabular}{@{}lrrrrrr@{}}",
        r"    \toprule",
        r"    Method & CWRU & MFPT & MIT-BIH & PTB-XL & CNC & PMU \\",
        r"    \midrule",
    ]
    col_keys = [
        "cwru_bearings",
        "mfpt_bearings",
        "mitbih_ecg",
        "ptbxl_ecg",
        "synth_cnc_g01",
        "synth_pmu_cycle",
    ]
    row_specs = [
        ("no_bake", "no-bake"),
        ("dual_cosine", "DualCosine"),
        ("ours_hybrid_lstm", r"Ours (\texttt{hybrid})"),
        ("n2n_corrupt_corrupt", r"N2N (WT$\to$dom.)"),
    ]

    def cell(ds: str, method: str) -> str:
        row = (table.get(ds) or {}).get(method) or {}
        v = row.get("ms_per_batch")
        if v is None:
            return r"---"
        try:
            return f"{float(v):.2f}"
        except Exception:
            return r"---"

    for key, label in row_specs:
        cells = [cell(ds, key) for ds in col_keys]
        lines.append(f"    {label} & " + " & ".join(cells) + r" \\")
    lines += [
        r"    \bottomrule",
        r"  \end{tabular}",
        r"\end{table}",
        "",
        r"\begin{figure}[t]",
        r"  \centering",
        r"  \includegraphics[width=\columnwidth]{figures/fig_signal_heal_transfer_latency.png}",
        r"  \caption{Transfer-domain inference latency (ms/batch). "
        r"N2N is wavetable-trained zero-shot when available.}",
        r"  \label{fig:signal-heal-transfer-latency}",
        r"\end{figure}",
        "",
    ]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--batch", type=int, default=64)
    ap.add_argument("--warmup", type=int, default=20)
    ap.add_argument("--repeats", type=int, default=100)
    ap.add_argument(
        "--device",
        type=str,
        default="auto",
        help="cuda|cpu|auto (auto→cpu if GPU looks busy with a search)",
    )
    ap.add_argument(
        "--datasets",
        type=str,
        default=",".join(DOMAINS),
    )
    ap.add_argument("--force-rebuild", action="store_true")
    args = ap.parse_args()

    hint = gpu_busy_hint()
    if args.device == "auto":
        if hint:
            device = torch.device("cpu")
            print(f"auto device=cpu ({hint})")
        elif torch.cuda.is_available():
            device = torch.device("cuda")
            print("auto device=cuda")
        else:
            device = torch.device("cpu")
            print("auto device=cpu (no cuda)")
    else:
        want = args.device
        if want == "cuda" and not torch.cuda.is_available():
            want = "cpu"
        device = torch.device(want)
        if hint and device.type == "cuda":
            print(f"WARNING: {hint} — still using cuda as requested")

    OUT.mkdir(parents=True, exist_ok=True)
    wanted = [x.strip() for x in args.datasets.split(",") if x.strip()]
    print("Loading domain bundles…")
    bundles = ensure_bundles(force=args.force_rebuild, n_periods=256)

    n2n_pack = load_n2n(device)
    if n2n_pack is None:
        print("N2N ckpt missing — will record null per domain")
    else:
        print(f"N2N loaded: {n2n_pack[1]}")

    table: dict[str, dict[str, Any]] = {}
    per_ds: dict[str, Any] = {}

    for name in wanted:
        bundle = bundles.get(name)
        if bundle is None:
            print(f"SKIP {name}: unavailable")
            continue
        print(f"=== latency {name} n={bundle.ideal.shape[0]} L={bundle.ideal.shape[1]} ===")
        batcher = DomainBatcher(bundle, device)
        ideal, eng = batcher.holdout(args.batch)
        # Pad holdout if smaller than batch by repeating
        if ideal.shape[0] < args.batch:
            reps = (args.batch + ideal.shape[0] - 1) // ideal.shape[0]
            ideal = ideal.repeat(reps, 1)[: args.batch]
            eng = eng.repeat(reps, 1)[: args.batch]

        row: dict[str, Any] = {}

        # no_bake
        m = time_fn(lambda x: x, ideal, eng, device=device, warmup=args.warmup, repeats=args.repeats)
        row["no_bake"] = {**m, "n_params": 0, "kind": "classical"}
        print(f"  no_bake           {m['ms_per_batch']:.3f} ms/batch  {m['ms_per_period']:.4f} ms/period")

        # DualCosine
        m = time_fn(
            og.dual_cosine_blend,
            ideal,
            eng,
            device=device,
            warmup=args.warmup,
            repeats=args.repeats,
        )
        row["dual_cosine"] = {**m, "n_params": 0, "kind": "classical"}
        print(f"  dual_cosine       {m['ms_per_batch']:.3f} ms/batch  {m['ms_per_period']:.4f} ms/period")

        # Ours
        ours = load_ours_cell(OUT / name, device)
        if ours is None:
            row["ours_hybrid_lstm"] = {
                "ms_per_batch": None,
                "ms_per_period": None,
                "residual_live": None,
                "n_params": None,
                "kind": "ours",
                "skip_reason": "no summary/champ_arch",
            }
            print("  ours_hybrid_lstm  --- (no champ)")
        else:
            cell, cfg, summary = ours
            n_params = sum(p.numel() for p in cell.parameters())

            def ours_fn(x, _cell=cell, _ops=cfg.ops):
                return og.apply_ops(x, _cell, _ops)

            m = time_fn(ours_fn, ideal, eng, device=device, warmup=args.warmup, repeats=args.repeats)
            row["ours_hybrid_lstm"] = {
                **m,
                "n_params": n_params,
                "kind": "ours",
                "champ_raw": summary.get("champ_raw"),
                "iters_done": summary.get("iters_done"),
                "partial": summary.get("partial"),
                "weights": summary.get("weights", "champ_cell.pt"),
            }
            print(
                f"  ours_hybrid_lstm  {m['ms_per_batch']:.3f} ms/batch  "
                f"{m['ms_per_period']:.4f} ms/period  params={n_params}"
            )

        # N2N
        if n2n_pack is None:
            row["n2n_corrupt_corrupt"] = {
                "ms_per_batch": None,
                "ms_per_period": None,
                "residual_live": None,
                "n_params": None,
                "kind": "n2n",
                "skip_reason": "no transferable wavetable N2N ckpt",
            }
            print("  n2n               --- (no ckpt)")
        else:
            n2n_model, n2n_meta = n2n_pack

            def n2n_fn(x, _m=n2n_model):
                return _m(x)

            m = time_fn(n2n_fn, ideal, eng, device=device, warmup=args.warmup, repeats=args.repeats)
            row["n2n_corrupt_corrupt"] = {**m, **n2n_meta, "kind": "n2n"}
            print(
                f"  n2n_corrupt       {m['ms_per_batch']:.3f} ms/batch  "
                f"{m['ms_per_period']:.4f} ms/period"
            )

        table[name] = row
        per_ds[name] = {"meta": bundle.meta, "n_cycles": int(bundle.ideal.shape[0])}

    payload = {
        "finished_at": utc_now(),
        "config": {
            "batch": args.batch,
            "warmup": args.warmup,
            "repeats": args.repeats,
            "device": str(device),
            "gpu_busy_hint": hint,
            "metric_timed": "forward + prolonged residual_score (same as wavetable benches)",
            "n2n_policy": (
                "wavetable n2n_corrupt_corrupt.pt zero-shot on L=256 domains; "
                "null if ckpt missing; no domain-trained N2N"
            ),
        },
        "table": table,
        "per_dataset": per_ds,
        "method_order": METHOD_ORDER,
    }

    json_path = OUT / "latency_table.json"
    json_path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    png = OUT / "fig_signal_heal_transfer_latency.png"
    pdf = OUT / "fig_signal_heal_transfer_latency.pdf"
    plot_latency(table, png, pdf, args.batch)

    # Paper mirrors
    paper_v9 = (
        META_PAPER
        / "paper"
        / "Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9"
    )
    if paper_v9.is_dir():
        fig_dst = paper_v9 / "figures"
        fig_dst.mkdir(parents=True, exist_ok=True)
        for src in (png, pdf, json_path):
            shutil.copy2(src, fig_dst / src.name)
        # Also alias short names used in tex
        shutil.copy2(png, fig_dst / "fig_signal_heal_transfer_latency.png")
        snippet = paper_v9 / "subsections" / "results_transfer_latency.tex"
        write_tex_snippet(snippet, payload)
        # Inject \input into results_transfer.tex if not already present
        rt = paper_v9 / "subsections" / "results_transfer.tex"
        if rt.is_file():
            text = rt.read_text(encoding="utf-8")
            marker = r"\input{subsections/results_transfer_latency}"
            if marker not in text and "results_transfer_latency" not in text:
                # Insert before listening protocol subsection if present
                needle = r"\subsection{Listening / spectrogram protocol"
                if needle in text:
                    text = text.replace(
                        needle,
                        marker + "\n\n" + needle,
                        1,
                    )
                else:
                    text = text.rstrip() + "\n\n" + marker + "\n"
                rt.write_text(text, encoding="utf-8")
                print("patched", rt)
        print("paper figures ->", fig_dst)

    # Local docs mirror
    local_docs = ROOT / "docs" / "papers" / "denoise_opt"
    if local_docs.is_dir():
        write_tex_snippet(local_docs / "results_transfer_latency.tex", payload)

    print("Wrote", json_path)
    print("Wrote", png)
    return 0 if table else 1


if __name__ == "__main__":
    raise SystemExit(main())
