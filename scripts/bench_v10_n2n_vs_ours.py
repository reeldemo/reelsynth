#!/usr/bin/env python3
"""Paper v10: N2N vs Ours on holdout + multi-family (+ optional real WT corpora).

Primary metric: R_seam. Also reports latency and J for the hybrid champion.
"""
from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
import bench_sota_matrix as bsm  # noqa: E402
import metrics_snr_sdr as msm  # noqa: E402
from baselines.n2n_seam import SeamN2N  # noqa: E402

HOLDOUT_SEED = 20260719
OUT_DEFAULT = ROOT / "brand" / "artifacts" / "v10_n2n_vs_ours"


def set_seed(seed: int, device: torch.device) -> None:
    torch.manual_seed(seed)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(seed)


def load_n2n(device: torch.device):
    path = ROOT / "brand" / "artifacts" / "n2n_seam_baselines" / "n2n_corrupt_corrupt.pt"
    blob = torch.load(path, map_location=device, weights_only=False)
    model = SeamN2N.from_state(blob["state_dict"], device)
    model.eval()
    return model, path, blob


def load_ours(champ_pt: Path, device: torch.device):
    blob = torch.load(champ_pt, map_location=device, weights_only=False)
    cfg = og.ArchConfig.from_dict(blob["architecture"])
    cell = og.SeamCell(cfg).to(device)
    cell.load_state_dict(blob["cell_state_dict"])
    cell.eval()
    return cell, cfg, blob


