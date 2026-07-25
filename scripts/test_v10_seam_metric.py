#!/usr/bin/env python3
"""Smoke: v10.1 R_blend + J + n2n_unet block."""
from __future__ import annotations

import math
import sys
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import denoise_arch_blocks as dab  # noqa: E402
import metrics_snr_sdr as msm  # noqa: E402
import overnight_gpu_rl_arch as og  # noqa: E402
from baselines.n2n_seam import SeamN2N, n_params as n2n_n_params  # noqa: E402


def test_residual_score_seam_geometry() -> None:
    device = torch.device("cpu")
    b, n = 4, 256
    ideal = torch.randn(b, n, device=device)
    out = ideal.clone()
    r_perf = msm.residual_score_seam(ideal, out, periods=16, seam_w=8)
    assert torch.allclose(r_perf, torch.ones(b), atol=1e-5), r_perf

    cliff = ideal.clone()
    cliff[:, 0] = -3.0
    cliff[:, -1] = 3.0
    mid = ideal.clone()
    mid[:, n // 2] = 3.0
    r_cliff = msm.residual_score_seam(ideal, cliff, periods=16, seam_w=8)
    r_mid = msm.residual_score_seam(ideal, mid, periods=16, seam_w=8)
    assert (r_cliff < r_mid).all(), (r_cliff, r_mid)
    r_whole = og.residual_score(ideal, cliff)
    assert torch.isfinite(r_whole).all()


def test_residual_score_blend_body_identity() -> None:
    """Body vs engine: identity on mid-cycle should score R_body≈1; morphing body hurts."""
    device = torch.device("cpu")
    b, n, w = 4, 256, 8
    ideal = torch.linspace(-0.5, 0.5, n, device=device).unsqueeze(0).expand(b, -1).clone()
    eng = ideal.clone()
    eng[:, :w] += 0.5
    eng[:, -w:] -= 0.5
    # Heal seams toward ideal but keep body = eng → high R_body, improved R_seam
    healed = eng.clone()
    healed[:, :w] = ideal[:, :w]
    healed[:, -w:] = ideal[:, -w:]
    r_seam_h = msm.residual_score_seam(ideal, healed, periods=16, seam_w=w)
    r_body_h = msm.residual_score_body(eng, healed, periods=16, seam_w=w)
    r_blend_h = msm.residual_score_blend(ideal, eng, healed, alpha=0.7, periods=16, seam_w=w)
    assert (r_body_h > 0.99).all(), r_body_h
    # Morph mid-cycle away from engine → R_body drops
    morph = healed.clone()
    morph[:, n // 2 - 10 : n // 2 + 10] += 1.5
    r_body_m = msm.residual_score_body(eng, morph, periods=16, seam_w=w)
    r_blend_m = msm.residual_score_blend(ideal, eng, morph, alpha=0.7, periods=16, seam_w=w)
    assert (r_body_m < r_body_h).all(), (r_body_m, r_body_h)
    assert (r_blend_m < r_blend_h).all(), (r_blend_m, r_blend_h)
    # Blend formula check
    expected = 0.7 * r_seam_h + 0.3 * r_body_h
    assert torch.allclose(r_blend_h, expected, atol=1e-5)
    assert abs(msm.BLEND_ALPHA - 0.7) < 1e-12
    assert abs(og.BLEND_ALPHA - 0.7) < 1e-12


def test_blend_differentiable_for_fit() -> None:
    eng = torch.randn(2, 256, requires_grad=False)
    ideal = eng.detach() + 0.01
    out = (eng * 0.5).detach().requires_grad_(True)
    r = msm.residual_score_blend(ideal, eng, out, alpha=0.7)
    loss = (1.0 - r).mean()
    loss.backward()
    assert out.grad is not None and torch.isfinite(out.grad).all()


def test_objective_j_latency() -> None:
    j_fast = og.objective_j(0.99, 1.0)
    j_slow = og.objective_j(0.99, 50.0)
    assert j_fast > j_slow
    assert abs(og.latency_norm(50.0) - 1.0) < 1e-9
    assert abs(j_slow - (0.99 - 0.02 * 1.0)) < 1e-9


def test_n2n_unet_block_parity() -> None:
    assert "n2n_unet" in dab.BLOCKS
    assert "n2n_unet" in dab.CELL_KINDS
    net = dab.SeamN2NParity(256)
    ref = SeamN2N()
    n_net = sum(p.numel() for p in net.parameters())
    n_ref = n2n_n_params(ref)
    assert n_net == n_ref, (n_net, n_ref)
    x = torch.randn(2, 256)
    y = net(x)
    assert y.shape == x.shape
    assert torch.isfinite(y).all()
    composed = dab.ComposedSeamNet(256, 16, 3, "gelu", "n2n_unet", ["n2n_unet"])
    z = composed(x)
    assert z.shape == x.shape


def test_overnight_wrappers() -> None:
    device = torch.device("cpu")
    ideal = torch.zeros(2, 256)
    ideal[:, :] = torch.linspace(-0.5, 0.5, 256)
    eng = ideal.clone()
    eng[:, :8] += 0.4
    eng[:, -8:] -= 0.4
    r_b = float(og.residual_score_blend(ideal, eng, eng).mean().item())
    r_s = float(og.residual_score_seam(ideal, eng).mean().item())
    r_w = float(og.residual_score(ideal, eng).mean().item())
    assert 0.0 <= r_b <= 1.0 and 0.0 <= r_s <= 1.0 and 0.0 <= r_w <= 1.0
    assert math.isfinite(r_b)
    # No-bake: perfect body identity → blend = 0.7*R_seam + 0.3*1
    assert abs(r_b - (0.7 * r_s + 0.3)) < 1e-4


if __name__ == "__main__":
    test_residual_score_seam_geometry()
    test_residual_score_blend_body_identity()
    test_blend_differentiable_for_fit()
    test_objective_j_latency()
    test_n2n_unet_block_parity()
    test_overnight_wrappers()
    print("OK: v10.1 R_blend / J / n2n_unet smoke passed")
