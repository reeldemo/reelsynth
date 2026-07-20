#!/usr/bin/env python3
"""Cliff-stratum scoring: all / top-25% / top-10% wrap-jump tiles (Phase A).

Holdout seed 20260719. Scores no-bake (passthrough), DualCosine, seam_fir3, neural favorite
(legacy JSON alias: identity ≡ no_bake).
(and optional N2N/seq if checkpoints exist). Writes cliff_strata.json.
"""
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
import bench_classical_vs_ai as cav  # noqa: E402
import bench_sota_matrix as bsm  # noqa: E402
import metrics_snr_sdr as msm  # noqa: E402

HOLDOUT_SEED = 20260719
META_OUT = (
    Path(__file__).resolve().parents[2]
    / "denoise-opt-meta"
    / "paper"
    / "v5"
    / "figures"
    / "cliff_strata.json"
)
MIRROR_OUT = ROOT / "docs" / "papers" / "denoise_opt" / "v5" / "figures" / "cliff_strata.json"


def set_seed(seed: int, device: torch.device) -> None:
    torch.manual_seed(seed)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(seed)


@torch.no_grad()
def per_tile_metrics(
    ideal: torch.Tensor, out: torch.Tensor, *, seam_w: int = 8
) -> dict[str, torch.Tensor]:
    r = og.residual_score(ideal, out)
    snr = msm.tiled_snr_db(ideal, out, periods=int(og.PROLONG))
    sdr = msm.tiled_sdr_db(ideal, out, periods=int(og.PROLONG))
    jump = msm.wrap_jump_abs(out)
    ermse = msm.edge_rmse(ideal, out, seam_w=seam_w)
    click = msm.click_energy(out, periods=4)
    return {
        "R": r,
        "snr_db": snr,
        "sdr_db": sdr,
        "wrap_jump": jump,
        "edge_rmse": ermse,
        "click_energy": click,
    }


def summarize(metrics: dict[str, torch.Tensor], mask: torch.Tensor) -> dict:
    n = int(mask.sum().item())
    out = {"n": n}
    for key, tens in metrics.items():
        sel = tens[mask]
        out[f"{key}_mean"] = float(sel.mean().item()) if n else float("nan")
        out[f"{key}_std"] = float(sel.std(unbiased=False).item()) if n else float("nan")
    return out


