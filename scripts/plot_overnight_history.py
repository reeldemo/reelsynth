#!/usr/bin/env python3
"""Publication plots from overnight GPU RL history.jsonl (twocolumn-safe).

Sized for arXiv twocolumn: ~3.3in column width at 220 dpi, large labels,
minimal in-plot annotation. DualCosine baseline when available.
Copies to denoise-opt-meta/paper/v5/figures/ and docs/papers/denoise_opt/v5/figures/.

Line series use distinct markers + Okabe-Ito colorblind-safe colors so branches
remain distinguishable in grayscale print (markevery keeps markers sparse).
"""
from __future__ import annotations

import argparse
import json
import shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
META_FIG = Path(
    r"C:\Users\Julian\Documents\Programming\github\reeldemo\denoise-opt-meta\paper\v5\figures"
)
DOCS_FIG = ROOT / "docs" / "papers" / "denoise_opt" / "v5" / "figures"

# Physical inches: readable when shrunk to \\columnwidth (~3.3in)
COL_W, COL_H = 5.6, 3.5
PANEL_H = 6.2
FONT = {
    "axes.titlesize": 11,
    "axes.labelsize": 11,
    "xtick.labelsize": 10,
    "ytick.labelsize": 10,
    "legend.fontsize": 9,
    "figure.titlesize": 11,
}

# Okabe-Ito palette (colorblind-safe) + distinct markers for B&W print
CHAMP_STYLE = {
    "color": "#000000",
    "marker": "o",
    "linestyle": "-",
    "linewidth": 1.8,
}
BASELINE_STYLE = {
    "color": "#999999",
    "linestyle": "--",
    "linewidth": 1.4,
}
BRANCH_STYLE = {
    "rl": {"color": "#0072B2", "marker": "o", "linestyle": "-"},
    "ppo": {"color": "#0072B2", "marker": "o", "linestyle": "-"},
    "ga": {"color": "#009E73", "marker": "s", "linestyle": "-"},
    "pbt": {"color": "#E69F00", "marker": "^", "linestyle": "-"},
    "nas": {"color": "#CC79A7", "marker": "D", "linestyle": "-"},
    "combo": {"color": "#D55E00", "marker": "v", "linestyle": "-"},
}
# Legacy alias used by scatter fig
BRANCH_COLORS = {k: v["color"] for k, v in BRANCH_STYLE.items()}


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


def series(rows: list[dict], *keys: str) -> list[float | None]:
    out: list[float | None] = []
    for r in rows:
        v = None
        for k in keys:
            if r.get(k) is not None:
                v = float(r[k])
                break
        out.append(v)
    return out


def style_axes(ax, *, xmax: int | None = None) -> None:
    ax.spines["top"].set_visible(False)
    ax.spines["right"].set_visible(False)
    ax.grid(True, alpha=0.25, linewidth=0.55)
    ax.tick_params(labelsize=10)
    if xmax is not None:
        lo, _ = ax.get_xlim()
        ax.set_xlim(lo, float(xmax))


