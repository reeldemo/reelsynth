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
def edge_rmse(
    ideal: torch.Tensor, out: torch.Tensor, *, seam_w: int = 8, eps: float = 1e-12
) -> torch.Tensor:
    """RMS of (out - ideal) on seam indices [0:W] U [L-W:L] (per sample)."""
    w = int(seam_w)
    n = out.shape[1]
    err = torch.nan_to_num(out - ideal, nan=0.0, posinf=0.0, neginf=0.0)
    edge = torch.cat([err[:, :w], err[:, n - w :]], dim=1)
    return edge.pow(2).mean(dim=1).sqrt().clamp_min(eps)


# v10.1 blend: seam heal vs ideal + mid-cycle identity vs engine.
BLEND_ALPHA = 0.7


def _seam_windows_tiled(
    tiled: torch.Tensor, *, n_cycle: int, periods: int, seam_w: int
) -> torch.Tensor:
    """Gather wrap neighborhoods after tiling: each period's [0:W] ∪ [L-W:L]."""
    w = int(seam_w)
    chunks: list[torch.Tensor] = []
    for k in range(int(periods)):
        base = k * int(n_cycle)
        chunks.append(tiled[:, base : base + w])
        chunks.append(tiled[:, base + n_cycle - w : base + n_cycle])
    return torch.cat(chunks, dim=1)


def _body_windows_tiled(
    tiled: torch.Tensor, *, n_cycle: int, periods: int, seam_w: int
) -> torch.Tensor:
    """Gather mid-cycle body (outside seam windows) after tiling.

    Body mask = indices outside SEAM_W at each period head/tail:
    ``[W : L-W]`` per tile. Encourages healers not to morph the curve body.
    """
    w = int(seam_w)
    n = int(n_cycle)
    if n <= 2 * w:
        # Degenerate: keep a single mid sample so the tensor stays non-empty.
        mid = n // 2
        chunks = [tiled[:, k * n + mid : k * n + mid + 1] for k in range(int(periods))]
        return torch.cat(chunks, dim=1)
    chunks: list[torch.Tensor] = []
    for k in range(int(periods)):
        base = k * n
        chunks.append(tiled[:, base + w : base + n - w])
    return torch.cat(chunks, dim=1)


def _unit_residual_r(
    ref: torch.Tensor, out: torch.Tensor, *, eps: float = 1e-6
) -> torch.Tensor:
    """Unit-interval R = clamp(1 - rms(out-ref) / rms(ref), 0, 1). Differentiable."""
    otp = torch.nan_to_num(out, nan=0.0, posinf=0.0, neginf=0.0)
    resid = otp - ref
    residual_rms = resid.pow(2).mean(dim=1).sqrt()
    ref_rms = ref.pow(2).mean(dim=1).sqrt().clamp_min(eps)
    r = (1.0 - residual_rms / ref_rms).clamp(0.0, 1.0)
    return torch.nan_to_num(r, nan=0.0, posinf=0.0, neginf=0.0).clamp(0.0, 1.0)


