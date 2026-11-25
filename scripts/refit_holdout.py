#!/usr/bin/env python3
"""Refit DenoiseOpt champions on the frozen procedural holdout (canonical_eval_dataset).

Reads champion arch/HP from summary JSON (meta-approach, overnight run, or unfitted arch
JSON). By default writes refit_holdout.json only. With --save-state DIR, also persists
fitted cell weights as .pt files plus a sha256 manifest for eval_real_seam / overnight
champion loaders.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterator

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402

DEFAULT_HOLDOUT = ROOT / "brand" / "artifacts" / "canonical_eval_dataset" / "holdout_batch.pt"
SEARCH_SEED = og.DEFAULT_SEED
HOLDOUT_SEED = 20260719


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def atomic_write_bytes(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f"{path.name}.{os.getpid()}.tmp")
    tmp.write_bytes(data)
    os.replace(str(tmp), str(path))


def atomic_write_text(path: Path, text: str) -> None:
    atomic_write_bytes(path, text.encode("utf-8"))


def load_holdout(device: torch.device) -> tuple[torch.Tensor, torch.Tensor, str]:
    holdout = DEFAULT_HOLDOUT
    if holdout.is_file():
        blob = torch.load(holdout, map_location="cpu", weights_only=False)
        return (
            blob["ideal"].to(device).float(),
            blob["engine"].to(device).float(),
            str(holdout.relative_to(ROOT)),
        )
    torch.manual_seed(HOLDOUT_SEED)
    ideal, eng = og.make_batch(64, og.N, device)
    return ideal, eng, f"synthetic_holdout_seed_{HOLDOUT_SEED}"


def iter_sources(paths: list[Path]) -> Iterator[tuple[str, dict[str, Any], Path]]:
    for src in paths:
        if src.is_dir():
            for name in ("summary.json", "final_summary.json"):
                p = src / name
                if p.is_file():
                    doc = json.loads(p.read_text(encoding="utf-8"))
                    tag = doc.get("approach") or doc.get("run_id") or src.name
                    yield str(tag), doc, p
            for p in sorted(src.glob("unfitted/*_arch.json")):
                doc = json.loads(p.read_text(encoding="utf-8"))
                yield p.stem, doc, p
        elif src.is_file():
            doc = json.loads(src.read_text(encoding="utf-8"))
            tag = doc.get("approach") or doc.get("tag") or src.stem
            yield str(tag), doc, src


def champ_from_doc(doc: dict[str, Any]) -> tuple[og.ArchConfig, og.HyperParams, dict[str, Any]]:
    arch = doc.get("champ_arch") or doc.get("champion_arch") or doc.get("architecture")
    if not arch:
        raise ValueError("no architecture in document")
    hp_raw = doc.get("champ_hp") or doc.get("champion_hp") or doc.get("hyperparams") or {}
    cfg = og.ArchConfig.from_dict(arch)
    hp = og.HyperParams.from_dict(hp_raw if isinstance(hp_raw, dict) else None)
    meta = {
        "search_score": doc.get("champ_raw") or doc.get("champion_residual") or doc.get("residual"),
        "source_iters": doc.get("iters_done") or doc.get("champ_iter"),
    }
    return cfg, hp, meta


@torch.no_grad()
def score_holdout(
    ideal: torch.Tensor, eng: torch.Tensor, out: torch.Tensor
) -> dict[str, float]:
    r_blend = og.residual_score_blend(ideal, eng, out)
    r_seam = og.residual_score_seam(ideal, out)
    r_body = og.residual_score_body(eng, out)
    return {
        "r_blend_mean": float(r_blend.mean().item()),
        "r_seam_mean": float(r_seam.mean().item()),
        "r_body_mean": float(r_body.mean().item()),
        "n": int(ideal.shape[0]),
    }


def refit_one(
    tag: str,
    cfg: og.ArchConfig,
    hp: og.HyperParams,
    device: torch.device,
    *,
    fit_seed: int,
) -> tuple[og.SeamCell, dict[str, Any]]:
    cell = og.SeamCell(cfg).to(device)
    torch.manual_seed(fit_seed)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(fit_seed)
    fit_steps = int(hp.fit_steps or 48)
    batch = int(hp.batch or 48)
    lr = float(hp.lr or 3e-3)
    adv = float(hp.adv_coef or 0.0)
    train_r, converged = og.fit_cell(
        cell,
        cfg.ops,
        device,
        steps=fit_steps,
        batch=batch,
        lr=lr,
        adv_coef=adv if cfg.use_adv_aux else 0.0,
    )
    cell.eval()
    fit_meta = {
        "fit_steps": fit_steps,
        "batch": batch,
        "lr": lr,
        "adv_coef": adv,
        "converged": bool(converged),
        "train_r_last": float(train_r),
        "refit_seed": fit_seed,
    }
    return cell, fit_meta


def save_state_pt(
    out_dir: Path,
    tag: str,
    cfg: og.ArchConfig,
    hp: og.HyperParams,
    cell: og.SeamCell,
    holdout_r: float,
    *,
    fit_seed: int,
    fit_steps: int,
) -> tuple[Path, str]:
    out_dir.mkdir(parents=True, exist_ok=True)
    safe = "".join(c if c.isalnum() or c in "-_" else "_" for c in tag)[:80]
    path = out_dir / f"{safe}_refit.pt"
    payload = {
        "schema": "denoiseopt.refit_holdout.pt.v1",
        "architecture": cfg.to_dict(),
        "hyperparams": hp.to_dict(),
        "residual": float(holdout_r),
        "r_blend": float(holdout_r),
        "cell_state_dict": cell.state_dict(),
        "policy_state_dict": None,
        "algorithms": og.ALGORITHMS,
        "tag": tag,
        "refit_seed": fit_seed,
        "fit_steps": fit_steps,
        "holdout_seed": HOLDOUT_SEED,
    }
    buf_path = path.with_suffix(path.suffix + ".tmp")
    torch.save(payload, buf_path)
    os.replace(str(buf_path), str(path))
    digest = sha256_file(path)
    return path, digest


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "sources",
        nargs="*",
        type=Path,
        help="summary.json paths, run dirs, or unfitted arch JSON",
    )
    ap.add_argument(
        "--meta-dir",
        type=Path,
        default=ROOT / "brand" / "artifacts" / "meta_approach_compare",
        help="When no sources given, refit every */summary.json under this tree",
    )
    ap.add_argument("--device", default="cpu")
    ap.add_argument("--fit-seed", type=int, default=SEARCH_SEED)
    ap.add_argument(
        "--out-json",
        type=Path,
        default=ROOT / "brand" / "artifacts" / "refit_holdout.json",
    )
    ap.add_argument(
        "--save-state",
        type=Path,
        default=None,
        help="Directory for fitted .pt weights + manifest.json (sha256)",
    )
    args = ap.parse_args()

    device = torch.device(
        args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu"
    )
    sources = list(args.sources)
    if not sources and args.meta_dir.is_dir():
        sources = [args.meta_dir]

    ideal, eng, hold_note = load_holdout(device)
    dual = og.dual_cosine_blend(eng)
    dual_scores = score_holdout(ideal, eng, dual)

    models: list[dict[str, Any]] = []
    manifest_entries: list[dict[str, Any]] = []
    t0 = time.time()

    for tag, doc, src_path in iter_sources(sources):
        try:
            cfg, hp, src_meta = champ_from_doc(doc)
        except ValueError as exc:
            models.append({"tag": tag, "source": str(src_path), "error": str(exc)})
            continue
        cell, fit_meta = refit_one(tag, cfg, hp, device, fit_seed=args.fit_seed)
        out = og.apply_ops(eng, cell, cfg.ops)
        scores = score_holdout(ideal, eng, out)
        row: dict[str, Any] = {
            "tag": tag,
            "source": str(src_path.relative_to(ROOT)) if src_path.is_relative_to(ROOT) else str(src_path),
            "architecture": cfg.to_dict(),
            "hyperparams": hp.to_dict(),
            "holdout": scores,
            "delta_vs_dual_cosine": scores["r_blend_mean"] - dual_scores["r_blend_mean"],
            "refit": fit_meta,
            "source_meta": src_meta,
        }
        if args.save_state:
            pt_path, digest = save_state_pt(
                args.save_state,
                tag,
                cfg,
                hp,
                cell,
                scores["r_blend_mean"],
                fit_seed=args.fit_seed,
                fit_steps=int(fit_meta["fit_steps"]),
            )
            row["weights_path"] = str(pt_path.relative_to(ROOT)) if pt_path.is_relative_to(ROOT) else str(pt_path)
            row["sha256"] = digest
            manifest_entries.append(
                {
                    "tag": tag,
                    "weights_path": pt_path.name,
                    "sha256": digest,
                    "architecture": cfg.to_dict(),
                    "hyperparams": hp.to_dict(),
                    "refit_seed": args.fit_seed,
                    "fit_steps": fit_meta["fit_steps"],
                    "holdout_r_blend": scores["r_blend_mean"],
                }
            )
        models.append(row)
        print(
            f"{tag:24} holdout R_blend={scores['r_blend_mean']:.4f} "
            f"Δdual={row['delta_vs_dual_cosine']:+.4f}"
        )

    report = {
        "schema": "denoiseopt.refit_holdout.v1",
        "written_at": datetime.now(timezone.utc).isoformat(),
        "holdout_source": hold_note,
        "holdout_seed": HOLDOUT_SEED,
        "fit_seed": args.fit_seed,
        "dual_cosine_holdout": dual_scores,
        "device": str(device),
        "elapsed_s": time.time() - t0,
        "models": models,
        "pt_format": {
            "description": "Compatible with scripts/eval_real_seam.load_fitted and overnight save_fitted",
            "required_keys": [
                "architecture",
                "cell_state_dict",
                "residual",
            ],
            "optional_keys": ["hyperparams", "tag", "refit_seed", "fit_steps", "schema"],
        },
    }
    args.out_json.parent.mkdir(parents=True, exist_ok=True)
    atomic_write_text(args.out_json, json.dumps(report, indent=2))
    print(f"wrote {args.out_json}")

    if args.save_state and manifest_entries:
        args.save_state.mkdir(parents=True, exist_ok=True)
        man = {
            "schema": "denoiseopt.refit_holdout.manifest.v1",
            "holdout_seed": HOLDOUT_SEED,
            "fit_seed": args.fit_seed,
            "entries": manifest_entries,
        }
        man_path = args.save_state / "manifest.json"
        atomic_write_text(man_path, json.dumps(man, indent=2))
        print(f"wrote {man_path} ({len(manifest_entries)} weights)")

    return 0 if models and not all("error" in m for m in models) else 2


if __name__ == "__main__":
    raise SystemExit(main())