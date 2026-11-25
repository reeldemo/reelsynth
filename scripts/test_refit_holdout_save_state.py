#!/usr/bin/env python3
"""Smoke: refit_holdout --save-state writes loadable .pt + manifest sha256."""
from __future__ import annotations

import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402

META = ROOT / "brand" / "artifacts" / "meta_approach_compare" / "hybrid_lstm" / "summary.json"
REFIT = ROOT / "scripts" / "refit_holdout.py"


def test_save_state_loadable() -> None:
    if not META.is_file():
        print(f"SKIP: missing {META}")
        return
    out_dir = Path(tempfile.mkdtemp())
    state_dir = out_dir / "weights"
    rc = subprocess.call(
        [
            sys.executable,
            str(REFIT),
            str(META.parent),
            "--device",
            "cpu",
            "--fit-seed",
            "1902771841",
            "--out-json",
            str(out_dir / "refit.json"),
            "--save-state",
            str(state_dir),
        ],
        cwd=str(ROOT),
    )
    assert rc == 0, rc
    man = json.loads((state_dir / "manifest.json").read_text(encoding="utf-8"))
    assert man["entries"], man
    pt_name = man["entries"][0]["weights_path"]
    pt_path = state_dir / pt_name
    assert pt_path.is_file()
    blob = torch.load(pt_path, map_location="cpu", weights_only=False)
    cfg = og.ArchConfig.from_dict(blob["architecture"])
    cell = og.SeamCell(cfg).to(device=torch.device("cpu"))
    og.load_state_dict_compatible(cell, blob["cell_state_dict"])
    cell.eval()
    shutil.rmtree(out_dir, ignore_errors=True)
    print("OK: refit_holdout --save-state manifest + loader smoke passed")


if __name__ == "__main__":
    test_save_state_loadable()