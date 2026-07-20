"""Harvest SOTA-ish RL + genetic/evolutionary hybrid literature via Klaut (local).

Writes brand/artifacts/literature_rl_ga_nas_hybrid.json with used vs screened roles
for DenoiseOpt outer-loop design (PPO + GA crossover/mutation + PBT — not claimed SOTA).
"""
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
    r"C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth"
    r"\brand\artifacts\literature_rl_ga_nas_hybrid.json"
)
OUT_META = Path(
    r"C:\Users\Julian\Documents\Programming\github\reeldemo\denoise-opt-meta"
    r"\artifacts\literature_rl_ga_nas_hybrid.json"
)

QUERIES = [
    "evolutionary reinforcement learning hybrid ERL",
    "genetic algorithm policy gradient hybrid",
    "neuroevolution PPO evolutionary strategies",
    "population based training PBT Jaderberg",
    "evolution-guided policy gradient Khadka Tumer",
    "CEM-RL cross-entropy method reinforcement learning",
    "genetic programming neural architecture search",
    "evolutionary neural architecture search Real AmoebaNet",
    "aging evolution NAS Real et al",
    "mixture of experts soft gating Shazeer",
    "algorithm portfolio multi-strategy metaheuristics",
    "hyperheuristic evolutionary algorithm selection",
    "depth neural architecture search progressive growing",
    "progressive neural architecture search PNAS",
    "ENAS reinforcement learning architecture search",
    "DARTS differentiable architecture search",
    "quality diversity MAP-Elites neuroevolution",
    "soft actor critic evolutionary hybrid",
    "multi-objective evolutionary NAS NSGA",
    "deep residual network depth optimization He",
]

# Families we implement / cite as design inspiration for the outer loop.
USED_FAMILY_KEYWORDS = [
    ("evolution-guided policy", "erl_hybrid"),
    ("evolutionary reinforcement", "erl_hybrid"),
    ("erl", "erl_hybrid"),
    ("cem-rl", "cem_rl"),
    ("cross-entropy method", "cem_rl"),
    ("population based training", "pbt"),
    ("population-based training", "pbt"),
    ("pbt", "pbt"),
    ("neuroevolution", "neuroevolution"),
    ("evolutionary strategi", "neuroevolution"),
    ("genetic algorithm", "ga"),
    ("genetic programming", "ga"),
    ("crossover", "ga"),
    ("amoebanet", "evo_nas"),
    ("aging evolution", "evo_nas"),
    ("evolutionary neural architecture", "evo_nas"),
    ("enas", "rl_nas"),
    ("neural architecture search", "rl_nas"),
    ("reinforcement learning.*architecture", "rl_nas"),
    ("darts", "diff_nas"),
    ("differentiable architecture", "diff_nas"),
    ("pnas", "progressive_nas"),
    ("progressive neural architecture", "progressive_nas"),
    ("mixture of experts", "moe"),
    ("moe", "moe"),
    ("soft gating", "moe"),
    ("algorithm portfolio", "portfolio"),
    ("hyperheuristic", "portfolio"),
    ("multi-strategy", "portfolio"),
    ("map-elites", "qd"),
    ("quality diversity", "qd"),
    ("residual network", "depth"),
    ("network depth", "depth"),
    ("deepening", "depth"),
]

SCREEN_OUT_KEYWORDS = [
    "protein folding",
    "drug discovery",
    "covid",
    "clinical trial",
    "medical imaging",
    "mri reconstruction",
    "remote sensing",
    "autonomous driving dataset",
]

SCREENED_RELEVANT_KEYWORDS = [
    "autoML",
    "automl",
    "hyperparameter optimization",
    "bayesian optimization",
    "meta-learning",
    "learned optimizer",
    "nas",
    "architecture search",
    "reinforcement learning",
    "evolutionary",
    "genetic",
]


