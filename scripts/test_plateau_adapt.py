#!/usr/bin/env python3
"""Smoke: plateau adapt deepens caps + pop genomes; logs deeper=true."""
from __future__ import annotations

import random
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import denoise_arch_blocks as arch_blocks  # noqa: E402
from overnight_gpu_rl_arch import (  # noqa: E402
    ArchConfig,
    HyperParams,
    Individual,
    PlateauAdaptState,
    apply_plateau_adapt,
    deepen_arch_inplace,
)


def main() -> int:
    # Reset caps to defaults for deterministic smoke
    arch_blocks.set_search_caps(depth=12, graph=6, width=48)
    assert arch_blocks.MAX_SEARCH_DEPTH == 12

    rng = random.Random(0)
    pop = [
        Individual(
            cfg=ArchConfig(depth=10, width=16, blocks=["residual", "unet"]),
            hp=HyperParams(),
            score=0.9,
        )
        for _ in range(4)
    ]
    adapt = PlateauAdaptState()
    ev = apply_plateau_adapt(adapt, pop, rng, it=1000, max_level=4)
    assert ev["plateau_adapt"] is True
    assert ev["deeper"] is True
    assert ev["depth_cap"] >= 14, ev
    assert arch_blocks.MAX_SEARCH_DEPTH >= 14
    assert adapt.level == 1
    assert adapt.soft_boredom == 0
    assert all(p.cfg.depth >= 12 for p in pop), [p.cfg.depth for p in pop]

    # Second escalate
    adapt.soft_boredom = 1000
    ev2 = apply_plateau_adapt(adapt, pop, rng, it=2000, max_level=4)
    assert ev2["level"] == 2
    assert arch_blocks.MAX_SEARCH_DEPTH >= 16

    # deepen helper
    c = deepen_arch_inplace(ArchConfig(depth=12, blocks=["mlp"]), bump=3)
    assert c.depth == min(arch_blocks.MAX_SEARCH_DEPTH, 15)

    # Depth-aware U-Net stages (legacy @<=12, deepen @>12)
    u_shallow = arch_blocks.TinyUNet1D(16, 8, "gelu", depth=2)
    u_legacy = arch_blocks.TinyUNet1D(16, 8, "gelu", depth=12)
    u_deep = arch_blocks.TinyUNet1D(16, 8, "gelu", depth=14)
    assert u_shallow.n_stages == 2 and u_shallow.legacy
    assert u_legacy.n_stages == 2 and u_legacy.legacy
    assert u_deep.n_stages == 3 and not u_deep.legacy

    import torch

    x = torch.randn(2, 16)
    y = u_deep(x)
    assert y.shape == x.shape
    assert torch.isfinite(y).all()
    y2 = u_legacy(x)
    assert y2.shape == x.shape

    print(
        f"OK: plateau adapt deepen smoke passed "
        f"depth_cap={arch_blocks.MAX_SEARCH_DEPTH} "
        f"graph={arch_blocks.MAX_GRAPH_LEN} level={adapt.level}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
