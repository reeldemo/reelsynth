"""Harvest literature via klaut-research local MCP tools (in-process)."""
from __future__ import annotations

import asyncio
import json
import os
import sys
from pathlib import Path

# Ensure gateway package is importable when run via uv from that cwd.
os.environ.setdefault("KLAUT_RESEARCH_MODE", "local")
os.environ.setdefault("OPENALEX_MAILTO", "research@klaut.pro")
os.environ.setdefault("CROSSREF_MAILTO", "research@klaut.pro")

from klaut_mcp import tools  # noqa: E402

OUT = Path(__file__).resolve().parents[2] / "brand" / "artifacts" / "literature_klaut_research.json"

QUERIES = [
    "BLEP bandlimited step oscillator virtual analog",
    "wavetable synthesis periodic waveform interpolation",
    "audio denoising unsupervised deep learning without paired data",
    "cycle periodization seam discontinuity audio",
    "meta-learning hyperparameter optimization audio DSP",
    "differentiable digital signal processing DDSP synthesis",
    "polyBLEP oscillator aliasing reduction",
    "self-supervised audio restoration",
]


async def main() -> None:
    all_papers: list[dict] = []
    seen: set[str] = set()
    for q in QUERIES:
        r = await tools.search_papers(q, limit=8, year_from=1995)
        for p in r.get("papers") or []:
            pid = str(p.get("id") or p.get("doi") or p.get("title") or "")
            if not pid or pid in seen:
                continue
            seen.add(pid)
            all_papers.append(
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
    OUT.write_text(json.dumps({"n": len(all_papers), "papers": all_papers}, indent=2), encoding="utf-8")
    print(f"wrote {len(all_papers)} papers -> {OUT}")
    for p in all_papers[:15]:
        title = p.get("title") or p.get("id")
        print(f"- {p.get('year')} | {title} | {p.get('source')}")


if __name__ == "__main__":
    asyncio.run(main())