def markevery_for(n: int, target_marks: int = 14) -> int | slice:
    """Sparse markers so dense overnight traces stay readable in print."""
    if n <= 1:
        return 1
    if n <= target_marks:
        return 1
    return max(1, n // target_marks)


def plot_marked(ax, xs, ys, *, color, marker, linestyle="-", linewidth=1.5, label=None, zorder=2):
    n = len(xs)
    ax.plot(
        xs,
        ys,
        color=color,
        marker=marker,
        linestyle=linestyle,
        linewidth=linewidth,
        markersize=5.5,
        markevery=markevery_for(n),
        markerfacecolor=color,
        markeredgecolor="white",
        markeredgewidth=0.55,
        label=label,
        zorder=zorder,
    )


def pick_annotate(events: list[dict], max_n: int = 3) -> list[dict]:
    """Keep sparse labels: first, last, and one mid update spaced in iteration."""
    if not events:
        return []
    if len(events) <= max_n:
        return events
    first, last = events[0], events[-1]
    mid_candidates = [e for e in events[1:-1] if e["iter"] > first["iter"] + 30]
    mid = None
    if mid_candidates:
        # prefer a late mid update so labels do not pile on the left
        target = first["iter"] + 0.45 * (last["iter"] - first["iter"])
        mid = min(mid_candidates, key=lambda e: abs(e["iter"] - target))
    chosen = [first]
    if mid is not None:
        chosen.append(mid)
    chosen.append(last)
    return chosen[:max_n]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("history", type=Path, help="Path to history.jsonl")
    ap.add_argument("--out-dir", type=Path, default=None)
    ap.add_argument("--baseline", type=float, default=None)
    ap.add_argument(
        "--max-iter",
        type=int,
        default=None,
        help="Truncate history to iter <= N (paper freeze)",
    )
    ap.add_argument("--dpi", type=int, default=220)
    ap.add_argument("--also-meta-v5", action="store_true", default=True)
    ap.add_argument("--also-docs-v5", action="store_true", default=True)
    args = ap.parse_args()

    try:
        import matplotlib

        matplotlib.use("Agg")
        import matplotlib.pyplot as plt
    except ImportError:
        print("ERROR: matplotlib required (pip install matplotlib)", flush=True)
        return 2

    plt.rcParams.update(FONT)

    rows = load_rows(args.history)
    if not rows:
        print("ERROR: no history rows", flush=True)
        return 1

    if args.max_iter is not None:
        rows = [r for r in rows if int(r["iter"]) <= args.max_iter]
        if not rows:
            print(f"ERROR: no rows with iter <= {args.max_iter}", flush=True)
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
    branches = [(r.get("branch") or "").lower() for r in rows]
    bb_rl = series(rows, "branch_best_rl", "branch_best_ppo")
    bb_nas = series(rows, "branch_best_nas")
    bb_combo = series(rows, "branch_best_combo")
    bb_ga = series(rows, "branch_best_ga")
    bb_pbt = series(rows, "branch_best_pbt")
    events = champ_events(rows)

    final_champ = float(champ[-1]) if champ[-1] is not None else float("nan")
    n = len(rows)

    # --- Fig 1: champion + baseline ---
    fig, ax = plt.subplots(figsize=(COL_W, COL_H))
    plot_marked(
        ax,
        iters,
        champ,
        color=CHAMP_STYLE["color"],
        marker=CHAMP_STYLE["marker"],
        linestyle=CHAMP_STYLE["linestyle"],
        linewidth=CHAMP_STYLE["linewidth"],
        label="Champion $R$",
        zorder=3,
    )
    if baseline is not None:
        ax.axhline(
            baseline,
            color=BASELINE_STYLE["color"],
            linestyle=BASELINE_STYLE["linestyle"],
            linewidth=BASELINE_STYLE["linewidth"],
            label=f"DualCosine ({baseline:.3f})",
            zorder=1,
        )
    if events:
        ax.scatter(
            [e["iter"] for e in events],
            [e["champ"] for e in events],
            s=36,
            c=CHAMP_STYLE["color"],
            marker="*",
            zorder=5,
            label="Updates",
            edgecolors="white",
            linewidths=0.4,
        )
    ax.set_xlabel("Iteration")
    ax.set_ylabel(r"Residual $R$ (1 = best)")
    ax.set_title("Champion residual vs DualCosine")
    style_axes(ax, xmax=args.max_iter)
    ax.legend(loc="lower right", frameon=False, fontsize=9)
    fig.tight_layout()
    p1 = out_dir / "champ_residual_vs_iter.png"
    fig.savefig(p1, dpi=args.dpi, bbox_inches="tight")
    plt.close(fig)

    # --- Fig 2: branch bests ---
    fig, ax = plt.subplots(figsize=(COL_W, COL_H))
    branch_series = [
        ("ppo", bb_rl, "PPO/RL best"),
        ("ga", bb_ga, "GA best"),
        ("pbt", bb_pbt, "PBT best"),
        ("nas", bb_nas, "NAS best"),
        ("combo", bb_combo, "Combo best"),
    ]
    for key, ys, lab in branch_series:
        if any(v is not None for v in ys):
            st = BRANCH_STYLE[key]
            plot_marked(
                ax,
                iters,
                ys,
                color=st["color"],
                marker=st["marker"],
                linestyle=st["linestyle"],
                linewidth=1.5,
                label=lab,
            )
    if baseline is not None:
        ax.axhline(
            baseline,
            color=BASELINE_STYLE["color"],
            linestyle=BASELINE_STYLE["linestyle"],
            linewidth=1.2,
            label="DualCosine",
        )
    ax.set_xlabel("Iteration")
    ax.set_ylabel(r"Branch-best $R$")
    ax.set_title("Branch competition")
    style_axes(ax, xmax=args.max_iter)
    ax.legend(loc="lower right", frameon=False, fontsize=8, ncol=2)
    fig.tight_layout()
    p2 = out_dir / "branch_bests_vs_iter.png"
    fig.savefig(p2, dpi=args.dpi, bbox_inches="tight")
    plt.close(fig)

    # --- Fig 3: per-iter residual by branch ---
    fig, ax = plt.subplots(figsize=(COL_W, COL_H))
    step = max(1, n // 12000)
    present = sorted({b for b in branches if b})
    scatter_markers = {
        "rl": "o",
        "ppo": "o",
        "ga": "s",
        "pbt": "^",
        "nas": "D",
        "combo": "v",
    }
    for bname in present:
        col = BRANCH_COLORS.get(bname, "#888888")
        mk = scatter_markers.get(bname, "o")
        xs = [iters[i] for i in range(0, n, step) if branches[i] == bname]
        ys = [resid[i] for i in range(0, n, step) if branches[i] == bname]
        if xs:
            ax.scatter(
                xs,
                ys,
                s=10,
                alpha=0.35,
                c=col,
                marker=mk,
                linewidths=0,
                label=bname.upper(),
            )
    plot_marked(
        ax,
        iters,
        champ,
        color=CHAMP_STYLE["color"],
        marker=CHAMP_STYLE["marker"],
        linestyle=CHAMP_STYLE["linestyle"],
        linewidth=1.5,
        label="Champ",
        zorder=4,
    )
    if baseline is not None:
        ax.axhline(
            baseline,
            color=BASELINE_STYLE["color"],
            linestyle=BASELINE_STYLE["linestyle"],
            linewidth=1.1,
        )
    ax.set_xlabel("Iteration")
    ax.set_ylabel(r"Per-trial $R$")
    ax.set_title("Trial residuals by branch")
    style_axes(ax, xmax=args.max_iter)
    ax.legend(loc="lower right", frameon=False, fontsize=8, markerscale=1.4, ncol=2)
    fig.tight_layout()
    p3 = out_dir / "residual_by_branch.png"
    fig.savefig(p3, dpi=args.dpi, bbox_inches="tight")
    plt.close(fig)

    # --- Fig 4: champion timeline (sparse labels) ---
    fig, ax = plt.subplots(figsize=(COL_W, COL_H * 0.95))
    if events:
        xs = [e["iter"] for e in events] + [iters[-1]]
        ys = [e["champ"] for e in events] + [events[-1]["champ"]]
        ax.step(xs, ys, where="post", color=CHAMP_STYLE["color"], linewidth=1.8)
        ax.scatter(
            [e["iter"] for e in events],
            [e["champ"] for e in events],
            s=40,
            c=CHAMP_STYLE["color"],
            marker="o",
            zorder=5,
            edgecolors="white",
            linewidths=0.5,
        )
        labels = pick_annotate(events, max_n=3)
        for e in labels:
            ax.axvline(e["iter"], color="#adb5bd", linewidth=0.7, alpha=0.75)
        # Place last label below the plateau so it is not clipped at the top
        offsets = [(8, 8), (8, -16), (-36, -14)]
        for e, off in zip(labels, offsets):
            ax.annotate(
                f"{e['champ']:.3f}",
                (e["iter"], e["champ"]),
                textcoords="offset points",
                xytext=off,
                fontsize=10,
                color="#0b3d4a",
                clip_on=False,
            )
        y_vals = [e["champ"] for e in events]
        if baseline is not None:
            y_vals.append(baseline)
        ymin, ymax = min(y_vals), max(y_vals)
        pad = max(0.02, 0.08 * (ymax - ymin + 1e-6))
        ax.set_ylim(ymin - pad, min(1.002, ymax + pad))
    else:
        plot_marked(
            ax,
            iters,
            champ,
            color=CHAMP_STYLE["color"],
            marker=CHAMP_STYLE["marker"],
            linestyle=CHAMP_STYLE["linestyle"],
            linewidth=1.6,
        )
    if baseline is not None:
        ax.axhline(
            baseline,
            color=BASELINE_STYLE["color"],
            linestyle=BASELINE_STYLE["linestyle"],
            linewidth=1.2,
        )
    ax.set_xlabel("Iteration")
    ax.set_ylabel(r"Champion $R$")
    ax.set_title(f"Champion updates ($n$={len(events)})")
    style_axes(ax, xmax=args.max_iter)
    fig.tight_layout()
    p4 = out_dir / "champion_timeline.png"
    fig.savefig(p4, dpi=args.dpi, bbox_inches="tight", pad_inches=0.2)
    plt.close(fig)

    # --- Fig 5: full-width panel ---
    fig, axes = plt.subplots(2, 1, figsize=(COL_W * 1.55, PANEL_H), sharex=True)
    plot_marked(
        axes[0],
        iters,
        champ,
        color=CHAMP_STYLE["color"],
        marker=CHAMP_STYLE["marker"],
        linestyle=CHAMP_STYLE["linestyle"],
        linewidth=1.7,
        label="Champion $R$",
        zorder=3,
    )
    if baseline is not None:
        axes[0].axhline(
            baseline,
            color=BASELINE_STYLE["color"],
            linestyle=BASELINE_STYLE["linestyle"],
            linewidth=1.3,
            label=f"DualCosine ({baseline:.3f})",
        )
    axes[0].set_ylabel(r"Champion $R$")
    axes[0].set_title(f"Overnight monitoring ({n:,} steps, champ $R$={final_champ:.3f})")
    style_axes(axes[0], xmax=args.max_iter)
    axes[0].legend(loc="lower right", frameon=False, fontsize=9)

    panel_branch = [
        ("ppo", bb_rl, "PPO/RL"),
        ("ga", bb_ga, "GA"),
        ("pbt", bb_pbt, "PBT"),
        ("nas", bb_nas, "NAS"),
        ("combo", bb_combo, "Combo"),
    ]
    for key, ys, lab in panel_branch:
        if any(v is not None for v in ys):
            st = BRANCH_STYLE[key]
            plot_marked(
                axes[1],
                iters,
                ys,
                color=st["color"],
                marker=st["marker"],
                linestyle=st["linestyle"],
                linewidth=1.4,
                label=lab,
            )
    if baseline is not None:
        axes[1].axhline(
            baseline,
            color=BASELINE_STYLE["color"],
            linestyle=BASELINE_STYLE["linestyle"],
            linewidth=1.1,
        )
    axes[1].set_xlabel("Iteration")
    axes[1].set_ylabel(r"Branch-best $R$")
    style_axes(axes[1], xmax=args.max_iter)
    axes[1].legend(loc="lower right", frameon=False, fontsize=8, ncol=3)
    fig.tight_layout()
    p5 = out_dir / "overnight_panel.png"
    fig.savefig(p5, dpi=args.dpi, bbox_inches="tight")
    plt.close(fig)

    summary = {
        "run_dir": str(run_dir),
        "n_points": n,
        "final_iter": iters[-1],
        "max_iter_arg": args.max_iter,
        "final_champ": final_champ,
        "baseline_dual_cosine": baseline,
        "delta_vs_baseline": (final_champ - baseline) if baseline is not None else None,
        "n_champ_updates": len(events),
        "champ_events_tail": events[-10:],
        "figures": [p.name for p in (p1, p2, p3, p4, p5)],
        "bw_markers": True,
        "palette": "okabe_ito",
    }
    (out_dir / "plot_summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")

    written = [p1, p2, p3, p4, p5, out_dir / "plot_summary.json"]
    if args.also_meta_v5:
        META_FIG.mkdir(parents=True, exist_ok=True)
        for p in written:
            shutil.copy2(p, META_FIG / p.name)
        print(f"also copied to {META_FIG}", flush=True)
    if args.also_docs_v5:
        DOCS_FIG.mkdir(parents=True, exist_ok=True)
        for p in written:
            shutil.copy2(p, DOCS_FIG / p.name)
        print(f"also copied to {DOCS_FIG}", flush=True)

    print(
        f"wrote {out_dir} ({n} points, champ={final_champ:.6f}, baseline={baseline}"
        f", max_iter={args.max_iter})",
        flush=True,
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
