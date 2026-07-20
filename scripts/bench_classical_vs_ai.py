#!/usr/bin/env python3
"""Compare classical (non-AI) bake denoisers vs neural favorite on R + latency.

Same residual metric and MakeBatch distribution as overnight_gpu_rl_arch.
"""
from __future__ import annotations

import json
import math
import sys
import time
from pathlib import Path

import matplotlib.pyplot as plt
import torch
import torch.nn.functional as F

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
import bench_inference_same_score as bib  # noqa: E402

SEAM_W = og.SEAM_W


def classic_quadratic(frames: torch.Tensor) -> torch.Tensor:
    w = SEAM_W
    n = frames.shape[1]
    out = frames.clone()
    for i in range(w):
        a = (i / max(w - 1, 1)) ** 2
        out[:, i] = (1 - a) * frames[:, i] + a * frames[:, n - w + i]
        out[:, n - w + i] = (1 - a) * frames[:, n - w + i] + a * frames[:, i]
    return out


def cosine_fade(frames: torch.Tensor) -> torch.Tensor:
    w = SEAM_W
    n = frames.shape[1]
    out = frames.clone()
    for i in range(w):
        a = 0.5 - 0.5 * math.cos(math.pi * i / max(w, 1))
        out[:, i] = (1 - a) * frames[:, i] + a * frames[:, n - w + i]
        out[:, n - w + i] = (1 - a) * frames[:, n - w + i] + a * frames[:, i]
    return out


