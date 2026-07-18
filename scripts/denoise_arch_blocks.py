"""Composable seam/cycle architecture blocks for DenoiseOpt NAS.

Lit-inspired families scaled for N=256 residual bake (RTX 3090, overnight):
  residual, dense, unet, conv1d, dilated, attn, dual_path, gated, soft_mix,
  noise_cond, mlp — composed as short graphs, not full Demucs/diffusion/GAN.
"""
from __future__ import annotations

import math
from typing import Sequence

import torch
import torch.nn as nn
import torch.nn.functional as F

# Searchable block vocabulary (order matters for soft-mix indexing helpers).
BLOCKS = [
    "mlp",
    "residual",
    "dense",
    "gated",
    "bottleneck",
    "unet",
    "conv1d",
    "dilated",
    "attn",
    "dual_path",
    "tf_split",
    "noise_cond",
    "soft_mix",
]

# Keep CELL_KINDS as primary cell mode; graphs compose additional blocks.
CELL_KINDS = list(BLOCKS)


def _act_module(act: str) -> nn.Module:
    if act == "tanh":
        return nn.Tanh()
    if act == "gelu":
        return nn.GELU()
    if act == "silu":
        return nn.SiLU()
    return nn.ReLU()


class TinyMLP(nn.Module):
    def __init__(self, in_d: int, width: int, depth: int, act: str, out_d: int | None = None):
        super().__init__()
        out_d = in_d if out_d is None else out_d
        layers: list[nn.Module] = []
        d_in = in_d
        for i in range(max(1, depth)):
            d_out = out_d if i == depth - 1 else width
            layers.append(nn.Linear(d_in, d_out))
            if i < depth - 1:
                layers.append(_act_module(act))
            d_in = d_out
        self.net = nn.Sequential(*layers)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.net(x)


