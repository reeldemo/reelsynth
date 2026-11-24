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
sys.path.insert(0, str(ROOT / "scripts"))

from phase2_experiments._seeds import (  # noqa: E402
    load_config,
    require_min_search_seeds,
    resolve_search_seeds,
)
from phase2_experiments.report import print_alpha_report, print_holdout_report  # noqa: E402


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
    ap.add_argument(
        "--allow-fewer-seeds",
        action="store_true",
        help="Override Julian ≥3 search-seed rule (not for paper runs)",
    )
    ap.add_argument(
        "--smoke",
        action="store_true",
        help="CI/dev: tiny hybrid budget for plateau seeds + full CPU holdout/α/compute",
    )
    ap.add_argument(
        "--cases",
        action="store_true",
        help="Run pre-registered targeted case suite (PREREGISTRATION.md)",
    )
    ap.add_argument(
        "--cases-only",
        action="store_true",
        help="With --cases: skip main holdout/α/compute pipeline",
    )
    args = ap.parse_args()

    cfg_doc = load_config(args.config)
    out_dir = ROOT / "brand/artifacts/phase2_experiments"
    seeds = resolve_search_seeds(cfg_doc, args.search_seeds)
    require_min_search_seeds(seeds, allow_fewer=args.allow_fewer_seeds, cfg=cfg_doc)

    status: dict = {
        "started_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "device": args.device,
        "cuda_available": bool(__import__("torch").cuda.is_available()),
        "smoke": args.smoke,
        "search_seeds": seeds,
        "min_search_seeds": int(cfg_doc.get("min_search_seeds", 3)),
        "steps": {},
    }

    reuse_map = cfg_doc.get("reuse_search_summary") or {}
    gpu_plateau_seeds = {int(s) for s in (cfg_doc.get("gpu_plateau_search_seeds") or [20_260_727_01, 20_260_727_02])}

    run_cases = args.cases

    if args.cases_only:
        args.skip_search = True

    if not args.skip_search:
        for seed in seeds:
            seed_s = str(seed)
            reuse = reuse_map.get(seed_s)
            if (
                reuse
                and not args.force_fresh_search
                and not (args.smoke and seed in gpu_plateau_seeds)
            ):
                summary = ROOT / reuse
                if summary.is_file():
                    print(f"SKIP search seed {seed}: reuse {summary}")
                    status["steps"][f"search_{seed}"] = "reused_existing_summary"
                    continue
            extra = [
                "--config",
                str(args.config),
                "--search-seed",
                str(seed),
                "--device",
                args.device,
            ]
            if args.smoke:
                extra.append("--smoke")
            if args.force_fresh_search or (args.smoke and seed in gpu_plateau_seeds):
                extra.extend(["--force-fresh", "--skip-if-summary", "false"])
            else:
                extra.extend(["--skip-if-summary", "true"])
            rc = run_py("run_hybrid_search.py", *extra)
            status["steps"][f"search_{seed}"] = "ok" if rc == 0 else f"failed_rc_{rc}"
            if rc != 0:
                status["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
                (out_dir / "RUN_STATUS.json").write_text(json.dumps(status, indent=2), encoding="utf-8")
                return rc
    else:
        print(
            "NOTE: --skip-search skips hybrid plateau training. "
            "Seeds 2026072701 / 2026072702 still need GPU for production 5k runs:\n"
            "  python scripts/phase2_experiments/run_all.py --device cuda",
            flush=True,
        )

    if run_cases:
        extra = ["--config", str(args.config), "--device", args.device]
        if args.smoke:
            extra.append("--smoke")
        rc = run_py("run_cases.py", *extra)
        status["steps"]["cases"] = "ok" if rc == 0 else f"failed_rc_{rc}"
        if rc != 0:
            status["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
            (out_dir / "RUN_STATUS.json").write_text(json.dumps(status, indent=2), encoding="utf-8")
            return rc

    if args.cases_only:
        status["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
        (out_dir / "RUN_STATUS.json").write_text(json.dumps(status, indent=2), encoding="utf-8")
        print(json.dumps(status, indent=2))
        return 0

    for step, script in (
        ("holdout_eval", "holdout_eval.py"),
        ("alpha_sweep", "alpha_sweep.py"),
        ("compute_table", "compute_table.py"),
    ):
        extra = ["--config", str(args.config)]
        if script != "compute_table.py":
            extra.extend(["--device", args.device])
        if args.smoke and script == "holdout_eval.py":
            extra.append("--smoke")
        rc = run_py(script, *extra)
        status["steps"][step] = "ok" if rc == 0 else f"failed_rc_{rc}"
        if rc != 0:
            status["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
            (out_dir / "RUN_STATUS.json").write_text(json.dumps(status, indent=2), encoding="utf-8")
            return rc

    print_holdout_report(out_dir / "holdout_eval.json")
    print_alpha_report(out_dir / "alpha_sensitivity.json")

    status["finished_at"] = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())
    (out_dir / "RUN_STATUS.json").write_text(json.dumps(status, indent=2), encoding="utf-8")
    print(json.dumps(status, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
