"""SeamN2N U-Net-lite baseline (parity with ``src/n2n_seam.rs`` embedded weights)."""
from __future__ import annotations

from pathlib import Path

import numpy as np
import torch
import torch.nn as nn

ROOT = Path(__file__).resolve().parents[2]
WEIGHTS_BIN = ROOT / "src" / "n2n_seam_weights.bin"

# Re-export topology from denoise_arch_blocks for a single import path in benches.
import sys

sys.path.insert(0, str(ROOT / "scripts"))
from denoise_arch_blocks import SeamN2NParity  # noqa: E402

class SeamN2N(SeamN2NParity):
    """Default L=256 seam N2N (training / eval geometry)."""

    def __init__(self, length: int = 256, **kwargs):
        super().__init__(length, **kwargs)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        dev = self.wet.device
        if x.device != dev:
            x = x.to(dev)
        return super().forward(x)


def n_params(model: nn.Module) -> int:
    return sum(p.numel() for p in model.parameters())


def _take_floats(buf: np.ndarray, off: int, n: int) -> tuple[np.ndarray, int]:
    return buf[off : off + n], off + n


def load_embedded_weights(model: SeamN2NParity, path: Path | None = None) -> None:
    """Load flat f32 LE dump in the same order as ``src/n2n_seam.rs``."""
    path = path or WEIGHTS_BIN
    raw = path.read_bytes()
    floats = np.frombuffer(raw, dtype=np.float32)
    off = 0
    dev = next(model.parameters()).device
    wet, off = _take_floats(floats, off, 1)
    model.wet.data = torch.tensor(float(wet[0]), dtype=model.wet.dtype, device=dev)

    def w(shape: tuple[int, ...]) -> torch.Tensor:
        nonlocal off
        n = int(np.prod(shape))
        chunk, off = _take_floats(floats, off, n)
        return torch.tensor(chunk, dtype=torch.float32, device=dev).view(*shape)

    c, mc = 32, 8
    model.enc1[0].weight.data = w((c, 1, 5))
    model.enc1[0].bias.data = w((c,))
    model.enc1[2].weight.data = w((c, c, 5))
    model.enc1[2].bias.data = w((c,))
    model.down.weight.data = w((64, c, 4))
    model.down.bias.data = w((64,))
    model.mid[0].weight.data = w((64, 64, 3))
    model.mid[0].bias.data = w((64,))
    model.mid[2].weight.data = w((64, 64, 3))
    model.mid[2].bias.data = w((64,))
    # ConvTranspose1d weight: (in_channels, out_channels, kernel)
    model.up.weight.data = w((64, c, 4))
    model.up.bias.data = w((c,))
    model.dec[0].weight.data = w((c, 64, 3))
    model.dec[0].bias.data = w((c,))
    model.dec[2].weight.data = w((mc, c, 3))
    model.dec[2].bias.data = w((mc,))
    model.dec[4].weight.data = w((1, mc, 3))
    model.dec[4].bias.data = w((1,))
    if off != floats.size:
        raise ValueError(f"weights.bin size mismatch: consumed {off} floats, file has {floats.size}")


def from_state(state: dict, device: torch.device) -> SeamN2NParity:
    model = SeamN2N(256).to(device)
    model.load_state_dict(state)
    model.eval()
    return model


def embedded(device: torch.device) -> SeamN2NParity:
    model = SeamN2N(256).to(device)
    load_embedded_weights(model)
    model.eval()
    return model