class DenseBlock(nn.Module):
    """Tiny DenseNet-style concat growth on feature dim (capped)."""

    def __init__(self, in_d: int, width: int, depth: int, act: str):
        super().__init__()
        self.layers = nn.ModuleList()
        cur = in_d
        growth = max(2, min(8, width // 2))
        for _ in range(max(1, depth)):
            self.layers.append(
                nn.Sequential(nn.Linear(cur, growth), _act_module(act))
            )
            cur = cur + growth
        self.proj = nn.Linear(cur, in_d)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        feats = [x]
        h = x
        for layer in self.layers:
            y = layer(h)
            feats.append(y)
            h = torch.cat(feats, dim=-1)
        return self.proj(h)


class TinyUNet1D(nn.Module):
    """Tiny 1D U-Net on length-L vectors (channel=1). Wave-U-Net inspired, tiny."""

    def __init__(self, length: int, width: int, act: str):
        super().__init__()
        c = max(4, min(16, width // 2))
        self.enc1 = nn.Conv1d(1, c, 3, padding=1)
        self.enc2 = nn.Conv1d(c, c * 2, 3, stride=2, padding=1)
        self.bot = nn.Conv1d(c * 2, c * 2, 3, padding=1)
        self.up = nn.ConvTranspose1d(c * 2, c, 4, stride=2, padding=1)
        self.dec = nn.Conv1d(c * 2, 1, 3, padding=1)
        self.act = _act_module(act)
        self.length = length

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        # x: (B, L)
        h = x.unsqueeze(1)
        e1 = self.act(self.enc1(h))
        e2 = self.act(self.enc2(e1))
        b = self.act(self.bot(e2))
        u = self.up(b)
        if u.shape[-1] != e1.shape[-1]:
            u = F.interpolate(u, size=e1.shape[-1], mode="linear", align_corners=False)
        d = torch.cat([u, e1], dim=1)
        y = self.dec(d).squeeze(1)
        if y.shape[-1] != x.shape[-1]:
            y = F.interpolate(y.unsqueeze(1), size=x.shape[-1], mode="linear", align_corners=False).squeeze(1)
        return y


class Conv1dStack(nn.Module):
    def __init__(self, length: int, width: int, depth: int, act: str, dilation: bool = False):
        super().__init__()
        c = max(4, min(24, width))
        layers: list[nn.Module] = []
        in_c = 1
        for i in range(max(1, min(4, depth))):
            dil = (2**i) if dilation else 1
            pad = dil
            layers.append(nn.Conv1d(in_c, c, 3, padding=pad, dilation=dil))
            layers.append(_act_module(act))
            in_c = c
        layers.append(nn.Conv1d(c, 1, 1))
        self.net = nn.Sequential(*layers)
        self.length = length

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        y = self.net(x.unsqueeze(1)).squeeze(1)
        if y.shape[-1] != x.shape[-1]:
            y = y[..., : x.shape[-1]]
            if y.shape[-1] < x.shape[-1]:
                y = F.pad(y, (0, x.shape[-1] - y.shape[-1]))
        return y


class TinyAttention(nn.Module):
    """Lightweight self-attention over L tokens with tiny dim."""

    def __init__(self, length: int, width: int, act: str):
        super().__init__()
        d = max(4, min(32, width))
        n_heads = 2 if d % 2 == 0 else 1
        self.proj_in = nn.Linear(1, d)
        self.attn = nn.MultiheadAttention(d, n_heads, batch_first=True)
        self.ff = nn.Sequential(nn.Linear(d, d), _act_module(act), nn.Linear(d, 1))
        self.length = length

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        # (B, L) -> (B, L, 1)
        h = self.proj_in(x.unsqueeze(-1))
        a, _ = self.attn(h, h, h, need_weights=False)
        y = self.ff(a + h).squeeze(-1)
        return y


class DualPathLite(nn.Module):
    """DPRNN-inspired: split length into chunks, local MLP + global MLP."""

    def __init__(self, length: int, width: int, act: str):
        super().__init__()
        self.chunk = max(2, min(8, length // 2))
        h = max(4, min(32, width))
        self.local = TinyMLP(self.chunk, h, 2, act)
        self.global_mix = TinyMLP(max(1, length // self.chunk), h, 2, act, out_d=max(1, length // self.chunk))
        self.length = length
        self.gate = nn.Parameter(torch.tensor(0.3))

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        b, l = x.shape
        c = self.chunk
        n_chunks = max(1, l // c)
        usable = n_chunks * c
        head = x[:, :usable].view(b, n_chunks, c)
        local = self.local(head)
        # inter-chunk: mix across chunk index per feature mean
        g_in = local.mean(dim=-1)  # (B, n_chunks)
        g = self.global_mix(g_in).unsqueeze(-1)
        mixed = local * torch.sigmoid(g)
        y = mixed.reshape(b, usable)
        if usable < l:
            y = torch.cat([y, x[:, usable:]], dim=1)
        g = torch.sigmoid(self.gate)
        return g * y + (1 - g) * x


class TFSplitLite(nn.Module):
    """Cheap time/freq style split via even/odd + low/high average branches."""

    def __init__(self, length: int, width: int, act: str):
        super().__init__()
        h = max(4, min(24, width))
        self.time_mlp = TinyMLP(length, h, 2, act)
        self.freq_mlp = TinyMLP(length, h, 2, act)
        self.mix = nn.Parameter(torch.tensor(0.5))

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        # "freq" proxy: difference from local mean (highpass-ish)
        kernel = 5
        pad = kernel // 2
        xp = F.pad(x.unsqueeze(1), (pad, pad), mode="reflect")
        avg = F.avg_pool1d(xp, kernel_size=kernel, stride=1).squeeze(1)
        hi = x - avg
        yt = self.time_mlp(x)
        yf = self.freq_mlp(hi)
        m = torch.sigmoid(self.mix)
        return m * yt + (1 - m) * yf


class NoiseCondResidual(nn.Module):
    """Single noise-conditioned residual step (tiny diffusion/score proxy — not full sampling)."""

    def __init__(self, length: int, width: int, act: str):
        super().__init__()
        h = max(4, min(32, width))
        self.mlp = TinyMLP(length + 1, h, 2, act, out_d=length)
        self.noise_log = nn.Parameter(torch.tensor(-2.0))

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        b = x.shape[0]
        # Use learned noise level embedding (not stochastic during eval path for stability)
        sigma = torch.sigmoid(self.noise_log).expand(b, 1)
        inp = torch.cat([x, sigma], dim=-1)
        return x + torch.tanh(self.mlp(inp)) * sigma


class SoftOpGate(nn.Module):
    """Learnable gate scalar for blending block output with identity."""

    def __init__(self, init: float = 0.25):
        super().__init__()
        self.gate = nn.Parameter(torch.tensor(init))

    def forward(self, x: torch.Tensor, y: torch.Tensor) -> torch.Tensor:
        g = torch.sigmoid(self.gate)
        return g * y + (1 - g) * x


def normalize_graph(blocks: Sequence[str] | None, cell_kind: str) -> list[str]:
    """Build a short composable graph: primary cell + optional extras."""
    raw = list(blocks) if blocks else []
    cleaned: list[str] = []
    for b in raw:
        if b in BLOCKS and b not in cleaned:
            cleaned.append(b)
    if cell_kind in BLOCKS and cell_kind not in cleaned:
        cleaned.insert(0, cell_kind)
    if not cleaned:
        cleaned = ["residual"]
    # Cap graph length for overnight tractability
    return cleaned[:4]


class ComposedSeamNet(nn.Module):
    """Compose searchable blocks on seam window vectors (B, L)."""

    def __init__(
        self,
        length: int,
        width: int,
        depth: int,
        act: str,
        cell_kind: str,
        blocks: Sequence[str] | None = None,
    ):
        super().__init__()
        self.length = length
        self.graph = normalize_graph(blocks, cell_kind)
        self.modules_by_name = nn.ModuleDict()
        self.gates = nn.ModuleDict()
        w = max(2, min(48, width))
        d = max(1, min(6, depth))
        for name in self.graph:
            self.gates[name] = SoftOpGate(0.3)
            if name in ("mlp", "residual", "bottleneck", "soft_mix"):
                self.modules_by_name[name] = TinyMLP(length, w, d, act)
            elif name == "dense":
                self.modules_by_name[name] = DenseBlock(length, w, d, act)
            elif name == "gated":
                self.modules_by_name[name] = TinyMLP(length, w, d, act)
            elif name == "unet":
                self.modules_by_name[name] = TinyUNet1D(length, w, act)
            elif name == "conv1d":
                self.modules_by_name[name] = Conv1dStack(length, w, d, act, dilation=False)
            elif name == "dilated":
                self.modules_by_name[name] = Conv1dStack(length, w, d, act, dilation=True)
            elif name == "attn":
                self.modules_by_name[name] = TinyAttention(length, w, act)
            elif name == "dual_path":
                self.modules_by_name[name] = DualPathLite(length, w, act)
            elif name == "tf_split":
                self.modules_by_name[name] = TFSplitLite(length, w, act)
            elif name == "noise_cond":
                self.modules_by_name[name] = NoiseCondResidual(length, w, act)
            else:
                self.modules_by_name[name] = TinyMLP(length, w, d, act)

        # dual branch mix for dual_path cell compatibility
        self.alt = TinyMLP(length, w, 2, act) if "dual_path" in self.graph else None
        self.alt_mix = nn.Parameter(torch.tensor(0.5)) if self.alt is not None else None
        self.res_gate = nn.Parameter(torch.tensor(0.25))

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        h = x
        for name in self.graph:
            mod = self.modules_by_name[name]
            y = mod(h)
            if name in ("residual", "bottleneck", "soft_mix"):
                y = h + torch.tanh(y) * torch.sigmoid(self.res_gate)
            elif name == "gated":
                y = self.gates[name](h, y)
            elif name == "mlp":
                y = h + torch.tanh(y) * torch.sigmoid(self.res_gate)
            else:
                y = self.gates[name](h, y)
            h = y
        if self.alt is not None and self.alt_mix is not None:
            m = torch.sigmoid(self.alt_mix)
            h = m * h + (1 - m) * self.alt(x)
        return h


class TinyAdvHead(nn.Module):
    """Optional tiny discriminator head for auxiliary adversarial loss (not full GAN train)."""

    def __init__(self, length: int, width: int = 16):
        super().__init__()
        h = max(4, min(32, width))
        self.net = nn.Sequential(
            nn.Linear(length, h),
            nn.LeakyReLU(0.2),
            nn.Linear(h, 1),
        )

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.net(x).squeeze(-1)


def random_block_graph(rng, cell_kind: str, max_extra: int = 2) -> list[str]:
    extras = [b for b in BLOCKS if b != cell_kind]
    k = rng.randint(0, max_extra)
    chosen = rng.sample(extras, k=k) if k else []
    return normalize_graph([cell_kind] + chosen, cell_kind)
