"""Harvest RL / NAS / meta-learning algorithm-design literature via Klaut Research MCP."""
from __future__ import annotations

import asyncio
import json
import os
import sys
from pathlib import Path

os.environ.setdefault("KLAUT_RESEARCH_MODE", "local")
os.environ.setdefault("OPENALEX_MAILTO", "research@klaut.pro")
os.environ.setdefault("CROSSREF_MAILTO", "research@klaut.pro")

GW = Path(r"C:\Users\Julian\Documents\Programming\github\klaut-pro\klaut-research-gateway")
sys.path.insert(0, str(GW))

from klaut_mcp import tools  # noqa: E402

OUT_REEL = Path(
    r"C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth\brand\artifacts\literature_rl_meta_nas.json"
)
OUT_META = Path(
    r"C:\Users\Julian\Documents\Programming\github\reeldemo\denoise-opt-meta\artifacts\literature_rl_meta_nas.json"
)

QUERIES = [
    "reinforcement learning neural architecture search ENAS",
    "RL-NAS controller policy architecture search",
    "neural architecture search survey AutoML",
    "population based training hyperparameter optimization",
    "differentiable architecture search DARTS",
    "evolutionary neural architecture search",
    "reinforcement learning AutoML algorithm configuration",
    "meta-learning algorithm discovery deep learning",
    "learned optimizer meta-learning gradient descent",
    "hyperparameter optimization racing irace SMAC",
    "multi-objective neural architecture search",
    "controller RNN architecture search Zoph Le",
    "efficient neural architecture search progressive",
    "audio neural architecture search DSP operators",
    "differentiable digital signal processing DDSP",
    "learned FIR filter audio processing",
    "meta-learning hyperparameter architecture search survey",
    "REINFORCE policy gradient architecture search",
    "bandit algorithm configuration AutoML",
    "evolutionary strategies continuous control NAS",
]

# Papers we already lean on in DenoiseOpt meta (used in design).
USED_KEYWORDS = [
    "population based training",
    "irace",
    "racing",
    "noise2noise",
    "darts",
    "enas",
    "neural architecture search",
    "bayesian optimization",
    "maml",
    "bilevel",
    "moea/d",
    "learned optimizer",
    "ddsp",
    "reinforce",
]


def classify(title: str, query: str) -> str:
    t = (title or "").lower()
    q = (query or "").lower()
    blob = t + " " + q
    for kw in USED_KEYWORDS:
        if kw in blob:
            return "used"
    # High-relevance NAS/RL/meta even if not in used list → screened_relevant
    for kw in (
        "architecture search",
        "automl",
        "meta-learning",
        "reinforcement",
        "evolutionary",
        "hyperparameter",
        "algorithm configuration",
        "controller",
        "darts",
        "enas",
        "ddsp",
        "audio",
        "filter",
    ):
        if kw in blob:
            return "screened_relevant"
    return "screened_out"


async def main() -> None:
    papers: list[dict] = []
    seen: set[str] = set()
    for q in QUERIES:
        print(f"query: {q}", flush=True)
        try:
            r = await tools.search_papers(q, limit=12, year_from=1985)
        except Exception as e:
            print(f"  ERROR: {e}", flush=True)
            continue
        for p in r.get("papers") or []:
            pid = str(p.get("id") or p.get("doi") or p.get("title") or "")
            if not pid or pid in seen:
                continue
            seen.add(pid)
            title = p.get("title") or ""
            role = classify(title, q)
            papers.append(
                {
                    "query": q,
                    "role": role,
                    **{
                        k: p.get(k)
                        for k in (
                            "id",
                            "doi",
                            "title",
                            "year",
                            "authors",
                            "venue",
                            "citation_count",
                            "url",
                            "source",
                            "oa_url",
                        )
                    },
                }
            )

    used = [p for p in papers if p["role"] == "used"]
    screened_rel = [p for p in papers if p["role"] == "screened_relevant"]
    screened_out = [p for p in papers if p["role"] == "screened_out"]

    design_notes = {
        "primary_metric": "residual_score = clamp(1 - residual_rms / max(ideal_rms, 1e-6), 0, 1)",
        "branches": [
            "lit_combo — PBT / irace / evo / MOEA·D / bilevel / residual_primary / bake combos",
            "arch_search — discrete seam-op DAG + small MLP/FIR on seam windows, fit-to-convergence",
            "rl_policy — REINFORCE / bandit hybrid proposing arch edits + θ actions; reward=residual",
            "combo — lit strategies × architectures",
        ],
        "used_vs_screened": {
            "used": "directly informed operator families or overnight RL/NAS design choices",
            "screened_relevant": "on-topic but not wired into code this overnight",
            "screened_out": "retrieved but off-topic / weak match",
        },
        "convergence": "rel |J_prev-J_cur|/max(|J_prev|,1e-6) < 1e-4 for 3 consecutive sweeps; max 16",
    }

    payload = {
        "title": "RL / NAS / meta-learning literature for DenoiseOpt overnight",
        "n": len(papers),
        "n_used": len(used),
        "n_screened_relevant": len(screened_rel),
        "n_screened_out": len(screened_out),
        "design_notes": design_notes,
        "used": used,
        "screened_relevant": screened_rel,
        "screened_out": screened_out,
        "papers": papers,
        "queries": QUERIES,
    }

    for out in (OUT_REEL, OUT_META):
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        print(f"wrote {len(papers)} papers -> {out}", flush=True)

    top = sorted(papers, key=lambda x: -(x.get("citation_count") or 0))[:20]
    for p in top:
        title = (p.get("title") or "")[:90]
        print(
            f"{p.get('role')} {p.get('year')} cites={p.get('citation_count') or 0} | {title}",
            flush=True,
        )


if __name__ == "__main__":
    asyncio.run(main())
