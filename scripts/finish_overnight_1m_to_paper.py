#!/usr/bin/env python3
"""When 500k overnight completes: plots → ingest → write remaining sections → revise → export → paper/v4."""
from __future__ import annotations

import asyncio
import json
import os
import shutil
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

REEL = Path(r"C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth")
META = Path(r"C:\Users\Julian\Documents\Programming\github\reeldemo\denoise-opt-meta")
KLAUT = Path(r"C:\Users\Julian\Documents\Programming\github\klaut-pro\klaut-research-gateway")
TARGET = 500_000  # paper-facing; matches live --iters (1M infeasible for hybrid+depth/MoE in 240h)
PAPER_ID_FILE = META / "paper" / "v4" / "KLAUT_PAPER_ID.txt"


def load_latest() -> dict:
    return json.loads(
        (REEL / "brand" / "artifacts" / "overnight_gpu_rl_arch_latest.json").read_text(encoding="utf-8")
    )


def main() -> int:
    latest = load_latest()
    it = int(latest.get("iter") or 0)
    if it < TARGET:
        print(f"NOT DONE iter={it}", flush=True)
        return 2

    run_dir = Path(latest["run_dir"])
    champ = float(latest["champion_residual"])
    baseline = float(latest["baseline_dual_cosine"])
    py = REEL / ".venv_gpu" / "Scripts" / "python.exe"
    hist = run_dir / "history.jsonl"
    subprocess.check_call(
        [str(py), str(REEL / "scripts" / "plot_overnight_history.py"), str(hist), "--baseline", str(baseline)]
    )

    v4 = META / "paper" / "v4"
    v4.mkdir(parents=True, exist_ok=True)
    blob = {
        "written_at": datetime.now(timezone.utc).isoformat(),
        "status": "FINAL_500K",
        "target_iters": TARGET,
        "iters_completed": it,
        "champion_residual": champ,
        "dual_cosine_baseline": baseline,
        "delta_vs_dual_cosine": champ - baseline,
        "champion_arch": latest.get("champion_arch"),
        "branch_best": latest.get("branch_best"),
        "run_dir": str(run_dir),
        "gpu": latest.get("gpu"),
        "elapsed_sec": latest.get("elapsed_sec"),
        "metric_definition": "R=clamp(1 - residual_rms/max(ideal_rms,eps), 0, 1); 1=best",
        "figures": {
            "overnight_panel": "figures/overnight_panel.png",
            "champ_residual": "figures/champ_residual_vs_iter.png",
            "branch_bests": "figures/branch_bests_vs_iter.png",
            "champion_timeline": "figures/champion_timeline.png",
            "residual_by_branch": "figures/residual_by_branch.png",
        },
        "honest": True,
        "primary_metric": champ,
        "baseline_comparison": {
            "dual_cosine": baseline,
            "champion": champ,
            "delta": champ - baseline,
        },
        "trial_budget": TARGET,
        "baselines": ["DualCosine"],
        "table_rows": [
            {"algo": "champion_500k", "residual": champ, "prior": "rl_nas_combo"},
            {"algo": "DualCosine", "residual": baseline, "prior": "baseline"},
        ],
    }
    (v4 / "results_blob.json").write_text(json.dumps(blob, indent=2), encoding="utf-8")
    (REEL / "brand" / "artifacts" / "overnight_gpu_DONE.flag").write_text(
        datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"), encoding="ascii"
    )

    paper_id = PAPER_ID_FILE.read_text(encoding="utf-8").strip()
    os.environ["RESEARCH_PAPERS_DIR"] = str(META / "paper" / "klaut_artifacts")
    os.environ["OLLAMA_BASE_URL"] = "http://127.0.0.1:11434/v1"
    os.environ["OLLAMA_MODEL"] = "qwen3.5:9b"
    os.environ["OLLAMA_API_KEY"] = "ollama"
    sys.path.insert(0, str(KLAUT))

    async def write_rest() -> dict:
        from klaut_mcp.tools import (
            paper_export,
            paper_ingest_data,
            paper_revise,
            paper_write_subsection,
        )

        paper_ingest_data(paper_id, data=blob, section_id="results")
        needs = (
            "FINAL 500k results ingested. Follow PAPER_WRITING_QUALITY.md. "
            "Honest numbers only from results_blob. Strong Discussion vs DualCosine and branches. "
            f"Champ R={champ:.6f}, DualCosine={baseline:.6f}, delta={champ-baseline:.6f}."
        )
        out = {}
        for sec in ("abstract", "results", "discussion", "limitations", "conclusion"):
            out[sec] = await paper_write_subsection(
                paper_id,
                sec,
                user_needs=needs,
                results_blob=blob,
                use_llm=True,
                use_ollama=True,
                force=True,
            )
        out["revise"] = await paper_revise(paper_id, use_llm=True, use_ollama=True)
        out["export"] = paper_export(paper_id, compile_pdf=True, bump_version=False)
        return out

    result = asyncio.run(write_rest())
    print(json.dumps({k: {kk: vv for kk, vv in (v or {}).items() if kk in ("status", "message", "path", "pdf_path")} for k, v in result.items() if isinstance(v, dict)}, indent=2), flush=True)

    # Copy export into paper/v4
    art = META / "paper" / "klaut_artifacts" / paper_id
    current = (art / "CURRENT").read_text(encoding="utf-8").strip() if (art / "CURRENT").is_file() else "v01"
    ver = art / current
    for name in ("main.tex", "main.pdf"):
        src = ver / name
        if src.is_file():
            shutil.copy2(src, v4 / name)
            print(f"copied {src} -> {v4 / name}", flush=True)
    if (ver / "subsections").is_dir():
        dst = v4 / "subsections"
        dst.mkdir(exist_ok=True)
        for f in (ver / "subsections").glob("*.tex"):
            shutil.copy2(f, dst / f.name)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
