#!/usr/bin/env python3
"""Run hybrid_lstm meta-search to plateau (5k outer iters) for one search seed."""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def hybrid_params(cfg_doc: dict, smoke: bool) -> dict:
    if smoke:
        return dict(cfg_doc.get("smoke_hybrid") or {})
    return dict(cfg_doc.get("hybrid") or {})


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--config", type=Path, default=ROOT / "brand/artifacts/phase2_experiments/config.json")
    ap.add_argument("--search-seed", type=int, required=True)
    ap.add_argument("--device", default="cuda" if __import__("torch").cuda.is_available() else "cpu")
    ap.add_argument("--force-fresh", action="store_true")
    ap.add_argument("--smoke", action="store_true")
    ap.add_argument(
        "--skip-if-summary",
        type=lambda x: str(x).lower() in ("1", "true", "yes"),
        default=True,
    )
    args = ap.parse_args()

    cfg_doc = json.loads(args.config.read_text(encoding="utf-8"))
    h = hybrid_params(cfg_doc, args.smoke)
    base = cfg_doc.get("hybrid") or {}
    iters = int(h.get("iters", base.get("iters", 5000)))
    batch = int(h.get("batch", base.get("batch", 48)))
    fit_steps = int(h.get("fit_steps", base.get("fit_steps", 24)))
    pop_size = int(h.get("pop_size", base.get("pop_size", 12)))
    ckpt_every = int(h.get("ckpt_every", base.get("ckpt_every", 25)))

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
        str(iters),
        "--batch",
        str(batch),
        "--fit-steps",
        str(fit_steps),
        "--pop-size",
        str(pop_size),
        "--ckpt-every",
        str(ckpt_every),
        "--device",
        args.device,
        "--out-dir",
        str(out_dir),
    ]
    if args.force_fresh:
        cmd.append("--force-fresh")
    label = "smoke" if args.smoke else "production"
    print(f"RUN hybrid_lstm ({label}) seed={args.search_seed} iters={iters}", flush=True)
    print("RUN", " ".join(cmd), flush=True)
    return subprocess.call(cmd, cwd=str(ROOT))


if __name__ == "__main__":
    raise SystemExit(main())
