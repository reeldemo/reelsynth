#!/usr/bin/env python3
"""
Overnight DenoiseOpt meta: PPO + PBT architecture search on CUDA.

Algorithms (named accurately — not claimed as SOTA):
  - PPO (Schulman et al.): clipped surrogate actor-critic for discrete arch/edit actions
  - PBT-style population (Jaderberg et al.-inspired): exploit elites + mutate arch/hyperparams
  - Discrete NAS over an expanded seam-operator cell space (+ soft op-mixture cell)

Primary score: prolonged residual R in [0,1] (1=best), vs DualCosine baseline.
Dense history.jsonl every iter. Saves unfitted (arch JSON) and fitted (weights+arch).
"""
from __future__ import annotations

import argparse
import json
import math
import os
import random
import sys
import time
from collections import deque
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import torch
import torch.nn as nn
import torch.nn.functional as F

ROOT = Path(__file__).resolve().parents[1]
META_ROOT = ROOT.parent / "denoise-opt-meta"
SEAM_W = 8
MLP_IN = SEAM_W * 2
N = 256
PROLONG = 16

# Expanded discrete NAS op vocabulary (broader than toy REINFORCE run).
OPS = [
    "fade_pull",
    "polish",
    "pin",
    "dual_cosine",
    "classic",
    "soft_seam",
    "fir3",
    "mlp_seam",
    "hann_blend",
    "median3",
    "fir5",
    "skip_blend",
    "edge_pin",
    "asym_wet",
]
CELL_KINDS = ["mlp", "residual", "gated", "bottleneck", "dual_path", "soft_mix"]
ACTS = ["relu", "tanh", "gelu", "silu"]

# PPO action space: mutate depth/width/act/ops/wet/fir/cell/softmix/lr/reset
N_ACTIONS = 10
STATE_DIM = 24
DEFAULT_SEED = 0x5EED_A11C  # new seed — escape prior plateau seed 0x0A172730


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


@dataclass
class ArchConfig:
    depth: int = 2
    width: int = 8
    act: str = "gelu"
    ops: list[str] = field(default_factory=lambda: ["mlp_seam", "dual_cosine", "fir3"])
    wet: float = 0.55
    fir: list[float] = field(default_factory=lambda: [0.25, 0.5, 0.25])
    cell_kind: str = "residual"
    # Soft mixture logits over OPS (used when cell_kind == soft_mix or as bias)
    soft_logits: list[float] = field(default_factory=lambda: [0.0] * len(OPS))

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass
class HyperParams:
    """Per-individual trainable/search hyperparams (PBT genome)."""

    lr: float = 3e-3
    fit_steps: int = 20
    batch: int = 48
    entropy_coef: float = 0.02
    ppo_clip: float = 0.2

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass
class Individual:
    cfg: ArchConfig
    hp: HyperParams
    score: float = -1.0
    age: int = 0


