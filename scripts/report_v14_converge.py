#!/usr/bin/env python3
"""Summarize v14 plateau hybrid search artifacts (training geometry + optional holdout)."""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--out-dir",
        type=Path,
        default=ROOT / "brand/artifacts/meta_approach_compare_v14_converge",
    )
    args = ap.parse_args()
    summary = args.out_dir / "hybrid_lstm" / "summary.json"
    ckpt = args.out_dir / "hybrid_lstm" / "checkpoint.json"
    if not summary.is_file() and not ckpt.is_file():
        print(f"No v14 converge run under {args.out_dir}", file=sys.stderr)
        return 1
    if summary.is_file():
        s = json.loads(summary.read_text(encoding="utf-8"))
        print(
            json.dumps(
                {
                    "protocol": s.get("protocol"),
                    "seed": s.get("seed"),
                    "iters_done": s.get("iters_done"),
                    "champ_R_blend": s.get("champ_raw") or s.get("champ_r"),
                    "champ_hp": s.get("champ_hp"),
                    "wall_h": s.get("wall_h"),
                },
                indent=2,
            )
        )
    if ckpt.is_file():
        c = json.loads(ckpt.read_text(encoding="utf-8"))
        print(f"checkpoint iters_done={c.get('iters_done')} champ_raw={c.get('champ_raw')}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
