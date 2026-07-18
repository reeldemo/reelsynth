#!/usr/bin/env python3
"""Publication-quality plots from dense overnight GPU RL history.jsonl.

Outputs (default):
  - champ residual vs iter (+ DualCosine baseline)
  - branch bests (rl / nas / combo)
  - per-iter residual scatter by branch
  - champion timeline (when champ improves)
Copied to both denoise-opt-meta/paper/v4/figures/ and reelsynth brand/artifacts/figures/.
"""
from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
META_FIG = Path(
    r"C:\Users\Julian\Documents\Programming\github\reeldemo\denoise-opt-meta\paper\v4\figures"
)


def load_rows(path: Path) -> list[dict]:
    rows: list[dict] = []
    with path.open(encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            rows.append(json.loads(line))
    return rows


def load_baseline(run_dir: Path, rows: list[dict]) -> float | None:
    meta = run_dir / "run_meta.json"
    if meta.is_file():
        try:
            d = json.loads(meta.read_text(encoding="utf-8"))
            for k in ("dual_cosine_baseline", "baseline_dual_cosine", "baseline"):
                if k in d and d[k] is not None:
                    return float(d[k])
        except Exception:
            pass
    latest = ROOT / "brand" / "artifacts" / "overnight_gpu_rl_arch_latest.json"
    if latest.is_file():
        try:
            d = json.loads(latest.read_text(encoding="utf-8"))
            if "baseline_dual_cosine" in d:
                return float(d["baseline_dual_cosine"])
        except Exception:
            pass
    # history may not store baseline; leave None
    _ = rows
    return None


def champ_events(rows: list[dict]) -> list[dict]:
    events: list[dict] = []
    best = -1.0
    for r in rows:
        c = r.get("champ")
        if c is None:
            continue
        c = float(c)
        if c > best + 1e-12:
            best = c
            events.append(
                {
                    "iter": int(r["iter"]),
                    "champ": c,
                    "branch": r.get("branch"),
                    "arch_id": r.get("arch_id") or r.get("tag"),
                }
            )
    return events


def style_axes(ax) -> None:
    ax.spines["top"].set_visible(False)
    ax.spines["right"].set_visible(False)
    ax.grid(True, alpha=0.28, linewidth=0.6)
    ax.tick_params(labelsize=9)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("history", type=Path, help="Path to history.jsonl")
    ap.add_argument(
        "--out-dir",
        type=Path,
        default=None,
        help="Primary output directory (default: brand/artifacts/figures/<run>)",
    )
    ap.add_argument(
        "--baseline",
        type=float,
        default=None,
        help="DualCosine baseline R (auto-detect if omitted)",
    )
    ap.add_argument("--dpi", type=int, default=200)
    ap.add_argument("--also-meta-v4", action="store_true", default=True)
    args = ap.parse_args()

    try:
        import matplotlib

        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
        from matplotlib.lines import Line2D
    except ImportError:
        print("ERROR: matplotlib required (pip install matplotlib)", flush=True)
        return 2

    rows = load_rows(args.history)
    if not rows:
        print("ERROR: no history rows", flush=True)
        return 1

    run_dir = args.history.parent
    baseline = args.baseline if args.baseline is not None else load_baseline(run_dir, rows)

    out_dir = args.out_dir
    if out_dir is None:
        out_dir = ROOT / "brand" / "artifacts" / "figures" / run_dir.name
    out_dir.mkdir(parents=True, exist_ok=True)

    iters = [int(r["iter"]) for r in rows]
    champ = [r.get("champ") for r in rows]
    resid = [r.get("residual") for r in rows]
    branches = [r.get("branch") for r in rows]
    bb_rl = [r.get("branch_best_rl") for r in rows]
    bb_nas = [r.get("branch_best_nas") for r in rows]
    bb_combo = [r.get("branch_best_combo") for r in rows]
    events = champ_events(rows)

    final_champ = float(champ[-1]) if champ[-1] is not None else float("nan")
    n = len(rows)

    # --- Fig 1: champion + baseline ---
    fig, ax = plt.subplots(figsize=(9.5, 4.2))
    ax.plot(iters, champ, color="#1a5f7a", linewidth=1.6, label="Champion $R$")
    if baseline is not None:
        ax.axhline(
            baseline,
            color="#c45c26",
            linestyle="--",
            linewidth=1.3,
            label=f"DualCosine baseline ({baseline:.4f})",
        )
    if events:
        ax.scatter(
            [e["iter"] for e in events],
            [e["champ"] for e in events],
            s=18,
            c="#0b3d4a",
            zorder=5,
            label="Champion updates",
        )
    ax.set_xlabel("Iteration")
    ax.set_ylabel(r"Residual score $R$ (1 = best)")
    ax.set_title(f"Overnight GPU RL/NAS — champion residual ({run_dir.name})")
    style_axes(ax)
    ax.legend(loc="lower right", frameon=False, fontsize=9)
    fig.tight_layout()
    p1 = out_dir / "champ_residual_vs_iter.png"
    fig.savefig(p1, dpi=args.dpi)
    plt.close(fig)

    # --- Fig 2: branch bests ---
    fig, ax = plt.subplots(figsize=(9.5, 4.2))
    ax.plot(iters, bb_rl, color="#2a9d8f", linewidth=1.3, label="Branch best: RL")
    ax.plot(iters, bb_nas, color="#e9c46a", linewidth=1.3, label="Branch best: NAS")
    ax.plot(iters, bb_combo, color="#e76f51", linewidth=1.3, label="Branch best: combo")
    if baseline is not None:
        ax.axhline(baseline, color="#6c757d", linestyle="--", linewidth=1.1, label="DualCosine")
    ax.set_xlabel("Iteration")
    ax.set_ylabel(r"Best residual $R$ within branch")
    ax.set_title("Branch competition (running bests)")
    style_axes(ax)
    ax.legend(loc="lower right", frameon=False, fontsize=9)
    fig.tight_layout()
    p2 = out_dir / "branch_bests_vs_iter.png"
    fig.savefig(p2, dpi=args.dpi)
    plt.close(fig)

    # --- Fig 3: per-iter residual by branch ---
    colors = {"rl": "#2a9d8f", "nas": "#e9c46a", "combo": "#e76f51"}
    fig, ax = plt.subplots(figsize=(9.5, 4.2))
    # downsample for readability if huge
    step = max(1, n // 25000)
    for bname, col in colors.items():
        xs = [iters[i] for i in range(0, n, step) if branches[i] == bname]
        ys = [resid[i] for i in range(0, n, step) if branches[i] == bname]
        if xs:
            ax.scatter(xs, ys, s=3, alpha=0.35, c=col, linewidths=0, label=bname)
    ax.plot(iters, champ, color="#1a5f7a", linewidth=1.2, label="champ")
    if baseline is not None:
        ax.axhline(baseline, color="#6c757d", linestyle="--", linewidth=1.0)
    ax.set_xlabel("Iteration")
    ax.set_ylabel(r"Per-iter residual $R$")
    ax.set_title("Trial residuals by search branch")
    style_axes(ax)
    ax.legend(loc="lower right", frameon=False, fontsize=9, markerscale=3)
    fig.tight_layout()
    p3 = out_dir / "residual_by_branch.png"
    fig.savefig(p3, dpi=args.dpi)
    plt.close(fig)

    # --- Fig 4: architecture / champion timeline ---
    fig, ax = plt.subplots(figsize=(9.5, 3.6))
    if events:
        ax.step(
            [e["iter"] for e in events] + [iters[-1]],
            [e["champ"] for e in events] + [events[-1]["champ"]],
            where="post",
            color="#1a5f7a",
            linewidth=1.6,
        )
        for e in events:
            ax.axvline(e["iter"], color="#adb5bd", linewidth=0.5, alpha=0.7)
        # annotate a few late updates
        for e in events[-min(6, len(events)) :]:
            ax.annotate(
                f"{e['champ']:.4f}",
                (e["iter"], e["champ"]),
                textcoords="offset points",
                xytext=(4, 6),
                fontsize=7,
                color="#0b3d4a",
            )
    else:
        ax.plot(iters, champ, color="#1a5f7a", linewidth=1.4)
    if baseline is not None:
        ax.axhline(baseline, color="#c45c26", linestyle="--", linewidth=1.1)
    ax.set_xlabel("Iteration")
    ax.set_ylabel(r"Champion $R$")
    ax.set_title(f"Champion timeline ({len(events)} updates)")
    style_axes(ax)
    fig.tight_layout()
    p4 = out_dir / "champion_timeline.png"
    fig.savefig(p4, dpi=args.dpi)
    plt.close(fig)

    # --- Fig 5: combined panel for paper ---
    fig, axes = plt.subplots(2, 1, figsize=(9.5, 7.2), sharex=True)
    axes[0].plot(iters, champ, color="#1a5f7a", linewidth=1.5, label="Champion $R$")
    if baseline is not None:
        axes[0].axhline(
            baseline,
            color="#c45c26",
            linestyle="--",
            linewidth=1.2,
            label=f"DualCosine ({baseline:.4f})",
        )
    axes[0].set_ylabel(r"Champion $R$")
    axes[0].set_title(
        f"DenoiseOpt overnight RL/NAS ({n:,} dense steps)  |  final champ={final_champ:.4f}"
    )
    style_axes(axes[0])
    axes[0].legend(loc="lower right", frameon=False, fontsize=9)

    axes[1].plot(iters, bb_rl, color="#2a9d8f", linewidth=1.2, label="RL best")
    axes[1].plot(iters, bb_nas, color="#e9c46a", linewidth=1.2, label="NAS best")
    axes[1].plot(iters, bb_combo, color="#e76f51", linewidth=1.2, label="Combo best")
    if baseline is not None:
        axes[1].axhline(baseline, color="#6c757d", linestyle="--", linewidth=1.0)
    axes[1].set_xlabel("Iteration")
    axes[1].set_ylabel("Branch best $R$")
    style_axes(axes[1])
    axes[1].legend(loc="lower right", frameon=False, fontsize=9)
    fig.tight_layout()
    p5 = out_dir / "overnight_panel.png"
    fig.savefig(p5, dpi=args.dpi)
    plt.close(fig)

    # summary json beside figures
    summary = {
        "run_dir": str(run_dir),
        "n_points": n,
        "final_iter": iters[-1],
        "final_champ": final_champ,
        "baseline_dual_cosine": baseline,
        "delta_vs_baseline": (final_champ - baseline) if baseline is not None else None,
        "n_champ_updates": len(events),
        "champ_events_tail": events[-10:],
        "figures": [p.name for p in (p1, p2, p3, p4, p5)],
    }
    (out_dir / "plot_summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")

    written = [p1, p2, p3, p4, p5]
    if args.also_meta_v4:
        META_FIG.mkdir(parents=True, exist_ok=True)
        for p in written:
            shutil.copy2(p, META_FIG / p.name)
        shutil.copy2(out_dir / "plot_summary.json", META_FIG / "plot_summary.json")
        print(f"also copied to {META_FIG}", flush=True)

    print(f"wrote {out_dir} ({n} points, champ={final_champ:.6f}, baseline={baseline})", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
