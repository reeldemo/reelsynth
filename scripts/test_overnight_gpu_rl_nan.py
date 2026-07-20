#!/usr/bin/env python3
"""Smoke: NaN logits / rewards must not crash Categorical or ppo_update."""
from __future__ import annotations

import sys
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from overnight_gpu_rl_arch import (  # noqa: E402
    ActorCritic,
    RolloutBuffer,
    categorical_from_logits,
    params_finite,
    ppo_update,
    sanitize_logits,
    snapshot_state_dict,
)


def test_categorical_nan_logits_no_crash() -> None:
    bad = torch.full((4, 16), float("nan"))
    safe = sanitize_logits(bad)
    assert torch.isfinite(safe).all()
    dist = categorical_from_logits(bad)
    a = dist.sample()
    assert a.shape == (4,)
    _ = dist.log_prob(a)
    _ = dist.entropy()


def test_actor_critic_forward_nan_state() -> None:
    pol = ActorCritic()
    state = torch.full((2, 36), float("nan"))
    logits, value = pol(state)
    assert torch.isfinite(logits).all()
    assert torch.isfinite(value).all()
    dist = categorical_from_logits(logits)
    _ = dist.sample()


def test_ppo_update_nan_rewards_skips_not_crash() -> None:
    device = torch.device("cpu")
    pol = ActorCritic().to(device)
    opt = torch.optim.Adam(pol.parameters(), lr=3e-4)
    good = snapshot_state_dict(pol)
    buf = RolloutBuffer()
    for i in range(8):
        buf.states.append(torch.randn(36))
        buf.actions.append(i % 16)
        buf.logprobs.append(torch.tensor(0.0))
        buf.rewards.append(float("nan") if i % 2 == 0 else 0.1)
        buf.values.append(torch.tensor(0.0))
        buf.dones.append(False)
    # Corrupt actor weights to force NaN logits path through sanitize + restore.
    with torch.no_grad():
        pol.actor.weight.fill_(float("nan"))
    assert not params_finite(pol)
    stats = ppo_update(pol, opt, buf, device, last_good=good)
    assert "nan_skipped" in stats
    assert params_finite(pol)
    # Fresh finite policy + NaN rewards must still complete without raising.
    pol2 = ActorCritic().to(device)
    opt2 = torch.optim.Adam(pol2.parameters(), lr=3e-4)
    stats2 = ppo_update(pol2, opt2, buf, device, last_good=snapshot_state_dict(pol2))
    assert math_isfinite_stats(stats2)


def math_isfinite_stats(stats: dict[str, float]) -> bool:
    return all(isinstance(v, float) and (v == v) for v in stats.values())  # NaN != NaN


if __name__ == "__main__":
    test_categorical_nan_logits_no_crash()
    test_actor_critic_forward_nan_state()
    test_ppo_update_nan_rewards_skips_not_crash()
    print("OK: NaN logits / PPO smoke passed")
