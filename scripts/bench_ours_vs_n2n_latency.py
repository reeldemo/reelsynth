#!/usr/bin/env python3
"""Ours vs Noise2Noise inference-latency comparison across boards + batch sizes.

Dimensions:
  - boards: holdout, 10 multi-family keys (seed0), Factory+FX, AKWF
  - batch: 1, 8, 64, 256
  - metrics: ms/batch, ms/sample, Ours/N2N ratio

Writes brand/artifacts/v13_ours_vs_n2n_latency/latency_compare.json (+ TeX table).
"""
from __future__ import annotations

import argparse
import json
import statistics as st
import sys
import time
from pathlib import Path
from typing import Any, Callable

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
import bench_sota_matrix as bsm  # noqa: E402
import bench_v10_n2n_vs_ours as v10  # noqa: E402

OUT = ROOT / "brand" / "artifacts" / "v13_ours_vs_n2n_latency"
BATCHES = (1, 8, 64, 256)
WARMUP = 10
REPEATS = 50


def time_ms(fn: Callable[[], Any], device: torch.device, *, warmup: int, repeats: int) -> float:
    for _ in range(warmup):
        fn()
    if device.type == "cuda":
        torch.cuda.synchronize()
    t0 = time.perf_counter()
    for _ in range(repeats):
        fn()
    if device.type == "cuda":
        torch.cuda.synchronize()
    return 1000.0 * (time.perf_counter() - t0) / repeats


