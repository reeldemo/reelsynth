#!/usr/bin/env python3
"""Smoke: jump-local metric responds at discontinuities."""
from __future__ import annotations

import sys
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import metrics_snr_sdr as msm  # noqa: E402
from phase2_experiments.case_generators import make_case_batch, smoke_cases  # noqa: E402


def test_jump_local_r() -> None:
    device = torch.device("cpu")
    spec = smoke_cases()[0]
    ideal, eng, jump_idx = make_case_batch(spec, 4, 256, device, 20260719)
    r_eng = msm.jump_local_r(ideal, eng, jump_idx, half_w=8).mean().item()
    r_id = msm.jump_local_r(ideal, ideal, jump_idx, half_w=8).mean().item()
    assert r_id > r_eng, (r_id, r_eng)


if __name__ == "__main__":
    test_jump_local_r()
    print("OK: jump-local metric smoke")