class SeamCell(nn.Module):
    """Searchable seam-window operator network (architecture cell)."""

    def __init__(self, cfg: ArchConfig):
        super().__init__()
        self.cfg = cfg
        h = max(2, min(48, cfg.width))
        d = max(1, min(6, cfg.depth))
        act = cfg.act
        layers: list[nn.Module] = []
        in_d = MLP_IN
        for i in range(d):
            if cfg.cell_kind == "bottleneck" and i == 0 and d > 1:
                out_d = max(2, h // 2)
            elif i < d - 1:
                out_d = h
            else:
                out_d = MLP_IN
            layers.append(nn.Linear(in_d, out_d))
            if i < d - 1:
                layers.append(_act_module(act))
                if cfg.cell_kind == "gated":
                    layers.append(nn.Linear(out_d, out_d))
            in_d = out_d
        self.net = nn.Sequential(*layers)
        # dual_path: second branch
        if cfg.cell_kind == "dual_path":
            self.net_b = nn.Sequential(
                nn.Linear(MLP_IN, h),
                _act_module(act),
                nn.Linear(h, MLP_IN),
            )
            self.mix = nn.Parameter(torch.tensor(0.5))
        else:
            self.net_b = None
            self.mix = None
        self.gate = nn.Parameter(torch.tensor(0.25))
        fir = list(cfg.fir) + [0.1, 0.1]
        self.fir = nn.Parameter(torch.tensor(fir[:5], dtype=torch.float32))
        self.wet = nn.Parameter(torch.tensor(cfg.wet, dtype=torch.float32))
        self.wet_asym = nn.Parameter(torch.tensor([cfg.wet, cfg.wet], dtype=torch.float32))
        # Differentiable soft op mixture (always present; used when soft_mix / skip_blend)
        logits = torch.tensor(cfg.soft_logits[: len(OPS)], dtype=torch.float32)
        if logits.numel() < len(OPS):
            logits = F.pad(logits, (0, len(OPS) - logits.numel()))
        self.soft_logits = nn.Parameter(logits)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        y = self.net(x)
        if self.cfg.cell_kind == "dual_path" and self.net_b is not None and self.mix is not None:
            yb = self.net_b(x)
            m = torch.sigmoid(self.mix)
            y = m * y + (1 - m) * yb
        if self.cfg.cell_kind == "residual":
            y = x + torch.tanh(y) * self.gate
        elif self.cfg.cell_kind == "gated":
            g = torch.sigmoid(self.gate)
            y = g * y + (1 - g) * x
        elif self.cfg.cell_kind == "bottleneck":
            y = x + torch.tanh(y) * torch.sigmoid(self.gate)
        elif self.cfg.cell_kind == "soft_mix":
            # Residual with soft-gated scale from mixture entropy proxy
            w = F.softmax(self.soft_logits, dim=0)
            scale = 0.15 + 0.85 * w.max()
            y = x + torch.tanh(y) * self.gate * scale
        else:
            y = x + torch.tanh(y) * self.gate
        return y


def _act_module(act: str) -> nn.Module:
    if act == "tanh":
        return nn.Tanh()
    if act == "gelu":
        return nn.GELU()
    if act == "silu":
        return nn.SiLU()
    return nn.ReLU()


class ActorCritic(nn.Module):
    """PPO actor-critic over discrete architecture/hyperparam edit actions."""

    def __init__(self, n_actions: int = N_ACTIONS, state_dim: int = STATE_DIM, hidden: int = 128):
        super().__init__()
        self.shared = nn.Sequential(
            nn.Linear(state_dim, hidden),
            nn.Tanh(),
            nn.Linear(hidden, hidden),
            nn.Tanh(),
        )
        self.actor = nn.Linear(hidden, n_actions)
        self.critic = nn.Linear(hidden, 1)

    def forward(self, state: torch.Tensor) -> tuple[torch.Tensor, torch.Tensor]:
        h = self.shared(state)
        return self.actor(h), self.critic(h).squeeze(-1)


def make_batch(batch: int, n: int, device: torch.device) -> tuple[torch.Tensor, torch.Tensor]:
    """Synthetic wrap cycles: ideal continuous vs engine with seam cliff."""
    t = torch.linspace(0, 1, n, device=device).unsqueeze(0).expand(batch, -1)
    freqs = 1.0 + 3.0 * torch.rand(batch, 1, device=device)
    phase = 2 * math.pi * torch.rand(batch, 1, device=device)
    ideal = torch.sin(2 * math.pi * freqs * t + phase)
    ideal = ideal + 0.15 * torch.sin(4 * math.pi * freqs * t + phase * 0.7)
    eng = ideal.clone()
    cliff = (0.08 + 0.35 * torch.rand(batch, 1, device=device)) * (
        1.0 - 2.0 * torch.rand(batch, 1, device=device)
    )
    w = SEAM_W
    for i in range(w):
        a = i / max(w - 1, 1)
        eng[:, i] = eng[:, i] + cliff.squeeze(-1) * (1 - a)
        eng[:, -w + i] = eng[:, -w + i] - cliff.squeeze(-1) * a
    noise = 0.02 * torch.randn(batch, n, device=device)
    noise[:, w:-w] *= 0.15
    eng = eng + noise
    return ideal, eng


def pack_seam(frames: torch.Tensor) -> torch.Tensor:
    w = SEAM_W
    return torch.cat([frames[:, :w], frames[:, -w:]], dim=1)


def write_seam(frames: torch.Tensor, y: torch.Tensor, wet: torch.Tensor) -> torch.Tensor:
    w = SEAM_W
    wet_c = wet.clamp(0.0, 1.0).view(-1, 1)
    head = frames[:, :w] * (1 - wet_c) + y[:, :w] * wet_c
    mid = frames[:, w:-w]
    tail = frames[:, -w:] * (1 - wet_c) + y[:, w:] * wet_c
    return torch.cat([head, mid, tail], dim=1)


def write_seam_asym(frames: torch.Tensor, y: torch.Tensor, wet_asym: torch.Tensor) -> torch.Tensor:
    w = SEAM_W
    wh = wet_asym[0].clamp(0.0, 1.0)
    wt = wet_asym[1].clamp(0.0, 1.0)
    head = frames[:, :w] * (1 - wh) + y[:, :w] * wh
    mid = frames[:, w:-w]
    tail = frames[:, -w:] * (1 - wt) + y[:, w:] * wt
    return torch.cat([head, mid, tail], dim=1)


def apply_fir3(frames: torch.Tensor, fir: torch.Tensor) -> torch.Tensor:
    k = fir[:3] / (fir[:3].abs().sum() + 1e-8)
    left = torch.roll(frames, 1, dims=1)
    right = torch.roll(frames, -1, dims=1)
    filtered = k[0] * left + k[1] * frames + k[2] * right
    w = SEAM_W + 2
    mask = frames.new_zeros(1, frames.shape[1])
    mask[:, :w] = 1.0
    mask[:, -w:] = 1.0
    return frames * (1.0 - mask) + filtered * mask


def apply_fir5(frames: torch.Tensor, fir: torch.Tensor) -> torch.Tensor:
    k = fir[:5] / (fir[:5].abs().sum() + 1e-8)
    acc = k[2] * frames
    for off, coef in ((-2, k[0]), (-1, k[1]), (1, k[3]), (2, k[4])):
        acc = acc + coef * torch.roll(frames, int(off), dims=1)
    w = SEAM_W + 3
    mask = frames.new_zeros(1, frames.shape[1])
    mask[:, :w] = 1.0
    mask[:, -w:] = 1.0
    return frames * (1.0 - mask) + acc * mask


def apply_median3(frames: torch.Tensor) -> torch.Tensor:
    left = torch.roll(frames, 1, dims=1)
    right = torch.roll(frames, -1, dims=1)
    stacked = torch.stack([left, frames, right], dim=-1)
    med, _ = stacked.median(dim=-1)
    w = SEAM_W + 1
    mask = frames.new_zeros(1, frames.shape[1])
    mask[:, :w] = 1.0
    mask[:, -w:] = 1.0
    return frames * (1.0 - mask) + med * mask


def hann_blend(frames: torch.Tensor) -> torch.Tensor:
    w = SEAM_W
    n = frames.shape[1]
    # Raised-cosine / Hann crossfade across seam
    t = torch.linspace(0, math.pi, w, device=frames.device)
    a = 0.5 - 0.5 * torch.cos(t)
    head = frames[:, :w] * (1 - a) + frames[:, n - w :] * a
    tail = frames[:, n - w :] * (1 - a) + frames[:, :w] * a
    return torch.cat([head, frames[:, w : n - w], tail], dim=1)


def dual_cosine_blend(frames: torch.Tensor) -> torch.Tensor:
    w = SEAM_W
    n = frames.shape[1]
    head_parts = []
    tail_parts = []
    for i in range(w):
        a = 0.5 - 0.5 * math.cos(math.pi * i / max(w, 1))
        head_parts.append(((1 - a) * frames[:, i] + a * frames[:, n - w + i]).unsqueeze(1))
        tail_parts.append(((1 - a) * frames[:, n - w + i] + a * frames[:, i]).unsqueeze(1))
    head = torch.cat(head_parts, dim=1)
    tail = torch.cat(tail_parts, dim=1)
    mid = frames[:, w : n - w]
    return torch.cat([head, mid, tail], dim=1)


def apply_ops(frames: torch.Tensor, cell: SeamCell, ops: list[str]) -> torch.Tensor:
    out = frames
    use_soft = cell.cfg.cell_kind == "soft_mix" or "skip_blend" in ops
    if use_soft:
        # Soft mixture: blend dual_cosine / hann / identity by softmax weights
        w = F.softmax(cell.soft_logits, dim=0)
        idx = {name: i for i, name in enumerate(OPS)}
        dc = dual_cosine_blend(out)
        hb = hann_blend(out)
        w_dc = w[idx["dual_cosine"]] + w[idx["classic"]] + w[idx["soft_seam"]]
        w_hb = w[idx["hann_blend"]]
        w_id = (1.0 - w_dc - w_hb).clamp(0.0, 1.0)
        denom = (w_dc + w_hb + w_id).clamp_min(1e-6)
        out = (w_dc * dc + w_hb * hb + w_id * out) / denom
    else:
        if "dual_cosine" in ops or "classic" in ops or "soft_seam" in ops:
            out = dual_cosine_blend(out)
        if "hann_blend" in ops:
            out = hann_blend(out)

    if "median3" in ops:
        out = apply_median3(out)
    if "fir5" in ops:
        out = apply_fir5(out, cell.fir)
    elif "fir3" in ops:
        out = apply_fir3(out, cell.fir)

    mlp_ops = {"mlp_seam", "fade_pull", "polish", "pin", "edge_pin", "asym_wet"}
    if set(ops) & mlp_ops or cell.cfg.cell_kind == "soft_mix":
        x = pack_seam(out)
        y = cell(x)
        if "asym_wet" in ops:
            out = write_seam_asym(out, y, cell.wet_asym)
        else:
            out = write_seam(out, y, cell.wet)
    return out


def prolong_tile(cycle: torch.Tensor, periods: int = PROLONG) -> torch.Tensor:
    return cycle.repeat(1, periods)


def residual_score(ideal: torch.Tensor, out: torch.Tensor) -> torch.Tensor:
    """R = clamp(1 - residual_rms / max(ideal_rms, eps), 0, 1); mean over batch."""
    idp = prolong_tile(ideal)
    otp = prolong_tile(out)
    resid = otp - idp
    residual_rms = resid.pow(2).mean(dim=1).sqrt()
    ideal_rms = idp.pow(2).mean(dim=1).sqrt().clamp_min(1e-6)
    r = (1.0 - residual_rms / ideal_rms).clamp(0.0, 1.0)
    return r


def arch_state_vec(cfg: ArchConfig, hp: HyperParams, device: torch.device) -> torch.Tensor:
    op_bits = [1.0 if o in cfg.ops else 0.0 for o in OPS]  # 14
    act_id = {a: i / max(len(ACTS) - 1, 1) for i, a in enumerate(ACTS)}.get(cfg.act, 0.0)
    cell_id = {c: i / max(len(CELL_KINDS) - 1, 1) for i, c in enumerate(CELL_KINDS)}.get(
        cfg.cell_kind, 0.0
    )
    soft_max = max(cfg.soft_logits) if cfg.soft_logits else 0.0
    extras = [
        cfg.depth / 6.0,
        cfg.width / 48.0,
        act_id,
        cfg.wet,
        cell_id,
        abs(cfg.fir[1]) if cfg.fir else 0.5,
        math.log10(max(hp.lr, 1e-6)) / -2.0,  # ~0.5 at 1e-3
        hp.fit_steps / 64.0,
        hp.entropy_coef,
        soft_max,
    ]
    vec = (op_bits + extras)[:STATE_DIM]
    while len(vec) < STATE_DIM:
        vec.append(0.0)
    return torch.tensor(vec, dtype=torch.float32, device=device)


def ensure_trainable_ops(ops: list[str]) -> list[str]:
    trainable = {"mlp_seam", "fade_pull", "polish", "pin", "fir3", "fir5", "edge_pin", "asym_wet"}
    if not (set(ops) & trainable):
        return list(dict.fromkeys(list(ops) + ["mlp_seam"]))
    return ops


def random_arch(rng: random.Random) -> ArchConfig:
    k = rng.randint(2, min(6, len(OPS)))
    return ArchConfig(
        depth=rng.randint(1, 6),
        width=rng.choice([2, 4, 6, 8, 12, 16, 24, 32, 40, 48]),
        act=rng.choice(ACTS),
        ops=ensure_trainable_ops(rng.sample(OPS, k=k)),
        wet=rng.uniform(0.1, 0.95),
        fir=[rng.uniform(0.05, 0.55) for _ in range(5)],
        cell_kind=rng.choice(CELL_KINDS),
        soft_logits=[rng.uniform(-1.0, 1.0) for _ in range(len(OPS))],
    )


def random_hp(rng: random.Random) -> HyperParams:
    return HyperParams(
        lr=10 ** rng.uniform(-4.0, -2.0),
        fit_steps=rng.choice([12, 16, 20, 24, 32, 40]),
        batch=rng.choice([32, 48, 64]),
        entropy_coef=rng.uniform(0.005, 0.05),
        ppo_clip=rng.choice([0.1, 0.2, 0.3]),
    )


def mutate_arch(cfg: ArchConfig, action: int, rng: random.Random) -> ArchConfig:
    c = ArchConfig(**cfg.to_dict())
    if action == 0:
        c.depth = max(1, min(6, c.depth + rng.choice([-1, 1])))
    elif action == 1:
        c.width = max(2, min(48, c.width + rng.choice([-4, -2, -1, 1, 2, 4])))
    elif action == 2:
        c.act = rng.choice(ACTS)
    elif action == 3:
        op = rng.choice(OPS)
        if op in c.ops and len(c.ops) > 1:
            c.ops = [x for x in c.ops if x != op]
        else:
            c.ops = list(dict.fromkeys(c.ops + [op]))
        c.ops = ensure_trainable_ops(c.ops)
    elif action == 4:
        c.wet = float(max(0.05, min(0.95, c.wet + rng.uniform(-0.25, 0.25))))
    elif action == 5:
        c.fir = [rng.uniform(0.05, 0.6) for _ in range(5)]
    elif action == 6:
        c.cell_kind = rng.choice(CELL_KINDS)
    elif action == 7:
        # Perturb soft mixture logits
        c.soft_logits = [
            float(x + rng.uniform(-0.5, 0.5)) for x in (c.soft_logits or [0.0] * len(OPS))
        ]
        while len(c.soft_logits) < len(OPS):
            c.soft_logits.append(rng.uniform(-0.5, 0.5))
        c.soft_logits = c.soft_logits[: len(OPS)]
    elif action == 8:
        # Widen / deepen jump
        c.depth = rng.randint(1, 6)
        c.width = rng.choice([4, 8, 12, 16, 24, 32, 48])
    else:
        c = random_arch(rng)
    c.ops = ensure_trainable_ops(c.ops)
    return c


def mutate_hp(hp: HyperParams, rng: random.Random) -> HyperParams:
    h = HyperParams(**hp.to_dict())
    h.lr = float(max(1e-5, min(1e-1, h.lr * (10 ** rng.uniform(-0.4, 0.4)))))
    h.fit_steps = int(max(8, min(64, h.fit_steps + rng.choice([-8, -4, 0, 4, 8]))))
    h.entropy_coef = float(max(0.001, min(0.1, h.entropy_coef + rng.uniform(-0.01, 0.01))))
    h.ppo_clip = float(max(0.05, min(0.4, h.ppo_clip + rng.choice([-0.05, 0.0, 0.05]))))
    h.batch = int(rng.choice([32, 48, 64]))
    return h


def pbt_exploit_mutate(pop: list[Individual], rng: random.Random, elite_frac: float = 0.25) -> None:
    """In-place PBT: bottom half copies elite arch+hp then mutates (multi-step)."""
    ranked = sorted(pop, key=lambda ind: ind.score, reverse=True)
    n_elite = max(1, int(len(pop) * elite_frac))
    elites = ranked[:n_elite]
    for i, ind in enumerate(ranked[n_elite:]):
        parent = elites[i % n_elite]
        cfg = ArchConfig(**parent.cfg.to_dict())
        # Stronger exploration after exploit to avoid diversity collapse
        n_mut = rng.randint(2, 4)
        for _ in range(n_mut):
            cfg = mutate_arch(cfg, rng.randrange(N_ACTIONS), rng)
        if rng.random() < 0.25:
            cfg = random_arch(rng)
        ind.cfg = cfg
        ind.hp = mutate_hp(HyperParams(**parent.hp.to_dict()), rng)
        ind.hp = mutate_hp(ind.hp, rng)
        ind.age = 0
        # Keep score as prior until re-evaluated (slight decay)
        ind.score = parent.score * 0.92


def arch_diversity(pop: list[Individual]) -> float:
    """Crude diversity: mean pairwise Hamming over ops + cell/act mismatch."""
    if len(pop) < 2:
        return 0.0
    total = 0.0
    pairs = 0
    for i in range(len(pop)):
        for j in range(i + 1, len(pop)):
            a, b = pop[i].cfg, pop[j].cfg
            ops_a, ops_b = set(a.ops), set(b.ops)
            ham = len(ops_a.symmetric_difference(ops_b)) / max(len(OPS), 1)
            cell_diff = 0.0 if a.cell_kind == b.cell_kind else 1.0
            act_diff = 0.0 if a.act == b.act else 1.0
            wdiff = abs(a.width - b.width) / 48.0
            total += 0.4 * ham + 0.3 * cell_diff + 0.2 * act_diff + 0.1 * wdiff
            pairs += 1
    return total / max(pairs, 1)


def fit_cell(
    cell: SeamCell,
    ops: list[str],
    device: torch.device,
    steps: int = 24,
    batch: int = 32,
    lr: float = 3e-3,
) -> tuple[float, bool]:
    trainable_ops = {
        "mlp_seam",
        "fade_pull",
        "polish",
        "pin",
        "fir3",
        "fir5",
        "edge_pin",
        "asym_wet",
        "skip_blend",
    }
    can_train = bool(set(ops) & trainable_ops) or cell.cfg.cell_kind == "soft_mix"
    opt = torch.optim.Adam(cell.parameters(), lr=lr) if can_train else None
    prev = None
    patience = 0
    last_r = 0.0
    converged = False
    for _ in range(steps):
        ideal, eng = make_batch(batch, N, device)
        out = apply_ops(eng, cell, ops)
        r = residual_score(ideal, out).mean()
        last_r = float(r.detach().item())
        if can_train and opt is not None:
            loss = 1.0 - r
            if loss.requires_grad:
                opt.zero_grad(set_to_none=True)
                loss.backward()
                opt.step()
        if prev is not None:
            rel = abs(prev - last_r) / max(abs(prev), 1e-6)
            if rel < 1e-4:
                patience += 1
                if patience >= 3:
                    converged = True
                    break
            else:
                patience = 0
        prev = last_r
        if not can_train:
            converged = True
            break
    return last_r, converged


@torch.no_grad()
def eval_cell(cell: SeamCell, ops: list[str], device: torch.device, batch: int = 64) -> float:
    ideal, eng = make_batch(batch, N, device)
    out = apply_ops(eng, cell, ops)
    return float(residual_score(ideal, out).mean().item())


@torch.no_grad()
def dual_cosine_baseline(device: torch.device, batch: int = 128) -> float:
    ideal, eng = make_batch(batch, N, device)
    out = dual_cosine_blend(eng)
    return float(residual_score(ideal, out).mean().item())


def save_unfitted(run_dir: Path, cfg: ArchConfig, tag: str, hp: HyperParams | None = None) -> Path:
    d = run_dir / "unfitted"
    d.mkdir(parents=True, exist_ok=True)
    path = d / f"{tag}_arch.json"
    payload: dict[str, Any] = {"architecture": cfg.to_dict(), "tag": tag}
    if hp is not None:
        payload["hyperparams"] = hp.to_dict()
    path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    return path


def save_fitted(
    run_dir: Path,
    cfg: ArchConfig,
    cell: SeamCell,
    policy: ActorCritic | None,
    residual: float,
    tag: str,
    hp: HyperParams | None = None,
) -> Path:
    d = run_dir / "fitted"
    d.mkdir(parents=True, exist_ok=True)
    path = d / f"{tag}_fitted.pt"
    payload = {
        "architecture": cfg.to_dict(),
        "hyperparams": hp.to_dict() if hp else None,
        "residual": residual,
        "cell_state_dict": cell.state_dict(),
        "policy_state_dict": policy.state_dict() if policy is not None else None,
        "algorithms": ["PPO", "PBT", "discrete_NAS", "soft_mix_cell"],
        "tag": tag,
    }
    torch.save(payload, path)
    meta = d / f"{tag}_fitted.json"
    meta.write_text(
        json.dumps(
            {
                "architecture": cfg.to_dict(),
                "hyperparams": hp.to_dict() if hp else None,
                "residual": residual,
                "weights_path": str(path),
                "algorithms": ["PPO", "PBT", "discrete_NAS", "soft_mix_cell"],
                "tag": tag,
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    return path


def log_line(log_path: Path, msg: str) -> None:
    line = f"{datetime.now().isoformat(timespec='seconds')} {msg}"
    print(line, flush=True)
    with log_path.open("a", encoding="utf-8") as f:
        f.write(line + "\n")


def append_history(history_path: Path, row: dict[str, Any]) -> None:
    with history_path.open("a", encoding="utf-8") as f:
        f.write(json.dumps(row, separators=(",", ":")) + "\n")


class RolloutBuffer:
    """On-policy buffer for PPO updates."""

    def __init__(self) -> None:
        self.states: list[torch.Tensor] = []
        self.actions: list[int] = []
        self.logprobs: list[torch.Tensor] = []
        self.rewards: list[float] = []
        self.values: list[torch.Tensor] = []
        self.dones: list[bool] = []

    def clear(self) -> None:
        self.__init__()

    def __len__(self) -> int:
        return len(self.rewards)


def ppo_update(
    policy: ActorCritic,
    opt: torch.optim.Optimizer,
    buf: RolloutBuffer,
    device: torch.device,
    clip_eps: float = 0.2,
    entropy_coef: float = 0.02,
    value_coef: float = 0.5,
    epochs: int = 4,
    gamma: float = 0.99,
    lam: float = 0.95,
) -> dict[str, float]:
    if len(buf) == 0:
        return {"policy_loss": 0.0, "value_loss": 0.0, "entropy": 0.0}

    states = torch.stack(buf.states).to(device)
    actions = torch.tensor(buf.actions, dtype=torch.long, device=device)
    old_logprobs = torch.stack(buf.logprobs).detach().to(device)
    rewards = torch.tensor(buf.rewards, dtype=torch.float32, device=device)
    values = torch.stack(buf.values).detach().to(device)

    # GAE advantages
    advantages = torch.zeros_like(rewards)
    last_gae = 0.0
    next_value = 0.0
    for t in reversed(range(len(rewards))):
        mask = 0.0 if buf.dones[t] else 1.0
        delta = rewards[t] + gamma * next_value * mask - values[t]
        last_gae = delta + gamma * lam * mask * last_gae
        advantages[t] = last_gae
        next_value = values[t]
    returns = advantages + values
    adv = advantages
    adv = (adv - adv.mean()) / (adv.std() + 1e-8)

    total_pi = 0.0
    total_v = 0.0
    total_ent = 0.0
    n_upd = 0
    for _ in range(epochs):
        logits, vals = policy(states)
        dist = torch.distributions.Categorical(logits=logits)
        logprobs = dist.log_prob(actions)
        entropy = dist.entropy().mean()
        ratio = (logprobs - old_logprobs).exp()
        surr1 = ratio * adv
        surr2 = torch.clamp(ratio, 1.0 - clip_eps, 1.0 + clip_eps) * adv
        policy_loss = -torch.min(surr1, surr2).mean()
        value_loss = F.mse_loss(vals, returns)
        loss = policy_loss + value_coef * value_loss - entropy_coef * entropy
        opt.zero_grad(set_to_none=True)
        loss.backward()
        nn.utils.clip_grad_norm_(policy.parameters(), 1.0)
        opt.step()
        total_pi += float(policy_loss.detach().item())
        total_v += float(value_loss.detach().item())
        total_ent += float(entropy.detach().item())
        n_upd += 1

    return {
        "policy_loss": total_pi / max(n_upd, 1),
        "value_loss": total_v / max(n_upd, 1),
        "entropy": total_ent / max(n_upd, 1),
    }


def main() -> int:
    ap = argparse.ArgumentParser(
        description="PPO + PBT overnight seam-arch search (not claimed SOTA)"
    )
    ap.add_argument("--iters", type=int, default=270_000)
    ap.add_argument("--ckpt-every", type=int, default=500)
    ap.add_argument(
        "--history-every",
        type=int,
        default=1,
        help="Append one JSONL history row every N iters (default 1 = every iter).",
    )
    ap.add_argument("--batch", type=int, default=48)
    ap.add_argument("--fit-steps", type=int, default=20)
    ap.add_argument("--device", type=str, default="cuda")
    ap.add_argument("--run-id", type=str, default="")
    ap.add_argument("--max-hours", type=float, default=24.0)
    ap.add_argument("--seed", type=int, default=DEFAULT_SEED)
    ap.add_argument("--pop-size", type=int, default=12)
    ap.add_argument("--ppo-horizon", type=int, default=32)
    ap.add_argument("--pbt-every", type=int, default=50)
    ap.add_argument("--algo-tag", type=str, default="PPO+PBT+NAS")
    args = ap.parse_args()
    if args.history_every < 1:
        print("ERROR: --history-every must be >= 1", file=sys.stderr)
        return 2
    if args.pop_size < 2:
        print("ERROR: --pop-size must be >= 2", file=sys.stderr)
        return 2

    if args.device.startswith("cuda") and not torch.cuda.is_available():
        print("ERROR: CUDA requested but torch.cuda.is_available() is False", file=sys.stderr)
        return 2

    device = torch.device(
        args.device if torch.cuda.is_available() and args.device.startswith("cuda") else "cpu"
    )
    gpu_name = torch.cuda.get_device_name(0) if device.type == "cuda" else "cpu"
    run_id = args.run_id or f"gpu-rl-arch-{utc_now()}"
    run_dir = ROOT / "brand" / "artifacts" / "models" / run_id
    run_dir.mkdir(parents=True, exist_ok=True)
    meta_run = META_ROOT / "artifacts" / "models" / run_id
    meta_run.mkdir(parents=True, exist_ok=True)

    log_path = ROOT / "brand" / "artifacts" / f"overnight_gpu_rl_arch_{run_id}.log"
    ckpt_dir = run_dir / "checkpoints"
    ckpt_dir.mkdir(parents=True, exist_ok=True)

    rng = random.Random(args.seed)
    torch.manual_seed(args.seed)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(args.seed)

    history_path = run_dir / "history.jsonl"
    if not history_path.exists():
        history_path.write_text("", encoding="utf-8")

    baseline = dual_cosine_baseline(device)
    now_local = datetime.now().astimezone()
    algorithms = ["PPO", "PBT", "discrete_NAS", "soft_mix_cell"]
    log_line(
        log_path,
        f"START run_id={run_id} algorithms={algorithms} algo_tag={args.algo_tag} "
        f"device={device} gpu={gpu_name} torch={torch.__version__} "
        f"cuda_available={torch.cuda.is_available()} "
        f"dual_cosine_baseline={baseline:.4f} target_iters={args.iters} "
        f"max_hours={args.max_hours} history_every={args.history_every} "
        f"seed={args.seed} pop_size={args.pop_size} ppo_horizon={args.ppo_horizon} "
        f"pbt_every={args.pbt_every} history_path={history_path} "
        f"local_start={now_local.isoformat(timespec='seconds')} "
        f"note=not_claimed_SOTA",
    )
    (run_dir / "run_meta.json").write_text(
        json.dumps(
            {
                "run_id": run_id,
                "algorithms": algorithms,
                "algo_tag": args.algo_tag,
                "device": str(device),
                "gpu": gpu_name,
                "torch": torch.__version__,
                "cuda_available": torch.cuda.is_available(),
                "dual_cosine_baseline": baseline,
                "target_iters": args.iters,
                "max_hours": args.max_hours,
                "history_every": args.history_every,
                "history_path": str(history_path),
                "seed": args.seed,
                "pop_size": args.pop_size,
                "ppo_horizon": args.ppo_horizon,
                "pbt_every": args.pbt_every,
                "n_ops": len(OPS),
                "ops": OPS,
                "cell_kinds": CELL_KINDS,
                "pid": os.getpid(),
                "started_at": utc_now(),
                "note": "PPO+PBT+expanded NAS — not claimed SOTA",
            },
            indent=2,
        ),
        encoding="utf-8",
    )

    policy = ActorCritic().to(device)
    policy_opt = torch.optim.Adam(policy.parameters(), lr=3e-4)
    buf = RolloutBuffer()

    # PBT population
    pop: list[Individual] = [
        Individual(cfg=random_arch(rng), hp=random_hp(rng), score=-1.0) for _ in range(args.pop_size)
    ]
    # Seed one individual near a strong prior (not the stuck champion copy)
    pop[0].cfg = ArchConfig(
        depth=2,
        width=16,
        act="gelu",
        ops=["mlp_seam", "dual_cosine", "fir3", "hann_blend"],
        wet=0.45,
        fir=[0.2, 0.5, 0.2, 0.05, 0.05],
        cell_kind="soft_mix",
        soft_logits=[rng.uniform(-0.3, 0.3) for _ in range(len(OPS))],
    )
    save_unfitted(run_dir, pop[0].cfg, "init", pop[0].hp)
    save_unfitted(meta_run, pop[0].cfg, "init", pop[0].hp)

    champion_r = -1.0
    champion_cfg = pop[0].cfg
    champion_hp = pop[0].hp
    champion_cell: SeamCell | None = None
    iters_since_improve = 0
    branch_best = {"ppo": 0.0, "nas": 0.0, "pbt": 0.0, "combo": 0.0}
    last_ppo_stats = {"policy_loss": 0.0, "value_loss": 0.0, "entropy": 0.0}
    recent_residuals: deque[float] = deque(maxlen=100)

    t0 = time.time()
    max_sec = args.max_hours * 3600.0
    keepalive = torch.zeros(1, device=device)
    old_plateau = 0.9779  # prior stuck champ for logging context only

    for it in range(1, args.iters + 1):
        if time.time() - t0 > max_sec:
            log_line(log_path, f"STOP time budget reached at iter={it}")
            break

        # Rotate branches: PPO policy mutate | random NAS | PBT member | combo
        branch = ("ppo", "nas", "pbt", "combo")[it % 4]
        ind_idx = (it - 1) % len(pop)
        ind = pop[ind_idx]
        cfg = ind.cfg
        hp = ind.hp

        state = arch_state_vec(cfg, hp, device).unsqueeze(0)
        logits, value = policy(state)
        dist = torch.distributions.Categorical(logits=logits)
        action_t = dist.sample()
        action = int(action_t.item())
        logprob = dist.log_prob(action_t)
        entropy_now = float(dist.entropy().item())

        if branch == "nas":
            trial_cfg = random_arch(rng)
            trial_hp = mutate_hp(hp, rng)
        elif branch == "pbt":
            # Evaluate / lightly mutate current population member
            trial_cfg = mutate_arch(cfg, action, rng) if rng.random() < 0.5 else cfg
            trial_hp = hp
        elif branch == "combo":
            trial_cfg = mutate_arch(mutate_arch(cfg, action, rng), rng.randrange(N_ACTIONS), rng)
            trial_hp = mutate_hp(hp, rng)
        else:
            trial_cfg = mutate_arch(cfg, action, rng)
            trial_hp = hp

        if it == 1 or it % args.ckpt_every == 1:
            save_unfitted(run_dir, trial_cfg, f"iter_{it:06d}", trial_hp)

        cell = SeamCell(trial_cfg).to(device)
        fit_steps = trial_hp.fit_steps or args.fit_steps
        batch = trial_hp.batch or args.batch
        r_fit, converged = fit_cell(
            cell,
            trial_cfg.ops,
            device,
            steps=fit_steps,
            batch=batch,
            lr=trial_hp.lr,
        )
        r_eval = eval_cell(cell, trial_cfg.ops, device, batch=max(64, batch))
        residual = 0.5 * r_fit + 0.5 * r_eval
        branch_best[branch] = max(branch_best[branch], residual)
        recent_residuals.append(residual)

        # Reward vs DualCosine baseline (advantage signal for PPO)
        reward = residual - baseline
        buf.states.append(state.squeeze(0).detach())
        buf.actions.append(action)
        buf.logprobs.append(logprob.detach())
        buf.rewards.append(reward)
        buf.values.append(value.squeeze().detach())
        buf.dones.append(False)

        if len(buf) >= args.ppo_horizon:
            last_ppo_stats = ppo_update(
                policy,
                policy_opt,
                buf,
                device,
                clip_eps=trial_hp.ppo_clip,
                entropy_coef=trial_hp.entropy_coef,
            )
            buf.clear()

        # Update population member if this trial is competitive
        if residual >= ind.score:
            ind.cfg = trial_cfg
            ind.hp = trial_hp
            ind.score = residual
        ind.age += 1

        if it % args.pbt_every == 0:
            before_div = arch_diversity(pop)
            pbt_exploit_mutate(pop, rng)
            after_div = arch_diversity(pop)
            log_line(
                log_path,
                f"PBT_STEP iter={it} diversity_before={before_div:.4f} "
                f"diversity_after={after_div:.4f} elite_score={max(p.score for p in pop):.4f}",
            )

        pop_div = arch_diversity(pop)
        champ_now = residual if residual > champion_r else (champion_r if champion_r >= 0 else residual)

        if it == 1 or (it % args.history_every == 0):
            tag = f"iter_{it:06d}"
            append_history(
                history_path,
                {
                    "iter": it,
                    "t_sec": round(time.time() - t0, 6),
                    "residual": residual,
                    "champ": champ_now,
                    "iters_since_improve": iters_since_improve,
                    "branch": branch,
                    "branch_best_ppo": branch_best["ppo"],
                    "branch_best_nas": branch_best["nas"],
                    "branch_best_pbt": branch_best["pbt"],
                    "branch_best_combo": branch_best["combo"],
                    "policy_loss": last_ppo_stats["policy_loss"],
                    "value_loss": last_ppo_stats["value_loss"],
                    "entropy": last_ppo_stats["entropy"] if last_ppo_stats["entropy"] else entropy_now,
                    "action_entropy": entropy_now,
                    "pop_diversity": pop_div,
                    "pop_size": len(pop),
                    "ind_idx": ind_idx,
                    "algorithms": algorithms,
                    "arch_id": tag,
                    "tag": tag,
                    "converged": converged,
                    "vs_old_plateau": residual - old_plateau,
                },
            )

        if residual > champion_r:
            champion_r = residual
            champion_cfg = trial_cfg
            champion_hp = trial_hp
            champion_cell = cell
            iters_since_improve = 0
            # Climb: inject champion into a random pop slot
            victim = rng.randrange(len(pop))
            pop[victim].cfg = ArchConfig(**trial_cfg.to_dict())
            pop[victim].hp = HyperParams(**trial_hp.to_dict())
            pop[victim].score = residual
            save_fitted(
                run_dir, trial_cfg, cell, policy, residual, f"champion_iter_{it:06d}", trial_hp
            )
            save_fitted(
                meta_run, trial_cfg, cell, policy, residual, f"champion_iter_{it:06d}", trial_hp
            )
            log_line(
                log_path,
                f"NEW_CHAMPION iter={it} residual={residual:.4f} "
                f"delta_vs_dual={residual - baseline:+.4f} "
                f"vs_old_plateau={residual - old_plateau:+.4f} "
                f"iters_since_improve=0 algorithms={algorithms} "
                f"arch={trial_cfg.to_dict()} hp={trial_hp.to_dict()}",
            )
        else:
            iters_since_improve += 1

        def write_latest(iter_n: int, *, checkpoint: bool) -> None:
            ckpt = {
                "iter": iter_n,
                "champion_residual": champion_r,
                "champion_arch": champion_cfg.to_dict(),
                "champion_hp": champion_hp.to_dict(),
                "baseline_dual_cosine": baseline,
                "branch_best": branch_best,
                "iters_since_improve": iters_since_improve,
                "pop_diversity": pop_div,
                "entropy": last_ppo_stats.get("entropy", entropy_now),
                "algorithms": algorithms,
                "elapsed_sec": time.time() - t0,
                "gpu": gpu_name,
                "pid": os.getpid(),
                "seed": args.seed,
                "history_path": str(history_path),
            }
            if checkpoint:
                ckpt_path = ckpt_dir / f"ckpt_iter_{iter_n:06d}.json"
                ckpt_path.write_text(json.dumps(ckpt, indent=2), encoding="utf-8")
                if champion_cell is not None:
                    save_fitted(
                        run_dir,
                        champion_cfg,
                        champion_cell,
                        policy,
                        champion_r,
                        f"ckpt_iter_{iter_n:06d}",
                        champion_hp,
                    )
                log_line(log_path, f"CHECKPOINT iter={iter_n} wrote {ckpt_path}")
            summary = {
                **ckpt,
                "log_path": str(log_path),
                "run_dir": str(run_dir),
                "unfitted_dir": str(run_dir / "unfitted"),
                "fitted_dir": str(run_dir / "fitted"),
            }
            (ROOT / "brand" / "artifacts" / "overnight_gpu_rl_arch_latest.json").write_text(
                json.dumps(summary, indent=2), encoding="utf-8"
            )
            try:
                (META_ROOT / "artifacts" / "overnight_gpu_rl_arch_latest.json").write_text(
                    json.dumps(summary, indent=2), encoding="utf-8"
                )
            except OSError:
                pass

        if it % 25 == 0 or it == 1:
            mem = torch.cuda.memory_allocated(device) / (1024**2) if device.type == "cuda" else 0.0
            elapsed = time.time() - t0
            rate = it / max(elapsed, 1e-6)
            log_line(
                log_path,
                f"progress {it}/{args.iters} branch={branch} residual={residual:.4f} "
                f"champ={champion_r:.4f} baseline={baseline:.4f} "
                f"iters_since_improve={iters_since_improve} "
                f"entropy={entropy_now:.4f} pop_div={pop_div:.4f} "
                f"converged={converged} gpu_mem_mb={mem:.1f} "
                f"iters_per_sec={rate:.2f} elapsed_h={elapsed/3600:.3f} "
                f"algo={args.algo_tag}",
            )
            keepalive = keepalive + 0.0
            write_latest(it, checkpoint=False)

        if it % args.ckpt_every == 0:
            write_latest(it, checkpoint=True)

    if champion_cell is not None:
        save_fitted(
            run_dir, champion_cfg, champion_cell, policy, champion_r, "final_champion", champion_hp
        )
        save_fitted(
            meta_run, champion_cfg, champion_cell, policy, champion_r, "final_champion", champion_hp
        )
    final = {
        "run_id": run_id,
        "iters_done": it if args.iters else 0,
        "champion_residual": champion_r,
        "dual_cosine_baseline": baseline,
        "delta": champion_r - baseline,
        "iters_since_improve": iters_since_improve,
        "algorithms": algorithms,
        "gpu": gpu_name,
        "elapsed_sec": time.time() - t0,
        "run_dir": str(run_dir),
        "history_path": str(history_path),
        "log_path": str(log_path),
        "seed": args.seed,
    }
    (run_dir / "final_summary.json").write_text(json.dumps(final, indent=2), encoding="utf-8")
    log_line(log_path, f"DONE {json.dumps(final)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
