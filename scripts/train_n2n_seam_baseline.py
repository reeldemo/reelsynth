#!/usr/bin/env python3
"""Train N2N seam baselines (Phase B).

PRIMARY: n2n_corrupt_corrupt — two independent cliffs from same mid-cycle seed.
SECONDARY: n2n_sibling_supervised — engine → ideal.

Train seeds disjoint from holdout 20260719. No overnight champion leakage.
"""
from __future__ import annotations

import argparse
import json
import math
import sys
import time
from pathlib import Path

import torch
import torch.nn.functional as F

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
import metrics_snr_sdr as msm  # noqa: E402
from baselines.n2n_seam import SeamN2N, n_params  # noqa: E402

HOLDOUT_SEED = 20260719
TRAIN_SEED = 424242  # disjoint from holdout / overnight search
EVAL_SEED = HOLDOUT_SEED
OUT_DIR = ROOT / "brand" / "artifacts" / "n2n_seam_baselines"


def set_seed(seed: int, device: torch.device) -> None:
    torch.manual_seed(seed)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(seed)


def make_midcycle_pair(
    batch: int, n: int, device: torch.device
) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
    """Same mid-cycle content; two independent cliff+noise draws + ideal."""
    t = torch.linspace(0, 1, n, device=device).unsqueeze(0).expand(batch, -1)
    freqs = 1.0 + 3.0 * torch.rand(batch, 1, device=device)
    phase = 2 * math.pi * torch.rand(batch, 1, device=device)
    ideal = torch.sin(2 * math.pi * freqs * t + phase)
    ideal = ideal + 0.15 * torch.sin(4 * math.pi * freqs * t + phase * 0.7)

    def corrupt(base: torch.Tensor) -> torch.Tensor:
        eng = base.clone()
        cliff = (0.08 + 0.35 * torch.rand(batch, 1, device=device)) * (
            1.0 - 2.0 * torch.rand(batch, 1, device=device)
        )
        w = og.SEAM_W
        for i in range(w):
            a = i / max(w - 1, 1)
            eng[:, i] = eng[:, i] + cliff.squeeze(-1) * (1 - a)
            eng[:, -w + i] = eng[:, -w + i] - cliff.squeeze(-1) * a
        noise = 0.02 * torch.randn(batch, n, device=device)
        noise[:, w:-w] *= 0.15
        return eng + noise

    return ideal, corrupt(ideal), corrupt(ideal)


@torch.no_grad()
def eval_model(model: torch.nn.Module, device: torch.device, batch: int = 64) -> dict:
    set_seed(EVAL_SEED, device)
    ideal, eng = og.make_batch(batch, og.N, device)
    out = model(eng)
    # v10.1 primary: R_blend = α·R_seam(ideal,out) + (1-α)·R_body(eng,out)
    r = float(og.residual_score_blend(ideal, eng, out).mean().item())
    r_seam = float(og.residual_score_seam(ideal, out).mean().item())
    r_body = float(og.residual_score_body(eng, out).mean().item())
    r_whole = float(og.residual_score(ideal, out).mean().item())
    sec = msm.secondary_metrics(
        ideal, out, periods=int(og.PROLONG), seam_w=og.SEAM_W, eng=eng, alpha=og.BLEND_ALPHA
    )
    dc = og.dual_cosine_blend(eng)
    r_dc = float(og.residual_score_blend(ideal, eng, dc).mean().item())
    return {
        "residual_R": r,
        "residual_R_blend": r,
        "residual_R_seam": r_seam,
        "residual_R_body": r_body,
        "residual_R_whole": r_whole,
        "blend_alpha": og.BLEND_ALPHA,
        "primary_metric": "r_blend",
        "dual_cosine_R": r_dc,
        "delta_R_vs_dual_cosine": r - r_dc,
        **sec,
    }