def try_load_baseline_fn(name: str, device: torch.device):
    """Optional Phase-B checkpoints; skip silently if missing."""
    ckpt_root = ROOT / "brand" / "artifacts" / "n2n_seam_baselines"
    mapping = {
        "n2n_corrupt_corrupt": ckpt_root / "n2n_corrupt_corrupt.pt",
        "n2n_sibling_supervised": ckpt_root / "n2n_sibling_supervised.pt",
        "seq_lstm": ckpt_root / "seq_lstm.pt",
        "seq_cnn1d": ckpt_root / "seq_cnn1d.pt",
    }
    path = mapping.get(name)
    if path is None or not path.is_file():
        return None
    try:
        from baselines import n2n_seam, seq_seam_lstm, seq_seam_cnn1d

        blob = torch.load(path, map_location=device, weights_only=False)
        kind = blob.get("kind", name)
        if kind.startswith("n2n") or name.startswith("n2n"):
            model = n2n_seam.SeamN2N.from_state(blob["state_dict"], device)
        elif "lstm" in kind or name == "seq_lstm":
            model = seq_seam_lstm.SeamLSTM.from_state(blob["state_dict"], device)
        else:
            model = seq_seam_cnn1d.SeamCNN1D.from_state(blob["state_dict"], device)
        model.eval()

        def fn(eng: torch.Tensor) -> torch.Tensor:
            return model(eng)

        return fn
    except Exception as exc:  # noqa: BLE001
        print(f"skip {name}: {exc}")
        return None


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--n-tiles", type=int, default=4096)
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--out", type=Path, default=None)
    args = ap.parse_args()
    device = torch.device(args.device)

    set_seed(HOLDOUT_SEED, device)
    ideal, eng = og.make_batch(args.n_tiles, og.N, device)
    engine_jump = msm.wrap_jump_abs(eng)
    p75 = float(torch.quantile(engine_jump, 0.75).item())
    p90 = float(torch.quantile(engine_jump, 0.90).item())
    mask_all = torch.ones(args.n_tiles, dtype=torch.bool, device=device)
    mask_top25 = engine_jump >= p75
    mask_top10 = engine_jump >= p90

    methods: list[tuple[str, callable]] = [
        ("no_bake", lambda x: x),
        ("dual_cosine", og.dual_cosine_blend),
        ("seam_fir3", cav.seam_fir3),
    ]
    neural_fn, neural_meta, _, _ = bsm.load_neural_favorite(device)
    methods.append(("neural_favorite", neural_fn))
    for extra in (
        "n2n_corrupt_corrupt",
        "n2n_sibling_supervised",
        "seq_lstm",
        "seq_cnn1d",
    ):
        fn = try_load_baseline_fn(extra, device)
        if fn is not None:
            methods.append((extra, fn))

    strata_masks = {
        "all": mask_all,
        "top25_wrap": mask_top25,
        "top10_wrap": mask_top10,
    }
    results: dict = {
        "meta": {
            "holdout_seed": HOLDOUT_SEED,
            "n_tiles": int(args.n_tiles),
            "L": int(og.N),
            "SEAM_W": int(og.SEAM_W),
            "PROLONG": int(og.PROLONG),
            "wrap_jump_p75": p75,
            "wrap_jump_p90": p90,
            "engine_wrap_jump_mean": float(engine_jump.mean().item()),
            "favorite_meta": neural_meta,
            "stratum_rule": "empirical percentiles of engine wrap-jump |x0-xL-1|",
            "edge_rmse": "RMS(out-ideal) on indices [0:W] U [L-W:L]",
            "click_energy": "mean square first-diff across tiled wrap boundaries",
        },
        "strata": {},
    }

    chunk = 512

    def run_fn(fn, frames: torch.Tensor) -> torch.Tensor:
        if frames.shape[0] <= chunk:
            return fn(frames)
        parts = []
        for i in range(0, frames.shape[0], chunk):
            parts.append(fn(frames[i : i + chunk]))
        return torch.cat(parts, dim=0)

    for mname, fn in methods:
        if device.type == "cuda":
            torch.cuda.empty_cache()
        out = run_fn(fn, eng)
        metrics = per_tile_metrics(ideal, out, seam_w=og.SEAM_W)
        results["strata"][mname] = {}
        for sname, mask in strata_masks.items():
            results["strata"][mname][sname] = summarize(metrics, mask)
        del out, metrics

    # Flatten convenience view: stratum -> method -> stats
    flat = {}
    for sname in strata_masks:
        flat[sname] = {
            mname: results["strata"][mname][sname] for mname, _ in methods
        }
    results["by_stratum"] = flat

    # Sanity asserts
    assert flat["top10_wrap"]["no_bake"]["wrap_jump_mean"] > flat["top25_wrap"]["no_bake"]["wrap_jump_mean"]
    assert flat["top25_wrap"]["no_bake"]["wrap_jump_mean"] > flat["all"]["no_bake"]["wrap_jump_mean"]
    assert flat["top10_wrap"]["no_bake"]["n"] == int(mask_top10.sum().item())
    assert flat["top25_wrap"]["no_bake"]["n"] == int(mask_top25.sum().item())
    assert flat["all"]["no_bake"]["n"] == args.n_tiles
    # Legacy alias for frozen paper readers (identity ≡ no_bake)
    for s in flat.values():
        if isinstance(s, dict) and "no_bake" in s:
            s["identity"] = s["no_bake"]
    if "no_bake" in results.get("strata", {}):
        results["strata"]["identity"] = results["strata"]["no_bake"]
    results["nomenclature"] = {
        "no_bake": "Unrepaired cracked engine (passthrough); legacy key identity",
    }

    out_path = args.out or META_OUT
    if not out_path.parent.is_dir():
        # denoise-opt-meta may sit beside reelsynth
        alt = (
            ROOT.parent
            / "denoise-opt-meta"
            / "paper"
            / "v5"
            / "figures"
            / "cliff_strata.json"
        )
        out_path = alt if alt.parent.is_dir() else (ROOT / "brand" / "artifacts" / "cliff_strata.json")
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(json.dumps(results, indent=2), encoding="utf-8")
    print(f"wrote {out_path}")

    mirror = MIRROR_OUT
    try:
        mirror.parent.mkdir(parents=True, exist_ok=True)
        mirror.write_text(json.dumps(results, indent=2), encoding="utf-8")
        print(f"wrote {mirror}")
    except OSError as exc:
        print(f"mirror skip: {exc}")

    for sname in ("all", "top25_wrap", "top10_wrap"):
        row = flat[sname]
        print(
            f"{sname:12} n={row['no_bake']['n']:4d}  "
            f"nobake_R={row['no_bake']['R_mean']:.4f}  "
            f"dc_R={row['dual_cosine']['R_mean']:.4f}  "
            f"fav_R={row['neural_favorite']['R_mean']:.4f}  "
            f"nobake_jump={row['no_bake']['wrap_jump_mean']:.3f}"
        )


if __name__ == "__main__":
    main()
