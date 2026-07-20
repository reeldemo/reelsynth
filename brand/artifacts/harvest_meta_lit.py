"""Harvest meta-learning + audio algo engineering literature via Klaut Research MCP."""
from __future__ import annotations

import asyncio
import json
import os
from pathlib import Path

os.environ.setdefault("KLAUT_RESEARCH_MODE", "local")
os.environ.setdefault("OPENALEX_MAILTO", "research@klaut.pro")
os.environ.setdefault("CROSSREF_MAILTO", "research@klaut.pro")

from klaut_mcp import tools  # noqa: E402

OUT = Path(
    r"C:\Users\Julian\Documents\Programming\github\reeldemo\reelsynth\brand\artifacts\literature_meta_audio.json"
)

QUERIES = [
    "meta-learning hyperparameter optimization neural networks survey",
    "Bayesian optimization hyperparameter tuning machine learning",
    "population based training hyperparameter meta learning",
    "AutoML neural architecture search audio",
    "few-shot meta-learning signal processing",
    "MAML model-agnostic meta-learning",
    "hyperparameter optimization Gaussian process",
    "self-supervised audio denoising unpaired",
    "Noise2Noise unsupervised denoising",
    "deep learning audio restoration survey",
    "wavetable synthesis differentiable DDSP",
    "virtual analog synthesis BLEP bandlimited",
    "algorithm configuration racing SMAC irace",
    "evolutionary strategies continuous optimization",
    "multi-objective hyperparameter optimization Pareto",
]


async def main() -> None:
    papers: list[dict] = []
    seen: set[str] = set()
    for q in QUERIES:
        r = await tools.search_papers(q, limit=10, year_from=1990)
        for p in r.get("papers") or []:
            pid = str(p.get("id") or p.get("doi") or p.get("title") or "")
            if not pid or pid in seen:
                continue
            seen.add(pid)
            papers.append(
                {
                    "query": q,
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
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps({"n": len(papers), "papers": papers}, indent=2), encoding="utf-8")
    print(f"wrote {len(papers)} papers -> {OUT}")
    top = sorted(papers, key=lambda x: -(x.get("citation_count") or 0))[:25]
    for p in top:
        title = (p.get("title") or p.get("id") or "")[:100]
        print(f"{p.get('year')} cites={p.get('citation_count') or 0} | {title}")


if __name__ == "__main__":
    asyncio.run(main())
