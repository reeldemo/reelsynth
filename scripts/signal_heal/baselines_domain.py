#!/usr/bin/env python3
"""Classical + domain baselines for signal-heal transfer pilot.

Honesty labels:
  - classical_shared: DualCosine / Hann / linear fade / no-bake / endpoint pin / FIR
  - bearings_classical_cot: bad-COT linear periodize sibling (construction artifact control)
  - ecg_classical_sbmm: beat-average / spline join (NOT Cycle-GAN / BeatDiff)
"""
from __future__ import annotations

import math
from typing import Callable

import torch
import torch.nn.functional as F

import overnight_gpu_rl_arch as og
from baselines.endpoint_pin import endpoint_pin


@torch.no_grad()
def linear_fade(frames: torch.Tensor, seam_w: int | None = None) -> torch.Tensor:
    """Linear crossfade of head/tail over seam window."""
    w = int(seam_w or og.SEAM_W)
    n = frames.shape[1]
    out = frames.clone()
    for i in range(w):
        a = i / max(w, 1)
        out[:, i] = (1 - a) * frames[:, i] + a * frames[:, n - w + i]
        out[:, n - w + i] = (1 - a) * frames[:, n - w + i] + a * frames[:, i]
    return out


@torch.no_grad()
def soft_periodize(frames: torch.Tensor, seam_w: int | None = None) -> torch.Tensor:
    """Hann window soft periodize (classical DSP)."""
    return og.hann_blend(frames)


@torch.no_grad()
def seam_fir3(frames: torch.Tensor) -> torch.Tensor:
    k = torch.tensor([0.25, 0.5, 0.25], device=frames.device, dtype=frames.dtype).view(1, 1, 3)
    y = F.conv1d(frames.unsqueeze(1), k, padding=1).squeeze(1)
    w = og.SEAM_W
    out = frames.clone()
    out[:, :w] = y[:, :w]
    out[:, -w:] = y[:, -w:]
    return out


@torch.no_grad()
def spline_join(frames: torch.Tensor, seam_w: int | None = None) -> torch.Tensor:
    """Cubic-ish local join via two FIR passes + endpoint pin (ECG classical proxy)."""
    out = seam_fir3(frames)
    out = seam_fir3(out)
    return endpoint_pin(out, seam_w=seam_w or og.SEAM_W, mode="mean")


@torch.no_grad()
def beat_average_project(frames: torch.Tensor, template: torch.Tensor | None = None) -> torch.Tensor:
    """Project each cracked beat toward batch-mean template (SBMM-lite classical)."""
    if template is None:
        template = frames.mean(dim=0, keepdim=True)
    # Blend mid strongly toward template, keep some of cracked edges then DualCosine
    mid = 0.65 * template.expand_as(frames) + 0.35 * frames
    return og.dual_cosine_blend(mid)


def shared_classical_board() -> dict[str, Callable[[torch.Tensor], torch.Tensor]]:
    return {
        "no_bake": lambda x: x,
        "dual_cosine": og.dual_cosine_blend,
        "soft_periodize_hann": soft_periodize,
        "linear_fade": linear_fade,
        "endpoint_pin_mean": lambda x: endpoint_pin(x, seam_w=og.SEAM_W, mode="mean"),
        "seam_fir3": seam_fir3,
    }


def domain_baselines(domain: str) -> dict[str, Callable[[torch.Tensor], torch.Tensor]]:
    board = shared_classical_board()
    if domain == "bearings":
        # Label: classical COT-style board; no published deep COT network weights used.
        board["cot_linear_periodize"] = lambda x: x  # engine already bad-COT; score as-is control
        board["cot_cubic_then_dualcosine"] = og.dual_cosine_blend
        return board
    if domain == "ecg":
        board["spline_join"] = spline_join
        board["beat_average_sbmm_lite"] = beat_average_project
        return board
    if domain == "cnc":
        # Corner-rounding classical proxies (not Beudaert analytic G2 solver).
        board["spline_join"] = spline_join
        return board
    if domain == "power":
        # Cycle splice classical proxies (not trained PQ denoisers).
        board["spline_join"] = spline_join
        return board
    return board


BASELINE_LABELS = {
    "no_bake": "classical / passthrough",
    "dual_cosine": "classical DualCosine fade",
    "soft_periodize_hann": "classical Hann soft-periodize",
    "linear_fade": "classical linear fade",
    "endpoint_pin_mean": "classical endpoint pin",
    "seam_fir3": "classical seam FIR3",
    "cot_linear_periodize": "bearings classical bad-COT control (passthrough of linear resample)",
    "cot_cubic_then_dualcosine": "bearings classical: DualCosine on cracked (not published deep COT)",
    "spline_join": "domain classical spline/FIR join (not Cycle-GAN / deep SOTA)",
    "beat_average_sbmm_lite": "ECG classical SBMM-lite beat average (not BeatDiff/Cycle-GAN)",
    "ours_hybrid_lstm": "Ours (hybrid GA–PPO / hybrid_lstm outer loop)",
}
