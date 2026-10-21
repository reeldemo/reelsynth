#!/usr/bin/env python3
"""Run hybrid_lstm meta-search to plateau (5k outer iters) for one search seed."""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--config", type=Path, default=ROOT / "brand/artifacts/phase2_experiments/config.json")
    ap.add_argument("--search-seed", type=int, required=True)
    ap.add_argument("--device", default="cuda" if __import__("torch").cuda.is_available() else "cpu")
    ap.add_argument("--force-fresh", action="store_true")
    ap.add_argument(
        "--skip-if-summary",
        type=lambda x: str(x).lower() in ("1", "true", "yes"),
        default=True,
    )
    args = ap.parse_args()

    cfg_doc = json.loads(args.config.read_text(encoding="utf-8"))
    h = cfg_doc["hybrid"]
    out_dir = ROOT / "brand/artifacts/phase2_experiments" / f"search_{args.search_seed}"
    summary = out_dir / "hybrid_lstm" / "summary.json"
    if args.skip_if_summary and summary.is_file():
        print(f"SKIP existing summary {summary}")
        return 0

    cmd = [
        sys.executable,
        str(ROOT / "scripts/bench_meta_approaches_5k.py"),
        "--approaches",
        "hybrid_lstm",
        "--seed",
        str(args.search_seed),
        "--iters",
        str(h["iters"]),
        "--batch",
        str(h["batch"]),
        "--fit-steps",
        str(h["fit_steps"]),
        "--pop-size",
        str(h["pop_size"]),
        "--ckpt-every",
        str(h["ckpt_every"]),
        "--device",
        args.device,
        "--out-dir",
        str(out_dir),
    ]
    if args.force_fresh:
        cmd.append("--force-fresh")
    print("RUN", " ".join(cmd), flush=True)
    return subprocess.call(cmd, cwd=str(ROOT))


if __name__ == "__main__":
    raise SystemExit(main())
