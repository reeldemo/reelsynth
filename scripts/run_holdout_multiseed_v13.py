#!/usr/bin/env python3
"""Run holdout benches across multiple eval seeds for v13 multi-seed stats."""
from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "brand" / "artifacts" / "holdout_multiseed_v13"
SEEDS = [20260719, 20260720, 20260721, 20260722, 20260723]
CHAMP = ROOT / "brand/artifacts/meta_approach_compare_v10/hybrid_lstm/champ_cell.pt"
PY = ROOT / ".venv_gpu" / "Scripts" / "python.exe"


def run(cmd: list[str]) -> None:
    print("+", " ".join(cmd), flush=True)
    subprocess.check_call(cmd, cwd=str(ROOT))


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    py = str(PY if PY.is_file() else sys.executable)
    for seed in SEEDS:
        seed_dir = OUT / str(seed)
        seed_dir.mkdir(parents=True, exist_ok=True)

        # N2N vs Ours four-board
        n2n_dir = seed_dir / "n2n_vs_ours"
        run(
            [
                py,
                "scripts/bench_v10_n2n_vs_ours.py",
                "--eval-seed",
                str(seed),
                "--out-dir",
                str(n2n_dir),
                "--champ",
                str(CHAMP),
                "--skip-real-wt",
            ]
        )

        # Cliff strata
        cliff_out = seed_dir / "cliff_strata.json"
        run(
            [
                py,
                "scripts/bench_cliff_strata.py",
                "--eval-seed",
                str(seed),
                "--out",
                str(cliff_out),
                "--n-tiles",
                "1024",
            ]
        )

    # Canonical once with built-in multi-seed (eval_seed + k for k in 0..4)
    run(
        [
            py,
            "scripts/bench_canonical_eval_dataset.py",
            "--eval-seed",
            str(SEEDS[0]),
            "--multi-seeds",
            str(len(SEEDS)),
            "--score-batch",
            "64",
        ]
    )
    canon = ROOT / "brand" / "artifacts" / "canonical_eval_dataset"
    for name in ("method_scores.json", "dataset_metrics.json"):
        src = canon / name
        if src.is_file():
            (OUT / name).write_bytes(src.read_bytes())

    manifest = {
        "schema": "denoiseopt.holdout_multiseed_v13.v1",
        "seeds": SEEDS,
        "champ": str(CHAMP),
        "out": str(OUT),
    }
    (OUT / "manifest.json").write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    print(json.dumps(manifest, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
