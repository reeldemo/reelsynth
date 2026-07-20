"""Harvest SOTA-ish audio/signal denoising architecture literature via Klaut (local).

Writes brand/artifacts/literature_audio_denoise_arch.json with used vs screened roles
for DenoiseOpt NAS cell/block design (N=256 seam bake — keep eval tractable).
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
    r"\brand\artifacts\literature_audio_denoise_arch.json"
)
OUT_META = Path(
    r"C:\Users\Julian\Documents\Programming\github\reeldemo\denoise-opt-meta"
    r"\artifacts\literature_audio_denoise_arch.json"
)

QUERIES = [
    "Wave-U-Net audio source separation",
    "Demucs music source separation convolutional",
    "Conv-TasNet time domain audio separation",
    "dual-path RNN DPRNN speech separation",
    "transformer speech enhancement attention",
    "SEGAN speech enhancement generative adversarial",
    "diffusion model audio denoising speech enhancement",
    "U-Net speech enhancement spectrogram",
    "dilated convolutional residual network audio denoising",
    "dense convolutional network audio noise reduction",
    "CRN convolutional recurrent speech enhancement",
    "metricGAN speech enhancement adversarial",
    "Noise2Noise unsupervised denoising audio",
    "wavetable periodic signal restoration denoising",
    "self-supervised audio denoising deep learning",
    "lightweight neural network real-time speech enhancement",
    "temporal convolutional network TCN audio separation",
    "gated residual network audio denoising",
    "attention U-Net speech enhancement",
    "score-based generative model audio restoration",
]

# Families we will implement as searchable cells/blocks (scaled for N=256).
USED_FAMILY_KEYWORDS = [
    ("wave-u-net", "unet"),
    ("u-net", "unet"),
    ("demucs", "conv_stack"),  # Demucs-like encoder ideas → small 1D conv/unet
    ("conv-tasnet", "tcn_dilated"),
    ("tasnet", "tcn_dilated"),
    ("dilated", "tcn_dilated"),
    ("temporal convolutional", "tcn_dilated"),
    ("tcn", "tcn_dilated"),
    ("dual-path", "dual_path"),
    ("dprnn", "dual_path"),
    ("transformer", "attn"),
    ("attention", "attn"),
    ("self-attention", "attn"),
    ("residual", "residual"),
    ("dense", "dense"),
    ("gated", "gated"),
    ("noise2noise", "noise_cond"),
    ("self-supervised", "noise_cond"),
    ("unsupervised", "noise_cond"),
    ("crn", "dual_path"),
    ("convolutional recurrent", "dual_path"),
    ("lightweight", "mlp"),
    ("real-time", "mlp"),
]

# Too heavy / full generative stacks for overnight N=256 residual bake.
SCREEN_OUT_KEYWORDS = [
    "fullband",
    "48 khz",
    "48khz",
    "large language",
    "llm",
    "video",
    "image denoising",
    "medical imaging",
    "mri",
    "ct scan",
    "remote sensing",
]

SCREENED_RELEVANT_KEYWORDS = [
    "segan",
    "metricgan",
    "gan",
    "adversarial",
    "diffusion",
    "score-based",
    "score based",
    "demucs",
    "htdemucs",
    "source separation",
    "speech enhancement",
    "audio denoising",
    "noise reduction",
]


def classify(title: str, query: str) -> tuple[str, str | None]:
    t = (title or "").lower()
    q = (query or "").lower()
    blob = f"{t} {q}"
    for kw in SCREEN_OUT_KEYWORDS:
        if kw in blob and not any(
            a in blob for a in ("speech", "audio", "music", "sound", "wav", "voice")
        ):
            return "screened_out", None
        if kw in t and kw in ("image denoising", "medical imaging", "mri", "video", "llm"):
            return "screened_out", None

    family = None
    for kw, fam in USED_FAMILY_KEYWORDS:
        if kw in blob:
            family = fam
            break

    if family is not None:
        # Heavy full-stack papers still "used" as inspiration but noted
        if any(k in blob for k in ("diffusion", "score-based", "segan", "metricgan", "gan")):
            if family not in ("noise_cond", "attn", "unet"):
                return "screened_relevant", family
        return "used", family

    for kw in SCREENED_RELEVANT_KEYWORDS:
        if kw in blob:
            return "screened_relevant", None

    if any(k in blob for k in ("audio", "speech", "music", "sound", "denoising", "enhancement")):
        return "screened_relevant", None
    return "screened_out", None


async def main() -> None:
    papers: list[dict] = []
    seen: set[str] = set()
    for q in QUERIES:
        print(f"query: {q}", flush=True)
        try:
            r = await tools.search_papers(q, limit=10, year_from=2015)
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
                    "arch_family": family,
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
        "constraint": "N=256 cycle + SEAM_W=8 residual bake on RTX 3090; prefer meaningful arch complexity over max it/s",
        "primary_metric": "residual_score = clamp(1 - residual_rms / max(ideal_rms, 1e-6), 0, 1); 1=best",
        "baseline": "DualCosine seam blend",
        "implement_as_searchable_blocks": {
            "residual": "ResNet-style residual MLP/conv on seam or cycle (He et al. style)",
            "dense": "DenseNet-lite feature concat within tiny width budget",
            "unet": "Tiny 1D U-Net encoder-decoder with skip (Wave-U-Net / speech U-Net inspired)",
            "conv1d": "Shallow 1D conv stack on cycle neighborhood of seam",
            "dilated": "Dilated TCN-style 1D conv (Conv-TasNet / TCN inspired, tiny)",
            "attn": "Lightweight multi-head self-attention on seam tokens (tiny dim)",
            "dual_path": "Intra/inter chunk split on 1D cycle (DPRNN-lite)",
            "gated": "Gated residual / soft blend of ops",
            "soft_mix": "Softmax mixture over seam operators",
            "noise_cond": "Noise-level conditioned residual (tiny diffusion/score step proxy)",
            "mlp": "Baseline MLP seam cell",
        },
        "screened_heavy_for_overnight": {
            "full_demucs_htdemucs": "multi-scale / multi-band / large STFT hybrids — too slow per trial",
            "full_diffusion_sampling": "multi-step reverse SDE/ODE sampling — keep only 1 noise-cond residual step",
            "full_gan_train": "SEGAN/MetricGAN discriminator training loops — optional tiny adv loss term only",
            "large_transformers": "Whisper-scale / Conformer-large — dim/head capped for seam bake",
        },
        "outer_loop": "PPO + PBT population over arch graphs/strings; discrete NAS mutations",
        "claim_hygiene": "algorithm names accurate (PPO, PBT, discrete NAS); no false SOTA claims",
    }

    payload = {
        "title": "Audio / signal denoising architectures for DenoiseOpt NAS cells",
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
    print(f"wrote {OUT_REEL} n={len(papers)} used={len(used)} rel={len(screened_rel)} out={len(screened_out)}")
    try:
        OUT_META.parent.mkdir(parents=True, exist_ok=True)
        OUT_META.write_text(json.dumps(payload, indent=2), encoding="utf-8")
        print(f"synced {OUT_META}")
    except OSError as e:
        print(f"meta sync failed: {e}")


if __name__ == "__main__":
    asyncio.run(main())
