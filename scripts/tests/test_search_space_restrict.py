#!/usr/bin/env python3
"""CPU smoke: --cell-kinds / --extra-blocks search-space restriction flags."""
from __future__ import annotations

import json
import random
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPTS = ROOT / "scripts"
sys.path.insert(0, str(SCRIPTS))

import denoise_arch_blocks as arch_blocks  # noqa: E402
from denoise_arch_blocks import BLOCKS  # noqa: E402
import overnight_gpu_rl_arch as og  # noqa: E402
from overnight_gpu_rl_arch import (  # noqa: E402
    configure_search_space_flags,
    mutate_arch,
    random_arch,
    N_ACTIONS,
)

GOLDEN_PATH = Path(__file__).with_name("golden_search_space_default.json")


def _reset_default_search_space() -> None:
    arch_blocks.set_search_caps(depth=12, graph=6, width=48)
    full = list(BLOCKS)
    og.CELL_KINDS = full
    arch_blocks.CELL_KINDS = full
    og.GRAPH_BLOCKS_ALLOWED = None
    assert configure_search_space_flags("", "") == 0


def _draw_sequence(rng: random.Random, n: int) -> list:
    out: list = []
    for i in range(n):
        if i % 2 == 0:
            a = random_arch(rng)
            out.append(["random", a.cell_kind, list(a.blocks)])
        else:
            base = random_arch(rng)
            action = rng.randrange(N_ACTIONS)
            a = mutate_arch(base, action, rng)
            out.append(["mutate", action, a.cell_kind, list(a.blocks)])
    return out


def test_default_draws_match_golden() -> None:
    _reset_default_search_space()
    golden = json.loads(GOLDEN_PATH.read_text(encoding="utf-8"))
    rng = random.Random(424242)
    got = _draw_sequence(rng, 200)
    assert got == golden, "unrestricted draws diverged from golden (bit-identical regression)"


def test_residual_cell_kinds_only() -> None:
    assert configure_search_space_flags("residual", "") == 0
    assert og.CELL_KINDS == ["residual"]
    pool = set(BLOCKS if og.GRAPH_BLOCKS_ALLOWED is None else og.GRAPH_BLOCKS_ALLOWED)
    rng = random.Random(515151)
    for i in range(500):
        if i % 2 == 0:
            a = random_arch(rng)
        else:
            a = mutate_arch(random_arch(rng), rng.randrange(N_ACTIONS), rng)
        assert a.cell_kind == "residual", a
        assert all(b in pool for b in a.blocks), a.blocks


def test_invalid_cell_kind_exits_2() -> None:
    proc = subprocess.run(
        [sys.executable, str(SCRIPTS / "overnight_gpu_rl_arch.py"), "--cell-kinds", "not_a_block"],
        cwd=str(ROOT),
        capture_output=True,
        text=True,
    )
    assert proc.returncode == 2, proc.stdout + proc.stderr


def main() -> int:
    test_default_draws_match_golden()
    test_residual_cell_kinds_only()
    test_invalid_cell_kind_exits_2()
    print("OK: test_search_space_restrict passed")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())