def row(name: str, batch: int, t_ours: float, t_n2n: float, n: int) -> dict[str, Any]:
    return {
        "board": name,
        "batch": batch,
        "n_cycles": n,
        "ours_ms_batch": t_ours,
        "n2n_ms_batch": t_n2n,
        "ours_ms_sample": t_ours / max(batch, 1),
        "n2n_ms_sample": t_n2n / max(batch, 1),
        "ratio_ours_over_n2n": (t_ours / t_n2n) if t_n2n > 0 else float("inf"),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--seed", type=int, default=20260719)
    ap.add_argument("--out-dir", type=Path, default=OUT)
    ap.add_argument(
        "--champ",
        type=Path,
        default=ROOT / "brand/artifacts/meta_approach_compare_v10/hybrid_lstm/champ_cell.pt",
    )
    ap.add_argument("--warmup", type=int, default=WARMUP)
    ap.add_argument("--repeats", type=int, default=REPEATS)
    args = ap.parse_args()

    device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")
    args.out_dir.mkdir(parents=True, exist_ok=True)

    n2n, n2n_path, _ = v10.load_n2n(device)
    cell, cfg, champ_blob = v10.load_ours(args.champ, device)

    def ours_fn(eng: torch.Tensor) -> torch.Tensor:
        return og.apply_ops(eng, cell, cfg.ops)

    # Build board tensors at max batch, then slice for smaller batches
    v10.set_seed(args.seed, device)
    boards: list[tuple[str, torch.Tensor]] = []
    _, eng_h = og.make_batch(max(BATCHES), og.N, device)
    boards.append(("holdout", eng_h))

    for fam in bsm.FAMILIES:
        _, eng_f = bsm.make_family_batch(fam, max(BATCHES), og.N, device, seed=args.seed)
        boards.append((f"multifamily/{fam}", eng_f))

    try:
        import real_wt_wrap_protocol as rwp

        cycles_pt = ROOT / "brand" / "artifacts" / "real_wt_cycles" / "cycles.pt"
        if cycles_pt.is_file():
            blob = torch.load(cycles_pt, map_location="cpu", weights_only=False)
            for label, key in (("factory_fx", "reelsynth_factory"), ("akwf", "oa_instrument")):
                if key in blob and hasattr(blob[key], "shape"):
                    cyc = blob[key].to(device)
                    closed = rwp.close_seam_ideal(cyc)
                    _, eng_r = rwp.apply_open_wrap_cliff(closed, seed=args.seed)
                    # Cap to max batch for fair timed forward; report n_total separately
                    boards.append((label, eng_r[: max(BATCHES)].contiguous()))
    except Exception as exc:  # noqa: BLE001
        print(f"real_wt skip: {exc}", flush=True)

    rows: list[dict[str, Any]] = []
    for board_name, eng_full in boards:
        for batch in BATCHES:
            if eng_full.shape[0] < batch:
                continue
            eng = eng_full[:batch].contiguous()
            t_n2n = time_ms(lambda: n2n(eng), device, warmup=args.warmup, repeats=args.repeats)
            t_ours = time_ms(lambda: ours_fn(eng), device, warmup=args.warmup, repeats=args.repeats)
            r = row(board_name, batch, t_ours, t_n2n, int(eng.shape[0]))
            rows.append(r)
            print(
                f"{board_name:32s} batch={batch:4d}  "
                f"ours={t_ours:7.3f}ms  n2n={t_n2n:7.3f}ms  "
                f"ratio={r['ratio_ours_over_n2n']:5.2f}x  "
                f"per_samp ours={r['ours_ms_sample']:.4f} n2n={r['n2n_ms_sample']:.4f}",
                flush=True,
            )

    # Summaries at batch=64 (paper default)
    b64 = [r for r in rows if r["batch"] == 64]
    hold = next((r for r in b64 if r["board"] == "holdout"), None)
    mf = [r for r in b64 if r["board"].startswith("multifamily/")]
    summary = {
        "batch_default": 64,
        "holdout_batch64": hold,
        "multifamily_batch64": {
            "n_boards": len(mf),
            "ours_ms_mean": st.mean([r["ours_ms_batch"] for r in mf]) if mf else None,
            "n2n_ms_mean": st.mean([r["n2n_ms_batch"] for r in mf]) if mf else None,
            "ratio_mean": st.mean([r["ratio_ours_over_n2n"] for r in mf]) if mf else None,
            "ours_ms_std": st.pstdev([r["ours_ms_batch"] for r in mf]) if len(mf) > 1 else 0.0,
            "n2n_ms_std": st.pstdev([r["n2n_ms_batch"] for r in mf]) if len(mf) > 1 else 0.0,
        },
        "batch_sweep_holdout": [r for r in rows if r["board"] == "holdout"],
    }

    # Also fold multi-seed holdout timings from prior eval (batch 64, 20 timed forwards)
    ms_root = ROOT / "brand" / "artifacts" / "holdout_multiseed_v13"
    multiseed_lat: dict[str, Any] = {"seeds": [], "holdout": {}, "multifamily_mean": {}}
    if ms_root.is_dir():
        ho, hn, mo_all, mn_all = [], [], [], []
        for sd in sorted(ms_root.glob("20*")):
            p = sd / "n2n_vs_ours" / "n2n_vs_ours.json"
            if not p.is_file():
                continue
            d = json.loads(p.read_text(encoding="utf-8"))
            multiseed_lat["seeds"].append(int(sd.name))
            ho.append(float(d["holdout"]["ours"]["t_ms"]))
            hn.append(float(d["holdout"]["n2n"]["t_ms"]))
            for v in d.get("multifamily", {}).values():
                mo_all.append(float(v["ours"]["t_ms"]))
                mn_all.append(float(v["n2n"]["t_ms"]))
        if ho:
            multiseed_lat["holdout"] = {
                "ours_ms_mean": st.mean(ho),
                "ours_ms_std": st.stdev(ho) if len(ho) > 1 else 0.0,
                "n2n_ms_mean": st.mean(hn),
                "n2n_ms_std": st.stdev(hn) if len(hn) > 1 else 0.0,
                "ratio": st.mean(ho) / st.mean(hn),
                "per_seed_ours": ho,
                "per_seed_n2n": hn,
            }
        if mo_all:
            multiseed_lat["multifamily_mean"] = {
                "n_timings": len(mo_all),
                "ours_ms_mean": st.mean(mo_all),
                "ours_ms_std": st.stdev(mo_all) if len(mo_all) > 1 else 0.0,
                "n2n_ms_mean": st.mean(mn_all),
                "n2n_ms_std": st.stdev(mn_all) if len(mn_all) > 1 else 0.0,
                "ratio": st.mean(mo_all) / st.mean(mn_all),
            }

    payload = {
        "schema": "denoiseopt.v13_ours_vs_n2n_latency.v1",
        "device": str(device),
        "warmup": args.warmup,
        "repeats": args.repeats,
        "batches": list(BATCHES),
        "champ": str(args.champ),
        "n2n_checkpoint": str(n2n_path),
        "champ_n_params": sum(p.numel() for p in cell.parameters()),
        "n2n_n_params": sum(p.numel() for p in n2n.parameters()),
        "rows": rows,
        "summary": summary,
        "multiseed_from_holdout_eval": multiseed_lat,
        "finished_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
    }
    out_json = args.out_dir / "latency_compare.json"
    out_json.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    # TeX table: batch=64 boards + holdout batch sweep
    lines = [
        r"% Auto-generated by scripts/bench_ours_vs_n2n_latency.py",
        r"\begin{table}[t]",
        r"  \centering",
        r"  \caption{Inference latency: Ours (hybrid favorite) vs Noise2Noise.",
        rf"    Device \texttt{{{device}}}; warmup {args.warmup}, timed {args.repeats} forwards.",
        r"    Primary rows use batch $64$ (paper default).}",
        r"  \label{tab:ours-n2n-latency}",
        r"  \footnotesize",
        r"  \setlength{\tabcolsep}{3pt}",
        r"  \begin{tabular}{@{}lrrrr@{}}",
        r"    \toprule",
        r"    Board / batch & Ours ms & N2N ms & Ratio & Ours ms/samp \\",
        r"    \midrule",
    ]
    for r in b64:
        lines.append(
            f"    {r['board'].replace('_', r'\\_')} ($B{r['batch']}$) & "
            f"{r['ours_ms_batch']:.2f} & {r['n2n_ms_batch']:.2f} & "
            f"{r['ratio_ours_over_n2n']:.2f}$\\times$ & {r['ours_ms_sample']:.3f} \\\\"
        )
    lines.append(r"    \midrule")
    lines.append(r"    \multicolumn{5}{@{}l@{}}{\emph{Holdout batch sweep}} \\")
    for r in summary["batch_sweep_holdout"]:
        lines.append(
            f"    holdout ($B{r['batch']}$) & "
            f"{r['ours_ms_batch']:.2f} & {r['n2n_ms_batch']:.2f} & "
            f"{r['ratio_ours_over_n2n']:.2f}$\\times$ & {r['ours_ms_sample']:.3f} \\\\"
        )
    if multiseed_lat.get("holdout"):
        h = multiseed_lat["holdout"]
        lines.append(r"    \midrule")
        lines.append(
            f"    holdout 5-seed mean & "
            f"{h['ours_ms_mean']:.2f}$\\pm${h['ours_ms_std']:.2f} & "
            f"{h['n2n_ms_mean']:.2f}$\\pm${h['n2n_ms_std']:.2f} & "
            f"{h['ratio']:.2f}$\\times$ & --- \\\\"
        )
    lines += [
        r"    \bottomrule",
        r"  \end{tabular}",
        r"\end{table}",
        "",
    ]
    tex_path = args.out_dir / "tab_ours_n2n_latency.tex"
    tex_path.write_text("\n".join(lines), encoding="utf-8")
    print(f"wrote {out_json}")
    print(f"wrote {tex_path}")
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
