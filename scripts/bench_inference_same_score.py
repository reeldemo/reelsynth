#!/usr/bin/env python3
"""Benchmark fitted overnight models: residual score vs inference latency.

Selects models near the champion residual (same-score band), times GPU/CPU
forward passes, plots latency vs score, and picks the favorite
(highest residual, then lowest latency).

Run from reelsynth with CUDA venv:
  .venv_gpu/Scripts/python.exe scripts/bench_inference_same_score.py
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

import matplotlib.pyplot as plt
import torch

# Import overnight module helpers
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402


def load_fitted(pt_path: Path, device: torch.device):
    blob = torch.load(pt_path, map_location="cpu", weights_only=False)
    arch = blob["architecture"]
    cfg = og.ArchConfig(
        depth=int(arch["depth"]),
        width=int(arch["width"]),
        act=str(arch.get("act", "tanh")),
        ops=list(arch.get("ops") or ["mlp_seam"]),
        wet=float(arch.get("wet", 0.5)),
        fir=tuple(arch.get("fir") or (0.25, 0.5, 0.25)),
        cell_kind=str(arch.get("cell_kind", "mlp")),
        soft_logits=list(arch.get("soft_logits") or [0.0] * len(og.OPS)),
        blocks=list(arch.get("blocks") or [arch.get("cell_kind", "mlp")]),
        use_adv_aux=bool(arch.get("use_adv_aux", False)),
        moe_mode=str(arch.get("moe_mode", "sequential")),
    )
    cell = og.SeamCell(cfg).to(device)
    cell.load_state_dict(blob["cell_state_dict"], strict=False)
    cell.eval()
    residual = float(blob.get("residual") or -1.0)
    return cfg, cell, residual, arch


@torch.no_grad()
def time_inference(cell, ops, device, batch=64, warmup=10, repeats=50) -> dict:
    ideal, eng = og.make_batch(batch, og.N, device)
    for _ in range(warmup):
        _ = og.apply_ops(eng, cell, ops)
        _ = og.residual_score(ideal, _)
    if device.type == "cuda":
        torch.cuda.synchronize()
    t0 = time.perf_counter()
    for _ in range(repeats):
        out = og.apply_ops(eng, cell, ops)
        r = og.residual_score(ideal, out).mean()
    if device.type == "cuda":
        torch.cuda.synchronize()
    elapsed = time.perf_counter() - t0
    ms = 1000.0 * elapsed / repeats
    return {"ms_per_batch": ms, "ms_per_sample": ms / batch, "residual_live": float(r.item())}


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--run-dir",
        type=str,
        default="",
        help="fitted/ parent; default = latest overnight run under brand/artifacts/models",
    )
    ap.add_argument("--score-tol", type=float, default=0.005, help="|R - champ| band")
    ap.add_argument("--device", type=str, default="cuda")
    ap.add_argument("--batch", type=int, default=64)
    ap.add_argument("--out", type=str, default="")
    args = ap.parse_args()

    device = torch.device(args.device if torch.cuda.is_available() or args.device == "cpu" else "cpu")
    models_root = ROOT / "brand" / "artifacts" / "models"
    if args.run_dir:
        run_dir = Path(args.run_dir)
    else:
        latest = ROOT / "brand" / "artifacts" / "overnight_gpu_rl_arch_latest.json"
        run_id = None
        if latest.exists():
            meta = json.loads(latest.read_text(encoding="utf-8"))
            # history path often embeds run id
            hp = meta.get("history_path") or meta.get("run_dir") or ""
            for part in Path(str(hp)).parts:
                if part.startswith("gpu-rl-arch-"):
                    run_id = part
                    break
        cands = sorted(models_root.glob("gpu-rl-arch-*/fitted/*_fitted.pt"), key=lambda p: p.stat().st_mtime)
        if run_id:
            prefer = list((models_root / run_id / "fitted").glob("*_fitted.pt"))
            pts = prefer if prefer else cands
        else:
            pts = cands
        if not pts:
            raise SystemExit("no fitted .pt found")
        run_dir = pts[-1].parents[1]

    fitted_dir = run_dir / "fitted"
    pts = sorted(fitted_dir.glob("*_fitted.pt"))
    print(f"scanning {len(pts)} fitted models in {fitted_dir}")

    rows = []
    for pt in pts:
        try:
            cfg, cell, residual, arch = load_fitted(pt, device)
        except Exception as e:  # noqa: BLE001
            print(f"skip {pt.name}: {e}")
            continue
        rows.append(
            {
                "tag": pt.stem,
                "path": str(pt),
                "residual_saved": residual,
                "depth": cfg.depth,
                "width": cfg.width,
                "cell_kind": cfg.cell_kind,
                "blocks": list(cfg.blocks),
                "ops": list(cfg.ops),
                "n_params": sum(p.numel() for p in cell.parameters()),
                "cfg": cfg,
                "cell": cell,
            }
        )

    if not rows:
        raise SystemExit("no loadable models")

    champ_r = max(r["residual_saved"] for r in rows)
    band = [r for r in rows if abs(r["residual_saved"] - champ_r) <= args.score_tol]
    if len(band) < 2:
        # widen: top-K by residual
        band = sorted(rows, key=lambda x: -x["residual_saved"])[: max(8, min(20, len(rows)))]
        print(f"score band small; using top-{len(band)} by residual (champ={champ_r:.6f})")
    else:
        print(f"same-score band |R-champ|<={args.score_tol}: {len(band)} models (champ={champ_r:.6f})")

    results = []
    for r in band:
        timing = time_inference(r["cell"], r["ops"], device, batch=args.batch)
        results.append(
            {
                "tag": r["tag"],
                "residual_saved": r["residual_saved"],
                "residual_live": timing["residual_live"],
                "ms_per_batch": timing["ms_per_batch"],
                "ms_per_sample": timing["ms_per_sample"],
                "depth": r["depth"],
                "width": r["width"],
                "cell_kind": r["cell_kind"],
                "blocks": r["blocks"],
                "n_params": r["n_params"],
                "path": r["path"],
            }
        )
        print(
            f"{r['tag']}: R={r['residual_saved']:.6f} live={timing['residual_live']:.6f} "
            f"{timing['ms_per_batch']:.3f} ms/batch params={r['n_params']}"
        )

    # Favorite: max residual_saved, then min ms_per_batch
    favorite = sorted(results, key=lambda x: (-x["residual_saved"], x["ms_per_batch"]))[0]

    out_dir = Path(args.out) if args.out else ROOT / "brand" / "artifacts" / "inference_bench"
    out_dir.mkdir(parents=True, exist_ok=True)
    payload = {
        "run_dir": str(run_dir),
        "device": str(device),
        "batch": args.batch,
        "champ_residual": champ_r,
        "score_tol": args.score_tol,
        "favorite": favorite,
        "results": results,
    }
    (out_dir / "inference_bench.json").write_text(json.dumps(payload, indent=2), encoding="utf-8")

    # Plot: latency vs residual
    fig, ax = plt.subplots(figsize=(7.2, 4.2), dpi=140)
    xs = [r["residual_saved"] for r in results]
    ys = [r["ms_per_batch"] for r in results]
    ax.scatter(xs, ys, s=36, alpha=0.85, label="candidates")
    ax.scatter(
        [favorite["residual_saved"]],
        [favorite["ms_per_batch"]],
        s=120,
        marker="*",
        color="crimson",
        label=f"favorite {favorite['tag'][:24]}",
        zorder=5,
    )
    ax.set_xlabel("Residual score R (saved)")
    ax.set_ylabel(f"Inference latency (ms / batch={args.batch})")
    ax.set_title("Same-score band: residual vs inference time")
    ax.grid(True, alpha=0.3)
    ax.legend(fontsize=8)
    fig.tight_layout()
    fig.savefig(out_dir / "fig_inference_vs_residual.png")
    plt.close(fig)

    # Bar: top favorites by latency among high-R
    top = sorted(results, key=lambda x: (-x["residual_saved"], x["ms_per_batch"]))[:12]
    fig, ax = plt.subplots(figsize=(8.0, 4.0), dpi=140)
    labels = [t["tag"].replace("_fitted", "")[-28:] for t in top]
    ax.barh(range(len(top)), [t["ms_per_batch"] for t in top], color="#4C72B0")
    ax.set_yticks(range(len(top)))
    ax.set_yticklabels(labels, fontsize=7)
    ax.invert_yaxis()
    ax.set_xlabel("ms / batch")
    ax.set_title("Inference time (high-R candidates)")
    fig.tight_layout()
    fig.savefig(out_dir / "fig_inference_latency_bars.png")
    plt.close(fig)

    print("FAVORITE:", json.dumps(favorite, indent=2))
    print("wrote", out_dir)


if __name__ == "__main__":
    main()
