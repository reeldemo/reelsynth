#!/usr/bin/env python3
"""Periodic human-readable report for v14 FitCell-converge search."""
from __future__ import annotations

import argparse
import json
import statistics
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUT = ROOT / "brand" / "artifacts" / "meta_approach_compare_v14_converge"
SEEDS = (1902771841, 2026072701, 2026072702)
APPROACHES = ("hybrid_lstm",)


def last_hist(path: Path) -> dict | None:
    if not path.is_file() or path.stat().st_size <= 0:
        return None
    with path.open("rb") as f:
        f.seek(max(0, path.stat().st_size - 65536))
        chunk = f.read().decode("utf-8", errors="ignore")
    lines = [ln for ln in chunk.splitlines() if ln.strip()]
    if not lines:
        return None
    try:
        return json.loads(lines[-1])
    except Exception:
        return None


def load_json(path: Path) -> dict | None:
    if not path.is_file():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def hist_fit_stats(path: Path, n: int = 50) -> dict:
    if not path.is_file():
        return {}
    rows: list[dict] = []
    with path.open("rb") as f:
        f.seek(max(0, path.stat().st_size - 512_000))
        chunk = f.read().decode("utf-8", errors="ignore")
    for ln in chunk.splitlines():
        ln = ln.strip()
        if not ln:
            continue
        try:
            rows.append(json.loads(ln))
        except Exception:
            continue
    rows = rows[-n:]
    steps = [float(r["fit_steps_used"]) for r in rows if r.get("fit_steps_used") is not None]
    conv = [1.0 if r.get("fit_converged") else 0.0 for r in rows if "fit_converged" in r]
    out: dict = {"n": len(rows)}
    if steps:
        out["fit_steps_mean"] = round(statistics.mean(steps), 1)
        out["fit_steps_p50"] = round(statistics.median(steps), 1)
        out["fit_steps_max"] = int(max(steps))
    if conv:
        out["converged_frac"] = round(sum(conv) / len(conv), 3)
    return out


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--out-dir", type=Path, default=DEFAULT_OUT)
    ap.add_argument("--write-status", action="store_true")
    args = ap.parse_args()
    out_dir: Path = args.out_dir
    print(f"=== v14 converge report {datetime.now(timezone.utc).isoformat()} ===")
    print(f"out={out_dir}")
    report: dict = {
        "schema": "denoiseopt.v14_converge_report.v1",
        "updated_at": datetime.now(timezone.utc).isoformat(),
        "seeds": {},
    }
    for seed in SEEDS:
        seed_dir = out_dir / str(seed)
        print(f"\nseed {seed}")
        seed_blob: dict = {}
        for name in APPROACHES:
            ad = seed_dir / name
            ckpt = load_json(ad / "checkpoint.json") or {}
            summary = load_json(ad / "summary.json") or {}
            last = last_hist(ad / "history.jsonl") or {}
            fit = hist_fit_stats(ad / "history.jsonl")
            done = max(
                int(last.get("iter") or 0),
                int(ckpt.get("iters_done") or 0),
                int(summary.get("iters_done") or 0),
            )
            r = last.get("champ_raw", summary.get("champ_raw", ckpt.get("champ_raw", ckpt.get("champ_r"))))
            line = (
                f"  {name:12s} {done:4d}/750  champ_R={r}  "
                f"fit_mean={fit.get('fit_steps_mean')} conv={fit.get('converged_frac')} "
                f"last_fit={last.get('fit_steps_used')} ok={last.get('fit_converged')} "
                f"free_mib={last.get('cuda_free_mib')}"
            )
            print(line)
            seed_blob[name] = {
                "iters_done": done,
                "champ_r": r,
                "fit": fit,
                "last": {
                    k: last.get(k)
                    for k in (
                        "fit_steps_used",
                        "fit_converged",
                        "lambda_latency",
                        "t_ms",
                        "j",
                        "cuda_free_mib",
                        "hp",
                    )
                },
            }
        report["seeds"][str(seed)] = seed_blob
    if args.write_status:
        path = out_dir / "REPORT.json"
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(f"\nWrote {path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
