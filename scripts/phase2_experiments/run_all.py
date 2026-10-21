#!/usr/bin/env python3
"""Orchestrate phase-2 experiments (hybrid search → holdout → α sweep → compute table)."""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts" / "phase2_experiments"


def run_py(script: str, *extra: str) -> int:
    cmd = [sys.executable, str(SCRIPTS / script), *extra]
    print("RUN", " ".join(cmd), flush=True)
    return subprocess.call(cmd, cwd=str(ROOT))


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--config", type=Path, default=ROOT / "brand/artifacts/phase2_experiments/config.json")
    ap.add_argument("--device", default="cuda" if __import__("torch").cuda.is_available() else "cpu")
    ap.add_argument("--skip-search", action="store_true", help="Only eval / tables (reuse summaries)")
    ap.add_argument("--force-fresh-search", action="store_true")
    ap.add_argument("--search-seeds", type=str, default="", help="Comma subset; default all in config")
    args = ap.parse_args()

    cfg_doc = json.loads(args.config.read_text(encoding="utf-8"))
    out_dir = ROOT / "brand/artifacts/phase2_experiments"
    seeds = cfg_doc["search_seeds"]
    if args.search_seeds.strip():
        seeds = [int(s.strip()) for s in args.search_seeds.split(",") if s.strip()]

    status: dict = {
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "device": args.device,
        "cuda_available": bool(__import__("torch").cuda.is_available()),
        "search_seeds": seeds,
        "steps": {},
    }

    if not args.skip_search:
        for seed in seeds:
            reuse = (cfg_doc.get("reuse_search_summary") or {}).get(str(seed))
            if reuse and not args.force_fresh_search:
                summary = ROOT / reuse
                if summary.is_file():
                    print(f"SKIP search seed {seed}: reuse {summary}")
                    status["steps"][f"search_{seed}"] = "reused_existing_summary"
                    continue
            rc = run_py(
                "run_hybrid_search.py",
                "--config",
                str(args.config),
                "--search-seed",
                str(seed),
                "--device",
                args.device,
                *(["--force-fresh"] if args.force_fresh_search else []),
                "--skip-if-summary",
                "false" if args.force_fresh_search else "true",
            )
            status["steps"][f"search_{seed}"] = "ok" if rc == 0 else f"failed_rc_{rc}"
            if rc != 0:
                status["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
                (out_dir / "RUN_STATUS.json").write_text(json.dumps(status, indent=2), encoding="utf-8")
                return rc

    for step, script in (
        ("holdout_eval", "holdout_eval.py"),
        ("alpha_sweep", "alpha_sweep.py"),
        ("compute_table", "compute_table.py"),
    ):
        rc = run_py(script, "--config", str(args.config), "--device", args.device)
        status["steps"][step] = "ok" if rc == 0 else f"failed_rc_{rc}"
        if rc != 0:
            status["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
            (out_dir / "RUN_STATUS.json").write_text(json.dumps(status, indent=2), encoding="utf-8")
            return rc

    status["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    (out_dir / "RUN_STATUS.json").write_text(json.dumps(status, indent=2), encoding="utf-8")
    print(json.dumps(status, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
