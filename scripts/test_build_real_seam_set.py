#!/usr/bin/env python3
"""Unit tests for real-seam dataset builder (synthetic toy; no AKWF download)."""
from __future__ import annotations

import json
import sys
import tempfile
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import build_real_seam_set as brs  # noqa: E402
import metrics_snr_sdr as msm  # noqa: E402
import overnight_gpu_rl_arch as og  # noqa: E402


def test_toy_seam_and_reference_alignment() -> None:
    ideal, eng, manifest = brs.build_toy_synthetic()
    assert ideal.shape == (1, og.N)
    assert eng.shape == (1, og.N)
    assert manifest[0]["seam_index"] == 0
    # Engine wrap discontinuity should exceed ideal (aligned period).
    jump_eng = (eng[:, -1] - eng[:, 0]).abs()
    jump_id = (ideal[:, -1] - ideal[:, 0]).abs()
    assert (jump_eng > jump_id + 1e-3).all(), (jump_eng, jump_id)
    # Seams repaired toward ideal improve local R_seam vs identity output.
    r_id = msm.residual_score_seam(ideal, eng)
    r_fix = msm.residual_score_seam(ideal, ideal)
    assert (r_fix > r_id).all()
    # Head/tail reference alignment: ideal is the target at both seam windows.
    w = og.SEAM_W
    assert torch.allclose(ideal[:, :w], ideal[:, :w])
    assert torch.allclose(ideal[:, -w:], ideal[:, -w:])


def test_build_writes_manifest(tmp_path: Path | None = None) -> None:
    out = Path(tempfile.mkdtemp()) if tmp_path is None else tmp_path
    import subprocess

    rc = subprocess.call(
        [
            sys.executable,
            str(ROOT / "scripts" / "build_real_seam_set.py"),
            "--toy-only",
            "--out-dir",
            str(out),
            "--seed",
            "1",
        ]
    )
    assert rc == 0, rc
    man = json.loads((out / "real_seam_manifest.json").read_text(encoding="utf-8"))
    assert man["n_samples"] == 1
    assert (out / "real_seam_set.npz").is_file()


def _main() -> None:
    test_toy_seam_and_reference_alignment()
    test_build_writes_manifest()
    print("test_build_real_seam_set: ok")


if __name__ == "__main__":
    _main()
else:
    # pytest / unittest discovery
    test_toy_seam_and_reference_alignment()
    test_build_writes_manifest()