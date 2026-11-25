#!/usr/bin/env python3
"""Smoke: train_n2n_seam_baseline --train-seed and --out-dir (multi-seed runs)."""
from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SCRIPT = ROOT / "scripts" / "train_n2n_seam_baseline.py"
DEFAULT_SEED = 424242


def run_train(seed: int, out: Path) -> dict:
    env = {**os.environ, "PYTHONPATH": str(ROOT / "scripts")}
    cmd = [
        sys.executable,
        str(SCRIPT),
        "--device",
        "cpu",
        "--steps",
        "4",
        "--batch",
        "8",
        "--modes",
        "n2n_corrupt_corrupt",
        "--train-seed",
        str(seed),
        "--out-dir",
        str(out),
    ]
    subprocess.check_call(cmd, cwd=ROOT, env=env)
    blob = torch_load(out / "n2n_corrupt_corrupt.pt")
    assert blob["train_seed"] == seed
    summary = json.loads((out / "n2n_baseline.json").read_text(encoding="utf-8"))
    assert summary["meta"]["train_seed"] == seed
    assert summary["meta"]["out_dir"] == str(out)
    return blob


def torch_load(path: Path) -> dict:
    import torch

    return torch.load(path, map_location="cpu", weights_only=False)


def test_default_seed_unchanged() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "default"
        blob = run_train(DEFAULT_SEED, out)
        assert blob["train_seed"] == DEFAULT_SEED


def test_three_seeds_disjoint_outputs() -> None:
    seeds = [424242, 424243, 424244]
    losses: dict[int, float] = {}
    with tempfile.TemporaryDirectory() as tmp:
        base = Path(tmp)
        for seed in seeds:
            out = base / f"seed_{seed}"
            shutil.rmtree(out, ignore_errors=True)
            blob = run_train(seed, out)
            losses[seed] = float(blob["loss_first"] or 0.0)
            assert (out / "n2n_corrupt_corrupt.pt").is_file()
        # Different train seeds should not emit identical first-step loss.
        assert len(set(losses.values())) >= 2, losses
    print(f"OK: train_n2n --train-seed + --out-dir seeds={seeds} losses={losses}")


if __name__ == "__main__":
    test_default_seed_unchanged()
    test_three_seeds_disjoint_outputs()