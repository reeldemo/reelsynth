#!/usr/bin/env python3
"""Track champion parameter-count fluctuation across v13 D1 search histories.

Explains why 'Ours' size is not a single number: outer-loop champions jump
across compact and heavy graphs as R_blend improves.
"""
from __future__ import annotations

import json
import statistics as st
import sys
from pathlib import Path
from typing import Any

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402

SEARCH = ROOT / "brand" / "artifacts" / "meta_approach_compare_v13_rblend"
OUT = ROOT / "brand" / "artifacts" / "v13_param_fluctuation"
V10_CHAMP = ROOT / "brand/artifacts/meta_approach_compare_v10/hybrid_lstm/champ_cell.pt"
APPROACHES = ("hybrid_lstm", "random", "cmaes", "tpe", "aging_evo", "reinforce")


def params_from_arch(arch: Any) -> tuple[int, og.ArchConfig] | None:
    if arch is None:
        return None
    if isinstance(arch, str):
        try:
            arch = json.loads(arch)
        except json.JSONDecodeError:
            return None
    if not isinstance(arch, dict):
        return None
    if "architecture" in arch and isinstance(arch["architecture"], dict):
        arch = arch["architecture"]
    try:
        cfg = og.ArchConfig.from_dict(arch)
        cell = og.SeamCell(cfg)
        return sum(p.numel() for p in cell.parameters()), cfg
    except Exception:
        return None


def champ_updates_from_history(history: Path) -> list[dict[str, Any]]:
    rows = [json.loads(l) for l in history.read_text(encoding="utf-8").splitlines() if l.strip()]
    prev = -1.0
    updates: list[dict[str, Any]] = []
    for d in rows:
        cr = d.get("champ_raw")
        if cr is None:
            continue
        cr = float(cr)
        if cr <= prev + 1e-12:
            continue
        out = params_from_arch(d.get("champ_arch") or d.get("arch"))
        n = out[0] if out else None
        cfg = out[1] if out else None
        updates.append(
            {
                "iter": int(d.get("iter", -1)),
                "champ_raw": cr,
                "t_ms": d.get("t_ms"),
                "params": n,
                "blocks": list(cfg.blocks) if cfg else None,
                "width": int(cfg.width) if cfg else None,
                "depth": int(cfg.depth) if cfg else None,
                "cell_kind": cfg.cell_kind if cfg else None,
                "lstm_in_champ": d.get("lstm_in_champ"),
                "xlstm_in_champ": d.get("xlstm_in_champ"),
                "beats_n2n": d.get("beats_n2n"),
            }
        )
        prev = cr
    return updates


def summarize(updates: list[dict[str, Any]]) -> dict[str, Any]:
    ns = [u["params"] for u in updates if isinstance(u.get("params"), int)]
    if not ns:
        return {"n_updates": len(updates), "with_params": 0}
    return {
        "n_updates": len(updates),
        "with_params": len(ns),
        "params_min": min(ns),
        "params_median": float(st.median(ns)),
        "params_max": max(ns),
        "params_mean": float(st.mean(ns)),
        "params_std": float(st.pstdev(ns)) if len(ns) > 1 else 0.0,
        "final_params": ns[-1],
        "final_r_blend": updates[-1]["champ_raw"],
        "final_t_ms": updates[-1].get("t_ms"),
        "final_beats_n2n": updates[-1].get("beats_n2n"),
    }


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    report: dict[str, Any] = {
        "schema": "denoiseopt.v13_param_fluctuation.v1",
        "note": (
            "Champion parameter count is not fixed across search iterations or seeds. "
            "Outer-loop graphs jump between compact and heavy cells as R_blend improves."
        ),
        "v10_locked_hybrid": None,
        "seeds": {},
    }

    if V10_CHAMP.is_file():
        blob = torch.load(V10_CHAMP, map_location="cpu", weights_only=False)
        out = params_from_arch(blob.get("architecture"))
        report["v10_locked_hybrid"] = {
            "path": str(V10_CHAMP),
            "params": out[0] if out else None,
            "r_blend": blob.get("r_blend"),
            "t_ms": blob.get("t_ms"),
            "blocks": list(out[1].blocks) if out else None,
            "cell_kind": out[1].cell_kind if out else None,
        }

    for seed_dir in sorted(SEARCH.glob("*")):
        if not seed_dir.is_dir() or not seed_dir.name.isdigit():
            continue
        seed_rep: dict[str, Any] = {}
        for ap in APPROACHES:
            hist = seed_dir / ap / "history.jsonl"
            ckpt = seed_dir / ap / "checkpoint.json"
            pt = seed_dir / ap / "champ_cell.pt"
            if not hist.is_file() and not ckpt.is_file() and not pt.is_file():
                continue
            entry: dict[str, Any] = {"approach": ap}
            if hist.is_file():
                updates = champ_updates_from_history(hist)
                entry["champ_updates"] = updates
                entry["summary"] = summarize(updates)
            if pt.is_file():
                blob = torch.load(pt, map_location="cpu", weights_only=False)
                out = params_from_arch(blob.get("architecture"))
                entry["live_champ_cell"] = {
                    "params": out[0] if out else None,
                    "r_blend": blob.get("r_blend") or blob.get("champ_raw"),
                    "t_ms": blob.get("t_ms"),
                    "blocks": list(out[1].blocks) if out else None,
                    "cell_kind": out[1].cell_kind if out else None,
                    "depth": int(out[1].depth) if out else None,
                    "width": int(out[1].width) if out else None,
                }
            if ckpt.is_file():
                c = json.loads(ckpt.read_text(encoding="utf-8"))
                entry["checkpoint_iters_done"] = c.get("iters_done")
            seed_rep[ap] = entry
            s = entry.get("summary") or {}
            live = entry.get("live_champ_cell") or {}
            print(
                f"seed={seed_dir.name} {ap}: "
                f"iters={entry.get('checkpoint_iters_done')} "
                f"updates={s.get('n_updates')} "
                f"params[min/med/max]={s.get('params_min')}/{s.get('params_median')}/{s.get('params_max')} "
                f"live_params={live.get('params')} live_R={live.get('r_blend')} "
                f"beats_n2n={s.get('final_beats_n2n')}",
                flush=True,
            )
        report["seeds"][seed_dir.name] = seed_rep

    out_path = OUT / "param_fluctuation.json"
    out_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(f"wrote {out_path}")

    # Short markdown summary
    lines = [
        "# v13 champion parameter fluctuation",
        "",
        "Outer-loop champions are **not** a fixed size. Graph search jumps between compact",
        "and heavy cells; replication across seeds can land on different param counts.",
        "",
    ]
    v10 = report.get("v10_locked_hybrid") or {}
    if v10:
        lines.append(
            f"- **v10 locked hybrid (old primary Ours):** {v10.get('params')} params, "
            f"R_blend={v10.get('r_blend')}"
        )
    for seed, aps in report["seeds"].items():
        lines.append(f"## seed {seed}")
        for ap, entry in aps.items():
            s = entry.get("summary") or {}
            live = entry.get("live_champ_cell") or {}
            lines.append(
                f"- **{ap}** iters={entry.get('checkpoint_iters_done')}: "
                f"champ-update params min/med/max = "
                f"{s.get('params_min')}/{s.get('params_median')}/{s.get('params_max')}; "
                f"live champ = {live.get('params')} params, R={live.get('r_blend')}, "
                f"beats_n2n={s.get('final_beats_n2n')}"
            )
        lines.append("")
    md = OUT / "README.md"
    md.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {md}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
