#!/usr/bin/env python3
"""Short-budget isolated GA / PPO / GA+PPO / full branch ablations.

Same holdout protocol seed as overnight (DEFAULT_SEED). Labeled short-budget
(default 150 iters) so numbers are not claimed as multi-hour overnight equals.

  .venv_gpu/Scripts/python.exe scripts/run_isolated_ablations.py --iters 150
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PY = ROOT / ".venv_gpu" / "Scripts" / "python.exe"
OG = ROOT / "scripts" / "overnight_gpu_rl_arch.py"


CONFIGS = [
    ("GA-only (isolated short)", "ga", "ablate-ga-only"),
    ("PPO-only (isolated short)", "ppo", "ablate-ppo-only"),
    ("GA+PPO (isolated short)", "ga,ppo", "ablate-ga-ppo"),
    ("Full hybrid (isolated short)", "ppo,nas,pbt,ga,combo", "ablate-full"),
]


def last_history(run_dir: Path) -> dict | None:
    hist = run_dir / "history.jsonl"
    if not hist.exists():
        return None
    last = None
    for line in hist.open(encoding="utf-8"):
        if line.strip():
            last = json.loads(line)
    return last


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--iters", type=int, default=150)
    ap.add_argument("--seed", type=int, default=1902771841)
    ap.add_argument("--pop-size", type=int, default=12)
    ap.add_argument("--batch", type=int, default=48)
    ap.add_argument("--fit-steps", type=int, default=24)
    ap.add_argument("--device", default="cuda")
    ap.add_argument(
        "--out",
        type=Path,
        default=ROOT / "brand/artifacts/isolated_ablations.json",
    )
    ap.add_argument("--skip-existing", action="store_true")
    args = ap.parse_args()

    py = str(PY if PY.exists() else sys.executable)
    results = []
    t_all = time.time()
    for label, branches, run_tag in CONFIGS:
        run_id = f"{run_tag}-{args.iters}it-{args.seed}"
        run_dir = ROOT / "brand" / "artifacts" / "models" / run_id
        last = last_history(run_dir)
        if args.skip_existing and last is not None and int(last.get("iter") or 0) >= args.iters:
            print(f"SKIP existing {run_id}")
        else:
            cmd = [
                py,
                str(OG),
                "--iters",
                str(args.iters),
                "--seed",
                str(args.seed),
                "--pop-size",
                str(args.pop_size),
                "--batch",
                str(args.batch),
                "--fit-steps",
                str(args.fit_steps),
                "--device",
                args.device,
                "--run-id",
                run_id,
                "--branches",
                branches,
                "--ckpt-every",
                str(max(args.iters, 50)),
                "--plateau-adapt-every",
                "0",
                "--algo-tag",
                f"isolated:{branches}",
                "--max-hours",
                "6",
            ]
            print("RUN", " ".join(cmd), flush=True)
            t0 = time.time()
            proc = subprocess.run(cmd, cwd=str(ROOT))
            if proc.returncode != 0:
                print(f"ERROR: {run_id} exited {proc.returncode}", file=sys.stderr)
                return proc.returncode
            print(f"DONE {run_id} in {(time.time()-t0)/60:.1f} min", flush=True)

        last = last_history(run_dir)
        meta = {}
        meta_path = run_dir / "run_meta.json"
        if meta_path.exists():
            meta = json.loads(meta_path.read_text(encoding="utf-8"))
        if last is None:
            print(f"ERROR: no history for {run_id}", file=sys.stderr)
            return 2
        results.append(
            {
                "config": label,
                "branches": branches,
                "run_id": run_id,
                "iters_budget": args.iters,
                "final_iter": int(last.get("iter") or 0),
                "champ_R": float(last.get("champ") or 0.0),
                "dual_cosine_baseline": float(
                    meta.get("dual_cosine_baseline") or last.get("baseline") or 0.0
                ),
                "elapsed_sec": float(last.get("t_sec") or 0.0),
                "source": "isolated_short_budget_rerun",
                "seed": args.seed,
            }
        )

    payload = {
        "protocol": "EVAL_PROTOCOL v1 / Phase 3c isolated short-budget",
        "note": (
            "True isolated branch configs (GA-only, PPO-only, GA+PPO, full) on the same "
            f"search seed {args.seed}. Short budget ({args.iters} iters, plateau adapt off). "
            "Not interchangeable with multi-hour overnight branch-best freezes; report both."
        ),
        "iters": args.iters,
        "seed": args.seed,
        "pop_size": args.pop_size,
        "batch": args.batch,
        "fit_steps": args.fit_steps,
        "wall_hours_total": (time.time() - t_all) / 3600.0,
        "results": results,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload, indent=2))
    print(f"Wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
