#!/usr/bin/env python3
"""Cheap R_blend re-fit of frozen matched-5k champion architectures.

Does NOT re-run the multi-hour 5k outer-loop search (that remains D1).
Loads champ_arch + champ_hp from meta_approach_compare/*/summary.json,
re-fits once under residual_score_blend, and writes a rescored JSON + TeX.

  .venv_gpu/Scripts/python.exe scripts/rescore_meta_champs_rblend.py --device cuda
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

APPROACHES = ("random", "cmaes", "reinforce", "aging_evo", "tpe", "hybrid_lstm")
LABELS = {
    "random": "Random NAS",
    "cmaes": "Cont.\\ CMA-ES",
    "reinforce": "Arch REINFORCE",
    "aging_evo": "Aging evolution",
    "tpe": "TPE Bayes NAS",
    "hybrid_lstm": "Ours (hybrid GA--PPO)",
}


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument(
        "--compare-dir",
        type=Path,
        default=ROOT / "brand/artifacts/meta_approach_compare",
    )
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument(
        "--out",
        type=Path,
        default=ROOT / "brand/artifacts/meta_approach_compare_rblend_rescore.json",
    )
    ap.add_argument(
        "--out-tex",
        type=Path,
        default=ROOT / "brand/artifacts/meta_approaches_table_rblend_rescore.tex",
    )
    args = ap.parse_args()
    device = torch.device(args.device if torch.cuda.is_available() or args.device == "cpu" else "cpu")

    baseline = float(og.dual_cosine_baseline(device, batch=128))
    rows = []
    approaches: dict[str, dict] = {}
    for name in APPROACHES:
        summary_path = args.compare_dir / name / "summary.json"
        if not summary_path.exists():
            print(f"skip missing {summary_path}")
            continue
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        arch = summary.get("champ_arch") or summary.get("champ_cfg")
        hp_d = summary.get("champ_hp")
        if not arch or not hp_d:
            print(f"skip {name}: missing champ_arch/champ_hp")
            continue
        cfg = og.ArchConfig.from_dict(arch)
        hp = og.HyperParams.from_dict(hp_d)
        print(f"re-fit {name} under R_blend …")
        r_blend, j, j_scored, t_ms, cell = og.evaluate_candidate(
            cfg,
            hp,
            device,
            baseline=baseline,
            fit_steps_default=int(hp.fit_steps or 24),
            batch_default=int(hp.batch or 48),
        )
        lstm = "lstm" in (cfg.blocks or []) or cfg.cell_kind == "lstm"
        xlstm = "xlstm" in (cfg.blocks or []) or cfg.cell_kind == "xlstm"
        rec = {
            "approach": name,
            "method": name,
            "primary_metric": "r_blend",
            "blend_alpha": float(og.BLEND_ALPHA),
            "champ_r_blend": float(r_blend),
            "champ_r": float(r_blend),  # table helper
            "champ_j": float(j),
            "champ_j_scored": float(j_scored),
            "champ_t_ms": float(t_ms),
            "delta_r_vs_dual_cosine": float(r_blend) - baseline,
            "baseline_dual_cosine": baseline,
            "wall_h": float(summary.get("wall_h") or 0.0),
            "wall_h_note": "original prolonged-R 5k search wall (not re-search)",
            "lstm_in_champ": bool(lstm or summary.get("lstm_in_champ")),
            "xlstm_in_champ": bool(xlstm or summary.get("xlstm_in_champ")),
            "prolonged_champ_raw": float(summary.get("champ_raw") or summary.get("champ_r") or 0.0),
            "champ_arch": cfg.to_dict(),
            "champ_hp": hp.to_dict(),
            "n_params": int(sum(p.numel() for p in cell.parameters())),
            "source_summary": str(summary_path),
        }
        approaches[name] = rec
        rows.append(
            {
                "method": name,
                "champ_r": float(r_blend),
                "delta_r_vs_dual_cosine": float(r_blend) - baseline,
                "wall_h": float(summary.get("wall_h") or 0.0),
                "lstm_in_champ": rec["lstm_in_champ"],
                "xlstm_in_champ": rec["xlstm_in_champ"],
            }
        )
        print(
            f"  {name}: R_blend={r_blend:.5f} dR_vs_DC={r_blend - baseline:+.5f} "
            f"(prolonged was {rec['prolonged_champ_raw']:.5f})"
        )

    payload = {
        "schema": "denoiseopt.meta_approach_rblend_rescore.v1",
        "protocol": "EVAL_PROTOCOL v10.1 / frozen champ arch re-fit under R_blend",
        "primary_metric": "r_blend",
        "blend_alpha": float(og.BLEND_ALPHA),
        "note": (
            "Champion architectures and HPs come from the historical matched-5k search that "
            "maximized prolonged R. Each champ is re-fit once under R_blend (not a full 5k "
            "re-search). Full overnight re-search remains Deferred D1 "
            "(bench_meta_approaches_5k.py --iters 5000)."
        ),
        "seed": 1902771841,
        "baseline_dual_cosine": baseline,
        "approaches": approaches,
        "table": rows,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")

    tex_rows = []
    for row in rows:
        m = row["method"]
        name = LABELS.get(m, m)
        r = float(row["champ_r"])
        d = float(row["delta_r_vs_dual_cosine"])
        wh = float(row["wall_h"])
        lstm = "yes" if row.get("lstm_in_champ") else "no"
        xlstm = "yes" if row.get("xlstm_in_champ") else "no"
        tex_rows.append(
            f"    {name} & {r:.5f} & ${d:+.5f}$ & {wh:.2f} & {lstm} & {xlstm} \\\\"
        )
    body = "\n".join(tex_rows) if tex_rows else "    \\textit{(pending)} & -- & -- & -- & -- & -- \\\\"
    tex = (
        "\\begin{table}[t]\n"
        "  \\centering\n"
        "  \\caption{Frozen matched-5k champion architectures re-fit once under "
        "$R_{\\mathrm{blend}}$ ($\\alpha{=}0.7$; search seed \\texttt{1902771841}). "
        "Outer-loop search still maximized prolonged $R$; wall hours are the original "
        "5k-search walls. Full $R_{\\mathrm{blend}}$ re-search remains Deferred D1.}\n"
        "  \\label{tab:meta-approaches}\n"
        "  \\setlength{\\tabcolsep}{2pt}\n"
        "  \\scriptsize\n"
        "  \\begin{tabular}{@{}lrrrcc@{}}\n"
        "    \\toprule\n"
        "    Method & $R_{\\mathrm{blend}}$ & $\\Delta R$ vs DC & h & LSTM & xLSTM \\\\\n"
        "    \\midrule\n"
        f"{body}\n"
        "    \\bottomrule\n"
        "  \\end{tabular}\n"
        "\\end{table}\n"
    )
    args.out_tex.write_text(tex, encoding="utf-8")
    print(f"Wrote {args.out}")
    print(f"Wrote {args.out_tex}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
