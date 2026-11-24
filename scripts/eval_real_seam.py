#!/usr/bin/env python3
"""Evaluate DenoiseOpt checkpoints on the real-seam holdout (build_real_seam_set.py).

Reports per model and per subset (akwf / factory_fx):
  - R_blend (paper primary, ~0.98–0.99 on synthetic holdout)
  - R_seam / seam-window residual (local)
  - R_body (timbre / identity on mid-cycle)

Writes eval_real_seam.json and eval_real_seam.csv. CPU or single-GPU friendly.
"""
from __future__ import annotations

import argparse
import csv
import json
import sys
import time
from pathlib import Path
from typing import Any, Callable

import numpy as np
import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import metrics_snr_sdr as msm  # noqa: E402
import overnight_gpu_rl_arch as og  # noqa: E402


def load_fitted(pt_path: Path, device: torch.device):
    """Minimal loader (avoids bench_inference_same_score → matplotlib)."""
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
    state = {k: v for k, v in blob["cell_state_dict"].items()}
    if "fir" in state and state["fir"].shape[0] != cell.fir.shape[0]:
        pad = torch.zeros(cell.fir.shape[0], dtype=state["fir"].dtype)
        n = min(state["fir"].shape[0], pad.shape[0])
        pad[:n] = state["fir"][:n]
        state["fir"] = pad
    og.load_state_dict_compatible(cell, state)
    cell.eval()
    residual = float(blob.get("residual") or -1.0)
    return cfg, cell, residual, arch

DEFAULT_SET = ROOT / "brand" / "artifacts" / "real_seam_eval"
DEFAULT_HYBRID = ROOT / "brand" / "artifacts" / "meta_approach_compare_v10" / "hybrid_lstm" / "champ_cell.pt"
DEFAULT_E4_DIR = ROOT / "brand" / "artifacts" / "models" / "gpu-rl-arch-20260718T101140Z" / "fitted"
DEFAULT_E4_TAGS = (
    "champion_iter_000195_fitted.pt",
    "champion_iter_000038_fitted.pt",
    "champion_iter_000030_fitted.pt",
    "champion_iter_000029_fitted.pt",
)


def load_corpus(data_dir: Path, device: torch.device) -> tuple[torch.Tensor, torch.Tensor, list[dict], dict]:
    man_path = data_dir / "real_seam_manifest.json"
    if not man_path.is_file():
        raise SystemExit(f"missing manifest — run scripts/build_real_seam_set.py first ({man_path})")
    manifest_doc = json.loads(man_path.read_text(encoding="utf-8"))
    samples = manifest_doc.get("samples") or []
    pt_path = data_dir / "real_seam_set.pt"
    npz_path = data_dir / "real_seam_set.npz"
    if pt_path.is_file():
        blob = torch.load(pt_path, map_location="cpu", weights_only=False)
        ideal = blob["ideal"].float().to(device)
        eng = blob["engine"].float().to(device)
    elif npz_path.is_file():
        z = np.load(npz_path)
        ideal = torch.from_numpy(z["ideal"]).float().to(device)
        eng = torch.from_numpy(z["engine"]).float().to(device)
    else:
        raise SystemExit(f"missing real_seam_set.pt or .npz under {data_dir}")
    return ideal, eng, samples, manifest_doc


def subset_mask(samples: list[dict], key: str) -> torch.Tensor:
    flags = [1 if (s.get("subset") or "").startswith(key) or s.get("subset") == key else 0 for s in samples]
    return torch.tensor(flags, dtype=torch.bool)


@torch.no_grad()
def score_batch(
    ideal: torch.Tensor,
    eng: torch.Tensor,
    out: torch.Tensor,
) -> dict[str, float]:
    r_blend = og.residual_score_blend(ideal, eng, out)
    r_seam = og.residual_score_seam(ideal, out)
    r_body = og.residual_score_body(eng, out)
    edge = msm.edge_rmse(ideal, out, seam_w=og.SEAM_W)
    return {
        "r_blend_mean": float(r_blend.mean().item()),
        "r_blend_std": float(r_blend.std(unbiased=False).item()),
        "r_seam_mean": float(r_seam.mean().item()),
        "r_seam_std": float(r_seam.std(unbiased=False).item()),
        "r_body_mean": float(r_body.mean().item()),
        "r_body_std": float(r_body.std(unbiased=False).item()),
        "edge_rmse_mean": float(edge.mean().item()),
        "edge_rmse_std": float(edge.std(unbiased=False).item()),
    }


