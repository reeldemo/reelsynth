#!/usr/bin/env python3
"""Score methods on Rust-exported sound_bench tiles (≥20).

Loads brand/artifacts/sound_bench_tiles_20.json from
`cargo run --release --bin export_sound_bench_tiles`.

  .venv_gpu/Scripts/python.exe scripts/bench_rust_sound_bench_tiles.py
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


def load_tiles(path: Path, device: torch.device, batch_expand: int) -> list[dict]:
    blob = json.loads(path.read_text(encoding="utf-8"))
    out = []
    for t in blob["tiles"]:
        eng = torch.tensor(t["engine"], dtype=torch.float32, device=device)
        ideal = torch.tensor(t["ideal"], dtype=torch.float32, device=device)
        # Expand single cycle to a mini-batch of identical tiles (latency geometry)
        eng_b = eng.unsqueeze(0).expand(batch_expand, -1).contiguous()
        ideal_b = ideal.unsqueeze(0).expand(batch_expand, -1).contiguous()
        out.append(
            {
                "seed": int(t["seed"]),
                "family": str(t["family"]),
                "engine": eng_b,
                "ideal": ideal_b,
                "wrap_jump_engine": float(t.get("wrap_jump_engine") or 0.0),
            }
        )
    return out


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--tiles",
        type=Path,
        default=ROOT / "brand/artifacts/sound_bench_tiles_20.json",
    )
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--batch-expand", type=int, default=64)
    ap.add_argument(
        "--out",
        type=Path,
        default=ROOT / "brand/artifacts/rust_sound_bench_matrix.json",
    )
    args = ap.parse_args()
    if not args.tiles.exists():
        print(f"ERROR: missing {args.tiles}; run export_sound_bench_tiles first", file=sys.stderr)
        return 2

    device = torch.device(args.device)
    tiles = load_tiles(args.tiles, device, args.batch_expand)
    assert len(tiles) >= 20

    methods: list[tuple[str, object, int]] = []
    for name, fn, _kind in cav.CLASSICAL:
        methods.append((name, fn, 0))
    neural_fn, neural_meta, _cell, _cfg = bsm.load_neural_favorite(device)
    methods.append(("neural_favorite", neural_fn, int(neural_meta["n_params"])))

    per_method: dict[str, list[dict]] = {n: [] for n, _, _ in methods}
    for tile in tiles:
        for name, fn, n_params in methods:
            m = bsm.score_fn(fn, tile["ideal"], tile["engine"], n_params=n_params)
            m.update(
                {
                    "family": tile["family"],
                    "seed": tile["seed"],
                    "name": name,
                    "source": "rust_sound_bench",
                }
            )
            per_method[name].append(m)

    dual_by = {
        (r["family"], r["seed"]): r["residual_R"] for r in per_method["dual_cosine"]
    }
    results = []
    for name, _fn, n_params in methods:
        rows = per_method[name]
        agg = bsm.aggregate_rows(rows)
        deltas = [r["residual_R"] - dual_by[(r["family"], r["seed"])] for r in rows]
        results.append(
            {
                "name": name,
                "n_params": n_params,
                "n_waveforms": len(rows),
                **agg,
                "delta_R_vs_dual_cosine_mean": float(sum(deltas) / len(deltas)),
                "per_waveform": rows,
            }
        )

    fav_deltas = [
        r["residual_R"] - dual_by[(r["family"], r["seed"])]
        for r in per_method["neural_favorite"]
    ]
    stats = {
        "neural_favorite_vs_dual_cosine": bsm.wilcoxon_signed_rank_approx(fav_deltas),
    }

    payload = {
        "protocol": "EVAL_PROTOCOL v1 / Rust sound_bench tiles",
        "source_tiles": str(args.tiles),
        "n_tiles": len(tiles),
        "families": sorted({t["family"] for t in tiles}),
        "note": (
            "Waveforms exported from Rust sound_bench (generate_sound / generate_sound_ideal). "
            "Closes the Python stand-in gap for ≥20 family tiles. Residual geometry uses the "
            "Python overnight residual_score on tiled cycles (same secondary SNR/SDR helpers)."
        ),
        "results": results,
        "stats": stats,
        "favorite_meta": neural_meta,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(f"Wrote {args.out}")
    for r in sorted(results, key=lambda x: -x["residual_R_mean"]):
        print(
            f"{r['name']:28} R={r['residual_R_mean']:.4f}±{r['residual_R_std']:.4f}  "
            f"SNR={r['snr_db_mean_mean']:.2f}  dR={r['delta_R_vs_dual_cosine_mean']:+.4f}"
        )
    print("stats", json.dumps(stats, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