def train_mode(
    mode: str,
    *,
    steps: int,
    batch: int,
    lr: float,
    device: torch.device,
) -> dict:
    model = SeamN2N().to(device)
    opt = torch.optim.Adam(model.parameters(), lr=lr)
    set_seed(TRAIN_SEED, device)
    losses = []
    t0 = time.perf_counter()
    for step in range(1, steps + 1):
        # Fresh draws each step; never use holdout seed tensors
        ideal, a, b = make_midcycle_pair(batch, og.N, device)
        if mode == "n2n_corrupt_corrupt":
            # Predict one corruption from the other (symmetric N2N)
            pred = model(a)
            loss = F.mse_loss(pred, b)
            # Also train reverse for symmetry
            pred2 = model(b)
            loss = loss + F.mse_loss(pred2, a)
            loss = loss * 0.5
        elif mode == "n2n_sibling_supervised":
            pred = model(a)
            loss = F.mse_loss(pred, ideal)
        else:
            raise ValueError(mode)
        opt.zero_grad(set_to_none=True)
        loss.backward()
        opt.step()
        losses.append(float(loss.item()))
        if step % max(steps // 10, 1) == 0 or step == 1:
            print(f"  [{mode}] step {step}/{steps} loss={loss.item():.6f}")
    elapsed = time.perf_counter() - t0
    model.eval()
    metrics = eval_model(model, device)
    ckpt = {
        "kind": mode,
        "state_dict": model.state_dict(),
        "n_params": n_params(model),
        "train_seed": TRAIN_SEED,
        "holdout_seed": HOLDOUT_SEED,
        "steps": steps,
        "batch": batch,
        "lr": lr,
        "loss_first": losses[0] if losses else None,
        "loss_last": losses[-1] if losses else None,
        "elapsed_sec": elapsed,
        "eval": metrics,
    }
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    path = OUT_DIR / f"{mode}.pt"
    torch.save(ckpt, path)
    print(
        f"saved {path} params={ckpt['n_params']} "
        f"R_blend={metrics['residual_R']:.4f} "
        f"R_seam={metrics.get('residual_R_seam')} R_body={metrics.get('residual_R_body')}"
    )
    return ckpt


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--steps", type=int, default=20000)
    ap.add_argument("--batch", type=int, default=64)
    ap.add_argument("--lr", type=float, default=2e-3)
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument(
        "--modes",
        default="n2n_corrupt_corrupt,n2n_sibling_supervised",
        help="comma-separated modes",
    )
    args = ap.parse_args()
    device = torch.device(args.device)
    assert TRAIN_SEED != HOLDOUT_SEED
    assert TRAIN_SEED != 1902771841

    summary = {
        "meta": {
            "train_seed": TRAIN_SEED,
            "holdout_seed": HOLDOUT_SEED,
            "overnight_search_seed": 1902771841,
            "note": "no holdout tile leakage; no overnight champion init; v10.1 primary=R_blend",
            "primary": "n2n_corrupt_corrupt",
            "secondary": "n2n_sibling_supervised",
            "primary_metric": "r_blend",
            "blend_alpha": og.BLEND_ALPHA,
            "protocol": "paper_v10.1",
            "steps": args.steps,
            "batch": args.batch,
            "lr": args.lr,
            "L": og.N,
            "SEAM_W": og.SEAM_W,
        },
        "modes": {},
    }
    for mode in [m.strip() for m in args.modes.split(",") if m.strip()]:
        print(f"=== training {mode} ===")
        ckpt = train_mode(mode, steps=args.steps, batch=args.batch, lr=args.lr, device=device)
        summary["modes"][mode] = {
            "n_params": ckpt["n_params"],
            "loss_first": ckpt["loss_first"],
            "loss_last": ckpt["loss_last"],
            "elapsed_sec": ckpt["elapsed_sec"],
            "eval": ckpt["eval"],
            "checkpoint": str(OUT_DIR / f"{mode}.pt"),
        }
        # Smoke: loss should decrease
        if ckpt["loss_first"] is not None and ckpt["loss_last"] is not None:
            print(f"  loss {ckpt['loss_first']:.6f} -> {ckpt['loss_last']:.6f}")

    # Write JSON to denoise-opt-meta + local artifacts
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    local_json = OUT_DIR / "n2n_baseline.json"
    local_json.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    meta_json = (
        ROOT.parent / "denoise-opt-meta" / "paper" / "v5" / "figures" / "n2n_baseline.json"
    )
    if meta_json.parent.is_dir():
        meta_json.write_text(json.dumps(summary, indent=2), encoding="utf-8")
        print(f"wrote {meta_json}")
    print(f"wrote {local_json}")


if __name__ == "__main__":
    main()
