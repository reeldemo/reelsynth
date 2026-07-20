#!/usr/bin/env python3
"""Phase 3a/3c SOTA matrix: methods x (R, SNR, SDR, wrap-jump, latency, params).

≥20 diverse waveforms (10 generative families × 2 seeds), classical rows,
neural favorite, fixed MLP-on-R and CNN/U-Net-lite baselines, Wilcoxon /
bootstrap vs DualCosine, overnight compute summary.

No PESQ/STOI (non-speech cycles).
"""
from __future__ import annotations

import argparse
import json
import math
import sys
import time
from pathlib import Path

import torch
import torch.nn as nn

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
import bench_classical_vs_ai as cav  # noqa: E402
import bench_canonical_eval_dataset as bced  # noqa: E402
import bench_inference_same_score as bib  # noqa: E402
import metrics_snr_sdr as msm  # noqa: E402

FAMILIES = [
    "sine_cliff",
    "harmonic_fft",
    "am_fm",
    "nonlinear",
    "combo",
    "triple_mix",
    "extreme_overlay",
    "open_wrap_bias",
    "soft_noise",
    "wide_seam",
]
WAVEFORM_SEEDS = [bced.CANONICAL_EVAL_SEED + i for i in range(2)]  # 2 × 10 = 20


def set_seed(seed: int, device: torch.device) -> None:
    torch.manual_seed(seed)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(seed)