def soft_wide_fade(frames: torch.Tensor, mult: int = 2) -> torch.Tensor:
    """Wider raised-cosine fade (Soft seam style)."""
    w = min(SEAM_W * mult, frames.shape[1] // 4)
    n = frames.shape[1]
    out = frames.clone()
    for i in range(w):
        a = 0.5 - 0.5 * math.cos(math.pi * i / max(w, 1))
        out[:, i] = (1 - a) * frames[:, i] + a * frames[:, n - w + i]
        out[:, n - w + i] = (1 - a) * frames[:, n - w + i] + a * frames[:, i]
    return out


def detrend_wrap(frames: torch.Tensor) -> torch.Tensor:
    """Remove linear trend so first≈last (exact wrap close)."""
    n = frames.shape[1]
    t = torch.linspace(0, 1, n, device=frames.device).unsqueeze(0)
    delta = frames[:, -1:] - frames[:, :1]
    return frames - delta * t


def crossfade(frames: torch.Tensor) -> torch.Tensor:
    w = SEAM_W
    n = frames.shape[1]
    out = frames.clone()
    for i in range(w):
        a = i / max(w - 1, 1)
        # circular crossfade of head with mirrored tail
        out[:, i] = (1 - a) * frames[:, i] + a * frames[:, n - 1 - i]
        out[:, n - 1 - i] = (1 - a) * frames[:, n - 1 - i] + a * frames[:, i]
    return out


def seam_fir3(frames: torch.Tensor) -> torch.Tensor:
    """Light 3-tap lowpass only near ends."""
    w = SEAM_W
    k = torch.tensor([0.25, 0.5, 0.25], device=frames.device).view(1, 1, 3)
    x = frames.unsqueeze(1)
    y = F.conv1d(F.pad(x, (1, 1), mode="circular"), k).squeeze(1)
    out = frames.clone()
    out[:, :w] = y[:, :w]
    out[:, -w:] = y[:, -w:]
    return out


def ensemble_v2(frames: torch.Tensor) -> torch.Tensor:
    """Detrend → DualCosine → seam FIR (classical stack)."""
    return seam_fir3(og.dual_cosine_blend(detrend_wrap(frames)))


def hann_blend(frames: torch.Tensor) -> torch.Tensor:
    return og.hann_blend(frames)


CLASSICAL = [
    ("no_bake", lambda x: x, "non_ai"),
    ("classic_quadratic", classic_quadratic, "non_ai"),
    ("cosine_fade", cosine_fade, "non_ai"),
    ("dual_cosine", og.dual_cosine_blend, "non_ai"),
    ("soft_wide_fade", soft_wide_fade, "non_ai"),
    ("detrend", detrend_wrap, "non_ai"),
    ("crossfade", crossfade, "non_ai"),
    ("hann_blend", hann_blend, "non_ai"),
    ("seam_fir3", seam_fir3, "non_ai"),
    ("ensemble_detrend_dc_fir", ensemble_v2, "non_ai"),
]


@torch.no_grad()
def bench_fn(fn, device, batch=64, warmup=20, repeats=100):
    ideal, eng = og.make_batch(batch, og.N, device)
    for _ in range(warmup):
        out = fn(eng)
        _ = og.residual_score(ideal, out).mean()
    if device.type == "cuda":
        torch.cuda.synchronize()
    # score on fresh fixed batch for stability
    torch.manual_seed(0)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(0)
    ideal, eng = og.make_batch(batch, og.N, device)
    out = fn(eng)
    r = float(og.residual_score(ideal, out).mean().item())
    if device.type == "cuda":
        torch.cuda.synchronize()
    t0 = time.perf_counter()
    for _ in range(repeats):
        out = fn(eng)
        _ = og.residual_score(ideal, out).mean()
    if device.type == "cuda":
        torch.cuda.synchronize()
    ms = 1000.0 * (time.perf_counter() - t0) / repeats
    return {"residual": r, "ms_per_batch": ms, "ms_per_sample": ms / batch}


def main():
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    batch = 64
    rows = []
    for name, fn, kind in CLASSICAL:
        m = bench_fn(fn, device, batch=batch)
        rows.append({"name": name, "kind": kind, **m, "n_params": 0})
        print(f"{name:28} R={m['residual']:.4f}  {m['ms_per_batch']:.3f} ms/batch")

    # Neural favorite
    fav_meta = json.loads(
        (ROOT / "brand/artifacts/inference_bench/inference_bench.json").read_text(encoding="utf-8")
    )
    fav = fav_meta["favorite"]
    cfg, cell, residual, _ = bib.load_fitted(Path(fav["path"]), device)

    def neural_fn(eng):
        return og.apply_ops(eng, cell, cfg.ops)

    m = bench_fn(neural_fn, device, batch=batch)
    rows.append(
        {
            "name": "neural_favorite_meta",
            "kind": "ai",
            "residual": m["residual"],
            "residual_saved": residual,
            "ms_per_batch": m["ms_per_batch"],
            "ms_per_sample": m["ms_per_sample"],
            "n_params": sum(p.numel() for p in cell.parameters()),
            "tag": fav.get("tag"),
        }
    )
    print(
        f"{'neural_favorite_meta':28} R={m['residual']:.4f}  {m['ms_per_batch']:.3f} ms/batch  "
        f"params={rows[-1]['n_params']}"
    )

    out = ROOT / "brand/artifacts/classical_vs_ai_bench"
    out.mkdir(parents=True, exist_ok=True)
    payload = {"device": str(device), "batch": batch, "results": rows}
    (out / "classical_vs_ai.json").write_text(json.dumps(payload, indent=2), encoding="utf-8")

    # Scatter
    fig, ax = plt.subplots(figsize=(7.5, 4.4), dpi=140)
    for r in rows:
        color = "#C44E52" if r["kind"] == "ai" else "#4C72B0"
        marker = "*" if r["kind"] == "ai" else "o"
        ax.scatter(
            r["residual"],
            r["ms_per_batch"],
            s=140 if r["kind"] == "ai" else 50,
            c=color,
            marker=marker,
            zorder=5 if r["kind"] == "ai" else 3,
            label="AI (meta favorite)" if r["kind"] == "ai" else None,
        )
        ax.annotate(
            r["name"].replace("_", "\n") if r["kind"] == "ai" else r["name"],
            (r["residual"], r["ms_per_batch"]),
            fontsize=6,
            xytext=(4, 4),
            textcoords="offset points",
        )
    # only one AI label
    handles, labels = ax.get_legend_handles_labels()
    if handles:
        ax.legend(handles[:1], labels[:1], fontsize=8)
    ax.set_xlabel("Residual score R (1=best)")
    ax.set_ylabel(f"Inference latency (ms / batch={batch})")
    ax.set_title("Classical bake denoisers vs meta-learned neural favorite")
    ax.grid(True, alpha=0.3)
    fig.tight_layout()
    fig.savefig(out / "fig_classical_vs_ai_scatter.png")
    plt.close(fig)

    # Grouped bars: residual
    order = sorted(rows, key=lambda x: -x["residual"])
    fig, axes = plt.subplots(1, 2, figsize=(10.5, 4.2), dpi=140)
    colors = ["#C44E52" if r["kind"] == "ai" else "#4C72B0" for r in order]
    axes[0].barh(range(len(order)), [r["residual"] for r in order], color=colors)
    axes[0].set_yticks(range(len(order)))
    axes[0].set_yticklabels([r["name"] for r in order], fontsize=7)
    axes[0].invert_yaxis()
    axes[0].set_xlabel("Residual R")
    axes[0].set_title("Score")
    axes[0].set_xlim(0.7, 1.0)

    order_t = sorted(rows, key=lambda x: x["ms_per_batch"])
    colors_t = ["#C44E52" if r["kind"] == "ai" else "#4C72B0" for r in order_t]
    axes[1].barh(range(len(order_t)), [r["ms_per_batch"] for r in order_t], color=colors_t)
    axes[1].set_yticks(range(len(order_t)))
    axes[1].set_yticklabels([r["name"] for r in order_t], fontsize=7)
    axes[1].invert_yaxis()
    axes[1].set_xlabel("ms / batch")
    axes[1].set_title("Latency (lower better)")
    fig.suptitle("Non-AI classical methods vs AI meta favorite", fontsize=11)
    fig.tight_layout()
    fig.savefig(out / "fig_classical_vs_ai_bars.png")
    plt.close(fig)

    classical = [r for r in rows if r["kind"] == "non_ai"]
    # no_bake is a passthrough control (unrepaired engine), not a denoiser — report separately
    from baseline_names import is_no_bake, NO_BAKE_DISPLAY

    best_classical_incl_no_bake = max(classical, key=lambda x: x["residual"])
    best_classical = max(
        (r for r in classical if not is_no_bake(r["name"])),
        key=lambda x: x["residual"],
    )
    dual = next(r for r in classical if r["name"] == "dual_cosine")
    no_bake = next(r for r in classical if is_no_bake(r["name"]))
    ai = next(r for r in rows if r["kind"] == "ai")
    summary = {
        "best_classical": best_classical,
        "best_classical_incl_no_bake": best_classical_incl_no_bake,
        "best_classical_incl_identity": best_classical_incl_no_bake,  # legacy alias
        "no_bake_passthrough": no_bake,
        "identity_noop": no_bake,  # legacy alias
        "dual_cosine": dual,
        "ai_favorite": ai,
        "delta_R_ai_minus_best_classical": ai["residual"] - best_classical["residual"],
        "delta_R_ai_minus_dual_cosine": ai["residual"] - dual["residual"],
        "delta_R_ai_minus_no_bake": ai["residual"] - no_bake["residual"],
        "delta_R_ai_minus_identity": ai["residual"] - no_bake["residual"],  # legacy alias
        "latency_ratio_ai_over_best_classical_score": ai["ms_per_batch"]
        / max(best_classical["ms_per_batch"], 1e-9),
        "latency_ratio_ai_over_dual_cosine": ai["ms_per_batch"]
        / max(dual["ms_per_batch"], 1e-9),
        "no_bake_display": NO_BAKE_DISPLAY,
    }
    payload["summary"] = summary
    (out / "classical_vs_ai.json").write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print("SUMMARY", json.dumps(summary, indent=2))
    print("wrote", out)


if __name__ == "__main__":
    main()
