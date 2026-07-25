#!/usr/bin/env python3
"""Smoke: v10 R_seam + J + n2n_unet block."""
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
    # Whole-curve still exists for debug.
    r_whole = og.residual_score(ideal, cliff)
    assert torch.isfinite(r_whole).all()


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
    # Composed graph can build n2n_unet
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
    r_s = float(og.residual_score_seam(ideal, eng).mean().item())
    r_w = float(og.residual_score(ideal, eng).mean().item())
    assert 0.0 <= r_s <= 1.0 and 0.0 <= r_w <= 1.0
    assert math.isfinite(r_s)


if __name__ == "__main__":
    test_residual_score_seam_geometry()
    test_objective_j_latency()
    test_n2n_unet_block_parity()
    test_overnight_wrappers()
    print("OK: v10 R_seam / J / n2n_unet smoke passed")
