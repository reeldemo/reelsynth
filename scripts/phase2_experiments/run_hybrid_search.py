#!/usr/bin/env python3
"""Run hybrid_lstm meta-search for one search seed (default 5k or v14 plateau protocol)."""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PHASE2_OUT = "brand/artifacts/phase2_experiments"


def hybrid_search_out_dir(search_seed: int, *, protocol: str, root: Path = ROOT) -> Path:
    """Artifact root for bench_meta_approaches_5k (--out-dir); holds hybrid_lstm/."""
    _ = protocol
    return root / PHASE2_OUT / f"search_{search_seed}"


def hybrid_params(cfg_doc: dict, smoke: bool, protocol: str) -> dict:
    if protocol == "plateau":
        if smoke:
            return dict(cfg_doc.get("smoke_v14_plateau") or {})
        return dict(cfg_doc.get("v14_plateau") or {})
    if smoke:
        return dict(cfg_doc.get("smoke_hybrid") or {})
    return dict(cfg_doc.get("hybrid") or {})


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--config", type=Path, default=ROOT / "brand/artifacts/phase2_experiments/config.json")
    ap.add_argument("--search-seed", type=int, required=True)
    ap.add_argument("--protocol", choices=("default", "plateau"), default=None)
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
    plateau_seeds = {int(s) for s in (cfg_doc.get("gpu_plateau_search_seeds") or [])}
    protocol = args.protocol or (
        "plateau" if args.search_seed in plateau_seeds else "default"
    )
    h = hybrid_params(cfg_doc, args.smoke, protocol)
    base = cfg_doc.get("v14_plateau" if protocol == "plateau" else "hybrid") or {}
    iters = int(h.get("iters", base.get("iters", 750 if protocol == "plateau" else 5000)))
    batch = int(h.get("batch", base.get("batch", 96 if protocol == "plateau" else 48)))
    fit_steps = int(h.get("fit_steps", base.get("fit_steps", 2048 if protocol == "plateau" else 24)))
    pop_size = int(h.get("pop_size", base.get("pop_size", 12)))
    ckpt_every = int(h.get("ckpt_every", base.get("ckpt_every", 25)))
    proposals = int(h.get("proposals_per_iter", base.get("proposals_per_iter", 2 if protocol == "plateau" else 1)))

    out_dir = hybrid_search_out_dir(args.search_seed, protocol=protocol, root=ROOT)

    summary = out_dir / "hybrid_lstm" / "summary.json"
    if args.skip_if_summary and summary.is_file():
        print(f"SKIP existing summary {summary}")
        return 0

    cmd = [
        sys.executable,
        str(ROOT / "scripts/bench_meta_approaches_5k.py"),
        "--approaches",
        "hybrid_lstm",
        "--protocol",
        protocol,
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
        "--proposals-per-iter",
        str(proposals),
        "--device",
        args.device,
        "--out-dir",
        str(out_dir),
    ]
    if args.force_fresh:
        cmd.append("--force-fresh")
    label = "smoke" if args.smoke else "production"
    print(f"RUN hybrid_lstm ({label}) protocol={protocol} seed={args.search_seed} iters={iters}", flush=True)
    print("RUN", " ".join(cmd), flush=True)
    return subprocess.call(cmd, cwd=str(ROOT))


if __name__ == "__main__":
    raise SystemExit(main())
