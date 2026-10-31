#!/usr/bin/env python3
"""Unit tests for configurable FitCell plateau criteria."""
from __future__ import annotations

import sys
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
import overnight_gpu_rl_arch as og  # noqa: E402


def test_resolve_fit_max_steps_prefers_fit_max_steps() -> None:
    hp = og.HyperParams(fit_steps=24, fit_max_steps=1024)
    assert og.resolve_fit_max_steps(hp, 48) == 1024


def test_fit_cell_stops_on_patience() -> None:
    device = torch.device("cpu")
    cfg = og.random_arch(__import__("random").Random(0))
    cell = og.SeamCell(cfg).to(device)
    _, conv_fast, steps_fast = og.fit_cell(
        cell,
        cfg.ops,
        device,
        steps=32,
        batch=8,
        patience=1,
        rel_eps=1e-2,
    )
    cell2 = og.SeamCell(cfg).to(device)
    _, conv_slow, steps_slow = og.fit_cell(
        cell2,
        cfg.ops,
        device,
        steps=32,
        batch=8,
        patience=50,
        rel_eps=1e-8,
    )
    assert steps_fast <= steps_slow
    assert conv_fast or steps_fast < 32


def test_hyperparams_plateau_roundtrip() -> None:
    hp = og.random_hp(__import__("random").Random(1), search_space="plateau")
    assert hp.fit_max_steps in og.PLATEAU_FIT_MAX_STEPS
    assert hp.batch in og.PLATEAU_BATCH
    restored = og.HyperParams.from_dict(hp.to_dict())
    assert restored.fit_max_steps == hp.fit_max_steps


def main() -> int:
    test_resolve_fit_max_steps_prefers_fit_max_steps()
    test_fit_cell_stops_on_patience()
    test_hyperparams_plateau_roundtrip()
    print("OK test_fit_converge_criteria")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