def classify(title: str, query: str) -> tuple[str, str | None]:
    t = (title or "").lower()
    q = (query or "").lower()
    blob = f"{t} {q}"
    for kw in SCREEN_OUT_KEYWORDS:
        if kw in t and not any(
            a in blob for a in ("architecture", "nas", "reinforcement", "evolutionary", "genetic")
        ):
            return "screened_out", None

    family = None
    for kw, fam in USED_FAMILY_KEYWORDS:
        if kw in blob:
            family = fam
            break

    # Strong hybrid signals → used even if family keyword order missed
    hybrid_hits = sum(
        1
        for k in (
            "evolutionary reinforcement",
            "genetic",
            "neuroevolution",
            "population based",
            "cem-rl",
            "policy gradient",
            "ppo",
            "architecture search",
            "mixture of experts",
        )
        if k in blob
    )

    if family is not None:
        if family in ("erl_hybrid", "cem_rl", "pbt", "ga", "neuroevolution", "evo_nas", "moe"):
            return "used", family
        if hybrid_hits >= 2:
            return "used", family
        return "used", family

    if hybrid_hits >= 2:
        return "used", "erl_hybrid"

    for kw in SCREENED_RELEVANT_KEYWORDS:
        if kw.lower() in blob:
            return "screened_relevant", None

    if any(k in blob for k in ("nas", "reinforcement", "evolutionary", "genetic", "pbt")):
        return "screened_relevant", None
    return "screened_out", None


async def main() -> None:
    papers: list[dict] = []
    seen: set[str] = set()
    for q in QUERIES:
        print(f"query: {q}", flush=True)
        try:
            r = await tools.search_papers(q, limit=12, year_from=2015)
        except Exception as e:
            print(f"  ERROR: {e}", flush=True)
            continue
        n_new = 0
        for p in r.get("papers") or []:
            pid = str(p.get("id") or p.get("doi") or p.get("title") or "")
            if not pid or pid in seen:
                continue
            seen.add(pid)
            title = p.get("title") or ""
            role, family = classify(title, q)
            papers.append(
                {
                    "query": q,
                    "role": role,
                    "method_family": family,
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
            n_new += 1
        print(f"  +{n_new} (total {len(papers)})", flush=True)

    used = [p for p in papers if p["role"] == "used"]
    screened_rel = [p for p in papers if p["role"] == "screened_relevant"]
    screened_out = [p for p in papers if p["role"] == "screened_out"]

    design_notes = {
        "constraint": "Outer loop for DenoiseOpt N=256 residual bake; honest naming (not claimed SOTA)",
        "primary_inner_metric": "residual_score R in [0,1]; DualCosine baseline",
        "depth_objective": "Prefer deeper composable graphs when residual holds (depth bias + searchable max depth)",
        "mixture_objective": "MoE-style soft gates over heterogeneous blocks (unet+attn+dilated…)",
        "outer_loop_implemented": {
            "GA": "tournament parent selection + block/op/hp crossover + mutation (Holland/Real-style NAS evo)",
            "PPO": "Schulman clipped surrogate proposes discrete arch mutations / parent slots",
            "PBT": "Jaderberg-inspired exploit elites + mutate (not full distributed PBT)",
            "ERL_inspired": "Interleave GA population steps with PPO policy updates (Khadka/Tumer ERL spirit, tiny scale)",
        },
        "key_papers_to_cite_accurately": [
            "Khadka & Tumer — Evolution-Guided Policy Gradient (ERL)",
            "Jaderberg et al. — Population Based Training",
            "Real et al. — Regularized Evolution / Aging Evolution / AmoebaNet",
            "Zoph & Le / Pham et al. — RL-NAS / ENAS",
            "Liu et al. — DARTS (screened: differentiable, not our discrete loop)",
            "Shazeer et al. — Mixture-of-Experts soft gating",
            "He et al. — Residual depth / identity mappings enabling deeper nets",
        ],
        "claim_hygiene": "Log branch names GA_CROSSOVER / PPO_MUTATION / PBT_EXPLOIT / MoE_SOFTGATE — no false SOTA claims",
    }

    payload = {
        "title": "RL + genetic/evolutionary hybrids for NAS / HPO / architecture search",
        "n": len(papers),
        "n_used": len(used),
        "n_screened_relevant": len(screened_rel),
        "n_screened_out": len(screened_out),
        "design_notes": design_notes,
        "used": used,
        "screened_relevant": screened_rel,
        "screened_out": screened_out,
        "queries": QUERIES,
        "source": "klaut-research-gateway local MCP research_search_papers",
    }

    OUT_REEL.parent.mkdir(parents=True, exist_ok=True)
    OUT_REEL.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(
        f"wrote {OUT_REEL} n={len(papers)} used={len(used)} "
        f"rel={len(screened_rel)} out={len(screened_out)}"
    )
    try:
        OUT_META.parent.mkdir(parents=True, exist_ok=True)
        OUT_META.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        print(f"synced {OUT_META}")
    except OSError as e:
        print(f"meta sync failed: {e}")


if __name__ == "__main__":
    asyncio.run(main())
