"""Composable seam/cycle architecture blocks for DenoiseOpt NAS.

Lit-inspired families scaled for N=256 residual bake (RTX 3090-safe):
  residual, dense, unet, conv1d, dilated, attn, dual_path, lstm, gated, soft_mix,
  noise_cond, mlp, moe_mix — composed as graphs (depth-biased), not full Demucs/diffusion/GAN.

Mixture modes:
  sequential — chain blocks with residual/soft gates (default)
  moe_parallel — MoE-style soft gates over heterogeneous parallel experts (Shazeer-inspired, tiny)
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
    "lstm",
    "xlstm",
    "tf_split",
    "noise_cond",
    "soft_mix",
    "moe_mix",
]

# Keep CELL_KINDS as primary cell mode; graphs compose additional blocks.
CELL_KINDS = list(BLOCKS)

# Depth / mixture search caps (aligned with denoise_meta_evo).
# Mutable at runtime via raise_search_caps (plateau adapt); hard caps keep RTX 3090 trainable.
MAX_SEARCH_DEPTH = 12
MAX_GRAPH_LEN = 6
MAX_WIDTH = 48
HARD_MAX_DEPTH = 20
HARD_MAX_GRAPH = 8
HARD_MAX_WIDTH = 56
MOE_MODES = ("sequential", "moe_parallel")


def get_search_caps() -> dict[str, int]:
    return {
        "max_search_depth": MAX_SEARCH_DEPTH,
        "max_graph_len": MAX_GRAPH_LEN,
        "max_width": MAX_WIDTH,
        "hard_max_depth": HARD_MAX_DEPTH,
        "hard_max_graph": HARD_MAX_GRAPH,
        "hard_max_width": HARD_MAX_WIDTH,
    }


def set_search_caps(
    *,
    depth: int | None = None,
    graph: int | None = None,
    width: int | None = None,
) -> dict[str, int]:
    """Set live search ceilings (clamped to hard 3090-safe caps)."""
    global MAX_SEARCH_DEPTH, MAX_GRAPH_LEN, MAX_WIDTH
    if depth is not None:
        MAX_SEARCH_DEPTH = max(1, min(HARD_MAX_DEPTH, int(depth)))
    if graph is not None:
        MAX_GRAPH_LEN = max(1, min(HARD_MAX_GRAPH, int(graph)))
    if width is not None:
        MAX_WIDTH = max(4, min(HARD_MAX_WIDTH, int(width)))
    return get_search_caps()


def raise_search_caps(
    *,
    depth_delta: int = 2,
    graph_delta: int = 1,
    width_delta: int = 4,
) -> dict[str, int]:
    """Escalate ceilings — depth first, width only moderately."""
    return set_search_caps(
        depth=MAX_SEARCH_DEPTH + max(0, depth_delta),
        graph=MAX_GRAPH_LEN + max(0, graph_delta),
        width=MAX_WIDTH + max(0, width_delta),
    )


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
    """Tiny 1D U-Net on length-L vectors (channel=1). Wave-U-Net inspired; depth → more stages.

    depth <= 12 keeps the legacy 2-level encoder (warm-start compatible).
    depth > 12 unlocks a 3rd stage (plateau deepen).
    """

    def __init__(self, length: int, width: int, act: str, depth: int = 3):
        super().__init__()
        self.act = _act_module(act)
        self.length = length
        c = max(4, min(16, width // 2))
        # Legacy path (matches pre-plateau checkpoints at depth<=12)
        if depth <= 12:
            self.n_stages = 2
            self.legacy = True
            self.enc1 = nn.Conv1d(1, c, 3, padding=1)
            self.enc2 = nn.Conv1d(c, c * 2, 3, stride=2, padding=1)
            self.bot = nn.Conv1d(c * 2, c * 2, 3, padding=1)
            self.up = nn.ConvTranspose1d(c * 2, c, 4, stride=2, padding=1)
            self.dec = nn.Conv1d(c * 2, 1, 3, padding=1)
            return
        # Plateau deepen: 3-stage U-Net
        self.legacy = False
        self.n_stages = 3
        self.encoders = nn.ModuleList()
        self.downs = nn.ModuleList()
        in_c = 1
        ch = c
        for i in range(self.n_stages):
            self.encoders.append(nn.Conv1d(in_c, ch, 3, padding=1))
            if i < self.n_stages - 1:
                self.downs.append(nn.Conv1d(ch, ch * 2, 3, stride=2, padding=1))
                in_c = ch * 2
                ch = ch * 2
            else:
                in_c = ch
        self.bot = nn.Conv1d(ch, ch, 3, padding=1)
        self.ups = nn.ModuleList()
        self.decs = nn.ModuleList()
        for i in range(self.n_stages - 1):
            out_c = max(c, ch // 2)
            self.ups.append(nn.ConvTranspose1d(ch, out_c, 4, stride=2, padding=1))
            self.decs.append(nn.Conv1d(out_c * 2, out_c, 3, padding=1))
            ch = out_c
        self.out = nn.Conv1d(ch, 1, 3, padding=1)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        if getattr(self, "legacy", False):
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
                y = F.interpolate(
                    y.unsqueeze(1), size=x.shape[-1], mode="linear", align_corners=False
                ).squeeze(1)
            return y
        h = x.unsqueeze(1)
        skips: list[torch.Tensor] = []
        for i, enc in enumerate(self.encoders):
            h = self.act(enc(h))
            skips.append(h)
            if i < len(self.downs):
                h = self.act(self.downs[i](h))
        h = self.act(self.bot(h))
        for i, up in enumerate(self.ups):
            h = up(h)
            skip = skips[-(i + 2)]
            if h.shape[-1] != skip.shape[-1]:
                h = F.interpolate(h, size=skip.shape[-1], mode="linear", align_corners=False)
            h = self.act(self.decs[i](torch.cat([h, skip], dim=1)))
        y = self.out(h).squeeze(1)
        if y.shape[-1] != x.shape[-1]:
            y = F.interpolate(
                y.unsqueeze(1), size=x.shape[-1], mode="linear", align_corners=False
            ).squeeze(1)
        return y


class Conv1dStack(nn.Module):
    def __init__(self, length: int, width: int, depth: int, act: str, dilation: bool = False):
        super().__init__()
        c = max(4, min(24, width))
        layers: list[nn.Module] = []
        in_c = 1
        # Legacy cap 4 at depth<=12; allow up to 8 after plateau deepen
        n_layers = max(1, min(8 if depth > 12 else 4, depth))
        for i in range(n_layers):
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
    """Lightweight self-attention over L tokens with tiny dim; depth>12 deepens FF stack."""

    def __init__(self, length: int, width: int, act: str, depth: int = 2):
        super().__init__()
        d = max(4, min(32, width))
        n_heads = 2 if d % 2 == 0 else 1
        self.proj_in = nn.Linear(1, d)
        self.attn = nn.MultiheadAttention(d, n_heads, batch_first=True)
        ff_depth = 1 if depth <= 12 else max(1, min(4, depth // 2))
        ff_layers: list[nn.Module] = []
        for i in range(ff_depth):
            ff_layers.append(nn.Linear(d, d))
            ff_layers.append(_act_module(act))
        ff_layers.append(nn.Linear(d, 1))
        self.ff = nn.Sequential(*ff_layers)
        self.length = length

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        # (B, L) -> (B, L, 1)
        h = self.proj_in(x.unsqueeze(-1))
        a, _ = self.attn(h, h, h, need_weights=False)
        y = self.ff(a + h).squeeze(-1)
        return y


class DualPathLite(nn.Module):
    """DPRNN-inspired: split length into chunks, local MLP + global MLP (depth>12 → deeper MLPs)."""

    def __init__(self, length: int, width: int, act: str, depth: int = 2):
        super().__init__()
        self.chunk = max(2, min(8, length // 2))
        h = max(4, min(32, width))
        mlp_d = 2 if depth <= 12 else max(2, min(6, depth))
        self.local = TinyMLP(self.chunk, h, mlp_d, act)
        self.global_mix = TinyMLP(
            max(1, length // self.chunk), h, mlp_d, act, out_d=max(1, length // self.chunk)
        )
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


class TinyLSTMLite(nn.Module):
    """Lightweight 1–2 layer LSTM over length-L tokens; width-capped for N=256 bake safety.

    Distinct from the fixed supervised seq-LSTM SOTA baseline: this is a searchable
    bake-cell block inside ComposedSeamNet graphs (seam window or full cycle).
    """

    def __init__(self, length: int, width: int, act: str, depth: int = 2):
        super().__init__()
        self.length = length
        h = max(4, min(24, width))
        n_layers = 1 if depth <= 6 else 2
        self.lstm = nn.LSTM(
            input_size=1,
            hidden_size=h,
            num_layers=n_layers,
            batch_first=True,
            bidirectional=False,
        )
        self.proj = nn.Sequential(nn.Linear(h, h), _act_module(act), nn.Linear(h, 1))
        self.gate = nn.Parameter(torch.tensor(0.3))

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        # (B, L) -> (B, L, 1)
        h, _ = self.lstm(x.unsqueeze(-1))
        y = self.proj(h).squeeze(-1)
        g = torch.sigmoid(self.gate)
        return g * y + (1.0 - g) * x


class TinyXLSTMLite(nn.Module):
    """Lightweight xLSTM-inspired recurrent block (Beck et al. spirit, not full stack).

    Exponential input/forget gating + stabilized scalar cell state over length L.
    Searchable bake-cell proxy; width-capped for N=256 / RTX-safe graphs.
    """

    def __init__(self, length: int, width: int, act: str, depth: int = 2):
        super().__init__()
        self.length = length
        h = max(4, min(20, width))
        self.h = h
        # Per-step projections from scalar token -> gates / candidate
        self.in_proj = nn.Linear(1, 4 * h)
        self.out_proj = nn.Sequential(nn.Linear(h, h), _act_module(act), nn.Linear(h, 1))
        self.mix = nn.Parameter(torch.tensor(0.3))
        # Depth>8: second residual MLP polish
        self.deep = TinyMLP(length, h, 2, act) if depth > 8 else None

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        # x: (B, L)
        b, l = x.shape
        h = self.h
        device, dtype = x.device, x.dtype
        c = torch.zeros(b, h, device=device, dtype=dtype)
        n = torch.zeros(b, h, device=device, dtype=dtype)  # normalizer (mLSTM-lite)
        outs = []
        seq = x.unsqueeze(-1)  # (B, L, 1)
        for t in range(l):
            gates = self.in_proj(seq[:, t, :])  # (B, 4h)
            i, f, o, z = gates.chunk(4, dim=-1)
            # Exponential gates (stabilized)
            i = torch.exp(torch.clamp(i, -8.0, 8.0))
            f = torch.sigmoid(f)  # keep forget in (0,1) for bake stability
            o = torch.sigmoid(o)
            z = torch.tanh(z)
            c = f * c + i * z
            n = f * n + i
            h_t = o * (c / n.clamp_min(1e-3))
            outs.append(h_t)
        h_seq = torch.stack(outs, dim=1)  # (B, L, h)
        y = self.out_proj(h_seq).squeeze(-1)
        if self.deep is not None:
            y = 0.5 * y + 0.5 * self.deep(y)
        g = torch.sigmoid(self.mix)
        return g * y + (1.0 - g) * x


class TFSplitLite(nn.Module):
    """Cheap time/freq style split; depth>12 deepens branch MLPs."""

    def __init__(self, length: int, width: int, act: str, depth: int = 2):
        super().__init__()
        h = max(4, min(24, width))
        mlp_d = 2 if depth <= 12 else max(2, min(6, depth))
        self.time_mlp = TinyMLP(length, h, mlp_d, act)
        self.freq_mlp = TinyMLP(length, h, mlp_d, act)
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
    """Noise-conditioned residual; depth>12 deepens the conditioned MLP stack."""

    def __init__(self, length: int, width: int, act: str, depth: int = 2):
        super().__init__()
        h = max(4, min(32, width))
        mlp_d = 2 if depth <= 12 else max(2, min(6, depth))
        self.mlp = TinyMLP(length + 1, h, mlp_d, act, out_d=length)
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
    """Build a composable graph: primary cell + optional extras (depth/mixture biased)."""
    raw = list(blocks) if blocks else []
    cleaned: list[str] = []
    for b in raw:
        if b in BLOCKS and b not in cleaned:
            cleaned.append(b)
    if cell_kind in BLOCKS and cell_kind not in cleaned:
        cleaned.insert(0, cell_kind)
    if not cleaned:
        cleaned = ["residual"]
    return cleaned[:MAX_GRAPH_LEN]


def _make_block(name: str, length: int, w: int, d: int, act: str) -> nn.Module:
    if name in ("mlp", "residual", "bottleneck", "soft_mix", "moe_mix"):
        return TinyMLP(length, w, d, act)
    if name == "dense":
        return DenseBlock(length, w, d, act)
    if name == "gated":
        return TinyMLP(length, w, d, act)
    if name == "unet":
        return TinyUNet1D(length, w, act, depth=d)
    if name == "conv1d":
        return Conv1dStack(length, w, d, act, dilation=False)
    if name == "dilated":
        return Conv1dStack(length, w, d, act, dilation=True)
    if name == "attn":
        return TinyAttention(length, w, act, depth=d)
    if name == "dual_path":
        return DualPathLite(length, w, act, depth=d)
    if name == "lstm":
        return TinyLSTMLite(length, w, act, depth=d)
    if name == "xlstm":
        return TinyXLSTMLite(length, w, act, depth=d)
    if name == "tf_split":
        return TFSplitLite(length, w, act, depth=d)
    if name == "noise_cond":
        return NoiseCondResidual(length, w, act, depth=d)
    return TinyMLP(length, w, d, act)


class MoESoftGate(nn.Module):
    """Tiny MoE soft mixture over parallel expert outputs (Shazeer-inspired, not full MoE)."""

    def __init__(self, n_experts: int, length: int):
        super().__init__()
        self.n_experts = max(1, n_experts)
        # Input-conditioned gate: mean-pool → logits
        self.gate = nn.Linear(length, self.n_experts)
        self.temp = nn.Parameter(torch.tensor(1.0))

    def forward(self, x: torch.Tensor, expert_outs: list[torch.Tensor]) -> torch.Tensor:
        logits = self.gate(x) / self.temp.clamp_min(0.1).abs()
        w = F.softmax(logits, dim=-1)
        stacked = torch.stack(expert_outs, dim=-1)  # (B, L, E)
        return (stacked * w.unsqueeze(1)).sum(dim=-1)


class ComposedSeamNet(nn.Module):
    """Compose searchable blocks on seam window vectors (B, L).

    moe_mode:
      sequential — chain with residual/soft gates
      moe_parallel — run heterogeneous experts in parallel, soft-gate mix
    """

    def __init__(
        self,
        length: int,
        width: int,
        depth: int,
        act: str,
        cell_kind: str,
        blocks: Sequence[str] | None = None,
        moe_mode: str = "sequential",
    ):
        super().__init__()
        self.length = length
        self.graph = normalize_graph(blocks, cell_kind)
        self.moe_mode = moe_mode if moe_mode in MOE_MODES else "sequential"
        self.modules_by_name = nn.ModuleDict()
        self.gates = nn.ModuleDict()
        w = max(2, min(MAX_WIDTH, width))
        d = max(1, min(MAX_SEARCH_DEPTH, depth))
        for name in self.graph:
            self.gates[name] = SoftOpGate(0.3)
            self.modules_by_name[name] = _make_block(name, length, w, d, act)

        # dual branch mix for dual_path cell compatibility
        self.alt = TinyMLP(length, w, 2, act) if "dual_path" in self.graph else None
        self.alt_mix = nn.Parameter(torch.tensor(0.5)) if self.alt is not None else None
        self.res_gate = nn.Parameter(torch.tensor(0.25))
        self.moe: MoESoftGate | None
        if self.moe_mode == "moe_parallel" and len(self.graph) >= 2:
            self.moe = MoESoftGate(len(self.graph), length)
        else:
            self.moe = None
            if self.moe_mode == "moe_parallel":
                self.moe_mode = "sequential"

    def _apply_one(self, name: str, h: torch.Tensor) -> torch.Tensor:
        mod = self.modules_by_name[name]
        y = mod(h)
        if name in ("residual", "bottleneck", "soft_mix", "moe_mix"):
            return h + torch.tanh(y) * torch.sigmoid(self.res_gate)
        if name in ("gated", "mlp"):
            if name == "mlp":
                return h + torch.tanh(y) * torch.sigmoid(self.res_gate)
            return self.gates[name](h, y)
        return self.gates[name](h, y)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        if self.moe is not None and self.moe_mode == "moe_parallel":
            # Parallel heterogeneous experts from identity input, soft-gated mix
            outs = [self._apply_one(name, x) for name in self.graph]
            h = self.moe(x, outs)
        else:
            h = x
            for name in self.graph:
                h = self._apply_one(name, h)
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


def random_block_graph(
    rng,
    cell_kind: str,
    max_extra: int = 3,
    *,
    crazy_mix_p: float = 0.35,
) -> list[str]:
    extras = [b for b in BLOCKS if b != cell_kind]
    k = rng.randint(0, min(max_extra, MAX_GRAPH_LEN - 1))
    chosen = rng.sample(extras, k=k) if k else []
    # Bias toward heterogeneous mixtures (unet+attn+dilated+noise_cond style)
    if k >= 2 and rng.random() < crazy_mix_p:
        prefer = [
            b
            for b in (
                "unet",
                "attn",
                "lstm",
                "xlstm",
                "dilated",
                "dual_path",
                "dense",
                "moe_mix",
                "noise_cond",
                "soft_mix",
            )
            if b != cell_kind
        ]
        if prefer:
            chosen = list(dict.fromkeys(prefer[: max(2, k)] + chosen))[:max_extra]
    return normalize_graph([cell_kind] + chosen, cell_kind)


def random_moe_mode(rng, *, moe_p: float = 0.35) -> str:
    return "moe_parallel" if rng.random() < moe_p else "sequential"
