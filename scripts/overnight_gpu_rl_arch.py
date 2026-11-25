#!/usr/bin/env python3
"""
Overnight DenoiseOpt meta: PPO + GA + PBT architecture search on CUDA.

Algorithms (named accurately — not claimed as SOTA):
  - PPO (Schulman et al.): clipped surrogate actor-critic for discrete arch/edit actions
  - GA (Holland / Real aging-evo spirit): tournament + crossover + mutate on arch graphs
  - ERL-inspired interleave (Khadka & Tumer spirit): GA generations between PPO rollouts
  - PBT-style population (Jaderberg et al.-inspired): exploit elites + mutate arch/hyperparams
  - Discrete NAS over lit-inspired composable seam/cycle cells (U-Net, dilated, attn, MoE, …)
  - Depth bias: deeper graphs rewarded when residual holds above DualCosine + margin
  - MoE soft gates over heterogeneous parallel experts (Shazeer-inspired, tiny)

Primary score (v10.1): R_blend = α·R_seam(ideal,out) + (1-α)·R_body(eng,out), α=0.7.
  - R_seam: discontinuity-local residual vs ideal on SEAM_W wrap neighborhoods.
  - R_body: mid-cycle residual vs **engine** (identity on body — don't morph the curve).
Search objective: J = R_blend - λ·latency_norm (λ=0.02, latency_norm=log(1+t_ms)/log(1+50)).
Pure R_seam / whole-curve residual_score retained as debug JSON keys.
PPO advantage is centered as (score - DualCosine) for zero-mean early credit assignment; selection uses J.
Dense history.jsonl every iter. Saves unfitted (arch JSON) and fitted (weights+arch).

Arch complexity / depth / mixtures preferred over raw it/s; paper target remains 1M if rate allows,
else honest retarget documented in run_meta.json.
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

# Local module alongside this script
sys.path.insert(0, str(Path(__file__).resolve().parent))
from denoise_arch_blocks import (  # noqa: E402
    BLOCKS,
    CELL_KINDS,
    MAX_GRAPH_LEN,
    MAX_SEARCH_DEPTH,
    MAX_WIDTH,
    MOE_MODES,
    ComposedSeamNet,
    TinyAdvHead,
    get_search_caps,
    normalize_graph,
    raise_search_caps,
    random_block_graph,
    random_moe_mode,
)
import denoise_arch_blocks as arch_blocks  # noqa: E402  # live mutable caps
from denoise_meta_evo import (  # noqa: E402
    depth_mixture_bonus,
    ga_generation,
)
import metrics_snr_sdr as msm  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
META_ROOT = ROOT.parent / "denoise-opt-meta"

# Optional extra-graph restriction (None = full BLOCKS vocabulary); set in main().
GRAPH_BLOCKS_ALLOWED: list[str] | None = None


def _graph_allowed_blocks() -> list[str] | None:
    return GRAPH_BLOCKS_ALLOWED


def _graph_normalize(blocks, cell_kind: str) -> list[str]:
    return normalize_graph(blocks, cell_kind, _graph_allowed_blocks())


def _block_mutation_pool() -> list[str]:
    if GRAPH_BLOCKS_ALLOWED is not None:
        return list(GRAPH_BLOCKS_ALLOWED)
    return list(BLOCKS)


def _parse_csv_flag(raw: str) -> tuple[str, ...]:
    return tuple(x.strip() for x in str(raw).split(",") if x.strip())


def configure_search_space_flags(cell_kinds: str, extra_blocks: str) -> int:
    """Apply --cell-kinds / --extra-blocks; return 2 on invalid names."""
    global CELL_KINDS, GRAPH_BLOCKS_ALLOWED
    ck = _parse_csv_flag(cell_kinds)
    eb = _parse_csv_flag(extra_blocks)
    for label, names in (("cell-kinds", ck), ("extra-blocks", eb)):
        bad = [n for n in names if n not in BLOCKS]
        if bad:
            print(
                f"ERROR: unknown --{label} {bad}; each name must be in BLOCKS",
                file=sys.stderr,
            )
            return 2
    cell_allowed = list(ck) if ck else list(BLOCKS)
    CELL_KINDS = cell_allowed
    arch_blocks.CELL_KINDS = cell_allowed
    GRAPH_BLOCKS_ALLOWED = list(eb) if eb else None
    return 0
SEAM_W = 8
MLP_IN = SEAM_W * 2
N = 256
PROLONG = 16
LAMBDA_LATENCY = 0.02
LATENCY_REF_MS = 50.0
BLEND_ALPHA = msm.BLEND_ALPHA  # 0.7 — seam-weighted; body still strong
LATENCY_WARMUP = 1
LATENCY_REPEATS = 3

# Expanded discrete NAS op vocabulary (seam operators + learnable nets).
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
    "cycle_net",  # apply composed net on full cycle (seam-weighted)
]
ACTS = ["relu", "tanh", "gelu", "silu"]

# PPO action space expanded for block-graph / depth / MoE edits
# 0 depth, 1 width, 2 act, 3 ops, 4 wet, 5 fir, 6 cell, 7 softmix,
# 8 widen+deepen jump, 9 reset, 10 toggle block, 11 mutate graph,
# 12 adv aux, 13 deepen bias, 14 toggle moe_parallel, 15 diversify mix
N_ACTIONS = 16
STATE_DIM = 36
DEFAULT_SEED = 1_902_771_841  # RL+GA+depth+MoE restart (Int32-safe)
ALGORITHMS = [
    "PPO",
    "GA_crossover_mutate",
    "ERL_inspired_interleave",
    "PBT",
    "discrete_NAS",
    "depth_bias",
    "MoE_softgate_parallel",
    "composed_arch_graph",
    "lit_blocks_unet_dilated_attn_dualpath",
    "plateau_adapt_deepen",
]


@dataclass
class PlateauAdaptState:
    """Escalating boredom response when champ residual stalls.

    Depth is first-class: raise MAX_SEARCH_DEPTH, deepen pop genomes, deepen U-Net/MLP stacks.
    Soft boredom resets on each fire; champ + iters_since_improve keep accumulating so adapt
    re-fires every N flat iters with rising aggression (capped for RTX 3090).
    """

    level: int = 0
    soft_boredom: int = 0
    last_adapt_iter: int = 0
    crazy_mix_p: float = 0.35
    moe_p: float = 0.35
    hold_p: float = 0.50  # PBT/PPO hold probability (lower = crazier explore)
    nas_boost: float = 0.0
    deepen_bump: int = 2

    def mix_tag(self) -> str:
        return (
            f"crazy={self.crazy_mix_p:.2f}/moe={self.moe_p:.2f}/"
            f"hold={self.hold_p:.2f}/nas+={self.nas_boost:.2f}"
        )


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


@dataclass
class ArchConfig:
    depth: int = 3
    width: int = 8
    act: str = "gelu"
    ops: list[str] = field(default_factory=lambda: ["mlp_seam", "dual_cosine", "fir3"])
    wet: float = 0.55
    fir: list[float] = field(default_factory=lambda: [0.25, 0.5, 0.25])
    cell_kind: str = "residual"
    soft_logits: list[float] = field(default_factory=lambda: [0.0] * len(OPS))
    # Composable lit-inspired block graph (max MAX_GRAPH_LEN)
    blocks: list[str] = field(default_factory=lambda: ["residual"])
    # Optional tiny adversarial auxiliary (generator-side only; not full GAN)
    use_adv_aux: bool = False
    # sequential | moe_parallel (MoE soft gates over heterogeneous experts)
    moe_mode: str = "sequential"

    def to_dict(self) -> dict[str, Any]:
        d = asdict(self)
        d["blocks"] = normalize_graph(self.blocks, self.cell_kind)
        if d.get("moe_mode") not in MOE_MODES:
            d["moe_mode"] = "sequential"
        return d

    @classmethod
    def from_dict(cls, d: dict[str, Any]) -> "ArchConfig":
        known = {f.name for f in cls.__dataclass_fields__.values()}  # type: ignore[attr-defined]
        kwargs = {k: v for k, v in d.items() if k in known}
        cfg = cls(**kwargs)
        cfg.blocks = normalize_graph(list(cfg.blocks), cfg.cell_kind)
        if cfg.moe_mode not in MOE_MODES:
            cfg.moe_mode = "sequential"
        if len(cfg.soft_logits) < len(OPS):
            cfg.soft_logits = list(cfg.soft_logits) + [0.0] * (len(OPS) - len(cfg.soft_logits))
        elif len(cfg.soft_logits) > len(OPS):
            cfg.soft_logits = list(cfg.soft_logits[: len(OPS)])
        return cfg


@dataclass
class HyperParams:
    """Per-individual trainable/search hyperparams (PBT genome)."""

    lr: float = 3e-3
    fit_steps: int = 24
    batch: int = 48
    entropy_coef: float = 0.02
    ppo_clip: float = 0.2
    adv_coef: float = 0.05  # weight for optional adv aux
    # Reward shaping for outer RL credit (objective remains maximize absolute R).
    # abs_r | vs_dualcosine | vs_nobake | neglog_gap
    reward_mode: str = "vs_dualcosine"

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)

    @classmethod
    def from_dict(cls, d: dict[str, Any] | None) -> "HyperParams":
        if not d:
            return cls()
        known = {f.name for f in cls.__dataclass_fields__.values()}  # type: ignore[attr-defined]
        return cls(**{k: v for k, v in d.items() if k in known})


REWARD_MODES = ("abs_r", "vs_dualcosine", "vs_nobake", "neglog_gap")


def shaped_reward(
    r: float,
    *,
    mode: str,
    r_dualcosine: float,
    r_nobake: float,
) -> float:
    """Outer-loop credit signal. Larger when closer to best R (ideal sibling).

    Near-ceiling no-bake (~0.97) makes raw gaps tiny; modes re-scale credit for PPO/REINFORCE.
    Selection / reporting still use absolute R.
    """
    m = (mode or "vs_dualcosine").strip().lower()
    if m == "abs_r":
        return float(r)
    if m == "vs_nobake":
        return float(r - r_nobake)
    if m == "neglog_gap":
        return float(-math.log(max(1e-6, 1.0 - float(r))))
    # default: vs_dualcosine
    return float(r - r_dualcosine)


@dataclass
class Individual:
    cfg: ArchConfig
    hp: HyperParams
    score: float = -1.0
    age: int = 0


class SeamCell(nn.Module):
    """Searchable seam/cycle operator network (composed lit-inspired blocks)."""

    def __init__(self, cfg: ArchConfig):
        super().__init__()
        self.cfg = cfg
        graph = normalize_graph(cfg.blocks, cfg.cell_kind)
        self.cfg.blocks = graph
        if self.cfg.moe_mode not in MOE_MODES:
            self.cfg.moe_mode = "sequential"
        h = max(2, min(arch_blocks.MAX_WIDTH, cfg.width))
        d = max(1, min(arch_blocks.MAX_SEARCH_DEPTH, cfg.depth))
        self.seam_net = ComposedSeamNet(
            MLP_IN, h, d, cfg.act, cfg.cell_kind, graph, moe_mode=cfg.moe_mode
        )
        self.cycle_net = ComposedSeamNet(
            N,
            max(4, h // 2),
            max(1, d - 1),
            cfg.act,
            cfg.cell_kind,
            graph,
            moe_mode=cfg.moe_mode,
        )
        self.gate = nn.Parameter(torch.tensor(0.25))
        fir = list(cfg.fir) + [0.1, 0.1]
        self.fir = nn.Parameter(torch.tensor(fir[:5], dtype=torch.float32))
        self.wet = nn.Parameter(torch.tensor(cfg.wet, dtype=torch.float32))
        self.wet_asym = nn.Parameter(torch.tensor([cfg.wet, cfg.wet], dtype=torch.float32))
        logits = torch.tensor(cfg.soft_logits[: len(OPS)], dtype=torch.float32)
        if logits.numel() < len(OPS):
            logits = F.pad(logits, (0, len(OPS) - logits.numel()))
        self.soft_logits = nn.Parameter(logits)
        self.adv_head: TinyAdvHead | None
        if cfg.use_adv_aux:
            self.adv_head = TinyAdvHead(N, max(8, h // 2))
        else:
            self.adv_head = None

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.seam_net(x)

    def forward_cycle(self, frames: torch.Tensor) -> torch.Tensor:
        y = self.cycle_net(frames)
        g = torch.sigmoid(self.gate)
        # Prefer edits near seams
        w = SEAM_W + 4
        mask = frames.new_zeros(1, frames.shape[1])
        mask[:, :w] = 1.0
        mask[:, -w:] = 1.0
        return frames * (1 - mask * g) + y * (mask * g)


def finite_scalar(x: float, default: float = 0.0) -> float:
    try:
        v = float(x)
    except (TypeError, ValueError):
        return default
    return v if math.isfinite(v) else default


def sanitize_logits(logits: torch.Tensor, clamp: float = 20.0) -> torch.Tensor:
    """Replace non-finite logits and clamp so Categorical validate_args never trips."""
    return torch.nan_to_num(logits, nan=0.0, posinf=clamp, neginf=-clamp).clamp(-clamp, clamp)


def categorical_from_logits(logits: torch.Tensor) -> torch.distributions.Categorical:
    return torch.distributions.Categorical(logits=sanitize_logits(logits))


def params_finite(module: nn.Module) -> bool:
    return all(torch.isfinite(p).all().item() for p in module.parameters())


def snapshot_state_dict(module: nn.Module) -> dict[str, torch.Tensor]:
    return {k: v.detach().cpu().clone() for k, v in module.state_dict().items()}


def load_state_dict_compatible(module: nn.Module, state: dict[str, Any]) -> tuple[int, int]:
    """Load matching shapes only (architecture may deepen after plateau / code updates)."""
    model_sd = module.state_dict()
    filtered: dict[str, Any] = {}
    skipped = 0
    for k, v in state.items():
        if k in model_sd and hasattr(v, "shape") and model_sd[k].shape == v.shape:
            filtered[k] = v
        else:
            skipped += 1
    module.load_state_dict(filtered, strict=False)
    return len(filtered), skipped


def restore_state_dict(module: nn.Module, state: dict[str, torch.Tensor], device: torch.device) -> None:
    module.load_state_dict({k: v.to(device) for k, v in state.items()})


class ActorCritic(nn.Module):
    """PPO actor-critic over discrete architecture/hyperparam edit actions."""

    def __init__(self, n_actions: int = N_ACTIONS, state_dim: int = STATE_DIM, hidden: int = 160):
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
        state = torch.nan_to_num(state, nan=0.0, posinf=0.0, neginf=0.0)
        h = self.shared(state)
        logits = sanitize_logits(self.actor(h))
        value = torch.nan_to_num(self.critic(h).squeeze(-1), nan=0.0, posinf=0.0, neginf=0.0)
        return logits, value


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
    use_soft = cell.cfg.cell_kind == "soft_mix" or "skip_blend" in ops or "soft_mix" in cell.cfg.blocks
    if use_soft:
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

    net_ops = {
        "mlp_seam",
        "fade_pull",
        "polish",
        "pin",
        "edge_pin",
        "asym_wet",
        "cycle_net",
    }
    complex_cell = cell.cfg.cell_kind not in ("mlp",) or len(cell.cfg.blocks) > 1
    if set(ops) & net_ops or cell.cfg.cell_kind == "soft_mix" or complex_cell:
        if "cycle_net" in ops or cell.cfg.moe_mode == "moe_parallel" or any(
            b in cell.cfg.blocks
            for b in (
                "unet",
                "conv1d",
                "dilated",
                "attn",
                "dual_path",
                "lstm",
                "xlstm",
                "tf_split",
                "noise_cond",
                "moe_mix",
            )
        ):
            out = cell.forward_cycle(out)
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
    """Whole-curve prolonged R (debug / legacy). Prefer residual_score_blend for selection."""
    idp = prolong_tile(ideal)
    otp = prolong_tile(torch.nan_to_num(out, nan=0.0, posinf=0.0, neginf=0.0))
    resid = otp - idp
    residual_rms = resid.pow(2).mean(dim=1).sqrt()
    ideal_rms = idp.pow(2).mean(dim=1).sqrt().clamp_min(1e-6)
    r = (1.0 - residual_rms / ideal_rms).clamp(0.0, 1.0)
    return torch.nan_to_num(r, nan=0.0, posinf=0.0, neginf=0.0).clamp(0.0, 1.0)


def residual_score_seam(
    ideal: torch.Tensor,
    out: torch.Tensor,
    *,
    periods: int = PROLONG,
    seam_w: int = SEAM_W,
) -> torch.Tensor:
    """Debug/component: discontinuity-local R on tiled wrap neighborhoods (vs ideal)."""
    return msm.residual_score_seam(ideal, out, periods=periods, seam_w=seam_w)


def residual_score_body(
    ref: torch.Tensor,
    out: torch.Tensor,
    *,
    periods: int = PROLONG,
    seam_w: int = SEAM_W,
) -> torch.Tensor:
    """Debug/component: mid-cycle body R (prefer ref=engine for identity-on-body)."""
    return msm.residual_score_body(ref, out, periods=periods, seam_w=seam_w)


def residual_score_blend(
    ideal: torch.Tensor,
    eng: torch.Tensor,
    out: torch.Tensor,
    *,
    alpha: float = BLEND_ALPHA,
    periods: int = PROLONG,
    seam_w: int = SEAM_W,
) -> torch.Tensor:
    """Primary v10.1: R_blend = α·R_seam(ideal,out) + (1-α)·R_body(eng,out)."""
    return msm.residual_score_blend(
        ideal, eng, out, alpha=alpha, periods=periods, seam_w=seam_w
    )


def latency_norm(t_ms: float, ref_ms: float = LATENCY_REF_MS) -> float:
    return math.log1p(max(0.0, float(t_ms))) / math.log1p(max(ref_ms, 1e-6))


def objective_j(
    r_blend: float,
    t_ms: float,
    *,
    lam: float = LAMBDA_LATENCY,
    ref_ms: float = LATENCY_REF_MS,
) -> float:
    """Multi-obj search score: J = R_blend − λ · latency_norm."""
    return float(r_blend) - float(lam) * latency_norm(t_ms, ref_ms=ref_ms)


@torch.no_grad()
def measure_forward_latency_ms(
    cell: "SeamCell",
    ops: list[str],
    device: torch.device,
    *,
    batch: int = 32,
    warmup: int = LATENCY_WARMUP,
    repeats: int = LATENCY_REPEATS,
) -> float:
    """CPU/GPU wall time for forward + R_blend (same pattern as bench_signal_heal_latency)."""
    ideal, eng = make_batch(batch, N, device)
    for _ in range(max(0, warmup)):
        out = apply_ops(eng, cell, ops)
        _ = residual_score_blend(ideal, eng, out).mean()
    if device.type == "cuda":
        torch.cuda.synchronize()
    t0 = time.perf_counter()
    for _ in range(max(1, repeats)):
        out = apply_ops(eng, cell, ops)
        _ = residual_score_blend(ideal, eng, out).mean()
    if device.type == "cuda":
        torch.cuda.synchronize()
    return 1000.0 * (time.perf_counter() - t0) / max(repeats, 1)


def arch_state_vec(cfg: ArchConfig, hp: HyperParams, device: torch.device) -> torch.Tensor:
    op_bits = [1.0 if o in cfg.ops else 0.0 for o in OPS]
    act_id = {a: i / max(len(ACTS) - 1, 1) for i, a in enumerate(ACTS)}.get(cfg.act, 0.0)
    cell_id = {c: i / max(len(BLOCKS) - 1, 1) for i, c in enumerate(BLOCKS)}.get(
        cfg.cell_kind, 0.0
    )
    finite_soft = [x for x in (cfg.soft_logits or []) if math.isfinite(float(x))]
    soft_max = max(finite_soft) if finite_soft else 0.0
    block_bits = [1.0 if b in cfg.blocks else 0.0 for b in BLOCKS]
    lr_safe = finite_scalar(hp.lr, 1e-3)
    extras = [
        finite_scalar(cfg.depth / float(arch_blocks.MAX_SEARCH_DEPTH)),
        finite_scalar(cfg.width / float(arch_blocks.MAX_WIDTH)),
        finite_scalar(act_id),
        finite_scalar(cfg.wet),
        finite_scalar(cell_id),
        finite_scalar(abs(cfg.fir[1]) if cfg.fir else 0.5, 0.5),
        finite_scalar(math.log10(max(lr_safe, 1e-6)) / -2.0),
        finite_scalar(hp.fit_steps / 64.0),
        finite_scalar(hp.entropy_coef),
        finite_scalar(soft_max),
        1.0 if cfg.use_adv_aux else 0.0,
        finite_scalar(len(cfg.blocks) / float(arch_blocks.MAX_GRAPH_LEN)),
        1.0 if cfg.moe_mode == "moe_parallel" else 0.0,
        float(len(set(cfg.blocks) & {"unet", "attn", "lstm", "xlstm", "dilated", "dual_path"})) / 6.0,
    ]
    vec = (op_bits + block_bits[:10] + extras)[:STATE_DIM]
    while len(vec) < STATE_DIM:
        vec.append(0.0)
    return torch.tensor(vec, dtype=torch.float32, device=device)


def ensure_trainable_ops(ops: list[str]) -> list[str]:
    trainable = {
        "mlp_seam",
        "fade_pull",
        "polish",
        "pin",
        "fir3",
        "fir5",
        "edge_pin",
        "asym_wet",
        "cycle_net",
    }
    if not (set(ops) & trainable):
        return list(dict.fromkeys(list(ops) + ["mlp_seam"]))
    return ops


def random_arch(rng: random.Random, adapt: PlateauAdaptState | None = None) -> ArchConfig:
    k = rng.randint(2, min(7, len(OPS)))
    cell = rng.choice(CELL_KINDS)
    # Depth-biased prior: skew toward deeper nets (still capped); plateau adapt raises ceiling
    max_d = arch_blocks.MAX_SEARCH_DEPTH
    depth = int(round(rng.betavariate(2.5, 1.4) * (max_d - 1))) + 1
    if adapt is not None and adapt.level > 0:
        # Prefer deeper half of the searchable range after plateau fires
        depth = max(depth, rng.randint(max(3, max_d // 2), max_d))
    crazy_p = adapt.crazy_mix_p if adapt is not None else 0.35
    moe_p = adapt.moe_p if adapt is not None else 0.35
    width_choices = [4, 6, 8, 12, 16, 24, 32, 40]
    mw = arch_blocks.MAX_WIDTH
    if mw >= 48:
        width_choices.append(48)
    if mw > 48:
        width_choices.append(mw)
    return ArchConfig(
        depth=depth,
        width=rng.choice(width_choices),
        act=rng.choice(ACTS),
        ops=ensure_trainable_ops(rng.sample(OPS, k=k)),
        wet=rng.uniform(0.1, 0.95),
        fir=[rng.uniform(0.05, 0.55) for _ in range(5)],
        cell_kind=cell,
        soft_logits=[rng.uniform(-1.0, 1.0) for _ in range(len(OPS))],
        blocks=random_block_graph(
            rng,
            cell,
            max_extra=min(4 + (adapt.level if adapt else 0), arch_blocks.MAX_GRAPH_LEN - 1),
            crazy_mix_p=crazy_p,
            allowed_blocks=_graph_allowed_blocks(),
        ),
        use_adv_aux=rng.random() < 0.15,
        moe_mode=random_moe_mode(rng, moe_p=moe_p),
    )


def random_hp(rng: random.Random) -> HyperParams:
    return HyperParams(
        lr=10 ** rng.uniform(-4.0, -2.0),
        fit_steps=rng.choice([16, 20, 24, 32, 40, 48]),
        batch=rng.choice([32, 48, 64]),
        entropy_coef=rng.uniform(0.005, 0.06),
        ppo_clip=rng.choice([0.1, 0.2, 0.25, 0.3]),
        adv_coef=rng.choice([0.02, 0.05, 0.1]),
        reward_mode=rng.choice(list(REWARD_MODES)),
    )


def mutate_arch(
    cfg: ArchConfig,
    action: int,
    rng: random.Random,
    adapt: PlateauAdaptState | None = None,
) -> ArchConfig:
    c = ArchConfig(**cfg.to_dict())
    max_d = arch_blocks.MAX_SEARCH_DEPTH
    max_w = arch_blocks.MAX_WIDTH
    max_g = arch_blocks.MAX_GRAPH_LEN
    deepen_extra = adapt.deepen_bump if adapt is not None and adapt.level > 0 else 0
    if action == 0:
        # Depth step (asymmetric: slight deepen bias; stronger after plateau)
        steps = [-1, 1, 1] + ([1, 2] if deepen_extra else [])
        c.depth = max(1, min(max_d, c.depth + rng.choice(steps)))
    elif action == 1:
        c.width = max(4, min(max_w, c.width + rng.choice([-4, -2, -1, 1, 2, 4])))
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
        c.blocks = _graph_normalize(c.blocks, c.cell_kind)
    elif action == 7:
        c.soft_logits = [
            float(x + rng.uniform(-0.5, 0.5)) for x in (c.soft_logits or [0.0] * len(OPS))
        ]
        while len(c.soft_logits) < len(OPS):
            c.soft_logits.append(rng.uniform(-0.5, 0.5))
        c.soft_logits = c.soft_logits[: len(OPS)]
    elif action == 8:
        # Widen + deepen jump (depth prioritized)
        c.depth = rng.randint(max(3, c.depth), max_d)
        width_opts = [8, 12, 16, 24, 32]
        if max_w >= 48:
            width_opts.append(48)
        if max_w > 48:
            width_opts.append(max_w)
        c.width = rng.choice(width_opts)
    elif action == 9:
        c = random_arch(rng, adapt)
    elif action == 10:
        # Add/remove a lit block
        b = rng.choice(_block_mutation_pool())
        if b in c.blocks and len(c.blocks) > 1:
            c.blocks = [x for x in c.blocks if x != b]
        else:
            c.blocks = _graph_normalize(list(c.blocks) + [b], c.cell_kind)
    elif action == 11:
        crazy_p = adapt.crazy_mix_p if adapt is not None else 0.35
        c.blocks = random_block_graph(
            rng,
            c.cell_kind,
            max_extra=min(5 + (adapt.level if adapt else 0), max_g - 1),
            crazy_mix_p=crazy_p,
            allowed_blocks=_graph_allowed_blocks(),
        )
    elif action == 12:
        c.use_adv_aux = not c.use_adv_aux
    elif action == 13:
        # Explicit deepen bias action — primary deepen knob
        bump = rng.randint(1, 3 + deepen_extra)
        c.depth = max(1, min(max_d, c.depth + bump))
    elif action == 14:
        c.moe_mode = "moe_parallel" if c.moe_mode != "moe_parallel" else "sequential"
        if c.moe_mode == "moe_parallel" and len(c.blocks) < 2:
            crazy_p = adapt.crazy_mix_p if adapt is not None else 0.35
            c.blocks = random_block_graph(
                rng,
                c.cell_kind,
                max_extra=3,
                crazy_mix_p=crazy_p,
                allowed_blocks=_graph_allowed_blocks(),
            )
    else:
        # Diversify mixture toward heterogeneous experts
        pool = set(_block_mutation_pool())
        prefer = [
            b
            for b in ("unet", "attn", "dilated", "dual_path", "dense", "moe_mix", "noise_cond", "soft_mix")
            if b != c.cell_kind and b in pool
        ]
        n_extra = min(3 + (adapt.level if adapt else 0), len(prefer), max_g - 1)
        extra = rng.sample(prefer, k=max(1, n_extra)) if prefer else []
        c.blocks = _graph_normalize([c.cell_kind] + extra, c.cell_kind)
        moe_p = adapt.moe_p if adapt is not None else 0.5
        if rng.random() < moe_p:
            c.moe_mode = "moe_parallel"
        # Depth-first: also deepen when diversifying after plateau
        if deepen_extra:
            c.depth = max(1, min(max_d, c.depth + rng.randint(1, deepen_extra + 1)))
    c.ops = ensure_trainable_ops(c.ops)
    c.blocks = _graph_normalize(c.blocks, c.cell_kind)
    if c.moe_mode not in MOE_MODES:
        c.moe_mode = "sequential"
    c.depth = max(1, min(max_d, c.depth))
    c.width = max(4, min(max_w, c.width))
    return c


def deepen_arch_inplace(cfg: ArchConfig, bump: int) -> ArchConfig:
    """First-class deepen: raise residual/U-Net/MLP depth + lengthen graph moderately."""
    c = ArchConfig(**cfg.to_dict())
    max_d = arch_blocks.MAX_SEARCH_DEPTH
    max_g = arch_blocks.MAX_GRAPH_LEN
    c.depth = max(1, min(max_d, c.depth + max(1, bump)))
    # Prefer longer heterogeneous graphs without exploding width
    if len(c.blocks) < max_g:
        prefer = [
            b
            for b in (
                "n2n_unet",
                "unet",
                "attn",
                "dilated",
                "noise_cond",
                "moe_mix",
                "dense",
                "soft_mix",
            )
            if b not in c.blocks
        ]
        if prefer:
            c.blocks = normalize_graph(list(c.blocks) + [prefer[0]], c.cell_kind)
    return c


def apply_plateau_adapt(
    adapt: PlateauAdaptState,
    pop: list[Individual],
    rng: random.Random,
    *,
    it: int,
    max_level: int,
) -> dict[str, Any]:
    """Escalate search when champ stalls: deepen nets first, then crazier mixes + search shift."""
    adapt.level = min(max_level, adapt.level + 1)
    # Depth-first ceiling bump; width only moderate
    depth_delta = 2 + (1 if adapt.level >= 3 else 0)
    graph_delta = 1 if adapt.level <= 3 else 0
    width_delta = 4 if adapt.level % 2 == 0 else 0  # widen only every other escalate
    caps = raise_search_caps(
        depth_delta=depth_delta,
        graph_delta=graph_delta,
        width_delta=width_delta,
    )
    adapt.deepen_bump = 2 + adapt.level
    adapt.crazy_mix_p = min(0.85, 0.35 + 0.12 * adapt.level)
    adapt.moe_p = min(0.80, 0.35 + 0.10 * adapt.level)
    adapt.hold_p = max(0.08, 0.50 - 0.10 * adapt.level)
    adapt.nas_boost = min(0.35, 0.08 * adapt.level)
    adapt.last_adapt_iter = it
    adapt.soft_boredom = 0

    # Deepen every individual (champ genome preserved in champion_* outside pop)
    deepen_depths: list[int] = []
    for ind in pop:
        ind.cfg = deepen_arch_inplace(ind.cfg, adapt.deepen_bump)
        # Crazier mixtures on a subset
        if rng.random() < adapt.crazy_mix_p:
            prefer = [
                b
                for b in ("unet", "attn", "dilated", "dual_path", "moe_mix", "noise_cond", "soft_mix")
                if b != ind.cfg.cell_kind
            ]
            extra = rng.sample(prefer, k=min(3, len(prefer))) if prefer else []
            ind.cfg.blocks = normalize_graph([ind.cfg.cell_kind] + list(ind.cfg.blocks) + extra, ind.cfg.cell_kind)
            if rng.random() < adapt.moe_p:
                ind.cfg.moe_mode = "moe_parallel"
        deepen_depths.append(ind.cfg.depth)

    event = {
        "plateau_adapt": True,
        "deeper": True,
        "level": adapt.level,
        "iter": it,
        "depth_cap": caps["max_search_depth"],
        "graph_cap": caps["max_graph_len"],
        "width_cap": caps["max_width"],
        "pop_depth_min": min(deepen_depths) if deepen_depths else 0,
        "pop_depth_max": max(deepen_depths) if deepen_depths else 0,
        "mix": adapt.mix_tag(),
        "deepen_bump": adapt.deepen_bump,
    }
    return event


def pick_branch(
    it: int,
    adapt: PlateauAdaptState,
    rng: random.Random,
    branches: tuple[str, ...],
) -> str:
    """Round-robin by default; after plateau, weight GA/PBT/NAS/combo over conservative PPO."""
    if adapt.level <= 0:
        return branches[it % len(branches)]
    # Elevated explore mix
    weights = {
        "ppo": max(0.05, 0.20 - 0.04 * adapt.level),
        "nas": min(0.35, 0.20 + adapt.nas_boost),
        "pbt": min(0.30, 0.20 + 0.03 * adapt.level),
        "ga": min(0.32, 0.20 + 0.04 * adapt.level),
        "combo": min(0.28, 0.20 + 0.03 * adapt.level),
    }
    names = list(branches)
    ws = [weights.get(b, 0.1) for b in names]
    total = sum(ws)
    r = rng.random() * total
    acc = 0.0
    for b, w in zip(names, ws):
        acc += w
        if r <= acc:
            return b
    return names[-1]

def mutate_hp(hp: HyperParams, rng: random.Random) -> HyperParams:
    h = HyperParams(**hp.to_dict())
    h.lr = float(max(1e-5, min(1e-1, h.lr * (10 ** rng.uniform(-0.4, 0.4)))))
    h.fit_steps = int(max(8, min(64, h.fit_steps + rng.choice([-8, -4, 0, 4, 8]))))
    h.entropy_coef = float(max(0.001, min(0.1, h.entropy_coef + rng.uniform(-0.01, 0.01))))
    h.ppo_clip = float(max(0.05, min(0.4, h.ppo_clip + rng.choice([-0.05, 0.0, 0.05]))))
    h.batch = int(rng.choice([32, 48, 64]))
    h.adv_coef = float(max(0.0, min(0.2, h.adv_coef + rng.uniform(-0.02, 0.02))))
    if rng.random() < 0.25:
        h.reward_mode = rng.choice(list(REWARD_MODES))
    return h


def pbt_exploit_mutate(
    pop: list[Individual],
    rng: random.Random,
    elite_frac: float = 0.25,
    adapt: PlateauAdaptState | None = None,
) -> None:
    """In-place PBT: bottom half copies elite arch+hp then mutates (multi-step)."""
    ranked = sorted(pop, key=lambda ind: ind.score, reverse=True)
    n_elite = max(1, int(len(pop) * elite_frac))
    elites = ranked[:n_elite]
    n_mut_hi = 5 + (adapt.level if adapt else 0)
    for i, ind in enumerate(ranked[n_elite:]):
        parent = elites[i % n_elite]
        cfg = ArchConfig(**parent.cfg.to_dict())
        n_mut = rng.randint(2, n_mut_hi)
        for _ in range(n_mut):
            cfg = mutate_arch(cfg, rng.randrange(N_ACTIONS), rng, adapt)
        rand_p = 0.2 + (0.05 * adapt.level if adapt else 0.0)
        if rng.random() < min(0.5, rand_p):
            cfg = random_arch(rng, adapt)
        ind.cfg = cfg
        ind.hp = mutate_hp(HyperParams(**parent.hp.to_dict()), rng)
        ind.hp = mutate_hp(ind.hp, rng)
        ind.age = 0
        ind.score = parent.score * 0.92


def arch_diversity(pop: list[Individual]) -> float:
    if len(pop) < 2:
        return 0.0
    total = 0.0
    pairs = 0
    for i in range(len(pop)):
        for j in range(i + 1, len(pop)):
            a, b = pop[i].cfg, pop[j].cfg
            ops_a, ops_b = set(a.ops), set(b.ops)
            blocks_a, blocks_b = set(a.blocks), set(b.blocks)
            ham = len(ops_a.symmetric_difference(ops_b)) / max(len(OPS), 1)
            bham = len(blocks_a.symmetric_difference(blocks_b)) / max(len(BLOCKS), 1)
            cell_diff = 0.0 if a.cell_kind == b.cell_kind else 1.0
            act_diff = 0.0 if a.act == b.act else 1.0
            wdiff = abs(a.width - b.width) / float(arch_blocks.MAX_WIDTH)
            ddiff = abs(a.depth - b.depth) / float(arch_blocks.MAX_SEARCH_DEPTH)
            moe_diff = 0.0 if a.moe_mode == b.moe_mode else 1.0
            total += (
                0.22 * ham
                + 0.22 * bham
                + 0.16 * cell_diff
                + 0.08 * act_diff
                + 0.08 * wdiff
                + 0.12 * ddiff
                + 0.12 * moe_diff
            )
            pairs += 1
    return total / max(pairs, 1)


def fit_cell(
    cell: SeamCell,
    ops: list[str],
    device: torch.device,
    steps: int = 24,
    batch: int = 32,
    lr: float = 3e-3,
    adv_coef: float = 0.05,
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
        "cycle_net",
    }
    can_train = (
        bool(set(ops) & trainable_ops)
        or cell.cfg.cell_kind == "soft_mix"
        or len(cell.cfg.blocks) >= 1
    )
    lr_safe = finite_scalar(lr, 3e-3)
    if lr_safe <= 0:
        lr_safe = 3e-3
    opt = torch.optim.Adam(cell.parameters(), lr=lr_safe) if can_train else None
    prev = None
    patience = 0
    last_r = 0.0
    converged = False
    for _ in range(steps):
        ideal, eng = make_batch(batch, N, device)
        out = apply_ops(eng, cell, ops)
        r = residual_score_blend(ideal, eng, out).mean()
        last_r = finite_scalar(float(r.detach().item()), 0.0)
        if can_train and opt is not None:
            loss = 1.0 - r
            # Optional tiny adv aux: push generator outputs toward ideal discriminator score
            if cell.adv_head is not None and adv_coef > 0:
                fake_logit = cell.adv_head(out)
                real_logit = cell.adv_head(ideal.detach())
                # Non-saturating generator term + discriminator BCE (joint, light)
                adv_g = F.binary_cross_entropy_with_logits(
                    fake_logit, torch.ones_like(fake_logit)
                )
                adv_d = 0.5 * (
                    F.binary_cross_entropy_with_logits(real_logit, torch.ones_like(real_logit))
                    + F.binary_cross_entropy_with_logits(fake_logit.detach(), torch.zeros_like(fake_logit))
                )
                loss = loss + adv_coef * (adv_g + 0.5 * adv_d)
            if loss.requires_grad and torch.isfinite(loss).item():
                opt.zero_grad(set_to_none=True)
                loss.backward()
                nn.utils.clip_grad_norm_(cell.parameters(), 1.0)
                grads_ok = all(
                    p.grad is None or torch.isfinite(p.grad).all().item() for p in cell.parameters()
                )
                if grads_ok:
                    opt.step()
                else:
                    opt.zero_grad(set_to_none=True)
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
    return finite_scalar(float(residual_score_blend(ideal, eng, out).mean().item()), 0.0)


BASELINE_EVAL_SEED = 20260719


def rng_state_cpu_byte(state: torch.Tensor | object) -> torch.ByteTensor:
    """Torch / CUDA RNG checkpoints must be CPU uint8 for set_rng_state* APIs."""
    if not isinstance(state, torch.Tensor):
        state = torch.as_tensor(state)
    return state.detach().cpu().to(dtype=torch.uint8).clone()


def apply_checkpoint_rng_states(blob: dict[str, Any], device: torch.device) -> None:
    ts = blob.get("torch_rng_state")
    if ts is not None:
        torch.set_rng_state(rng_state_cpu_byte(ts))
    if device.type == "cuda":
        cuda_states = blob.get("cuda_rng_state")
        if cuda_states:
            torch.cuda.set_rng_state_all([rng_state_cpu_byte(s) for s in cuda_states])


def configure_torch_reproducibility(device: torch.device) -> None:
    """Best-effort deterministic Torch/CUDA for resume-fit parity (see test_overnight_gpu_rl_resume)."""
    os.environ.setdefault("CUBLAS_WORKSPACE_CONFIG", ":4096:8")
    torch.backends.cudnn.deterministic = True
    torch.backends.cudnn.benchmark = False
    try:
        torch.use_deterministic_algorithms(True, warn_only=True)
    except Exception:
        pass
    if hasattr(torch.backends, "cuda") and torch.backends.cuda.is_built():
        if hasattr(torch.backends.cuda, "matmul"):
            torch.backends.cuda.matmul.allow_tf32 = False
        if hasattr(torch.backends, "cudnn"):
            torch.backends.cudnn.allow_tf32 = False


@torch.no_grad()
def dual_cosine_baseline(device: torch.device, batch: int = 128) -> float:
    cpu_state = torch.get_rng_state()
    cuda_states = torch.cuda.get_rng_state_all() if device.type == "cuda" else None
    torch.manual_seed(BASELINE_EVAL_SEED)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(BASELINE_EVAL_SEED)
    ideal, eng = make_batch(batch, N, device)
    out = dual_cosine_blend(eng)
    score = float(residual_score_blend(ideal, eng, out).mean().item())
    torch.set_rng_state(rng_state_cpu_byte(cpu_state))
    if cuda_states is not None:
        torch.cuda.set_rng_state_all([rng_state_cpu_byte(s) for s in cuda_states])
    return score


@torch.no_grad()
def nobake_baseline(device: torch.device, batch: int = 128) -> float:
    """Unrepaired engine vs ideal sibling (near-ceiling body; seam still damaged)."""
    cpu_state = torch.get_rng_state()
    cuda_states = torch.cuda.get_rng_state_all() if device.type == "cuda" else None
    torch.manual_seed(BASELINE_EVAL_SEED + 1)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(BASELINE_EVAL_SEED + 1)
    ideal, eng = make_batch(batch, N, device)
    score = float(residual_score_blend(ideal, eng, eng).mean().item())
    torch.set_rng_state(rng_state_cpu_byte(cpu_state))
    if cuda_states is not None:
        torch.cuda.set_rng_state_all([rng_state_cpu_byte(s) for s in cuda_states])
    return score


def evaluate_candidate(
    cfg: ArchConfig,
    hp: HyperParams,
    device: torch.device,
    *,
    baseline: float,
    fit_steps_default: int,
    batch_default: int,
) -> tuple[float, float, float, float, SeamCell]:
    """Fit + R_blend eval + latency → (r_blend, j, j_scored, t_ms, cell)."""
    cell = SeamCell(cfg).to(device)
    fit_steps = int(hp.fit_steps or fit_steps_default)
    batch = int(hp.batch or batch_default)
    r_fit, _ = fit_cell(
        cell,
        cfg.ops,
        device,
        steps=fit_steps,
        batch=batch,
        lr=hp.lr,
        adv_coef=hp.adv_coef if cfg.use_adv_aux else 0.0,
    )
    r_eval = eval_cell(cell, cfg.ops, device, batch=max(64, batch))
    r_blend = finite_scalar(0.5 * r_fit + 0.5 * r_eval, 0.0)
    t_ms = finite_scalar(
        measure_forward_latency_ms(cell, cfg.ops, device, batch=min(32, max(8, batch))),
        0.0,
    )
    j = finite_scalar(objective_j(r_blend, t_ms), 0.0)
    dmb = finite_scalar(
        depth_mixture_bonus(
            r_blend,
            baseline,
            cfg.depth,
            len(cfg.blocks),
            cfg.moe_mode,
        ),
        0.0,
    )
    return r_blend, j, j + dmb, t_ms, cell


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
        "algorithms": ALGORITHMS,
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
                "algorithms": ALGORITHMS,
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


SEARCH_CKPT_PREFIX = "search_state_iter_"
SEARCH_CKPT_KEEP = 2
SEARCH_CKPT_SCHEMA = "denoiseopt.overnight_search.v1"


def atomic_torch_save(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_name(f"{path.stem}.{os.getpid()}.tmp")
    torch.save(payload, tmp)
    os.replace(str(tmp), str(path))


def atomic_json_save(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    text = json.dumps(payload, indent=2)
    tmp = path.with_name(f"{path.stem}.{os.getpid()}.tmp")
    tmp.write_text(text, encoding="utf-8")
    os.replace(str(tmp), str(path))


def prune_search_checkpoints(ckpt_dir: Path) -> None:
    pts = sorted(ckpt_dir.glob(f"{SEARCH_CKPT_PREFIX}*.pt"))
    while len(pts) > SEARCH_CKPT_KEEP:
        pts[0].unlink(missing_ok=True)
        pts = pts[1:]


def serialize_population(pop: list[Individual]) -> list[dict[str, Any]]:
    return [
        {
            "cfg": ind.cfg.to_dict(),
            "hp": ind.hp.to_dict(),
            "score": float(ind.score),
            "age": int(ind.age),
        }
        for ind in pop
    ]


def serialize_rollout_buffer(buf: RolloutBuffer) -> dict[str, Any]:
    return {
        "states": [s.detach().cpu() for s in buf.states],
        "actions": list(buf.actions),
        "logprobs": [lp.detach().cpu() for lp in buf.logprobs],
        "rewards": list(buf.rewards),
        "values": [v.detach().cpu() for v in buf.values],
        "dones": list(buf.dones),
    }


def deserialize_rollout_buffer(data: dict[str, Any] | None) -> RolloutBuffer:
    buf = RolloutBuffer()
    if not data:
        return buf
    buf.states = [s for s in data.get("states") or []]
    buf.actions = list(data.get("actions") or [])
    buf.logprobs = [lp for lp in data.get("logprobs") or []]
    buf.rewards = [float(x) for x in (data.get("rewards") or [])]
    buf.values = [v for v in data.get("values") or []]
    buf.dones = [bool(x) for x in (data.get("dones") or [])]
    return buf


def deserialize_population(rows: list[dict[str, Any]]) -> list[Individual]:
    out: list[Individual] = []
    for row in rows:
        out.append(
            Individual(
                cfg=ArchConfig.from_dict(row["cfg"]),
                hp=HyperParams.from_dict(row.get("hp")),
                score=float(row.get("score", -1.0)),
                age=int(row.get("age", 0)),
            )
        )
    return out


def search_protocol_fingerprint(args: argparse.Namespace) -> dict[str, Any]:
    return {
        "seed": int(args.seed),
        "pop_size": int(args.pop_size),
        "ppo_horizon": int(args.ppo_horizon),
        "batch": int(args.batch),
        "fit_steps": int(args.fit_steps),
        "branches": list(args.branch_tuple),
        "cell_kinds_allowed": list(CELL_KINDS),
        "extra_blocks_allowed": (
            list(GRAPH_BLOCKS_ALLOWED) if GRAPH_BLOCKS_ALLOWED is not None else list(BLOCKS)
        ),
        "plateau_adapt_every": int(args.plateau_adapt_every),
        "lambda_latency": float(LAMBDA_LATENCY),
    }


def latest_search_checkpoint(ckpt_dir: Path) -> Path | None:
    link = ckpt_dir / "search_state_latest.pt"
    if link.is_file():
        return link
    pts = sorted(ckpt_dir.glob(f"{SEARCH_CKPT_PREFIX}*.pt"))
    return pts[-1] if pts else None


def load_champion_cell_from_run(
    run_dir: Path,
    cfg: ArchConfig,
    device: torch.device,
) -> SeamCell | None:
    fitted = sorted((run_dir / "fitted").glob("champion_iter_*_fitted.pt"))
    if not fitted:
        return None
    try:
        blob = torch.load(fitted[-1], map_location="cpu", weights_only=False)
        cell = SeamCell(cfg).to(device)
        load_state_dict_compatible(cell, blob.get("cell_state_dict") or {})
        cell.eval()
        return cell
    except Exception:
        return None


def save_search_checkpoint(
    ckpt_dir: Path,
    *,
    iter_n: int,
    args: argparse.Namespace,
    run_id: str,
    pop: list[Individual],
    policy: ActorCritic,
    policy_opt: torch.optim.Optimizer,
    last_good_policy: dict[str, torch.Tensor],
    champion_r: float,
    champion_cfg: ArchConfig,
    champion_hp: HyperParams,
    champion_r_seam: float,
    champion_t_ms: float,
    iters_since_improve: int,
    plateau: PlateauAdaptState,
    branch_best: dict[str, float],
    recent_residuals: deque[float],
    rng: random.Random,
    device: torch.device,
    elapsed_sec: float,
    last_ppo_stats: dict[str, float],
    last_ga_stats: dict[str, float],
    rate_note_written: bool,
    buf: RolloutBuffer,
) -> Path:
    ckpt_path = ckpt_dir / f"{SEARCH_CKPT_PREFIX}{iter_n:06d}.pt"
    payload: dict[str, Any] = {
        "schema": SEARCH_CKPT_SCHEMA,
        "iter": iter_n,
        "run_id": run_id,
        "protocol_fingerprint": search_protocol_fingerprint(args),
        "pop": serialize_population(pop),
        "policy_state": {k: v.detach().cpu() for k, v in policy.state_dict().items()},
        "policy_opt_state": policy_opt.state_dict(),
        "last_good_policy": last_good_policy,
        "champion_r": float(champion_r),
        "champion_cfg": champion_cfg.to_dict(),
        "champion_hp": champion_hp.to_dict(),
        "champion_r_seam": float(champion_r_seam),
        "champion_t_ms": float(champion_t_ms),
        "iters_since_improve": int(iters_since_improve),
        "plateau": asdict(plateau),
        "branch_best": dict(branch_best),
        "recent_residuals": list(recent_residuals),
        "rng_state": rng.getstate(),
        "torch_rng_state": torch.get_rng_state(),
        "cuda_rng_state": (
            [t.cpu() for t in torch.cuda.get_rng_state_all()] if device.type == "cuda" else None
        ),
        "max_search_depth": arch_blocks.MAX_SEARCH_DEPTH,
        "max_graph_len": arch_blocks.MAX_GRAPH_LEN,
        "max_width": arch_blocks.MAX_WIDTH,
        "elapsed_sec": float(elapsed_sec),
        "last_ppo_stats": dict(last_ppo_stats),
        "last_ga_stats": dict(last_ga_stats),
        "rate_note_written": bool(rate_note_written),
        "rollout_buffer": serialize_rollout_buffer(buf),
    }
    atomic_torch_save(ckpt_path, payload)
    latest = ckpt_dir / "search_state_latest.pt"
    atomic_torch_save(latest, {**payload, "canonical_path": str(ckpt_path)})
    prune_search_checkpoints(ckpt_dir)
    return ckpt_path


def restore_search_checkpoint(
    ckpt_path: Path,
    *,
    args: argparse.Namespace,
    device: torch.device,
    policy: ActorCritic,
    policy_opt: torch.optim.Optimizer,
) -> dict[str, Any]:
    blob = torch.load(ckpt_path, map_location="cpu", weights_only=False)
    if blob.get("schema") != SEARCH_CKPT_SCHEMA:
        raise ValueError(f"unsupported checkpoint schema in {ckpt_path}")
    proto_now = search_protocol_fingerprint(args)
    proto_ck = blob.get("protocol_fingerprint") or {}
    for key in proto_now:
        if proto_now.get(key) != proto_ck.get(key):
            raise ValueError(
                f"checkpoint protocol mismatch on {key}: ckpt={proto_ck.get(key)!r} "
                f"now={proto_now.get(key)!r}"
            )
    policy.load_state_dict(
        {k: v.to(device) for k, v in (blob.get("policy_state") or {}).items()}
    )
    policy_opt.load_state_dict(blob.get("policy_opt_state") or {})
    rng = random.Random()
    if blob.get("rng_state") is not None:
        rng.setstate(blob["rng_state"])
    apply_checkpoint_rng_states(blob, device)
    pop = deserialize_population(blob.get("pop") or [])
    last_good = blob.get("last_good_policy") or snapshot_state_dict(policy)
    plateau_d = blob.get("plateau") or {}
    plateau = PlateauAdaptState(**{k: plateau_d[k] for k in plateau_d if k in PlateauAdaptState.__dataclass_fields__})  # type: ignore[attr-defined]
    champion_cfg = ArchConfig.from_dict(blob["champion_cfg"])
    champion_hp = HyperParams.from_dict(blob.get("champion_hp"))
    return {
        "start_it": int(blob.get("iter") or 0) + 1,
        "pop": pop,
        "rng": rng,
        "last_good_policy": last_good,
        "plateau": plateau,
        "champion_r": float(blob.get("champion_r", -1.0)),
        "champion_cfg": champion_cfg,
        "champion_hp": champion_hp,
        "champion_r_seam": float(blob.get("champion_r_seam", -1.0)),
        "champion_t_ms": float(blob.get("champion_t_ms", -1.0)),
        "iters_since_improve": int(blob.get("iters_since_improve", 0)),
        "branch_best": dict(blob.get("branch_best") or {}),
        "recent_residuals": deque(
            [float(x) for x in (blob.get("recent_residuals") or [])],
            maxlen=100,
        ),
        "elapsed_sec": float(blob.get("elapsed_sec", 0.0)),
        "last_ppo_stats": dict(blob.get("last_ppo_stats") or {}),
        "last_ga_stats": dict(blob.get("last_ga_stats") or {}),
        "rate_note_written": bool(blob.get("rate_note_written", False)),
        "resume_iter": int(blob.get("iter") or 0),
        "rollout_buffer": deserialize_rollout_buffer(blob.get("rollout_buffer")),
    }


class RolloutBuffer:
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
    last_good: dict[str, torch.Tensor] | None = None,
) -> dict[str, float]:
    if len(buf) == 0:
        return {"policy_loss": 0.0, "value_loss": 0.0, "entropy": 0.0, "nan_skipped": 0.0}

    states = torch.nan_to_num(torch.stack(buf.states).to(device), nan=0.0, posinf=0.0, neginf=0.0)
    actions = torch.tensor(buf.actions, dtype=torch.long, device=device)
    old_logprobs = torch.nan_to_num(
        torch.stack(buf.logprobs).detach().to(device), nan=0.0, posinf=0.0, neginf=0.0
    )
    rewards = torch.nan_to_num(
        torch.tensor(buf.rewards, dtype=torch.float32, device=device),
        nan=0.0,
        posinf=0.0,
        neginf=0.0,
    )
    values = torch.nan_to_num(
        torch.stack(buf.values).detach().to(device), nan=0.0, posinf=0.0, neginf=0.0
    )

    advantages = torch.zeros_like(rewards)
    last_gae = 0.0
    next_value = 0.0
    for t in reversed(range(len(rewards))):
        mask = 0.0 if buf.dones[t] else 1.0
        delta = float(rewards[t].item()) + gamma * next_value * mask - float(values[t].item())
        if not math.isfinite(delta):
            delta = 0.0
        last_gae = delta + gamma * lam * mask * last_gae
        if not math.isfinite(last_gae):
            last_gae = 0.0
        advantages[t] = last_gae
        next_value = float(values[t].item())
        if not math.isfinite(next_value):
            next_value = 0.0
    returns = torch.nan_to_num(advantages + values, nan=0.0, posinf=0.0, neginf=0.0)
    adv = torch.nan_to_num(advantages, nan=0.0, posinf=0.0, neginf=0.0)
    adv_std = float(adv.std().item()) if adv.numel() > 1 else 0.0
    if math.isfinite(adv_std) and adv_std > 1e-8:
        adv = (adv - adv.mean()) / (adv.std() + 1e-8)
    else:
        adv = torch.zeros_like(adv)
    adv = torch.nan_to_num(adv, nan=0.0, posinf=0.0, neginf=0.0)

    total_pi = 0.0
    total_v = 0.0
    total_ent = 0.0
    n_upd = 0
    nan_skipped = 0.0
    for _ in range(epochs):
        if not params_finite(policy):
            if last_good is not None:
                restore_state_dict(policy, last_good, device)
            nan_skipped = 1.0
            break
        logits, vals = policy(states)
        # Defensive: sanitize even if forward already did (corrupted weights path).
        logits = sanitize_logits(logits)
        dist = categorical_from_logits(logits)
        logprobs = dist.log_prob(actions)
        entropy = dist.entropy().mean()
        ratio = (logprobs - old_logprobs).clamp(-20.0, 20.0).exp()
        surr1 = ratio * adv
        surr2 = torch.clamp(ratio, 1.0 - clip_eps, 1.0 + clip_eps) * adv
        policy_loss = -torch.min(surr1, surr2).mean()
        value_loss = F.mse_loss(vals, returns)
        loss = policy_loss + value_coef * value_loss - entropy_coef * entropy
        if not torch.isfinite(loss).item():
            opt.zero_grad(set_to_none=True)
            nan_skipped = 1.0
            if last_good is not None:
                restore_state_dict(policy, last_good, device)
            break
        opt.zero_grad(set_to_none=True)
        loss.backward()
        grads_ok = all(
            p.grad is None or torch.isfinite(p.grad).all().item() for p in policy.parameters()
        )
        if not grads_ok:
            opt.zero_grad(set_to_none=True)
            nan_skipped = 1.0
            if last_good is not None:
                restore_state_dict(policy, last_good, device)
            break
        nn.utils.clip_grad_norm_(policy.parameters(), 1.0)
        opt.step()
        if not params_finite(policy):
            nan_skipped = 1.0
            if last_good is not None:
                restore_state_dict(policy, last_good, device)
            break
        total_pi += finite_scalar(float(policy_loss.detach().item()))
        total_v += finite_scalar(float(value_loss.detach().item()))
        total_ent += finite_scalar(float(entropy.detach().item()))
        n_upd += 1

    return {
        "policy_loss": total_pi / max(n_upd, 1),
        "value_loss": total_v / max(n_upd, 1),
        "entropy": total_ent / max(n_upd, 1),
        "nan_skipped": nan_skipped,
    }


def main() -> int:
    ap = argparse.ArgumentParser(
        description="PPO+GA+PBT overnight depth/mixture arch search (not claimed SOTA)"
    )
    ap.add_argument("--iters", type=int, default=1_000_000)
    ap.add_argument("--ckpt-every", type=int, default=500)
    ap.add_argument(
        "--history-every",
        type=int,
        default=1,
        help="Append one JSONL history row every N iters (default 1 = every iter).",
    )
    ap.add_argument("--batch", type=int, default=48)
    ap.add_argument("--fit-steps", type=int, default=24)
    ap.add_argument("--device", type=str, default="cuda")
    ap.add_argument("--run-id", type=str, default="")
    ap.add_argument("--max-hours", type=float, default=240.0)
    ap.add_argument("--seed", type=int, default=DEFAULT_SEED)
    ap.add_argument("--pop-size", type=int, default=12)
    ap.add_argument("--ppo-horizon", type=int, default=32)
    ap.add_argument("--pbt-every", type=int, default=50)
    ap.add_argument("--ga-every", type=int, default=40)
    ap.add_argument(
        "--algo-tag",
        type=str,
        default="PPO+GA+PBT+NAS+depth+MoE",
    )
    ap.add_argument(
        "--target-note",
        type=str,
        default="prefer_depth_and_mixtures; keep_1M_if_rate_ok_else_retarget_honestly",
    )
    ap.add_argument(
        "--seed-fitted",
        type=str,
        default="",
        help="Optional path to *_fitted.json or *_fitted.pt to warm-start pop[0] arch/hp "
        "(and cell weights when .pt is available).",
    )
    ap.add_argument(
        "--plateau-adapt-every",
        type=int,
        default=1000,
        help="Fire plateau deepen/crazy-mix adapt every N iters without champ improve "
        "(0 disables). Escalates each fire; soft boredom resets, champ kept.",
    )
    ap.add_argument(
        "--plateau-adapt-max-level",
        type=int,
        default=4,
        help="Cap escalate aggression (depth/graph/mix) for RTX 3090 trainability.",
    )
    ap.add_argument(
        "--branches",
        type=str,
        default="ppo,nas,pbt,ga,combo",
        help="Comma-separated branch names to rotate (isolated ablations: ga / ppo / ga,ppo / full set).",
    )
    ap.add_argument(
        "--cell-kinds",
        type=str,
        default="",
        help="Comma list restricting searchable cell kinds (names in BLOCKS); empty = full vocabulary.",
    )
    ap.add_argument(
        "--extra-blocks",
        type=str,
        default="",
        help="Comma list restricting composable graph blocks (names in BLOCKS); empty = full vocabulary.",
    )
    ap.add_argument(
        "--resume",
        action="store_true",
        help="Continue from the latest search_state checkpoint in --run-id's run folder.",
    )
    ap.add_argument(
        "--force-fresh",
        action="store_true",
        help="With --resume, ignore checkpoints and restart the loop state (keeps run_dir).",
    )
    args = ap.parse_args()
    if args.history_every < 1:
        print("ERROR: --history-every must be >= 1", file=sys.stderr)
        return 2
    if args.pop_size < 2:
        print("ERROR: --pop-size must be >= 2", file=sys.stderr)
        return 2
    if args.plateau_adapt_every < 0:
        print("ERROR: --plateau-adapt-every must be >= 0", file=sys.stderr)
        return 2
    if args.plateau_adapt_max_level < 1:
        print("ERROR: --plateau-adapt-max-level must be >= 1", file=sys.stderr)
        return 2
    allowed_branches = {"ppo", "nas", "pbt", "ga", "combo"}
    branch_list = tuple(
        b.strip().lower() for b in str(args.branches).split(",") if b.strip()
    )
    if not branch_list:
        print("ERROR: --branches must list at least one branch", file=sys.stderr)
        return 2
    bad = [b for b in branch_list if b not in allowed_branches]
    if bad:
        print(f"ERROR: unknown --branches {bad}; allowed={sorted(allowed_branches)}", file=sys.stderr)
        return 2
    args.branch_tuple = branch_list

    # Fresh process / repeated main() defaults (prior runs may have raised caps or narrowed CELL_KINDS)
    arch_blocks.set_search_caps(depth=12, graph=6, width=48)
    global CELL_KINDS, GRAPH_BLOCKS_ALLOWED
    CELL_KINDS = list(BLOCKS)
    arch_blocks.CELL_KINDS = list(BLOCKS)
    GRAPH_BLOCKS_ALLOWED = None

    rc = configure_search_space_flags(args.cell_kinds, args.extra_blocks)
    if rc != 0:
        return rc
    cell_kinds_allowed = list(CELL_KINDS)
    extra_blocks_allowed = (
        list(GRAPH_BLOCKS_ALLOWED) if GRAPH_BLOCKS_ALLOWED is not None else list(BLOCKS)
    )

    if args.device.startswith("cuda") and not torch.cuda.is_available():
        print("ERROR: CUDA requested but torch.cuda.is_available() is False", file=sys.stderr)
        return 2

    device = torch.device(
        args.device if torch.cuda.is_available() and args.device.startswith("cuda") else "cpu"
    )
    configure_torch_reproducibility(device)
    gpu_name = torch.cuda.get_device_name(0) if device.type == "cuda" else "cpu"
    if args.resume and not args.run_id:
        print("ERROR: --resume requires --run-id naming the interrupted run folder", file=sys.stderr)
        return 2
    run_id = args.run_id or f"gpu-rl-arch-{utc_now()}"
    run_dir = ROOT / "brand" / "artifacts" / "models" / run_id
    run_dir.mkdir(parents=True, exist_ok=True)
    meta_run = META_ROOT / "artifacts" / "models" / run_id
    meta_run.mkdir(parents=True, exist_ok=True)

    log_path = ROOT / "brand" / "artifacts" / f"overnight_gpu_rl_arch_{run_id}.log"
    ckpt_dir = run_dir / "checkpoints"
    ckpt_dir.mkdir(parents=True, exist_ok=True)

    resume_ckpt_path = latest_search_checkpoint(ckpt_dir) if args.resume and not args.force_fresh else None
    if args.resume and resume_ckpt_path is None:
        print(f"ERROR: --resume but no search_state checkpoint in {ckpt_dir}", file=sys.stderr)
        return 2

    history_path = run_dir / "history.jsonl"
    if not history_path.exists():
        history_path.write_text("", encoding="utf-8")

    if not resume_ckpt_path:
        torch.manual_seed(args.seed)
        if device.type == "cuda":
            torch.cuda.manual_seed_all(args.seed)
    baseline = dual_cosine_baseline(device)
    nobake_ref = nobake_baseline(device)
    now_local = datetime.now().astimezone()
    algorithms = list(ALGORITHMS)
    log_line(
        log_path,
        f"START run_id={run_id} algorithms={algorithms} algo_tag={args.algo_tag} "
        f"device={device} gpu={gpu_name} torch={torch.__version__} "
        f"cuda_available={torch.cuda.is_available()} "
        f"dual_cosine_baseline={baseline:.4f} target_iters={args.iters} "
        f"max_hours={args.max_hours} history_every={args.history_every} "
        f"seed={args.seed} pop_size={args.pop_size} ppo_horizon={args.ppo_horizon} "
        f"pbt_every={args.pbt_every} ga_every={args.ga_every} "
        f"max_depth={arch_blocks.MAX_SEARCH_DEPTH} max_graph={arch_blocks.MAX_GRAPH_LEN} "
        f"max_width={arch_blocks.MAX_WIDTH} "
        f"plateau_adapt_every={args.plateau_adapt_every} "
        f"plateau_adapt_max_level={args.plateau_adapt_max_level} "
        f"blocks={BLOCKS} cell_kinds={CELL_KINDS} "
        f"history_path={history_path} "
        f"local_start={now_local.isoformat(timespec='seconds')} "
        f"note=not_claimed_SOTA PPO+GA+depth+MoE_ERL_inspired",
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
                "ga_every": args.ga_every,
                "max_search_depth": arch_blocks.MAX_SEARCH_DEPTH,
                "max_graph_len": arch_blocks.MAX_GRAPH_LEN,
                "max_width": arch_blocks.MAX_WIDTH,
                "plateau_adapt_every": args.plateau_adapt_every,
                "plateau_adapt_max_level": args.plateau_adapt_max_level,
                "branches": list(args.branch_tuple),
                "n_ops": len(OPS),
                "ops": OPS,
                "cell_kinds": CELL_KINDS,
                "cell_kinds_allowed": cell_kinds_allowed,
                "extra_blocks_allowed": extra_blocks_allowed,
                "blocks": BLOCKS,
                "moe_modes": list(MOE_MODES),
                "primary_metric": "r_blend",
                "search_objective": "J=R_blend-lambda*latency_norm",
                "lambda_latency": LAMBDA_LATENCY,
                "latency_ref_ms": LATENCY_REF_MS,
                "seam_w": SEAM_W,
                "prolong": PROLONG,
                "target_note": args.target_note,
                "literature_artifact": "brand/artifacts/literature_rl_ga_nas_hybrid.json",
                "literature_arch_artifact": "brand/artifacts/literature_audio_denoise_arch.json",
                "pid": os.getpid(),
                "started_at": utc_now(),
                "note": (
                    "v10: R_seam + latency J; PPO+GA_crossover+PBT+depth_bias+MoE_softgate — "
                    "ERL-inspired interleave; not claimed SOTA"
                ),
            },
            indent=2,
        ),
        encoding="utf-8",
    )

    policy = ActorCritic().to(device)
    policy_opt = torch.optim.Adam(policy.parameters(), lr=3e-4)
    # No AMP / GradScaler in this loop (FP32); kept explicit so NaN paths stay deterministic.
    last_good_policy = snapshot_state_dict(policy)
    buf = RolloutBuffer()
    start_it = 1
    elapsed_prev = 0.0
    resumed_from: int | None = None
    pop: list[Individual] = []

    if resume_ckpt_path is not None:
        try:
            restored = restore_search_checkpoint(
                resume_ckpt_path,
                args=args,
                device=device,
                policy=policy,
                policy_opt=policy_opt,
            )
        except ValueError as exc:
            print(f"ERROR: {exc}", file=sys.stderr)
            return 2
        rng = restored["rng"]
        pop = restored["pop"]
        if len(pop) < args.pop_size:
            print(
                f"ERROR: checkpoint population size {len(pop)} < pop_size {args.pop_size}",
                file=sys.stderr,
            )
            return 2
        last_good_policy = restored["last_good_policy"]
        plateau = restored["plateau"]
        champion_r = restored["champion_r"]
        champion_cfg = restored["champion_cfg"]
        champion_hp = restored["champion_hp"]
        champion_r_seam = restored["champion_r_seam"]
        champion_t_ms = restored["champion_t_ms"]
        iters_since_improve = restored["iters_since_improve"]
        branch_best = restored["branch_best"]
        recent_residuals = restored["recent_residuals"]
        last_ppo_stats = restored["last_ppo_stats"]
        last_ga_stats = restored["last_ga_stats"]
        rate_note_written = restored["rate_note_written"]
        buf = restored["rollout_buffer"]
        start_it = restored["start_it"]
        elapsed_prev = restored["elapsed_sec"]
        resumed_from = restored["resume_iter"]
        champion_cell = load_champion_cell_from_run(run_dir, champion_cfg, device)
        last_plateau_event = None
        log_line(
            log_path,
            f"RESUME from {resume_ckpt_path.name} iter={resumed_from} "
            f"next_iter={start_it} champ_j={champion_r:.4f}",
        )
        resume_mode = True
    else:
        rng = random.Random(args.seed)
        pop = [
            Individual(cfg=random_arch(rng), hp=random_hp(rng), score=-1.0)
            for _ in range(args.pop_size)
        ]
        resume_mode = False
        champion_r = -1.0
        champion_r_seam = -1.0
        champion_t_ms = -1.0
        champion_cfg = pop[0].cfg
        champion_hp = pop[0].hp
        champion_cell = None
        iters_since_improve = 0
        plateau = PlateauAdaptState()
        last_plateau_event = None
        branch_best = {"ppo": 0.0, "nas": 0.0, "pbt": 0.0, "ga": 0.0, "combo": 0.0}
        last_ppo_stats = {
            "policy_loss": 0.0,
            "value_loss": 0.0,
            "entropy": 0.0,
            "nan_skipped": 0.0,
        }
        last_ga_stats = {"ga_crossover": 0.0, "ga_mutate": 0.0, "ga_elites": 0.0}
        recent_residuals = deque(maxlen=100)
        rate_note_written = False
    # Must not call rng (or torch RNG) on --resume: seed_specs are init-only.
    if not resume_mode:
        seed_specs = [
            ArchConfig(
                depth=6,
                width=24,
                act="gelu",
                ops=["mlp_seam", "dual_cosine", "fir3", "cycle_net"],
                wet=0.45,
                fir=[0.2, 0.5, 0.2, 0.05, 0.05],
                cell_kind="n2n_unet",
                blocks=["n2n_unet", "attn", "dilated", "residual"],
                soft_logits=[0.0] * len(OPS),
                moe_mode="moe_parallel",
            ),
            ArchConfig(
                depth=6,
                width=24,
                act="gelu",
                ops=["mlp_seam", "dual_cosine", "fir3", "cycle_net"],
                wet=0.45,
                fir=[0.2, 0.5, 0.2, 0.05, 0.05],
                cell_kind="unet",
                blocks=["unet", "attn", "dilated", "residual"],
                soft_logits=[0.0] * len(OPS),
                moe_mode="moe_parallel",
            ),
            ArchConfig(
                depth=5,
                width=20,
                act="silu",
                ops=["mlp_seam", "fir5", "cycle_net", "hann_blend"],
                wet=0.5,
                fir=[0.15, 0.2, 0.3, 0.2, 0.15],
                cell_kind="dilated",
                blocks=["dilated", "gated", "dense"],
                soft_logits=[0.0] * len(OPS),
                moe_mode="sequential",
            ),
            ArchConfig(
                depth=4,
                width=16,
                act="gelu",
                ops=["mlp_seam", "dual_cosine", "cycle_net"],
                wet=0.4,
                fir=[0.25, 0.5, 0.25, 0.0, 0.0],
                cell_kind="attn",
                blocks=["attn", "n2n_unet", "dual_path"],
                soft_logits=[0.0] * len(OPS),
                moe_mode="moe_parallel",
            ),
            ArchConfig(
                depth=7,
                width=18,
                act="relu",
                ops=["mlp_seam", "fir3", "cycle_net", "skip_blend"],
                wet=0.55,
                fir=[0.2, 0.5, 0.2, 0.05, 0.05],
                cell_kind="moe_mix",
                blocks=["moe_mix", "n2n_unet", "attn", "dilated"],
                soft_logits=[rng.uniform(-0.3, 0.3) for _ in range(len(OPS))],
                use_adv_aux=False,
                moe_mode="moe_parallel",
            ),
        ]
        allowed_cells = set(CELL_KINDS)
        for i, spec in enumerate(seed_specs):
            if i < len(pop) and spec.cell_kind in allowed_cells:
                pop[i].cfg = spec

    warm_residual = -1.0
    warm_cell: SeamCell | None = None
    if not resume_mode and args.seed_fitted:
        fitted_path = Path(args.seed_fitted)
        if not fitted_path.is_file():
            print(f"ERROR: --seed-fitted not found: {fitted_path}", file=sys.stderr)
            return 2
        meta_blob: dict[str, Any]
        weights_blob: dict[str, Any] | None = None
        if fitted_path.suffix == ".pt":
            weights_blob = torch.load(fitted_path, map_location="cpu", weights_only=False)
            meta_blob = {
                "architecture": weights_blob.get("architecture"),
                "hyperparams": weights_blob.get("hyperparams"),
                "residual": weights_blob.get("residual"),
                "tag": weights_blob.get("tag"),
            }
        else:
            meta_blob = json.loads(fitted_path.read_text(encoding="utf-8"))
            wp = meta_blob.get("weights_path")
            if wp and Path(wp).is_file():
                weights_blob = torch.load(wp, map_location="cpu", weights_only=False)
        arch_d = meta_blob.get("architecture") or {}
        hp_d = meta_blob.get("hyperparams")
        warm_cfg = ArchConfig.from_dict(arch_d)
        warm_hp = HyperParams.from_dict(hp_d)
        pop[0].cfg = warm_cfg
        pop[0].hp = warm_hp
        pop[0].score = float(meta_blob.get("residual") or -1.0)
        warm_residual = float(meta_blob.get("residual") or -1.0)
        if weights_blob and weights_blob.get("cell_state_dict") is not None:
            warm_cell = SeamCell(warm_cfg).to(device)
            loaded_n, skipped_n = load_state_dict_compatible(
                warm_cell, weights_blob["cell_state_dict"]
            )
            # Do NOT load policy_state_dict: prior fitted policies can inject NaNs into
            # Categorical logits and crash the run. Arch/hp/cell warm-start is enough.
            log_line(
                log_path,
                f"WARM_START_WEIGHTS loaded={loaded_n} skipped={skipped_n} "
                f"(shape-compatible filter)",
            )
        log_line(
            log_path,
            f"WARM_START seed_fitted={fitted_path} residual={warm_residual:.6f} "
            f"tag={meta_blob.get('tag')} cell_loaded={warm_cell is not None}",
        )

    if not resume_mode:
        save_unfitted(run_dir, pop[0].cfg, "init", pop[0].hp)
        save_unfitted(meta_run, pop[0].cfg, "init", pop[0].hp)
        if warm_residual >= 0:
            champion_r = warm_residual
            champion_cfg = pop[0].cfg
            champion_hp = pop[0].hp
            champion_cell = warm_cell
            champion_r_seam = warm_residual

    t0 = time.time() - elapsed_prev
    max_sec = args.max_hours * 3600.0
    keepalive = torch.zeros(1, device=device)
    old_plateau = 0.9809
    if not resume_mode:
        rate_note_written = False
    BRANCHES = args.branch_tuple
    log_line(log_path, f"branches={','.join(BRANCHES)}")

    for it in range(start_it, args.iters + 1):
        if time.time() - t0 > max_sec:
            log_line(log_path, f"STOP time budget reached at iter={it}")
            break

        branch = pick_branch(it, plateau, rng, BRANCHES)
        ind_idx = (it - 1) % len(pop)
        ind = pop[ind_idx]
        cfg = ind.cfg
        hp = ind.hp

        state = arch_state_vec(cfg, hp, device).unsqueeze(0)
        if not params_finite(policy):
            log_line(
                log_path,
                f"NAN_POLICY_RELOAD iter={it} reason=params_nonfinite before_sample",
            )
            restore_state_dict(policy, last_good_policy, device)
            buf.clear()
        try:
            logits, value = policy(state)
            dist = categorical_from_logits(logits)
            action_t = dist.sample()
            action = int(action_t.item())
            logprob = dist.log_prob(action_t)
            entropy_now = finite_scalar(float(dist.entropy().item()), 0.0)
        except (ValueError, RuntimeError) as exc:
            log_line(
                log_path,
                f"NAN_POLICY_RELOAD iter={it} reason=sample_failed err={exc!r}",
            )
            restore_state_dict(policy, last_good_policy, device)
            buf.clear()
            logits, value = policy(state)
            dist = categorical_from_logits(logits)
            action_t = dist.sample()
            action = int(action_t.item())
            logprob = dist.log_prob(action_t)
            entropy_now = finite_scalar(float(dist.entropy().item()), 0.0)

        # After plateau: bias deepen actions into the PPO sample occasionally
        if plateau.level > 0 and rng.random() < min(0.35, 0.08 * plateau.level):
            action = 13  # deepen bias

        if branch == "nas":
            trial_cfg = random_arch(rng, plateau)
            trial_hp = mutate_hp(hp, rng)
            proposal = "NAS_RANDOM"
        elif branch == "pbt":
            # Suppress PBT_HOLD after plateau (explore harder)
            mut_p = 1.0 - plateau.hold_p
            if rng.random() < mut_p:
                trial_cfg = mutate_arch(cfg, action, rng, plateau)
                proposal = "PBT_MUTATE"
            else:
                trial_cfg = cfg
                proposal = "PBT_HOLD"
            trial_hp = hp
        elif branch == "ga":
            parent = max(pop, key=lambda x: x.score)
            ga_cross_p = min(0.85, 0.6 + 0.05 * plateau.level)
            if rng.random() < ga_cross_p and parent.score > -0.5:
                from denoise_meta_evo import crossover_arch, crossover_hp

                trial_cfg = crossover_arch(
                    cfg,
                    parent.cfg,
                    rng,
                    ArchConfig=ArchConfig,
                    normalize_graph=_graph_normalize,
                    ensure_trainable_ops=ensure_trainable_ops,
                    CELL_KINDS=CELL_KINDS,
                    ACTS=ACTS,
                )
                trial_cfg = mutate_arch(trial_cfg, action, rng, plateau)
                trial_hp = crossover_hp(hp, parent.hp, rng, HyperParams=HyperParams)
                proposal = "GA_CROSSOVER+PPO_MUTATION"
            else:
                trial_cfg = mutate_arch(cfg, action, rng, plateau)
                trial_hp = mutate_hp(hp, rng)
                proposal = "GA_MUTATE+PPO_MUTATION"
        elif branch == "combo":
            trial_cfg = mutate_arch(
                mutate_arch(cfg, action, rng, plateau), rng.randrange(N_ACTIONS), rng, plateau
            )
            trial_hp = mutate_hp(hp, rng)
            proposal = "COMBO_PPO_DOUBLE_MUT"
        else:
            # PPO branch: after plateau, never "hold" — always mutate (optionally deepen)
            if plateau.level > 0 and rng.random() < plateau.hold_p:
                # Rare conservative hold only at low residual hold_p; else mutate
                trial_cfg = mutate_arch(cfg, action, rng, plateau)
                proposal = "PPO_MUTATION"
            else:
                trial_cfg = mutate_arch(cfg, action, rng, plateau)
                proposal = "PPO_MUTATION"
            trial_hp = mutate_hp(hp, rng)

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
            adv_coef=trial_hp.adv_coef if trial_cfg.use_adv_aux else 0.0,
        )
        r_eval = eval_cell(cell, trial_cfg.ops, device, batch=max(64, batch))
        raw_sum = 0.5 * r_fit + 0.5 * r_eval
        residual_raw = finite_scalar(raw_sum, 0.0)  # R_seam
        if not math.isfinite(raw_sum):
            log_line(
                log_path,
                f"NAN_RESIDUAL iter={it} r_fit={r_fit!r} r_eval={r_eval!r} -> 0.0",
            )
        t_ms = finite_scalar(
            measure_forward_latency_ms(
                cell, trial_cfg.ops, device, batch=min(32, max(8, batch))
            ),
            0.0,
        )
        j_raw = finite_scalar(objective_j(residual_raw, t_ms), 0.0)
        dmb = depth_mixture_bonus(
            residual_raw,
            baseline,
            trial_cfg.depth,
            len(trial_cfg.blocks),
            trial_cfg.moe_mode,
        )
        dmb = finite_scalar(dmb, 0.0)
        residual = j_raw + dmb  # scored selection objective (J + depth mix bonus)
        branch_best[branch] = max(branch_best[branch], j_raw)
        recent_residuals.append(residual_raw)

        reward = finite_scalar(
            shaped_reward(
                residual,
                mode=getattr(trial_hp, "reward_mode", "vs_dualcosine"),
                r_dualcosine=baseline,
                r_nobake=nobake_ref,
            ),
            0.0,
        )
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
                last_good=last_good_policy,
            )
            buf.clear()
            if last_ppo_stats.get("nan_skipped", 0.0) >= 1.0:
                log_line(
                    log_path,
                    f"NAN_PPO_SKIP iter={it} restored_last_good_policy=1",
                )
            elif params_finite(policy):
                last_good_policy = snapshot_state_dict(policy)

        if residual >= ind.score:
            ind.cfg = trial_cfg
            ind.hp = trial_hp
            ind.score = residual
        ind.age += 1

        if it % args.ga_every == 0:
            def _ga_mutate(cfg: ArchConfig, action: int, rng_: random.Random) -> ArchConfig:
                return mutate_arch(cfg, action, rng_, plateau)

            last_ga_stats = ga_generation(
                pop,
                rng,
                mutate_arch=_ga_mutate,
                mutate_hp=mutate_hp,
                ArchConfig=ArchConfig,
                HyperParams=HyperParams,
                normalize_graph=_graph_normalize,
                ensure_trainable_ops=ensure_trainable_ops,
                CELL_KINDS=CELL_KINDS,
                ACTS=ACTS,
                n_actions=N_ACTIONS,
            )
            log_line(
                log_path,
                f"GA_STEP iter={it} crossover={last_ga_stats['ga_crossover']:.0f} "
                f"mutate={last_ga_stats['ga_mutate']:.0f} elites={last_ga_stats['ga_elites']:.0f} "
                f"elite_score={max(p.score for p in pop):.4f} note=tournament_crossover_mutate",
            )

        if it % args.pbt_every == 0:
            before_div = arch_diversity(pop)
            pbt_exploit_mutate(pop, rng, adapt=plateau)
            after_div = arch_diversity(pop)
            log_line(
                log_path,
                f"PBT_STEP iter={it} diversity_before={before_div:.4f} "
                f"diversity_after={after_div:.4f} elite_score={max(p.score for p in pop):.4f}",
            )

        pop_div = arch_diversity(pop)
        champ_now = (
            j_raw
            if j_raw > champion_r
            else (champion_r if champion_r >= 0 else j_raw)
        )

        if it == 1 or (it % args.history_every == 0):
            tag = f"iter_{it:06d}"
            hist_row: dict[str, Any] = {
                "iter": it,
                "t_sec": round(time.time() - t0, 6),
                "residual": residual_raw,
                "r_blend": residual_raw,
                "t_ms": t_ms,
                "j": j_raw,
                "residual_scored": residual,
                "residual_whole_curve": None,
                "depth_mix_bonus": dmb,
                "champ": champ_now,
                "iters_since_improve": iters_since_improve,
                "branch": branch,
                "proposal": proposal,
                "branch_best_ppo": branch_best["ppo"],
                "branch_best_nas": branch_best["nas"],
                "branch_best_pbt": branch_best["pbt"],
                "branch_best_ga": branch_best["ga"],
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
                "vs_old_plateau": residual_raw - old_plateau,
                "cell_kind": trial_cfg.cell_kind,
                "blocks": trial_cfg.blocks,
                "depth": trial_cfg.depth,
                "moe_mode": trial_cfg.moe_mode,
                "use_adv_aux": trial_cfg.use_adv_aux,
                "ga_crossover": last_ga_stats["ga_crossover"],
                "ga_mutate": last_ga_stats["ga_mutate"],
                "plateau_level": plateau.level,
                "plateau_soft_boredom": plateau.soft_boredom,
                "max_search_depth": arch_blocks.MAX_SEARCH_DEPTH,
                "max_graph_len": arch_blocks.MAX_GRAPH_LEN,
                "lambda_latency": LAMBDA_LATENCY,
                "primary_metric": "r_blend",
                "search_objective": "J=R_blend-lambda*latency_norm",
            }
            # Optional whole-curve debug (cheap single batch already on device path)
            try:
                with torch.no_grad():
                    _ideal, _eng = make_batch(min(32, batch), N, device)
                    _out = apply_ops(_eng, cell, trial_cfg.ops)
                    hist_row["residual_whole_curve"] = float(
                        residual_score(_ideal, _out).mean().item()
                    )
            except Exception:
                pass
            if last_plateau_event is not None and last_plateau_event.get("iter") == it:
                hist_row["plateau_adapt"] = True
                hist_row["plateau_adapt_event"] = last_plateau_event
            append_history(history_path, hist_row)

        if j_raw > champion_r:
            champion_r = j_raw
            champion_cfg = trial_cfg
            champion_hp = trial_hp
            champion_cell = cell
            champion_r_seam = residual_raw
            champion_t_ms = t_ms
            iters_since_improve = 0
            plateau.soft_boredom = 0
            victim = rng.randrange(len(pop))
            pop[victim].cfg = ArchConfig(**trial_cfg.to_dict())
            pop[victim].hp = HyperParams(**trial_hp.to_dict())
            pop[victim].score = residual
            save_fitted(
                run_dir, trial_cfg, cell, policy, residual_raw, f"champion_iter_{it:06d}", trial_hp
            )
            save_fitted(
                meta_run, trial_cfg, cell, policy, residual_raw, f"champion_iter_{it:06d}", trial_hp
            )
            if params_finite(policy):
                last_good_policy = snapshot_state_dict(policy)
            log_line(
                log_path,
                f"NEW_CHAMPION iter={it} residual={residual_raw:.4f} "
                f"scored={residual:.4f} dmb={dmb:.5f} proposal={proposal} "
                f"delta_vs_dual={residual_raw - baseline:+.4f} "
                f"vs_old_plateau={residual_raw - old_plateau:+.4f} "
                f"iters_since_improve=0 algorithms={algorithms} "
                f"depth={trial_cfg.depth} moe={trial_cfg.moe_mode} "
                f"arch={trial_cfg.to_dict()} hp={trial_hp.to_dict()}",
            )
        else:
            iters_since_improve += 1
            plateau.soft_boredom += 1
            # Fire every plateau_adapt_every flat iters (soft boredom); escalate; keep champ
            if (
                args.plateau_adapt_every > 0
                and plateau.soft_boredom >= args.plateau_adapt_every
            ):
                if plateau.level < args.plateau_adapt_max_level:
                    last_plateau_event = apply_plateau_adapt(
                        plateau,
                        pop,
                        rng,
                        it=it,
                        max_level=args.plateau_adapt_max_level,
                    )
                else:
                    # Cap reached: re-inject deepen/crazy mixes without raising ceilings further
                    plateau.soft_boredom = 0
                    plateau.last_adapt_iter = it
                    for ind in pop:
                        ind.cfg = deepen_arch_inplace(ind.cfg, 1)
                        if rng.random() < plateau.crazy_mix_p:
                            prefer = [
                                b
                                for b in (
                                    "unet",
                                    "attn",
                                    "dilated",
                                    "moe_mix",
                                    "noise_cond",
                                    "soft_mix",
                                )
                                if b != ind.cfg.cell_kind
                            ]
                            extra = rng.sample(prefer, k=min(2, len(prefer))) if prefer else []
                            ind.cfg.blocks = normalize_graph(
                                list(ind.cfg.blocks) + extra, ind.cfg.cell_kind
                            )
                    caps = get_search_caps()
                    last_plateau_event = {
                        "plateau_adapt": True,
                        "deeper": True,
                        "level": plateau.level,
                        "iter": it,
                        "depth_cap": caps["max_search_depth"],
                        "graph_cap": caps["max_graph_len"],
                        "width_cap": caps["max_width"],
                        "pop_depth_min": min(p.cfg.depth for p in pop),
                        "pop_depth_max": max(p.cfg.depth for p in pop),
                        "mix": plateau.mix_tag(),
                        "deepen_bump": 1,
                        "capped": True,
                    }
                log_line(
                    log_path,
                    f"PLATEAU_ADAPT deeper=true depth={last_plateau_event['depth_cap']} "
                    f"graph={last_plateau_event['graph_cap']} "
                    f"width={last_plateau_event['width_cap']} "
                    f"level={last_plateau_event['level']} "
                    f"mix={last_plateau_event['mix']} "
                    f"deepen_bump={last_plateau_event['deepen_bump']} "
                    f"pop_depth={last_plateau_event['pop_depth_min']}-"
                    f"{last_plateau_event['pop_depth_max']} "
                    f"iters_since_improve={iters_since_improve} "
                    f"soft_boredom_reset=0 champ={champion_r:.4f}"
                    + (" capped=true" if last_plateau_event.get("capped") else ""),
                )
                append_history(
                    history_path,
                    {
                        "iter": it,
                        "t_sec": round(time.time() - t0, 6),
                        "residual": residual_raw,
                        "champ": champion_r if champion_r >= 0 else residual_raw,
                        "iters_since_improve": iters_since_improve,
                        "plateau_adapt": True,
                        "deeper": True,
                        "plateau_level": plateau.level,
                        "depth": last_plateau_event["depth_cap"],
                        "max_search_depth": last_plateau_event["depth_cap"],
                        "max_graph_len": last_plateau_event["graph_cap"],
                        "max_width": last_plateau_event["width_cap"],
                        "mix": last_plateau_event["mix"],
                        "plateau_adapt_event": last_plateau_event,
                        "branch": branch,
                        "proposal": proposal,
                        "tag": f"plateau_adapt_{it:06d}",
                    },
                )

        def write_latest(iter_n: int, *, checkpoint: bool) -> None:
            elapsed = time.time() - t0
            rate = iter_n / max(elapsed, 1e-6)
            hours_for_1m = 1_000_000 / max(rate, 1e-9) / 3600.0
            if hours_for_1m <= args.max_hours:
                target_plan = {"keep_target": 1_000_000, "eta_hours_at_rate": round(hours_for_1m, 2)}
            elif hours_for_1m <= args.max_hours * 2:
                target_plan = {
                    "retarget": 500_000,
                    "reason": "rate_too_slow_for_1M_in_max_hours",
                    "eta_hours_1M": round(hours_for_1m, 2),
                }
            else:
                target_plan = {
                    "retarget": 250_000,
                    "reason": "complex_arch_slower_than_1M_feasible",
                    "eta_hours_1M": round(hours_for_1m, 2),
                }
            ckpt = {
                "iter": iter_n,
                "champion_residual": champion_r,
                "champion_j": champion_r,
                "champion_r_seam": champion_r_seam,
                "champion_t_ms": champion_t_ms,
                "lambda_latency": LAMBDA_LATENCY,
                "primary_metric": "r_blend",
                "search_objective": "J=R_blend-lambda*latency_norm",
                "champion_arch": champion_cfg.to_dict(),
                "champion_hp": champion_hp.to_dict(),
                "baseline_dual_cosine": baseline,
                "branch_best": branch_best,
                "iters_since_improve": iters_since_improve,
                "pop_diversity": pop_div,
                "entropy": last_ppo_stats.get("entropy", entropy_now),
                "algorithms": algorithms,
                "elapsed_sec": elapsed,
                "iters_per_sec": rate,
                "target_plan": target_plan,
                "gpu": gpu_name,
                "pid": os.getpid(),
                "seed": args.seed,
                "history_path": str(history_path),
                "blocks": BLOCKS,
                "max_search_depth": arch_blocks.MAX_SEARCH_DEPTH,
                "max_graph_len": arch_blocks.MAX_GRAPH_LEN,
                "max_width": arch_blocks.MAX_WIDTH,
                "plateau_level": plateau.level,
                "plateau_adapt_every": args.plateau_adapt_every,
                "ga_every": args.ga_every,
                "algo_tag": args.algo_tag,
                "cell_kinds_allowed": cell_kinds_allowed,
                "extra_blocks_allowed": extra_blocks_allowed,
            }
            if checkpoint:
                ckpt_path = ckpt_dir / f"ckpt_iter_{iter_n:06d}.json"
                atomic_json_save(ckpt_path, ckpt)
                search_path = save_search_checkpoint(
                    ckpt_dir,
                    iter_n=iter_n,
                    args=args,
                    run_id=run_id,
                    pop=pop,
                    policy=policy,
                    policy_opt=policy_opt,
                    last_good_policy=last_good_policy,
                    champion_r=champion_r,
                    champion_cfg=champion_cfg,
                    champion_hp=champion_hp,
                    champion_r_seam=champion_r_seam,
                    champion_t_ms=champion_t_ms,
                    iters_since_improve=iters_since_improve,
                    plateau=plateau,
                    branch_best=branch_best,
                    recent_residuals=recent_residuals,
                    rng=rng,
                    device=device,
                    elapsed_sec=time.time() - t0,
                    last_ppo_stats=last_ppo_stats,
                    last_ga_stats=last_ga_stats,
                    rate_note_written=rate_note_written,
                    buf=buf,
                )
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
                log_line(
                    log_path,
                    f"CHECKPOINT iter={iter_n} wrote {ckpt_path} search={search_path.name}",
                )
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
            if it >= 50 and not rate_note_written:
                hours_1m = 1_000_000 / max(rate, 1e-9) / 3600.0
                log_line(
                    log_path,
                    f"RATE_NOTE iters_per_sec={rate:.3f} eta_1M_h={hours_1m:.2f} "
                    f"max_hours={args.max_hours} "
                    f"plan={'keep_1M' if hours_1m <= args.max_hours else 'retarget_if_needed'}",
                )
                rate_note_written = True
            log_line(
                log_path,
                f"progress {it}/{args.iters} branch={branch} proposal={proposal} "
                f"r_seam={residual_raw:.4f} t_ms={t_ms:.3f} j={j_raw:.4f} scored={residual:.4f} "
                f"dmb={dmb:.5f} champ_j={champion_r:.4f} baseline={baseline:.4f} "
                f"iters_since_improve={iters_since_improve} "
                f"entropy={entropy_now:.4f} pop_div={pop_div:.4f} "
                f"depth={trial_cfg.depth} moe={trial_cfg.moe_mode} "
                f"blocks={trial_cfg.blocks} cell={trial_cfg.cell_kind} "
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
        "champion_j": champion_r,
        "champion_r_seam": champion_r_seam,
        "champion_t_ms": champion_t_ms,
        "lambda_latency": LAMBDA_LATENCY,
        "primary_metric": "r_blend",
        "search_objective": "J=R_blend-lambda*latency_norm",
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
        "cell_kinds_allowed": cell_kinds_allowed,
        "extra_blocks_allowed": extra_blocks_allowed,
        "champion_arch": champion_cfg.to_dict(),
        "resumed_from_iter": resumed_from,
        "search_checkpoint_schema": SEARCH_CKPT_SCHEMA,
    }
    atomic_json_save(run_dir / "final_summary.json", final)
    log_line(log_path, f"DONE {json.dumps(final)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
