"""Deterministic targeted eval cases (pre-registered in PREREGISTRATION.md)."""
from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Callable

import torch

SEAM_W = 8
N_DEFAULT = 256


@dataclass(frozen=True)
class CaseSpec:
    case_id: str
    family: str
    params: dict
    seed_offset: int
    jump_indices: tuple[int, ...]  # per-cycle jump centers (includes wrap 0 and L-1 when applicable)


def _base_sine(batch: int, n: int, device: torch.device, g: torch.Generator) -> torch.Tensor:
    t = torch.linspace(0, 1, n, device=device).unsqueeze(0).expand(batch, -1)
    freqs = 1.0 + 3.0 * torch.rand(batch, 1, device=device, generator=g)
    phase = 2 * math.pi * torch.rand(batch, 1, device=device, generator=g)
    ideal = torch.sin(2 * math.pi * freqs * t + phase)
    ideal = ideal + 0.15 * torch.sin(4 * math.pi * freqs * t + phase * 0.7)
    return ideal


def _apply_cliff(eng: torch.Tensor, amp: float, sign: float = 1.0) -> torch.Tensor:
    w = SEAM_W
    cliff = sign * amp
    for i in range(w):
        a = i / max(w - 1, 1)
        eng[:, i] = eng[:, i] + cliff * (1 - a)
        eng[:, -w + i] = eng[:, -w + i] - cliff * a
    return eng


def _add_noise(eng: torch.Tensor, sigma: float, g: torch.Generator) -> torch.Tensor:
    w = SEAM_W
    noise = sigma * torch.randn(eng.shape, device=eng.device, generator=g)
    noise[:, w:-w] *= 0.15
    return eng + noise


def _internal_jumps(ideal: torch.Tensor, count: int) -> tuple[torch.Tensor, tuple[int, ...]]:
    n = ideal.shape[1]
    if count <= 0:
        return ideal, (0, n - 1)
    # Deterministic grid excluding seam neighborhoods
    margin = SEAM_W + 4
    positions = [
        int(round(margin + (n - 2 * margin) * k / max(count, 1)))
        for k in range(count)
    ]
    eng = ideal.clone()
    for p in positions:
        eng[:, p:] = eng[:, p:] + 0.15
    jumps = (0, n - 1) + tuple(positions)
    return eng, jumps


def make_case_batch(
    spec: CaseSpec,
    batch: int,
    n: int,
    device: torch.device,
    holdout_eval_seed: int,
) -> tuple[torch.Tensor, torch.Tensor, torch.Tensor]:
    """Return ideal, engine, jump_indices [B, J]."""
    g = torch.Generator(device=device)
    g.manual_seed(int(holdout_eval_seed) + int(spec.seed_offset))
    ideal = _base_sine(batch, n, device, g)
    p = spec.params
    family = spec.family

    if family == "wrap_cliff_height":
        eng = ideal.clone()
        eng = _apply_cliff(eng, float(p["amplitude"]), sign=float(p.get("sign", 1.0)))
        eng = _add_noise(eng, 0.02, g)
        jumps = (0, n - 1)
    elif family == "phase_jump":
        eng = ideal.clone()
        rad = float(p["radians"])
        eng[:, 0] = eng[:, 0] + rad
        eng[:, -1] = eng[:, -1] - rad
        eng = _add_noise(eng, 0.02, g)
        jumps = (0, n - 1)
    elif family == "period_drift":
        t = torch.linspace(0, 1, n, device=device).unsqueeze(0).expand(batch, -1)
        drift = float(p["drift_frac"])
        freqs = 1.0 + 3.0 * torch.rand(batch, 1, device=device, generator=g)
        phase = 2 * math.pi * torch.rand(batch, 1, device=device, generator=g)
        ideal_d = torch.sin(2 * math.pi * freqs * t * (1.0 + drift * t) + phase)
        eng = ideal_d.clone()
        eng = _apply_cliff(eng, 0.25, sign=1.0)
        eng = _add_noise(eng, 0.02, g)
        ideal = ideal_d
        jumps = (0, n - 1)
    elif family == "discontinuity_density":
        eng, jumps = _internal_jumps(ideal, int(p["count"]))
        eng = _add_noise(eng, 0.02, g)
    elif family == "snr_sweep":
        eng = ideal.clone()
        eng = _apply_cliff(eng, 0.25, sign=1.0)
        eng = _add_noise(eng, float(p["sigma"]), g)
        jumps = (0, n - 1)
    elif family == "control_smooth":
        eng = _add_noise(ideal.clone(), float(p.get("sigma", 0.005)), g)
        jumps = (0, n - 1)
    else:
        raise ValueError(f"unknown family {family}")

    j_idx = torch.tensor(list(jumps), device=device, dtype=torch.long)
    j_idx = j_idx.unsqueeze(0).expand(batch, -1)
    return ideal, eng, j_idx


def all_cases() -> list[CaseSpec]:
    cases: list[CaseSpec] = []
    for i, amp in enumerate((0.08, 0.20, 0.35, 0.50)):
        cases.append(
            CaseSpec(
                f"cliff_a{int(amp*100):02d}",
                "wrap_cliff_height",
                {"amplitude": amp},
                1000 + i,
                (0, N_DEFAULT - 1),
            )
        )
    for i, mult in enumerate((0.2, 0.5, 1.0, 2.0)):
        cases.append(
            CaseSpec(
                f"phase_j{int(mult*10):02d}",
                "phase_jump",
                {"radians": mult * 0.35},
                2000 + i,
                (0, N_DEFAULT - 1),
            )
        )
    for i, d in enumerate((0.02, 0.05, 0.10)):
        cases.append(
            CaseSpec(
                f"drift_m{int(d*100):02d}",
                "period_drift",
                {"drift_frac": d},
                3000 + i,
                (0, N_DEFAULT - 1),
            )
        )
    for i, c in enumerate((1, 3, 6, 12)):
        cases.append(
            CaseSpec(
                f"density_{c}",
                "discontinuity_density",
                {"count": c},
                4000 + i,
                tuple(),
            )
        )
    for i, sig in enumerate((0.01, 0.02, 0.05, 0.10)):
        cases.append(
            CaseSpec(
                f"snr_n{int(sig*100):02d}",
                "snr_sweep",
                {"sigma": sig},
                5000 + i,
                (0, N_DEFAULT - 1),
            )
        )
    cases.append(
        CaseSpec("control_smooth", "control_smooth", {"sigma": 0.005}, 9000, (0, N_DEFAULT - 1))
    )
    return cases


def smoke_cases() -> list[CaseSpec]:
    """One case per family for CI."""
    full = {c.family: c for c in all_cases()}
    order = [
        "wrap_cliff_height",
        "phase_jump",
        "period_drift",
        "discontinuity_density",
        "snr_sweep",
        "control_smooth",
    ]
    return [full[f] for f in order]