@torch.no_grad()
def score_pair(
    ideal: torch.Tensor,
    eng: torch.Tensor,
    *,
    n2n: torch.nn.Module,
    ours_fn,
    device: torch.device,
) -> dict:
    out_n2n = n2n(eng)
    out_ours = ours_fn(eng)
    r_n2n = float(og.residual_score_blend(ideal, eng, out_n2n).mean().item())
    r_ours = float(og.residual_score_blend(ideal, eng, out_ours).mean().item())
    r_dc = float(og.residual_score_blend(ideal, eng, og.dual_cosine_blend(eng)).mean().item())
    r_nb = float(og.residual_score_blend(ideal, eng, eng).mean().item())
    sec_n2n = msm.secondary_metrics(
        ideal, out_n2n, periods=int(og.PROLONG), seam_w=og.SEAM_W, eng=eng, alpha=og.BLEND_ALPHA
    )
    sec_ours = msm.secondary_metrics(
        ideal, out_ours, periods=int(og.PROLONG), seam_w=og.SEAM_W, eng=eng, alpha=og.BLEND_ALPHA
    )
    # latency
    for _ in range(3):
        _ = n2n(eng)
        _ = ours_fn(eng)
    if device.type == "cuda":
        torch.cuda.synchronize()
    t0 = time.perf_counter()
    for _ in range(20):
        _ = n2n(eng)
    if device.type == "cuda":
        torch.cuda.synchronize()
    t_n2n = 1000.0 * (time.perf_counter() - t0) / 20
    t0 = time.perf_counter()
    for _ in range(20):
        _ = ours_fn(eng)
    if device.type == "cuda":
        torch.cuda.synchronize()
    t_ours = 1000.0 * (time.perf_counter() - t0) / 20
    return {
        "no_bake_R_blend": r_nb,
        "dual_cosine_R_blend": r_dc,
        "n2n": {
            "R_blend": r_n2n,
            "R_seam": sec_n2n.get("r_seam_mean"),
            "R_body": sec_n2n.get("r_body_mean"),
            "J": og.objective_j(r_n2n, t_n2n),
            "t_ms": t_n2n,
            **sec_n2n,
        },
        "ours": {
            "R_blend": r_ours,
            "R_seam": sec_ours.get("r_seam_mean"),
            "R_body": sec_ours.get("r_body_mean"),
            "J": og.objective_j(r_ours, t_ours),
            "t_ms": t_ours,
            **sec_ours,
        },
        "ours_beats_n2n": r_ours > r_n2n,
        "delta_R_blend_ours_minus_n2n": r_ours - r_n2n,
        "blend_alpha": og.BLEND_ALPHA,
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--champ",
        type=Path,
        default=ROOT
        / "brand"
        / "artifacts"
        / "meta_approach_compare_v10"
        / "hybrid_lstm"
        / "champ_cell.pt",
    )
    ap.add_argument("--batch", type=int, default=64)
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--out-dir", type=Path, default=OUT_DEFAULT)
    ap.add_argument("--skip-real-wt", action="store_true")
    ap.add_argument("--eval-seed", type=int, default=HOLDOUT_SEED)
    args = ap.parse_args()
    device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")
    args.out_dir.mkdir(parents=True, exist_ok=True)
    eval_seed = int(args.eval_seed)

    n2n, n2n_path, n2n_blob = load_n2n(device)
    cell, cfg, champ_blob = load_ours(args.champ, device)

    def ours_fn(eng):
        return og.apply_ops(eng, cell, cfg.ops)

    set_seed(eval_seed, device)
    ideal, eng = og.make_batch(args.batch, og.N, device)
    holdout = score_pair(ideal, eng, n2n=n2n, ours_fn=ours_fn, device=device)

    multifamily = {}
    for fam in bsm.FAMILIES:
        for i, seed in enumerate(bsm.WAVEFORM_SEEDS):
            ideal_f, eng_f = bsm.make_family_batch(fam, args.batch, og.N, device, seed=seed + (eval_seed - HOLDOUT_SEED))
            key = f"{fam}/seed{seed}"
            multifamily[key] = score_pair(ideal_f, eng_f, n2n=n2n, ours_fn=ours_fn, device=device)

    mf_deltas = [v["delta_R_blend_ours_minus_n2n"] for v in multifamily.values()]
    mf_ours = [v["ours"]["R_blend"] for v in multifamily.values()]
    mf_n2n = [v["n2n"]["R_blend"] for v in multifamily.values()]
    mf_summary = {
        "n_boards": len(multifamily),
        "ours_R_blend_mean": sum(mf_ours) / len(mf_ours),
        "n2n_R_blend_mean": sum(mf_n2n) / len(mf_n2n),
        "delta_mean": sum(mf_deltas) / len(mf_deltas),
        "ours_beats_n2n_count": sum(1 for d in mf_deltas if d > 0),
        "ours_beats_n2n_frac": sum(1 for d in mf_deltas if d > 0) / len(mf_deltas),
        "blend_alpha": og.BLEND_ALPHA,
    }

    real_wt = {}
    if not args.skip_real_wt:
        import real_wt_wrap_protocol as rwp

        cycles_pt = ROOT / "brand" / "artifacts" / "real_wt_cycles" / "cycles.pt"
        if cycles_pt.is_file():
            blob = torch.load(cycles_pt, map_location="cpu", weights_only=False)
            for label, key in (("factory", "reelsynth_factory"), ("akwf", "oa_instrument")):
                if key in blob and hasattr(blob[key], "shape"):
                    cyc = blob[key].to(device)
                    closed = rwp.close_seam_ideal(cyc)
                    ideal_r, eng_r = rwp.apply_open_wrap_cliff(closed, seed=eval_seed)
                    real_wt[label] = {
                        "n_cycles": int(cyc.shape[0]),
                        **score_pair(ideal_r, eng_r, n2n=n2n, ours_fn=ours_fn, device=device),
                    }

    report = {
        "schema": "denoiseopt.v10_n2n_vs_ours.v1",
        "protocol": "paper_v10.1",
        "primary_metric": "r_blend",
        "blend_alpha": og.BLEND_ALPHA,
        "holdout_seed": eval_seed,
        "n2n_checkpoint": str(n2n_path),
        "n2n_train_eval_R_blend": (n2n_blob.get("eval") or {}).get("residual_R"),
        "champ_path": str(args.champ),
        "champ_meta": {
            "r_blend": champ_blob.get("r_seam") or champ_blob.get("r_blend"),
            "j": champ_blob.get("j"),
            "t_ms": champ_blob.get("t_ms"),
            "beats_n2n": champ_blob.get("beats_n2n"),
            "architecture": champ_blob.get("architecture"),
        },
        "holdout": holdout,
        "multifamily_summary": mf_summary,
        "multifamily": multifamily,
        "real_wt": real_wt,
        "finished_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    out = args.out_dir / "n2n_vs_ours.json"
    out.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps({
        "holdout_ours_R_blend": holdout["ours"]["R_blend"],
        "holdout_n2n_R_blend": holdout["n2n"]["R_blend"],
        "holdout_ours_beats_n2n": holdout["ours_beats_n2n"],
        "holdout_ours_J": holdout["ours"]["J"],
        "holdout_ours_t_ms": holdout["ours"]["t_ms"],
        "multifamily": mf_summary,
        "real_wt_keys": list(real_wt.keys()),
        "out": str(out),
    }, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
