#!/usr/bin/env python3
"""Broader high-R architecture inference sweep + plots."""
from __future__ import annotations

import json
import sys
from pathlib import Path

import matplotlib.pyplot as plt
import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import bench_inference_same_score as b  # noqa: E402

device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
root = ROOT / "brand" / "artifacts" / "models"
pts = sorted(root.glob("gpu-rl-arch-*/fitted/*_fitted.pt"), key=lambda p: p.stat().st_mtime)
cands = []
for pt in pts[-120:]:
    try:
        cfg, cell, residual, arch = b.load_fitted(pt, device)
    except Exception:
        continue
    if residual < 0.98:
        continue
    cands.append((residual, pt, cfg, cell))

seen = set()
uniq = []
for residual, pt, cfg, cell in sorted(cands, key=lambda x: -x[0]):
    sig = (cfg.depth, cfg.width, cfg.cell_kind, tuple(cfg.blocks), tuple(cfg.ops))
    if sig in seen:
        continue
    seen.add(sig)
    uniq.append((residual, pt, cfg, cell))
    if len(uniq) >= 15:
        break

print("unique high-R archs", len(uniq), "device", device)
results = []
for residual, pt, cfg, cell in uniq:
    timing = b.time_inference(cell, cfg.ops, device, batch=64)
    results.append(
        {
            "tag": pt.stem,
            "run": pt.parent.parent.name,
            "path": str(pt),
            "residual_saved": residual,
            "residual_live": timing["residual_live"],
            "ms_per_batch": timing["ms_per_batch"],
            "ms_per_sample": timing["ms_per_sample"],
            "depth": cfg.depth,
            "width": cfg.width,
            "cell_kind": cfg.cell_kind,
            "blocks": list(cfg.blocks),
            "n_params": sum(p.numel() for p in cell.parameters()),
        }
    )
    print(
        f"{pt.parent.parent.name}/{pt.stem}: R={residual:.4f} "
        f"{timing['ms_per_batch']:.3f}ms params={results[-1]['n_params']}"
    )

fav = sorted(results, key=lambda x: (-x["residual_saved"], x["ms_per_batch"]))[0]
out = ROOT / "brand" / "artifacts" / "inference_bench"
out.mkdir(parents=True, exist_ok=True)
payload = {
    "device": str(device),
    "batch": 64,
    "favorite": fav,
    "results": results,
}
(out / "inference_bench.json").write_text(json.dumps(payload, indent=2), encoding="utf-8")

fig, ax = plt.subplots(figsize=(7.2, 4.2), dpi=140)
ax.scatter(
    [r["residual_saved"] for r in results],
    [r["ms_per_batch"] for r in results],
    s=40,
    alpha=0.85,
)
ax.scatter(
    [fav["residual_saved"]],
    [fav["ms_per_batch"]],
    s=140,
    marker="*",
    c="crimson",
    label="favorite",
    zorder=5,
)
ax.set_xlabel("Residual R")
ax.set_ylabel("ms/batch (64)")
ax.legend()
ax.grid(True, alpha=0.3)
ax.set_title("High-R architectures: score vs inference time")
fig.tight_layout()
fig.savefig(out / "fig_inference_vs_residual.png")
plt.close(fig)

top = sorted(results, key=lambda x: (-x["residual_saved"], x["ms_per_batch"]))
fig, ax = plt.subplots(figsize=(8, 4.2), dpi=140)
ax.barh(range(len(top)), [t["ms_per_batch"] for t in top], color="#4C72B0")
ax.set_yticks(range(len(top)))
ax.set_yticklabels([f"{t['run'][-6:]}:{t['tag'][-22:]}" for t in top], fontsize=7)
ax.invert_yaxis()
ax.set_xlabel("ms/batch")
ax.set_title("Inference latency among diverse high-R models")
fig.tight_layout()
fig.savefig(out / "fig_inference_latency_bars.png")
plt.close(fig)
print("FAVORITE", json.dumps(fav, indent=2))
print("wrote", out)
