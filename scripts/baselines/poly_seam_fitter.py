"""Seam-local polynomial fitter (classical, no learning) -- Phase F2.1.

New implementation (2026-11-03) of the interface specified in
denoise-opt-meta@fcbc213 PEER_REVIEW_IMPROVEMENT_PLAN Task F2.1:

    fit_poly_seam(eng: Tensor[B, L], degree: int = 3, seam_w: int = 8) -> Tensor[B, L]

"Fit low-degree polynomial (or linear endpoint ramp) on seam neighborhoods
only; leave mid-cycle untouched; optional global DC match."

Interpretation (see PR text for the ambiguity notes):
  * The seam neighborhoods are the head ``[0:seam_w]`` and tail
    ``[L-seam_w:L]`` windows, the same windows used by ``edge_rmse`` and the
    other seam operators in ``overnight_gpu_rl_arch``.
  * Each window is replaced by its ordinary least-squares polynomial fit of
    the given degree (per row, per window, independently). This is a linear
    projection onto the polynomial subspace, so an exact polynomial of degree
    <= ``degree`` is returned unchanged and noise is attenuated.
  * Samples ``[seam_w:L-seam_w]`` are returned bit-identical.
  * ``dc_match=True`` (off by default) shifts the whole output so its per-row
    mean equals the input mean.
"""
from __future__ import annotations

import torch

__all__ = ["fit_poly_seam", "poly_projection"]


def poly_projection(n: int, degree: int, *, dtype=torch.float64, device=None) -> torch.Tensor:
    """Return the (n, n) least-squares projection onto polynomials of ``degree``.

    Abscissae are equispaced on [-1, 1] for conditioning; projection is
    invariant to the affine choice of abscissae.
    """
    if n < 1:
        raise ValueError(f"window length must be >= 1, got {n}")
    if degree < 0:
        raise ValueError(f"degree must be >= 0, got {degree}")
    if degree + 1 > n:
        raise ValueError(f"degree={degree} needs >= {degree + 1} samples per window, got {n}")
    x = torch.linspace(-1.0, 1.0, n, dtype=dtype, device=device) if n > 1 else torch.zeros(1, dtype=dtype, device=device)
    v = torch.stack([x**k for k in range(degree + 1)], dim=1)  # (n, d+1)
    q, _ = torch.linalg.qr(v, mode="reduced")
    return q @ q.T


def fit_poly_seam(
    eng: torch.Tensor,
    degree: int = 3,
    seam_w: int = 8,
    *,
    dc_match: bool = False,
) -> torch.Tensor:
    if eng.dim() != 2:
        raise ValueError(f"expected eng of shape [B, L], got {tuple(eng.shape)}")
    if not torch.is_floating_point(eng):
        raise TypeError("eng must be a floating-point tensor")
    w = int(seam_w)
    n = eng.shape[1]
    if w < 1:
        raise ValueError(f"seam_w must be >= 1, got {seam_w}")
    if 2 * w > n:
        raise ValueError(f"seam_w={w} too large for L={n} (need 2*seam_w <= L)")
    p = poly_projection(w, int(degree), device=eng.device)  # validates degree
    x64 = eng.to(torch.float64)
    out = x64.clone()
    out[:, :w] = x64[:, :w] @ p.T
    out[:, n - w :] = x64[:, n - w :] @ p.T
    if dc_match:
        out = out + (x64.mean(dim=1, keepdim=True) - out.mean(dim=1, keepdim=True))
    out = out.to(eng.dtype)
    if not dc_match:
        out[:, w : n - w] = eng[:, w : n - w]  # mid-cycle bit-identical
    return out
