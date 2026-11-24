#!/usr/bin/env python3
"""Resume smoke (CPU): one continuation step after search_state checkpoint.

Runs N=HALF+1 iters continuously vs HALF iters + --resume for one more step from the
same run's checkpoint. Branch/proposal must match; ``residual``/``r_blend`` within
``TOL`` (FitCell may differ slightly across process restarts on CPU). Full N-step
identity is not required after the first resumed step because training noise compounds.
"""
from __future__ import annotations

import json
import shutil
import sys
from pathlib import Path

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
TOL = 2e-2


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


def base_argv(run_id: str, iters: int, *, resume: bool = False) -> list[str]:
    cmd = [
        "overnight_gpu_rl_arch.py",
        "--device",
        "cpu",
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
    base = ROOT / "brand" / "artifacts" / "models"
    run_cont = f"resume-smoke-cont-{SEED}"
    run_split = f"resume-smoke-split-{SEED}"
    for rid in (run_cont, run_split):
        shutil.rmtree(base / rid, ignore_errors=True)

    assert run_main(base_argv(run_cont, N)) == 0
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

    assert run_main(base_argv(run_split, N, resume=True)) == 0
    rows_split = history_rows(base / run_split)
    assert len(rows_cont) == N == len(rows_split)

    ra, rb = rows_cont[HALF], rows_split[HALF]
    assert ra["iter"] == rb["iter"] == N
    assert ra.get("branch") == rb.get("branch")
    assert ra.get("proposal") == rb.get("proposal")
    for key in ("residual", "r_blend"):
        assert abs(float(ra[key]) - float(rb[key])) <= TOL, (key, ra[key], rb[key])

    ckpts = sorted(ckpt_dir.glob("search_state_iter_*.pt"))
    assert len(ckpts) <= 2, len(ckpts)
    print(f"OK: resume one-step smoke passed iter={N} tol={TOL} ckpts_kept={len(ckpts)}")


if __name__ == "__main__":
    test_resume_one_step()