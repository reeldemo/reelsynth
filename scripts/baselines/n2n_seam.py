"""SeamN2N baseline U-Net for wavetable wrap denoise (N2N corrupt→corrupt)."""
from __future__ import annotations

import sys
from pathlib import Path

import torch
import torch.nn as nn

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from denoise_arch_blocks import SeamN2NParity  # noqa: E402

DEFAULT_LENGTH = 256


class SeamN2N(SeamN2NParity):
    """Public baseline API; topology matches ``SeamN2NParity`` / Rust ``n2n_seam``."""

    def __init__(self, length: int = DEFAULT_LENGTH, **_kwargs: object):
        super().__init__(length)

    @classmethod
    def from_state(cls, state_dict: dict, device: torch.device | str) -> SeamN2N:
        m = cls()
        m.load_state_dict(state_dict)
        m.to(device)
        m.eval()
        return m


def n_params(model: nn.Module) -> int:
    return sum(p.numel() for p in model.parameters())