def score_subset(
    ideal: torch.Tensor,
    eng: torch.Tensor,
    out: torch.Tensor,
    mask: torch.Tensor,
) -> dict[str, float]:
    if not mask.any():
        return {"n": 0}
    return {**score_batch(ideal[mask], eng[mask], out[mask]), "n": int(mask.sum().item())}


def load_hybrid(path: Path, device: torch.device) -> tuple[str, Callable[[torch.Tensor], torch.Tensor], dict]:
    blob = torch.load(path, map_location=device, weights_only=False)
    cfg = og.ArchConfig.from_dict(blob["architecture"])
    cell = og.SeamCell(cfg).to(device)
    state = {k: v for k, v in blob["cell_state_dict"].items()}
    if "fir" in state and state["fir"].shape[0] != cell.fir.shape[0]:
        pad = torch.zeros(cell.fir.shape[0], dtype=state["fir"].dtype)
        n = min(state["fir"].shape[0], pad.shape[0])
        pad[:n] = state["fir"][:n]
        state["fir"] = pad
    og.load_state_dict_compatible(cell, state)
    cell.eval()

    def fn(eng: torch.Tensor) -> torch.Tensor:
        return og.apply_ops(eng, cell, cfg.ops)

    return (
        f"hybrid_lstm:{path.name}",
        fn,
        {"path": str(path), "r_blend_saved": blob.get("r_blend"), "architecture": blob.get("architecture")},
    )


def load_fitted_e4(path: Path, device: torch.device) -> tuple[str, Callable[[torch.Tensor], torch.Tensor], dict]:
    cfg, cell, residual, arch = load_fitted(path, device)

    def fn(eng: torch.Tensor) -> torch.Tensor:
        return og.apply_ops(eng, cell, cfg.ops)

    return (
        f"e4_champion:{path.stem}",
        fn,
        {"path": str(path), "residual_saved": residual, "architecture": arch},
    )


def load_n2n(path: Path, device: torch.device) -> tuple[str, Callable[[torch.Tensor], torch.Tensor], dict]:
    from baselines import n2n_seam

    blob = torch.load(path, map_location=device, weights_only=False)
    state = blob.get("state_dict") or blob
    model = n2n_seam.from_state(state, device)
    model.eval()
    eval_blob = blob.get("eval") or {}

    def fn(eng: torch.Tensor) -> torch.Tensor:
        return model(eng)

    return (
        f"n2n:{path.stem}",
        fn,
        {"path": str(path), "eval_R_blend": eval_blob.get("residual_R"), "train_seed": blob.get("train_seed")},
    )


def discover_default_e4() -> list[Path]:
    paths = []
    for tag in DEFAULT_E4_TAGS:
        p = DEFAULT_E4_DIR / tag
        if p.is_file():
            paths.append(p)
    if not paths:
        paths = sorted(DEFAULT_E4_DIR.glob("champion_iter_*_fitted.pt"))[:4]
    return paths