def make_family_batch(
    family: str, batch: int, n: int, device: torch.device, *, seed: int
) -> tuple[torch.Tensor, torch.Tensor]:
    """Procedural family variants of make_batch (Python stand-ins for Rust sound_bench)."""
    set_seed(seed, device)
    t = torch.linspace(0, 1, n, device=device).unsqueeze(0).expand(batch, -1)
    freqs = 1.0 + 3.0 * torch.rand(batch, 1, device=device)
    phase = 2 * math.pi * torch.rand(batch, 1, device=device)
    ideal = torch.sin(2 * math.pi * freqs * t + phase)
    ideal = ideal + 0.15 * torch.sin(4 * math.pi * freqs * t + phase * 0.7)

    if family == "harmonic_fft":
        for k, amp in ((3, 0.08), (5, 0.05), (7, 0.03)):
            ideal = ideal + amp * torch.sin(2 * math.pi * k * freqs * t + phase * (0.3 * k))
    elif family == "am_fm":
        am = 1.0 + 0.25 * torch.sin(2 * math.pi * 0.5 * t + phase)
        fm = freqs * (1.0 + 0.08 * torch.sin(2 * math.pi * t))
        ideal = am * torch.sin(2 * math.pi * fm * t + phase)
        ideal = ideal + 0.1 * torch.sin(4 * math.pi * fm * t)
    elif family == "nonlinear":
        ideal = torch.tanh(1.6 * ideal)
    elif family == "combo":
        ideal = ideal + 0.12 * torch.sin(6 * math.pi * freqs * t)
        ideal = torch.tanh(1.2 * ideal)
    elif family == "triple_mix":
        ideal = (
            0.55 * ideal
            + 0.25 * torch.sin(2 * math.pi * (freqs + 1.5) * t + phase * 1.3)
            + 0.20 * torch.sin(2 * math.pi * (freqs * 0.5) * t)
        )
    elif family == "extreme_overlay":
        ideal = ideal + 0.2 * torch.sin(10 * math.pi * freqs * t + phase)
        ideal = torch.tanh(2.0 * ideal)
    elif family == "soft_noise":
        ideal = ideal + 0.03 * torch.randn(batch, n, device=device)

    eng = ideal.clone()
    cliff_lo, cliff_hi = 0.08, 0.43
    noise_end, noise_mid = 0.02, 0.15
    w = og.SEAM_W
    if family == "open_wrap_bias":
        cliff_lo, cliff_hi = 0.25, 0.65
        noise_end = 0.04
    elif family == "wide_seam":
        w = min(og.SEAM_W * 2, n // 4)
    elif family == "extreme_overlay":
        cliff_lo, cliff_hi = 0.15, 0.55

    cliff = (cliff_lo + (cliff_hi - cliff_lo) * torch.rand(batch, 1, device=device)) * (
        1.0 - 2.0 * torch.rand(batch, 1, device=device)
    )
    for i in range(w):
        a = i / max(w - 1, 1)
        eng[:, i] = eng[:, i] + cliff.squeeze(-1) * (1 - a)
        eng[:, -w + i] = eng[:, -w + i] - cliff.squeeze(-1) * a
    noise = noise_end * torch.randn(batch, n, device=device)
    noise[:, w:-w] *= noise_mid
    eng = eng + noise
    return ideal, eng


@torch.no_grad()
def score_fn(
    fn,
    ideal: torch.Tensor,
    eng: torch.Tensor,
    *,
    n_params: int = 0,
    warmup: int = 2,
    repeats: int = 12,
) -> dict:
    device = eng.device
    for _ in range(warmup):
        _ = fn(eng)
    if device.type == "cuda":
        torch.cuda.synchronize()
    t0 = time.perf_counter()
    out = None
    for _ in range(repeats):
        out = fn(eng)
    if device.type == "cuda":
        torch.cuda.synchronize()
    dt_ms = (time.perf_counter() - t0) * 1000.0 / repeats
    assert out is not None
    r = og.residual_score(ideal, out)
    sec = msm.secondary_metrics(ideal, out, periods=int(og.PROLONG))
    return {
        "residual_R": float(r.mean().item()),
        "ms_per_batch": float(dt_ms),
        "n_params": int(n_params),
        **sec,
    }


def load_neural_favorite(device: torch.device):
    fav_meta = json.loads(
        (ROOT / "brand/artifacts/inference_bench/inference_bench.json").read_text(encoding="utf-8")
    )
    fav = fav_meta["favorite"]
    cfg, cell, residual_saved, _ = bib.load_fitted(Path(fav["path"]), device)

    def neural_fn(eng):
        return og.apply_ops(eng, cell, cfg.ops)

    return (
        neural_fn,
        {
            "tag": fav.get("tag"),
            "path": fav["path"],
            "residual_saved": residual_saved,
            "n_params": sum(p.numel() for p in cell.parameters()),
        },
        cell,
        cfg,
    )


def train_fixed_baseline(
    *,
    cell_kind: str,
    blocks: list[str],
    depth: int,
    width: int,
    device: torch.device,
    steps: int = 80,
    batch: int = 48,
    lr: float = 3e-3,
) -> tuple[callable, int, float]:
    """Fixed-arch MLP or CNN/U-Net-lite trained on 1-R (no outer NAS)."""
    cfg = og.ArchConfig(
        depth=depth,
        width=width,
        act="gelu",
        ops=["dual_cosine", "mlp_seam", "fir3", "cycle_net"],
        wet=0.55,
        fir=[0.25, 0.5, 0.25, 0.1, 0.1],
        cell_kind=cell_kind,
        soft_logits=[0.0] * len(og.OPS),
        blocks=blocks,
        use_adv_aux=False,
        moe_mode="sequential",
    )
    cell = og.SeamCell(cfg).to(device)
    train_r, _ = og.fit_cell(
        cell, cfg.ops, device, steps=steps, batch=batch, lr=lr, adv_coef=0.0
    )
    cell.eval()
    n_params = sum(p.numel() for p in cell.parameters())

    def fn(eng):
        return og.apply_ops(eng, cell, cfg.ops)

    return fn, n_params, float(train_r)


def wilcoxon_signed_rank_approx(diffs: list[float]) -> dict:
    """Two-sided Wilcoxon signed-rank without scipy (normal approx + bootstrap CI)."""
    d = torch.tensor([x for x in diffs if abs(x) > 1e-12], dtype=torch.float64)
    n = int(d.numel())
    if n < 5:
        return {"n": n, "note": "too_few_nonzero_pairs", "p_approx": None}
    abs_d = d.abs()
    order = torch.argsort(abs_d)
    ranks = torch.empty_like(abs_d)
    ranks[order] = torch.arange(1, n + 1, dtype=torch.float64)
    w_plus = float(ranks[d > 0].sum().item())
    w_minus = float(ranks[d < 0].sum().item())
    w = min(w_plus, w_minus)
    mean_w = n * (n + 1) / 4.0
    var_w = n * (n + 1) * (2 * n + 1) / 24.0
    z = (w - mean_w) / math.sqrt(max(var_w, 1e-12))
    # two-sided normal approx
    p = math.erfc(abs(z) / math.sqrt(2.0))
    # bootstrap CI of mean delta
    g = torch.Generator()
    g.manual_seed(20260719)
    boots = []
    for _ in range(2000):
        idx = torch.randint(0, n, (n,), generator=g)
        boots.append(float(d[idx].mean().item()))
    boots_t = torch.tensor(boots)
    lo = float(boots_t.quantile(0.025).item())
    hi = float(boots_t.quantile(0.975).item())
    return {
        "n": n,
        "W_plus": w_plus,
        "W_minus": w_minus,
        "W": w,
        "z_approx": z,
        "p_approx_two_sided": p,
        "mean_delta_R": float(d.mean().item()),
        "bootstrap_ci95_mean_delta_R": [lo, hi],
        "n_bootstrap": 2000,
    }


def mine_compute_budget(run_dir: Path) -> dict:
    hist = run_dir / "history.jsonl"
    meta_path = run_dir / "run_meta.json"
    meta = json.loads(meta_path.read_text(encoding="utf-8")) if meta_path.exists() else {}
    rows = [json.loads(l) for l in hist.open(encoding="utf-8") if l.strip()] if hist.exists() else []
    if not rows:
        return {"run_dir": str(run_dir), "error": "empty_history"}
    last = rows[-1]
    t_sec = float(last.get("t_sec") or 0.0)
    # arch evals ≈ clean iterations (one proposal scored per iter)
    n_iters = int(last.get("iter") or len(rows))
    peak_mem = None
    try:
        if torch.cuda.is_available():
            peak_mem = float(torch.cuda.max_memory_allocated() / (1024**3))
    except Exception:
        pass
    return {
        "run_id": run_dir.name,
        "gpu": meta.get("gpu") or "NVIDIA GeForce RTX 3090",
        "torch": meta.get("torch"),
        "seed": meta.get("seed"),
        "algo_tag": meta.get("algo_tag"),
        "dual_cosine_baseline": meta.get("dual_cosine_baseline"),
        "n_history_rows": len(rows),
        "final_iter": n_iters,
        "final_champ_R": float(last.get("champ") or 0.0),
        "elapsed_hours": t_sec / 3600.0,
        "elapsed_sec": t_sec,
        "arch_evaluations_approx": n_iters,
        "pop_size": meta.get("pop_size"),
        "peak_mem_GiB_this_process": peak_mem,
        "branch_bests_final": {
            "ppo": last.get("branch_best_ppo"),
            "ga": last.get("branch_best_ga"),
            "pbt": last.get("branch_best_pbt"),
            "nas": last.get("branch_best_nas"),
            "combo": last.get("branch_best_combo"),
        },
        "note": (
            "Arch evaluations ≈ clean search iterations (one fitted proposal per iter). "
            "Peak mem from this bench process is not overnight peak; overnight peak not "
            "logged — report GPU model + hours + eval count as primary budget."
        ),
    }


def ablation_from_history(run_dir: Path) -> list[dict]:
    """Minimal ablations via branch-best freeze (honest: not separate controlled runs)."""
    hist = run_dir / "history.jsonl"
    if not hist.exists():
        return []
    rows = [json.loads(l) for l in hist.open(encoding="utf-8") if l.strip()]
    last = rows[-1]
    mapping = [
        ("PPO-only (branch best)", "branch_best_ppo"),
        ("GA-only (branch best)", "branch_best_ga"),
        ("PBT (branch best)", "branch_best_pbt"),
        ("NAS (branch best)", "branch_best_nas"),
        ("combo (branch best)", "branch_best_combo"),
        ("Full hybrid champion", "champ"),
    ]
    out = []
    for name, key in mapping:
        out.append(
            {
                "config": name,
                "R": float(last.get(key) or 0.0),
                "source": "history_branch_best_freeze",
                "run_id": run_dir.name,
                "final_iter": int(last.get("iter") or 0),
            }
        )
    return out


def aggregate_rows(per_wave: list[dict]) -> dict:
    keys = [
        "residual_R",
        "snr_db_mean",
        "sdr_db_mean",
        "wrap_jump_mean",
        "ms_per_batch",
    ]
    agg = {}
    for k in keys:
        vals = torch.tensor([r[k] for r in per_wave], dtype=torch.float64)
        agg[f"{k}_mean"] = float(vals.mean().item())
        agg[f"{k}_std"] = float(vals.std(unbiased=False).item()) if len(per_wave) > 1 else 0.0
    return agg


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--batch", type=int, default=64)
    ap.add_argument(
        "--gate-run",
        type=Path,
        default=ROOT / "brand/artifacts/models/gpu-rl-arch-20260719T083019Z",
        help="5k-gate overnight run for ablations/compute",
    )
    ap.add_argument(
        "--out",
        type=Path,
        default=ROOT / "brand" / "artifacts" / "sota_matrix.json",
    )
    ap.add_argument("--mlp-steps", type=int, default=80)
    ap.add_argument("--cnn-steps", type=int, default=80)
    ap.add_argument("--skip-learned", action="store_true")
    args = ap.parse_args()
    device = torch.device(args.device)
    if device.type == "cuda":
        torch.cuda.reset_peak_memory_stats()

    wave_specs = [(fam, seed) for seed in WAVEFORM_SEEDS for fam in FAMILIES]
    assert len(wave_specs) >= 20

    # Build method callables
    methods: list[tuple[str, str, callable, int]] = []
    for name, fn, kind in cav.CLASSICAL:
        methods.append((name, kind, fn, 0))

    neural_fn, neural_meta, _cell, _cfg = load_neural_favorite(device)
    methods.append(("neural_favorite", "ai", neural_fn, int(neural_meta["n_params"])))

    learned_meta = {}
    if not args.skip_learned:
        print("Training fixed MLP-on-R baseline...")
        mlp_fn, mlp_params, mlp_train_r = train_fixed_baseline(
            cell_kind="mlp",
            blocks=["mlp"],
            depth=4,
            width=24,
            device=device,
            steps=args.mlp_steps,
        )
        methods.append(("mlp_on_R", "learned_fixed", mlp_fn, mlp_params))
        learned_meta["mlp_on_R"] = {"train_R_last": mlp_train_r, "n_params": mlp_params}

        print("Training fixed CNN/U-Net-lite-on-R baseline...")
        cnn_fn, cnn_params, cnn_train_r = train_fixed_baseline(
            cell_kind="unet",
            blocks=["unet", "conv1d"],
            depth=6,
            width=24,
            device=device,
            steps=args.cnn_steps,
        )
        methods.append(("cnn_unet_on_R", "learned_fixed", cnn_fn, cnn_params))
        learned_meta["cnn_unet_on_R"] = {"train_R_last": cnn_train_r, "n_params": cnn_params}

    # Score every method on every waveform
    per_method: dict[str, list[dict]] = {name: [] for name, _, _, _ in methods}
    wave_catalog = []
    for fam, seed in wave_specs:
        ideal, eng = make_family_batch(fam, args.batch, og.N, device, seed=seed)
        wave_catalog.append({"family": fam, "seed": seed, "batch": args.batch})
        print(f"scoring family={fam} seed={seed}")
        for name, kind, fn, n_params in methods:
            m = score_fn(fn, ideal, eng, n_params=n_params)
            m.update({"family": fam, "seed": seed, "kind": kind, "name": name})
            per_method[name].append(m)

    dual_rows = per_method["dual_cosine"]
    dual_by_key = {(r["family"], r["seed"]): r["residual_R"] for r in dual_rows}

    results = []
    for name, kind, _fn, n_params in methods:
        rows = per_method[name]
        agg = aggregate_rows(rows)
        deltas = [
            r["residual_R"] - dual_by_key[(r["family"], r["seed"])] for r in rows
        ]
        entry = {
            "name": name,
            "kind": kind,
            "n_params": n_params,
            "n_waveforms": len(rows),
            **agg,
            "delta_R_vs_dual_cosine_mean": float(sum(deltas) / len(deltas)),
            "delta_R_vs_dual_cosine_std": float(
                torch.tensor(deltas, dtype=torch.float64).std(unbiased=False).item()
            ),
            "per_waveform": rows,
        }
        if name == "neural_favorite":
            entry["favorite_meta"] = neural_meta
        results.append(entry)

    # Stats: neural favorite vs DualCosine
    fav_deltas = [
        r["residual_R"] - dual_by_key[(r["family"], r["seed"])]
        for r in per_method["neural_favorite"]
    ]
    stats = {
        "neural_favorite_vs_dual_cosine": wilcoxon_signed_rank_approx(fav_deltas),
    }
    if "mlp_on_R" in per_method:
        mlp_deltas = [
            r["residual_R"] - dual_by_key[(r["family"], r["seed"])]
            for r in per_method["mlp_on_R"]
        ]
        stats["mlp_on_R_vs_dual_cosine"] = wilcoxon_signed_rank_approx(mlp_deltas)
    if "cnn_unet_on_R" in per_method:
        cnn_deltas = [
            r["residual_R"] - dual_by_key[(r["family"], r["seed"])]
            for r in per_method["cnn_unet_on_R"]
        ]
        stats["cnn_unet_on_R_vs_dual_cosine"] = wilcoxon_signed_rank_approx(cnn_deltas)

    compute = mine_compute_budget(args.gate_run)
    ablations = ablation_from_history(args.gate_run)

    # Canonical single-seed freeze block (for paper primary table alignment)
    ideal_c, eng_c = bced.make_frozen_batch(args.batch, bced.CANONICAL_EVAL_SEED, device)
    canonical = []
    for name, kind, fn, n_params in methods:
        m = score_fn(fn, ideal_c, eng_c, n_params=n_params, warmup=3, repeats=20)
        canonical.append({"name": name, "kind": kind, **m})
    dual_r = next(c["residual_R"] for c in canonical if c["name"] == "dual_cosine")
    for c in canonical:
        c["delta_R_vs_dual_cosine"] = c["residual_R"] - dual_r

    payload = {
        "protocol": "EVAL_PROTOCOL v1 / Phase 3a+3c",
        "device": str(device),
        "canonical_eval_seed": bced.CANONICAL_EVAL_SEED,
        "overnight_search_seed": og.DEFAULT_SEED,
        "score_batch": int(args.batch),
        "cycle_length": int(og.N),
        "prolong_tiles": int(og.PROLONG),
        "n_waveforms": len(wave_specs),
        "families": FAMILIES,
        "waveform_seeds": WAVEFORM_SEEDS,
        "wave_catalog": wave_catalog,
        "note": (
            "SNR/SDR are tiled vs procedural ideal. No PESQ/STOI. "
            "Families are Python generative stand-ins spanning harmonic/AM-FM/nonlinear/"
            "overlay/open-wrap variants of make_batch (not a 1:1 Rust sound_bench port). "
            "Ablations are branch-best freezes from the 5k-gate history, not isolated re-runs."
        ),
        "learned_baselines": learned_meta,
        "canonical_holdout": canonical,
        "results_multifamily": results,
        "stats": stats,
        "ablations": ablations,
        "compute_budget": compute,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(f"Wrote {args.out}")
    print("--- multifamily mean R ---")
    for r in sorted(results, key=lambda x: -x["residual_R_mean"]):
        print(
            f"{r['name']:28} R={r['residual_R_mean']:.4f}±{r['residual_R_std']:.4f}  "
            f"SNR={r['snr_db_mean_mean']:.2f}  SDR={r['sdr_db_mean_mean']:.2f}  "
            f"jump={r['wrap_jump_mean_mean']:.3f}  dR={r['delta_R_vs_dual_cosine_mean']:+.4f}"
        )
    print("stats", json.dumps(stats, indent=2))
    print("compute", json.dumps(compute, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
