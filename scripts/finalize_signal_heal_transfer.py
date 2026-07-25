#!/usr/bin/env python3
"""Finalize signal-heal transfer table/figures from completed hybrid summaries."""
from __future__ import annotations

import json
import sys
from datetime import datetime, timezone
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import overnight_gpu_rl_arch as og  # noqa: E402
from bench_signal_heal_transfer import (  # noqa: E402
    OUT,
    META_PAPER,
    plot_bars,
    refit_and_score,
    score_baselines,
    write_paper_note,
    write_readme,
)
from signal_heal.baselines_domain import BASELINE_LABELS  # noqa: E402
from signal_heal.datasets import DomainBatcher, DatasetBundle, ensure_bundles  # noqa: E402


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def main() -> int:
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    bundles = ensure_bundles(force=False, n_periods=256)
    skipped = {
        "mfpt_bearings": "MFPT zip URL returned HTML (login/wall); skipped",
        "kit_cnc": "skipped — KIT CNC DOI browser/login flow",
        "ieee_pmu": "skipped — IEEE DataPort free-account wall",
        "bmrb_nmr": "skipped — BMRB FID deferred",
    }
    table: dict[str, dict[str, float]] = {}
    per_ds: dict = {}
    ran = []

    for name in ("cwru_bearings", "mitbih_ecg"):
        bundle = bundles.get(name)
        summ_path = OUT / name / "hybrid_lstm" / "summary.json"
        if bundle is None or not summ_path.is_file():
            skipped[name] = "missing bundle or hybrid summary"
            continue
        summary = json.loads(summ_path.read_text(encoding="utf-8"))
        if not summary.get("champ_arch"):
            skipped[name] = "summary lacks champ_arch"
            continue
        batcher = DomainBatcher(bundle, device)
        domain = str(bundle.meta.get("domain", "unknown"))
        base = score_baselines(name, domain, batcher)
        ours_r, _, _ = refit_and_score(summary, batcher, device)
        summary["holdout_refit_R"] = ours_r
        summ_path.write_text(json.dumps(summary, indent=2), encoding="utf-8")
        row = dict(base)
        row["ours_hybrid_lstm"] = ours_r
        table[name] = row
        (OUT / name / "scores.json").write_text(json.dumps(row, indent=2), encoding="utf-8")
        per_ds[name] = {
            "meta": bundle.meta,
            "baseline_labels": {k: BASELINE_LABELS.get(k, k) for k in row},
            "champ_raw": summary.get("champ_raw"),
            "holdout_refit_R": ours_r,
            "iters_done": summary.get("iters_done", summary.get("iters")),
            "note": summary.get("note"),
        }
        ran.append(name)
        print(name, "ours", ours_r, "dual", row.get("dual_cosine"), "nobake", row.get("no_bake"))

    results = {
        "finished_at": utc_now(),
        "config": {
            "iters": 250,
            "fit_steps": 40,
            "batch": 48,
            "pop_size": 8,
            "seed": 1902771841,
            "device": str(device),
            "period_l": 256,
            "metric": "prolonged residual R (DenoiseOpt)",
            "method": "hybrid_lstm only",
        },
        "datasets_ran": ran,
        "skipped": skipped,
        "skipped_optional": skipped,
        "table": table,
        "per_dataset": per_ds,
        "honesty": (
            "Classical board + domain classical proxies (bad-COT / SBMM-lite). "
            "Not a deep SOTA bake-off unless published model weights were run."
        ),
    }
    (OUT / "results_table.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
    plot_bars(table, OUT / "fig_signal_heal_transfer.png", OUT / "fig_signal_heal_transfer.pdf")
    write_readme(OUT / "README.md", results)
    write_paper_note(ROOT / "docs" / "papers" / "denoise_opt" / "SIGNAL_HEAL_TRANSFER_PILOT.md", results)
    if META_PAPER.is_dir():
        write_paper_note(META_PAPER / "docs" / "SIGNAL_HEAL_TRANSFER_PILOT.md", results)
        fig_dst = META_PAPER / "paper" / "v7" / "figures"
        if fig_dst.parent.is_dir():
            fig_dst.mkdir(parents=True, exist_ok=True)
            import shutil

            for ext in ("png", "pdf"):
                src = OUT / f"fig_signal_heal_transfer.{ext}"
                if src.is_file():
                    shutil.copy2(src, fig_dst / f"fig_signal_heal_transfer.{ext}")
    print("Wrote", OUT / "results_table.json")
    return 0 if ran else 1


if __name__ == "__main__":
    raise SystemExit(main())
