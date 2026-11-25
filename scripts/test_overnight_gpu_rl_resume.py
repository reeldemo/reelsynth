#!/usr/bin/env python3
"""Resume smoke: one continuation step after search_state checkpoint.

Runs N=HALF+1 iters continuously vs checkpoint at HALF + --resume for one more step.
Branch/proposal and residuals must match exactly (CPU; CUDA with cudnn deterministic).
"""
from __future__ import annotations

import json
import os
import shutil
import sys
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402

HALF = 4
N = HALF + 1
SEED = 42424242
POP = 4
CKPT_EVERY = 4
FIT = 4
BATCH = 8
TOL_RESIDUAL = 1e-6
TOL_R_BLEND = 1e-6


def resolve_test_device() -> str:
    forced = os.environ.get("DENOISE_RESUME_TEST_DEVICE", "").strip()
    if forced:
        return forced
    if torch.cuda.is_available():
        return "cuda"
    return "cpu"


def history_rows(run_dir: Path) -> list[dict]:
    hist = run_dir / "history.jsonl"
    return [json.loads(ln) for ln in hist.read_text(encoding="utf-8").splitlines() if ln.strip()]


def run_main(argv: list[str]) -> int:
    old = sys.argv
    sys.argv = argv
    try:
        return og.main()
    finally:
        sys.argv = old


def base_argv(run_id: str, iters: int, device: str, *, resume: bool = False) -> list[str]:
    cmd = [
        "overnight_gpu_rl_arch.py",
        "--device",
        device,
        "--seed",
        str(SEED),
        "--pop-size",
        str(POP),
        "--batch",
        str(BATCH),
        "--fit-steps",
        str(FIT),
        "--ckpt-every",
        str(CKPT_EVERY),
        "--history-every",
        "1",
        "--branches",
        "ppo",
        "--plateau-adapt-every",
        "0",
        "--run-id",
        run_id,
        "--iters",
        str(iters),
    ]
    if resume:
        cmd.append("--resume")
    return cmd


def test_resume_one_step() -> None:
    device = resolve_test_device()
    base = ROOT / "brand" / "artifacts" / "models"
    run_cont = f"resume-smoke-cont-{SEED}"
    run_split = f"resume-smoke-split-{SEED}"
    for rid in (run_cont, run_split):
        shutil.rmtree(base / rid, ignore_errors=True)

    assert run_main(base_argv(run_cont, N, device)) == 0
    rows_cont = history_rows(base / run_cont)

    (base / run_split).mkdir(parents=True, exist_ok=True)
    ckpt_dir = base / run_split / "checkpoints"
    ckpt_dir.mkdir(parents=True, exist_ok=True)
    src_ckpt = base / run_cont / "checkpoints" / f"search_state_iter_{HALF:06d}.pt"
    shutil.copy2(src_ckpt, ckpt_dir / f"search_state_iter_{HALF:06d}.pt")
    shutil.copy2(src_ckpt, ckpt_dir / "search_state_latest.pt")
    hist_lines = (base / run_cont / "history.jsonl").read_text(encoding="utf-8").splitlines()
    (base / run_split / "history.jsonl").write_text(
        "\n".join(hist_lines[:HALF]) + "\n", encoding="utf-8"
    )

    assert run_main(base_argv(run_split, N, device, resume=True)) == 0
    rows_split = history_rows(base / run_split)
    assert len(rows_cont) == N == len(rows_split)

    ra, rb = rows_cont[HALF], rows_split[HALF]
    assert ra["iter"] == rb["iter"] == N
    assert ra.get("branch") == rb.get("branch")
    assert ra.get("proposal") == rb.get("proposal")
    for key, tol in (("residual", TOL_RESIDUAL), ("r_blend", TOL_R_BLEND)):
        delta = abs(float(ra[key]) - float(rb[key]))
        assert delta <= tol, (device, key, ra[key], rb[key], delta, tol)

    ckpts = sorted(ckpt_dir.glob("search_state_iter_*.pt"))
    assert len(ckpts) <= 2, len(ckpts)
    print(
        f"OK: resume one-step smoke passed iter={N} device={device} "
        f"tol={TOL_RESIDUAL} ckpts_kept={len(ckpts)}"
    )


if __name__ == "__main__":
    test_resume_one_step()