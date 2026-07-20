#!/usr/bin/env python3
"""Tiled SNR / SDR helpers for DenoiseOpt seam-domain evaluation (Phase 3a).

Honest signal metrics vs the procedural ideal sibling. Not PESQ/STOI.
"""
from __future__ import annotations

import torch


def prolong_tile(cycle: torch.Tensor, periods: int) -> torch.Tensor:
    return cycle.repeat(1, periods)


@torch.no_grad()
def tiled_snr_db(
    ideal: torch.Tensor, out: torch.Tensor, *, periods: int = 16, eps: float = 1e-12
) -> torch.Tensor:
    """Per-sample SNR (dB) of prolonged out vs prolonged ideal."""
    idp = prolong_tile(ideal, periods)
    otp = prolong_tile(torch.nan_to_num(out, nan=0.0, posinf=0.0, neginf=0.0), periods)
    noise = otp - idp
    sig_pow = idp.pow(2).mean(dim=1).clamp_min(eps)
    noi_pow = noise.pow(2).mean(dim=1).clamp_min(eps)
    return 10.0 * torch.log10(sig_pow / noi_pow)


@torch.no_grad()
def tiled_sdr_db(
    ideal: torch.Tensor, out: torch.Tensor, *, periods: int = 16, eps: float = 1e-12
) -> torch.Tensor:
    """Scale-invariant-ish SDR (dB): project out onto ideal in prolonged space.

    Uses the BSS_Eval-style projection onto the reference:
    s_target = <otp, idp> / ||idp||^2 * idp, e_noise = otp - s_target.
    """
    idp = prolong_tile(ideal, periods)
    otp = prolong_tile(torch.nan_to_num(out, nan=0.0, posinf=0.0, neginf=0.0), periods)
    num = (otp * idp).sum(dim=1, keepdim=True)
    den = idp.pow(2).sum(dim=1, keepdim=True).clamp_min(eps)
    s_target = (num / den) * idp
    e_noise = otp - s_target
    sig_pow = s_target.pow(2).mean(dim=1).clamp_min(eps)
    noi_pow = e_noise.pow(2).mean(dim=1).clamp_min(eps)
    return 10.0 * torch.log10(sig_pow / noi_pow)


@torch.no_grad()
def wrap_jump_abs(cycle: torch.Tensor) -> torch.Tensor:
    """Absolute endpoint discontinuity |x0 - x_{L-1}| per sample."""
    return (cycle[:, 0] - cycle[:, -1]).abs()


@torch.no_grad()
def secondary_metrics(
    ideal: torch.Tensor, out: torch.Tensor, *, periods: int = 16
) -> dict[str, float]:
    """Mean SNR/SDR (dB) and mean wrap-jump on baked cycles."""
    snr = tiled_snr_db(ideal, out, periods=periods)
    sdr = tiled_sdr_db(ideal, out, periods=periods)
    jump = wrap_jump_abs(out)
    return {
        "snr_db_mean": float(snr.mean().item()),
        "snr_db_std": float(snr.std(unbiased=False).item()),
        "sdr_db_mean": float(sdr.mean().item()),
        "sdr_db_std": float(sdr.std(unbiased=False).item()),
        "wrap_jump_mean": float(jump.mean().item()),
        "wrap_jump_std": float(jump.std(unbiased=False).item()),
    }
