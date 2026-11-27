"""Endpoint-pin / hard wrap-closure control (classical) -- Phase F3.1.

New implementation (2026-11-03) of denoise-opt-meta@fcbc213 Task F3.1:
"pin/crossfade endpoints to minimize |y_0 - y_{L-1}|".

    endpoint_pin(x: Tensor[B, L], seam_w: int = 8, mode: str = "mean") -> Tensor[B, L]

A target value ``t`` per row is chosen by ``mode``:
  * ``"mean"``: t = (x_0 + x_{L-1}) / 2  (meet in the middle)
  * ``"zero"``: t = 0                     (pin both endpoints to zero)
The offsets ``t - x_0`` and ``t - x_{L-1}`` are added with linear crossfade
weights over the head/tail windows: head index i gets weight (1 - i/w), tail
index L-w+i gets weight (i+1)/w. Hence y_0 = y_{L-1} = t exactly (wrap jump
0), the correction fades to 0 toward mid-cycle, and samples
``[seam_w:L-seam_w]`` are untouched. Ideal endpoints are not available to the
benches (they only pass the engine signal), so the "match ideal endpoints"
variant mentioned in the plan is not implemented.
"""
from __future__ import annotations

import torch

__all__ = ["endpoint_pin"]

_MODES = ("mean", "zero")


def endpoint_pin(x: torch.Tensor, seam_w: int = 8, mode: str = "mean") -> torch.Tensor:
    if x.dim() != 2:
        raise ValueError(f"expected x of shape [B, L], got {tuple(x.shape)}")
    if not torch.is_floating_point(x):
        raise TypeError("x must be a floating-point tensor")
    if mode not in _MODES:
        raise ValueError(f"mode must be one of {_MODES}, got {mode!r}")
    w = int(seam_w)
    n = x.shape[1]
    if w < 1:
        raise ValueError(f"seam_w must be >= 1, got {seam_w}")
    if 2 * w > n:
        raise ValueError(f"seam_w={w} too large for L={n} (need 2*seam_w <= L)")
    x64 = x.to(torch.float64)
    first, last = x64[:, :1], x64[:, -1:]
    target = 0.5 * (first + last) if mode == "mean" else torch.zeros_like(first)
    i = torch.arange(w, dtype=torch.float64, device=x.device)
    w_head = (1.0 - i / w).unsqueeze(0)
    w_tail = ((i + 1.0) / w).unsqueeze(0)
    out = x64.clone()
    out[:, :w] = out[:, :w] + (target - first) * w_head
    out[:, n - w :] = out[:, n - w :] + (target - last) * w_tail
    out[:, 0:1] = target  # exact pin, no rounding drift
    out[:, n - 1 : n] = target
    res = out.to(x.dtype)
    res[:, w : n - w] = x[:, w : n - w]
    return res