def residual_score_seam(
    ideal: torch.Tensor,
    out: torch.Tensor,
    *,
    periods: int = 16,
    seam_w: int = 8,
    eps: float = 1e-6,
) -> torch.Tensor:
    """Discontinuity-local R on prolonged wrap neighborhoods (seam vs ideal).

    Differentiable (used in fit). Same unit form as whole-curve R, RMS only on
    SEAM_W samples at each tiled period head/tail.
    """
    n = int(ideal.shape[1])
    p = max(1, int(periods))
    w = max(1, min(int(seam_w), n // 2))
    idp = prolong_tile(ideal, p)
    otp = prolong_tile(out, p)
    id_s = _seam_windows_tiled(idp, n_cycle=n, periods=p, seam_w=w)
    ot_s = _seam_windows_tiled(otp, n_cycle=n, periods=p, seam_w=w)
    return _unit_residual_r(id_s, ot_s, eps=eps)


def residual_score_body(
    ref: torch.Tensor,
    out: torch.Tensor,
    *,
    periods: int = 16,
    seam_w: int = 8,
    eps: float = 1e-6,
) -> torch.Tensor:
    """Mid-cycle body R on non-seam samples after tiling.

    Prefer ``ref=eng`` (engine input) so high R_body means “don’t change the
    curve” / identity on the body. ``ref=ideal`` is also valid for diagnostics.
    """
    n = int(ref.shape[1])
    p = max(1, int(periods))
    w = max(1, min(int(seam_w), n // 2))
    ref_p = prolong_tile(ref, p)
    otp = prolong_tile(out, p)
    ref_b = _body_windows_tiled(ref_p, n_cycle=n, periods=p, seam_w=w)
    ot_b = _body_windows_tiled(otp, n_cycle=n, periods=p, seam_w=w)
    return _unit_residual_r(ref_b, ot_b, eps=eps)


def residual_score_blend(
    ideal: torch.Tensor,
    eng: torch.Tensor,
    out: torch.Tensor,
    *,
    alpha: float = BLEND_ALPHA,
    periods: int = 16,
    seam_w: int = 8,
    eps: float = 1e-6,
) -> torch.Tensor:
    """Primary v10.1 quality: R_blend = α·R_seam(ideal,out) + (1-α)·R_body(eng,out).

    - R_seam: heal discontinuity toward the ideal sibling at wrap neighborhoods.
    - R_body: keep mid-cycle close to the **engine input** (identity on body) so
      the healer does not wholesale-morph the waveform.
    Default α=0.7 (seam-weighted, body still strong). Differentiable for fit.
    """
    a = float(alpha)
    r_seam = residual_score_seam(ideal, out, periods=periods, seam_w=seam_w, eps=eps)
    r_body = residual_score_body(eng, out, periods=periods, seam_w=seam_w, eps=eps)
    return (a * r_seam + (1.0 - a) * r_body).clamp(0.0, 1.0)


@torch.no_grad()
def click_energy(cycle: torch.Tensor, *, periods: int = 2) -> torch.Tensor:
    """Mean square of first difference across tiled wrap boundaries.

    For cycle length L, tiled to P periods, jumps live at indices kL-1 → kL
    for k=1..P-1. Reports mean((y[kL] - y[kL-1])^2) per sample.
    """
    p = max(int(periods), 2)
    tiled = prolong_tile(cycle, p)
    n = cycle.shape[1]
    jumps = []
    for k in range(1, p):
        left = tiled[:, k * n - 1]
        right = tiled[:, k * n]
        jumps.append((right - left).pow(2))
    stacked = torch.stack(jumps, dim=1)
    return stacked.mean(dim=1)


@torch.no_grad()
def secondary_metrics(
    ideal: torch.Tensor,
    out: torch.Tensor,
    *,
    periods: int = 16,
    seam_w: int = 8,
    eng: torch.Tensor | None = None,
    alpha: float = BLEND_ALPHA,
) -> dict[str, float]:
    """Mean SNR/SDR (dB), wrap-jump, seam/body/blend residuals."""
    snr = tiled_snr_db(ideal, out, periods=periods)
    sdr = tiled_sdr_db(ideal, out, periods=periods)
    jump = wrap_jump_abs(out)
    ermse = edge_rmse(ideal, out, seam_w=seam_w)
    click = click_energy(out, periods=max(2, min(periods, 4)))
    r_seam = residual_score_seam(ideal, out, periods=periods, seam_w=seam_w)
    out_d: dict[str, float] = {
        "snr_db_mean": float(snr.mean().item()),
        "snr_db_std": float(snr.std(unbiased=False).item()),
        "sdr_db_mean": float(sdr.mean().item()),
        "sdr_db_std": float(sdr.std(unbiased=False).item()),
        "wrap_jump_mean": float(jump.mean().item()),
        "wrap_jump_std": float(jump.std(unbiased=False).item()),
        "edge_rmse_mean": float(ermse.mean().item()),
        "edge_rmse_std": float(ermse.std(unbiased=False).item()),
        "click_energy_mean": float(click.mean().item()),
        "click_energy_std": float(click.std(unbiased=False).item()),
        "r_seam_mean": float(r_seam.mean().item()),
        "r_seam_std": float(r_seam.std(unbiased=False).item()),
    }
    if eng is not None:
        r_body = residual_score_body(eng, out, periods=periods, seam_w=seam_w)
        r_blend = residual_score_blend(
            ideal, eng, out, alpha=alpha, periods=periods, seam_w=seam_w
        )
        out_d["r_body_mean"] = float(r_body.mean().item())
        out_d["r_body_std"] = float(r_body.std(unbiased=False).item())
        out_d["r_blend_mean"] = float(r_blend.mean().item())
        out_d["r_blend_std"] = float(r_blend.std(unbiased=False).item())
        out_d["blend_alpha"] = float(alpha)
    return out_d
