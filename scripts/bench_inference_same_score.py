#!/usr/bin/env python3
"""Benchmark fitted overnight models: R_blend vs inference latency.

Selects models near the champion residual (same-score band), times GPU/CPU
forward passes, plots latency vs score, and picks the favorite
(highest R_blend, then lowest latency).

Primary score is locked EVAL_PROTOCOL v10.1 residual_score_blend (α=0.7).
Saved prolonged-R fields on .pt files are legacy hints only.

Run from reelsynth with CUDA venv:
  .venv_gpu/Scripts/python.exe scripts/bench_inference_same_score.py
  .venv_gpu/Scripts/python.exe scripts/bench_inference_same_score.py --from-json brand/artifacts/inference_bench/inference_bench.json
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
    residual = float(
        blob.get("r_blend")
        if blob.get("r_blend") is not None
        else blob.get("residual")
        if blob.get("residual") is not None
        else blob.get("r_seam")
        if blob.get("r_seam") is not None
        else -1.0
    )
    return cfg, cell, residual, arch


@torch.no_grad()
def time_inference(cell, ops, device, batch=64, warmup=10, repeats=50) -> dict:
    ideal, eng = og.make_batch(batch, og.N, device)
    for _ in range(warmup):
        out = og.apply_ops(eng, cell, ops)
        _ = og.residual_score_blend(ideal, eng, out)
    if device.type == "cuda":
        torch.cuda.synchronize()
    t0 = time.perf_counter()
    r = None
    for _ in range(repeats):
        out = og.apply_ops(eng, cell, ops)
        r = og.residual_score_blend(ideal, eng, out).mean()
    if device.type == "cuda":
        torch.cuda.synchronize()
    elapsed = time.perf_counter() - t0
    ms = 1000.0 * elapsed / repeats
    assert r is not None
    return {
        "ms_per_batch": ms,
        "ms_per_sample": ms / batch,
        "residual_live": float(r.item()),
        "primary_metric": "r_blend",
        "blend_alpha": float(og.BLEND_ALPHA),
    }


def _topology_sig(r: dict) -> tuple:
    return (
        int(r.get("depth", -1)),
        int(r.get("width", -1)),
        str(r.get("cell_kind", "")),
        tuple(r.get("blocks") or []),
        int(r.get("n_params", -1)),
    )


def _rank_distinct_top5(results: list[dict]) -> list[dict]:
    """Prefer live R_blend, then lower latency; skip near-duplicate topologies."""
    ordered = sorted(
        results,
        key=lambda x: (-float(x["residual_live"]), float(x["ms_per_batch"])),
    )
    out: list[dict] = []
    seen: set[tuple] = set()
    for r in ordered:
        sig = _topology_sig(r)
        if sig in seen:
            continue
        seen.add(sig)
        out.append(r)
        if len(out) >= 5:
            break
    return out


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--run-dir",
        type=str,
        default="",
        help="fitted/ parent; default = latest overnight run under brand/artifacts/models",
    )
    ap.add_argument(
        "--from-json",
        type=str,
        default="",
        help="Re-score candidate paths from an existing inference_bench.json under R_blend",
    )
    ap.add_argument("--score-tol", type=float, default=0.005, help="|R - champ| band (saved hint)")
    ap.add_argument("--device", type=str, default="cuda")
    ap.add_argument("--batch", type=int, default=64)
    ap.add_argument("--out", type=str, default="")
    args = ap.parse_args()

    device = torch.device(args.device if torch.cuda.is_available() or args.device == "cpu" else "cpu")
    models_root = ROOT / "brand" / "artifacts" / "models"
    run_dir: Path | None = None
    rows = []

    if args.from_json:
        prior = json.loads(Path(args.from_json).read_text(encoding="utf-8"))
        paths = []
        for r in prior.get("results") or []:
            if r.get("path"):
                paths.append(Path(r["path"]))
        fav = (prior.get("favorite") or {}).get("path")
        if fav:
            paths.append(Path(fav))
        # de-dupe preserving order
        seen_p: set[str] = set()
        uniq_paths: list[Path] = []
        for p in paths:
            key = str(p.resolve()) if p.exists() else str(p)
            if key in seen_p:
                continue
            seen_p.add(key)
            uniq_paths.append(p)
        print(f"re-scoring {len(uniq_paths)} paths from {args.from_json} under R_blend")
        for pt in uniq_paths:
            if not pt.exists():
                print(f"skip missing {pt}")
                continue
            try:
                cfg, cell, residual, arch = load_fitted(pt, device)
            except Exception as e:  # noqa: BLE001
                print(f"skip {pt.name}: {e}")
                continue
            rows.append(
                {
                    "tag": pt.stem,
                    "path": str(pt),
                    "run": pt.parents[1].name if pt.parent.name == "fitted" else "",
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
        run_dir = Path(prior.get("run_dir") or (rows[0]["path"] if rows else ".")).resolve()
        if rows and "models" in str(run_dir):
            # keep a representative run_dir string
            run_dir = Path(rows[0]["path"]).parents[1]
    else:
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
            cands = sorted(
                models_root.glob("gpu-rl-arch-*/fitted/*_fitted.pt"),
                key=lambda p: p.stat().st_mtime,
            )
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
                    "run": run_dir.name,
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

    if args.from_json:
        band = rows
        champ_r = max(r["residual_saved"] for r in rows)
        print(f"from-json band size={len(band)} (saved-hint champ={champ_r:.6f})")
    else:
        champ_r = max(r["residual_saved"] for r in rows)
        band = [r for r in rows if abs(r["residual_saved"] - champ_r) <= args.score_tol]
        if len(band) < 2:
            # widen: top-K by residual
            band = sorted(rows, key=lambda x: -x["residual_saved"])[: max(8, min(20, len(rows)))]
            print(f"score band small; using top-{len(band)} by residual (champ={champ_r:.6f})")
        else:
            print(
                f"same-score band |R-champ|<={args.score_tol}: {len(band)} models "
                f"(champ={champ_r:.6f})"
            )

    results = []
    for r in band:
        timing = time_inference(r["cell"], r["ops"], device, batch=args.batch)
        results.append(
            {
                "tag": r["tag"],
                "run": r.get("run", ""),
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
                "primary_metric": "r_blend",
                "blend_alpha": float(og.BLEND_ALPHA),
            }
        )
        print(
            f"{r['tag']}: saved_hint={r['residual_saved']:.6f} "
            f"R_blend_live={timing['residual_live']:.6f} "
            f"{timing['ms_per_batch']:.3f} ms/batch params={r['n_params']}"
        )

    # Favorite: max live R_blend, then min ms_per_batch
    favorite = sorted(results, key=lambda x: (-x["residual_live"], x["ms_per_batch"]))[0]
    # Same-score-fastest under live R_blend band
    live_champ = max(r["residual_live"] for r in results)
    same_band = [r for r in results if live_champ - r["residual_live"] <= 5e-4]
    favorite_fast = sorted(same_band, key=lambda x: x["ms_per_batch"])[0] if same_band else favorite
    top5 = _rank_distinct_top5(results)

    out_dir = Path(args.out) if args.out else ROOT / "brand" / "artifacts" / "inference_bench"
    out_dir.mkdir(parents=True, exist_ok=True)
    payload = {
        "run_dir": str(run_dir),
        "device": str(device),
        "batch": args.batch,
        "primary_metric": "r_blend",
        "blend_alpha": float(og.BLEND_ALPHA),
        "champ_residual_saved_hint": champ_r,
        "champ_residual_live": live_champ,
        "score_tol": args.score_tol,
        "selection_rule": "maximize live R_blend, then minimize ms/batch; favorite_same_score_fastest within live_champ-5e-4",
        "favorite": favorite,
        "favorite_same_score_fastest": favorite_fast,
        "max_residual_model": sorted(results, key=lambda x: -x["residual_live"])[0],
        "top5_distinct": top5,
        "results": results,
        "note": (
            "residual_live is R_blend (α=0.7). residual_saved may still be a prolonged-R "
            "hint from older overnight checkpoints."
        ),
    }
    (out_dir / "inference_bench.json").write_text(json.dumps(payload, indent=2), encoding="utf-8")

    # Plot: latency vs residual
    fig, ax = plt.subplots(figsize=(7.2, 4.2), dpi=140)
    xs = [r["residual_live"] for r in results]
    ys = [r["ms_per_batch"] for r in results]
    ax.scatter(xs, ys, s=36, alpha=0.85, label="candidates")
    ax.scatter(
        [favorite_fast["residual_live"]],
        [favorite_fast["ms_per_batch"]],
        s=120,
        marker="*",
        color="crimson",
        label=f"favorite {favorite_fast['tag'][:24]}",
        zorder=5,
    )
    ax.set_xlabel(r"$R_{\mathrm{blend}}$ (live)")
    ax.set_ylabel(f"Inference latency (ms / batch={args.batch})")
    ax.set_title(r"Same-score band: $R_{\mathrm{blend}}$ vs inference time")
    ax.grid(True, alpha=0.3)
    ax.legend(fontsize=8)
    fig.tight_layout()
    fig.savefig(out_dir / "fig_inference_vs_residual.png")
    plt.close(fig)

    # Bar: top favorites by latency among high-R
    top = sorted(results, key=lambda x: (-x["residual_live"], x["ms_per_batch"]))[:12]
    fig, ax = plt.subplots(figsize=(8.0, 4.0), dpi=140)
    labels = [t["tag"].replace("_fitted", "")[-28:] for t in top]
    ax.barh(range(len(top)), [t["ms_per_batch"] for t in top], color="#4C72B0")
    ax.set_yticks(range(len(top)))
    ax.set_yticklabels(labels, fontsize=7)
    ax.invert_yaxis()
    ax.set_xlabel("ms / batch")
    ax.set_title(r"Inference time (high-$R_{\mathrm{blend}}$ candidates)")
    fig.tight_layout()
    fig.savefig(out_dir / "fig_inference_latency_bars.png")
    plt.close(fig)

    print("FAVORITE (max R_blend):", json.dumps(favorite, indent=2))
    print("FAVORITE_SAME_SCORE_FASTEST:", json.dumps(favorite_fast, indent=2))
    print("TOP5_DISTINCT:")
    for i, r in enumerate(top5, 1):
        print(
            f"  {i}. {r['tag']} R_blend={r['residual_live']:.4f} "
            f"ms={r['ms_per_batch']:.2f} params={r['n_params']}"
        )
    print("wrote", out_dir)


if __name__ == "__main__":
    main()
