#!/usr/bin/env python3
"""Train domain-specific Noise2Noise (corrupt→corrupt) on transfer boards.

Table 14 follow-up: domain-trained N2N quality on CWRU / MFPT / MIT-BIH /
PTB-XL subset / synth CNC / synth PMU.

Protocol:
  - Load DomainBundle periods (ideal + frozen engine).
  - Holdout: seed 20260719, n=64 (disjoint from train indices).
  - Train: SeamN2N on two independent cliffs of the same ideal (N2N).
  - Eval: prolonged residual R (matches transfer Table) + R_blend debug.
  - Does NOT invent Cycle-GAN / BeatDiff / deep-Paderborn / MOS scores.
    Classical paderborn_kat board may be trained when extracted.

Writes:
  brand/artifacts/signal_heal_transfer/domain_n2n/
  updates results_table.json with n2n_domain_trained column when --merge.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

import numpy as np
import torch
import torch.nn.functional as F

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import overnight_gpu_rl_arch as og  # noqa: E402
from baselines.n2n_seam import SeamN2N, n_params  # noqa: E402
from signal_heal.datasets import _inject_cliff, ensure_bundles  # noqa: E402

OUT = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "domain_n2n"
RESULTS = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "results_table.json"
HOLDOUT_SEED = 20260719
TRAIN_SEED = 424242
DOMAINS = [
    "cwru_bearings",
    "mfpt_bearings",
    "paderborn_kat",
    "mitbih_ecg",
    "ptbxl_ecg",
    "synth_cnc_g01",
    "synth_pmu_cycle",
]


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def split_indices(n: int, holdout_n: int, seed: int) -> tuple[torch.Tensor, torch.Tensor]:
    g = torch.Generator()
    g.manual_seed(seed)
    perm = torch.randperm(n, generator=g)
    h = min(holdout_n, max(1, n // 4))
    hold = perm[:h]
    train = perm[h:] if h < n else perm
    return train, hold


def cliff_pair(ideal: torch.Tensor, rng: np.random.Generator) -> tuple[torch.Tensor, torch.Tensor]:
    """Two independent DenoiseOpt-style cliffs from the same ideal batch."""
    a_list, b_list = [], []
    for row in ideal.detach().cpu().numpy():
        a_list.append(_inject_cliff(row, rng))
        b_list.append(_inject_cliff(row, rng))
    device = ideal.device
    return (
        torch.tensor(np.stack(a_list), device=device, dtype=torch.float32),
        torch.tensor(np.stack(b_list), device=device, dtype=torch.float32),
    )


@torch.no_grad()
def eval_holdout(
    model: SeamN2N,
    ideal_h: torch.Tensor,
    eng_h: torch.Tensor,
) -> dict:
    out = model(eng_h)
    r = float(og.residual_score(ideal_h, out).mean().item())
    r_blend = float(og.residual_score_blend(ideal_h, eng_h, out).mean().item())
    r_nb = float(og.residual_score(ideal_h, eng_h).mean().item())
    dc = og.dual_cosine_blend(eng_h)
    r_dc = float(og.residual_score(ideal_h, dc).mean().item())
    return {
        "R": r,
        "R_blend": r_blend,
        "no_bake_R": r_nb,
        "dual_cosine_R": r_dc,
        "n_holdout": int(ideal_h.shape[0]),
    }


def train_domain(
    name: str,
    ideal: torch.Tensor,
    engine: torch.Tensor,
    *,
    device: torch.device,
    steps: int,
    batch: int,
    lr: float,
    holdout_n: int,
) -> dict:
    n = int(ideal.shape[0])
    train_idx, hold_idx = split_indices(n, holdout_n, HOLDOUT_SEED)
    ideal_t = ideal[train_idx].to(device)
    ideal_h = ideal[hold_idx].to(device)
    eng_h = engine[hold_idx].to(device)

    model = SeamN2N().to(device)
    opt = torch.optim.Adam(model.parameters(), lr=lr)
    rng = np.random.default_rng(TRAIN_SEED)
    torch.manual_seed(TRAIN_SEED)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(TRAIN_SEED)

    losses: list[float] = []
    t0 = time.perf_counter()
    n_train = int(ideal_t.shape[0])
    for step in range(1, steps + 1):
        idx = torch.randint(0, n_train, (min(batch, n_train),), device=device)
        batch_ideal = ideal_t[idx]
        a, b = cliff_pair(batch_ideal, rng)
        pred = model(a)
        loss = F.mse_loss(pred, b)
        pred2 = model(b)
        loss = 0.5 * (loss + F.mse_loss(pred2, a))
        opt.zero_grad(set_to_none=True)
        loss.backward()
        opt.step()
        losses.append(float(loss.item()))
        if step == 1 or step % max(1, steps // 5) == 0 or step == steps:
            print(f"  [{name}] step {step}/{steps} loss={loss.item():.5f}", flush=True)

    model.eval()
    metrics = eval_holdout(model, ideal_h, eng_h)
    elapsed = time.perf_counter() - t0
    out_dir = OUT / name
    out_dir.mkdir(parents=True, exist_ok=True)
    ckpt = {
        "state_dict": {k: v.detach().cpu() for k, v in model.state_dict().items()},
        "domain": name,
        "protocol": "n2n_corrupt_corrupt_domain",
        "train_seed": TRAIN_SEED,
        "holdout_seed": HOLDOUT_SEED,
        "steps": steps,
        "batch": batch,
        "lr": lr,
        "n_params": n_params(model),
        "n_train": n_train,
        "eval": metrics,
        "loss_first": losses[0] if losses else None,
        "loss_last": losses[-1] if losses else None,
        "elapsed_sec": elapsed,
        "finished_at": utc_now(),
    }
    pt_path = out_dir / "n2n_corrupt_corrupt.pt"
    torch.save(ckpt, pt_path)
    (out_dir / "summary.json").write_text(
        json.dumps({k: v for k, v in ckpt.items() if k != "state_dict"}, indent=2),
        encoding="utf-8",
    )
    print(
        f"  [{name}] holdout R={metrics['R']:.4f} "
        f"R_blend={metrics['R_blend']:.4f} "
        f"vs DualCosine={metrics['dual_cosine_R']:.4f} "
        f"no-bake={metrics['no_bake_R']:.4f} "
        f"({elapsed:.1f}s)",
        flush=True,
    )
    return {
        "domain": name,
        "R": metrics["R"],
        "R_blend": metrics["R_blend"],
        "no_bake_R": metrics["no_bake_R"],
        "dual_cosine_R": metrics["dual_cosine_R"],
        "n_holdout": metrics["n_holdout"],
        "n_train": n_train,
        "n_params": n_params(model),
        "steps": steps,
        "elapsed_sec": elapsed,
        "ckpt": str(pt_path.resolve()),
    }


def merge_into_results(per_domain: dict[str, dict]) -> None:
    if not RESULTS.is_file():
        print(f"warn: missing {RESULTS}; skip merge", flush=True)
        return
    blob = json.loads(RESULTS.read_text(encoding="utf-8"))
    table = blob.setdefault("table", {})
    for name, row in per_domain.items():
        cell = table.setdefault(name, {})
        cell["n2n_domain_trained"] = {
            "R": row["R"],
            "R_blend": row["R_blend"],
            "kind": "n2n_domain_trained",
            "protocol": "n2n_corrupt_corrupt_domain",
            "ckpt": row["ckpt"],
            "n_holdout": row["n_holdout"],
            "steps": row["steps"],
        }
    blob["domain_n2n"] = {
        "finished_at": utc_now(),
        "primary_metric": "prolonged residual R (holdout)",
        "per_domain": per_domain,
    }
    skipped = blob.setdefault("skipped", {})
    skipped.pop("domain_trained_n2n", None)
    skipped["deep_sota_cyclegan_beatdiff"] = (
        "not run — no Cycle-GAN / BeatDiff weights under this residual protocol"
    )
    RESULTS.write_text(json.dumps(blob, indent=2), encoding="utf-8")
    print(f"merged into {RESULTS}", flush=True)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--steps", type=int, default=4000)
    ap.add_argument("--batch", type=int, default=48)
    ap.add_argument("--lr", type=float, default=2e-3)
    ap.add_argument("--holdout-n", type=int, default=64)
    ap.add_argument("--domains", nargs="*", default=DOMAINS)
    ap.add_argument("--merge", action="store_true", help="Merge into results_table.json")
    args = ap.parse_args()

    device = torch.device(args.device)
    OUT.mkdir(parents=True, exist_ok=True)
    print(f"loading domain bundles on {device}…", flush=True)
    bundles = ensure_bundles(force=False, n_periods=256)

    per_domain: dict[str, dict] = {}
    for name in args.domains:
        bundle = bundles.get(name)
        if bundle is None:
            print(f"skip {name}: bundle missing", flush=True)
            continue
        print(f"=== {name} n={bundle.ideal.shape[0]} ===", flush=True)
        per_domain[name] = train_domain(
            name,
            bundle.ideal,
            bundle.engine,
            device=device,
            steps=args.steps,
            batch=args.batch,
            lr=args.lr,
            holdout_n=args.holdout_n,
        )

    summary_path = OUT / "summary.json"
    prior_per: dict = {}
    if summary_path.is_file():
        try:
            prior = json.loads(summary_path.read_text(encoding="utf-8"))
            prior_per = dict(prior.get("per_domain") or {})
        except Exception:
            prior_per = {}
    prior_per.update(per_domain)
    summary = {
        "schema": "denoiseopt.domain_n2n.v1",
        "finished_at": utc_now(),
        "device": str(device),
        "steps": args.steps,
        "batch": args.batch,
        "lr": args.lr,
        "holdout_seed": HOLDOUT_SEED,
        "train_seed": TRAIN_SEED,
        "primary_metric": "prolonged residual R on disjoint holdout",
        "per_domain": prior_per,
        "domains_this_run": list(per_domain),
        "not_run": {
            "cycle_gan_ecg": "no adapted weights / training pipeline in-repo",
            "beatdiff": "no diffusion checkpoints under residual protocol",
            "paderborn_kat_deep": "K001 extracted; classical board ok; deep models unwired",
            "full_ptbxl": "subset pilot only (records500 lead-I n=256)",
            "kit_cnc_real": (
                "KIT DOI login wall — open https://doi.org/10.35097/hvvwn1kfwf7qt48z"
            ),
            "ieee_pmu_real": (
                "IEEE DataPort account wall — open "
                "https://ieee-dataport.org/open-access/pmu-measurements-ieee-39-bus-power-system-model"
            ),
            "formal_mos_mushra": "human listening not collected",
        },
    }
    summary_path.write_text(json.dumps(summary, indent=2), encoding="utf-8")
    if args.merge:
        merge_into_results(per_domain)
    print(json.dumps({"domains": list(per_domain), "out": str(OUT)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
