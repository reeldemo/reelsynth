#!/usr/bin/env python3
"""Plot dense overnight GPU RL history.jsonl → PNG learning curves."""
from __future__ import annotations

import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load_rows(path: Path) -> list[dict]:
    rows: list[dict] = []
    with path.open(encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rows.append(json.loads(line))
    return rows


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("history", type=Path, help="Path to history.jsonl")
    ap.add_argument(
        "--out",
        type=Path,
        default=None,
        help="Output PNG (default: brand/artifacts/figures/<run>_history.png)",
    )
    args = ap.parse_args()

    try:
        import matplotlib

        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        print("ERROR: matplotlib required (pip install matplotlib)", flush=True)
        return 2

    rows = load_rows(args.history)
    if not rows:
        print("ERROR: no history rows", flush=True)
        return 1

    iters = [r["iter"] for r in rows]
    champ = [r.get("champ") for r in rows]
    resid = [r.get("residual") for r in rows]
    bb_rl = [r.get("branch_best_rl") for r in rows]
    bb_nas = [r.get("branch_best_nas") for r in rows]
    bb_combo = [r.get("branch_best_combo") for r in rows]

    run_dir = args.history.parent
    out = args.out
    if out is None:
        fig_dir = ROOT / "brand" / "artifacts" / "figures"
        fig_dir.mkdir(parents=True, exist_ok=True)
        out = fig_dir / f"{run_dir.name}_history.png"

    out.parent.mkdir(parents=True, exist_ok=True)

    fig, axes = plt.subplots(2, 1, figsize=(10, 8), sharex=True)
    axes[0].plot(iters, champ, label="champ", linewidth=1.2)
    axes[0].plot(iters, resid, label="residual", alpha=0.35, linewidth=0.6)
    axes[0].set_ylabel("residual R")
    axes[0].set_title(f"Overnight GPU RL — {run_dir.name}")
    axes[0].legend(loc="lower right")
    axes[0].grid(True, alpha=0.3)

    axes[1].plot(iters, bb_rl, label="branch_best_rl", linewidth=1.0)
    axes[1].plot(iters, bb_nas, label="branch_best_nas", linewidth=1.0)
    axes[1].plot(iters, bb_combo, label="branch_best_combo", linewidth=1.0)
    axes[1].set_xlabel("iter")
    axes[1].set_ylabel("branch best R")
    axes[1].legend(loc="lower right")
    axes[1].grid(True, alpha=0.3)

    fig.tight_layout()
    fig.savefig(out, dpi=140)
    plt.close(fig)
    print(f"wrote {out} ({len(rows)} points)", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