def load_models(
    e4_paths: list[Path],
    hybrid_path: Path | None,
    n2n_path: Path | None,
    device: torch.device,
    *,
    include_baselines: bool,
) -> list[tuple[str, Callable[[torch.Tensor], torch.Tensor], dict]]:
    models: list[tuple[str, Callable[[torch.Tensor], torch.Tensor], dict]] = []
    for p in e4_paths:
        if p.is_file():
            models.append(load_fitted_e4(p, device))
    if hybrid_path and hybrid_path.is_file():
        models.append(load_hybrid(hybrid_path, device))
    if n2n_path and n2n_path.is_file():
        models.append(load_n2n(n2n_path, device))
    if include_baselines:
        models.extend(
            [
                (
                    "dual_cosine",
                    lambda eng: og.dual_cosine_blend(eng),
                    {"kind": "classical"},
                ),
                (
                    "no_bake",
                    lambda eng: eng,
                    {"kind": "identity"},
                ),
            ]
        )
    return models


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--data-dir", type=Path, default=DEFAULT_SET)
    ap.add_argument("--out-dir", type=Path, default=None)
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument(
        "--e4-champion",
        type=Path,
        action="append",
        default=None,
        help="Repeatable E4 / overnight fitted .pt (default: top-4 shipped champions)",
    )
    ap.add_argument("--hybrid-lstm", type=Path, default=DEFAULT_HYBRID)
    ap.add_argument("--n2n", type=Path, default=ROOT / "brand" / "artifacts" / "n2n_seam_baselines" / "n2n_corrupt_corrupt.pt")
    ap.add_argument("--no-baselines", action="store_true")
    ap.add_argument("--batch", type=int, default=0, help="0 = full corpus in one forward")
    args = ap.parse_args()
    args.out_dir = args.out_dir or args.data_dir
    args.out_dir.mkdir(parents=True, exist_ok=True)
    device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")

    e4 = args.e4_champion if args.e4_champion else discover_default_e4()
    ideal, eng, samples, manifest_doc = load_corpus(args.data_dir, device)
    mask_akwf = subset_mask(samples, "akwf")
    mask_ff = subset_mask(samples, "factory_fx")
    models = load_models(
        e4,
        args.hybrid_lstm,
        args.n2n,
        device,
        include_baselines=not args.no_baselines,
    )
    if not models:
        raise SystemExit("no checkpoints loaded — pass --e4-champion / --hybrid-lstm / --n2n")

    t0 = time.perf_counter()
    report_models: dict[str, Any] = {}
    csv_rows: list[dict[str, Any]] = []

    b = args.batch if args.batch > 0 else ideal.shape[0]
    for name, fn, meta in models:
        outs = []
        for start in range(0, ideal.shape[0], b):
            sl = slice(start, min(start + b, ideal.shape[0]))
            outs.append(fn(eng[sl]))
        out = torch.cat(outs, dim=0)
        overall = score_batch(ideal, eng, out)
        by_subset = {
            "akwf": score_subset(ideal, eng, out, mask_akwf.to(device)),
            "factory_fx": score_subset(ideal, eng, out, mask_ff.to(device)),
        }
        report_models[name] = {
            "meta": meta,
            "overall": overall,
            "by_subset": by_subset,
            "primary_metric": "r_blend",
            "blend_alpha": og.BLEND_ALPHA,
            "N": og.N,
            "SEAM_W": og.SEAM_W,
            "PROLONG": og.PROLONG,
        }
        for subset, stats in by_subset.items():
            csv_rows.append(
                {
                    "model": name,
                    "subset": subset,
                    **{k: stats.get(k) for k in stats if k != "n"},
                    "n": stats.get("n", 0),
                }
            )
        csv_rows.append({"model": name, "subset": "all", **overall, "n": int(ideal.shape[0])})
        print(
            f"{name:42} R_blend={overall['r_blend_mean']:.4f}±{overall['r_blend_std']:.4f} "
            f"R_seam={overall['r_seam_mean']:.4f} R_body={overall['r_body_mean']:.4f}"
        )

    elapsed = time.perf_counter() - t0
    doc = {
        "schema": "denoiseopt.eval_real_seam.v1",
        "data_dir": str(args.data_dir),
        "manifest_schema": manifest_doc.get("schema"),
        "seed": manifest_doc.get("seed"),
        "n_samples": int(ideal.shape[0]),
        "device": str(device),
        "elapsed_s": elapsed,
        "checkpoints": {
            "e4": [str(p) for p in e4],
            "hybrid_lstm": str(args.hybrid_lstm),
            "n2n": str(args.n2n),
        },
        "models": report_models,
    }
    json_path = args.out_dir / "eval_real_seam.json"
    json_path.write_text(json.dumps(doc, indent=2), encoding="utf-8")
    csv_path = args.out_dir / "eval_real_seam.csv"
    if csv_rows:
        fields = sorted({k for row in csv_rows for k in row})
        with csv_path.open("w", newline="", encoding="utf-8") as fh:
            w = csv.DictWriter(fh, fieldnames=fields)
            w.writeheader()
            w.writerows(csv_rows)
    print(json.dumps({"json": str(json_path), "csv": str(csv_path), "elapsed_s": elapsed}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())