#!/usr/bin/env python3
"""Launch meta-approach 5k bench with a single-instance lock (crash-safe resume).

Writes brand/artifacts/meta_approach_compare/bench.lock with PID.
Refuses to start a second copy while the lock holder is alive.

Publishable clean run:
  python scripts/launch_meta_approach_compare.py --iters 5000 --force --fresh
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "brand" / "artifacts" / "meta_approach_compare"
LOCK = OUT / "bench.lock"
PIDFILE = OUT / "bench.pid"
PY = ROOT / ".venv_gpu" / "Scripts" / "python.exe"
SCRIPT = ROOT / "scripts" / "bench_meta_approaches_5k.py"
PUBLISH_SEED = 1_902_771_841
APPROACHES = ("random", "cmaes", "reinforce", "aging_evo", "tpe", "hybrid_lstm")


def pid_alive(pid: int) -> bool:
    try:
        if os.name == "nt":
            import ctypes

            k = ctypes.windll.kernel32
            h = k.OpenProcess(0x1000, False, int(pid))
            if not h:
                return False
            k.CloseHandle(h)
            return True
        os.kill(pid, 0)
        return True
    except Exception:
        return False


def read_lock_pid() -> int | None:
    for path in (LOCK, PIDFILE):
        if not path.is_file():
            continue
        try:
            return int(path.read_text(encoding="utf-8").strip().splitlines()[0])
        except Exception:
            continue
    return None


def git_sha() -> str:
    try:
        return subprocess.check_output(
            ["git", "rev-parse", "HEAD"], cwd=str(ROOT), text=True
        ).strip()
    except Exception:
        return "unknown"


def write_repro_manifest(*, iters: int, ckpt_every: int, fresh: bool, seed: int) -> Path:
    import torch

    sys.path.insert(0, str(ROOT / "scripts"))
    from denoise_arch_blocks import BLOCKS  # type: ignore
    from overnight_gpu_rl_arch import REWARD_MODES  # type: ignore

    manifest = {
        "schema": "denoiseopt.meta_approach_compare.repro.v1",
        "created_at": datetime.now(timezone.utc).isoformat(),
        "publishable": True,
        "fresh_start": fresh,
        "git_sha": git_sha(),
        "script": str(SCRIPT.relative_to(ROOT)).replace("\\", "/"),
        "out_dir": str(OUT),
        "seed": seed,
        "iters": iters,
        "ckpt_every": ckpt_every,
        "approaches": list(APPROACHES),
        "batch_default": 48,
        "fit_steps_default": 24,
        "pop_size": 12,
        "plateau_every_hybrid": 500,
        "reward_modes": list(REWARD_MODES),
        "blocks": list(BLOCKS),
        "lstm_in_vocab": "lstm" in BLOCKS,
        "xlstm_in_vocab": "xlstm" in BLOCKS,
        "torch": torch.__version__,
        "cuda_available": bool(torch.cuda.is_available()),
        "cuda_device": torch.cuda.get_device_name(0) if torch.cuda.is_available() else None,
        "python": sys.version.split()[0],
        "notes": (
            "Matched 5k outer-loop compare is the sole GPU experimentation vehicle. "
            "Selection/reporting use absolute prolonged R vs ideal sibling; "
            "PPO/REINFORCE credit uses searchable reward_mode. "
            "No concurrent overnight hybrid during this run."
        ),
    }
    path = OUT / "REPRO_MANIFEST.json"
    path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    (OUT / "REPRO.md").write_text(
        "\n".join(
            [
                "# Meta-approach 5k compare — reproducibility",
                "",
                f"- Created (UTC): `{manifest['created_at']}`",
                f"- Git SHA: `{manifest['git_sha']}`",
                f"- Seed: `{seed}`",
                f"- Iters: `{iters}` per approach",
                f"- Approaches: {', '.join(APPROACHES)}",
                f"- Vocab: LSTM + xLSTM in `BLOCKS`",
                f"- Reward modes (hybrid/REINFORCE/PBT): {', '.join(REWARD_MODES)}",
                f"- Device: `{manifest['cuda_device'] or 'cpu'}` / torch `{manifest['torch']}`",
                "",
                "Relaunch clean publishable run:",
                "",
                "```bash",
                "python scripts/launch_meta_approach_compare.py --iters 5000 --force --fresh",
                "```",
                "",
                "Poll:",
                "",
                "```bash",
                "python scripts/meta_approach_status.py",
                "```",
                "",
            ]
        ),
        encoding="utf-8",
    )
    return path


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--iters", type=int, default=5000)
    ap.add_argument("--ckpt-every", type=int, default=25)
    ap.add_argument("--seed", type=int, default=PUBLISH_SEED)
    ap.add_argument("--force", action="store_true", help="Ignore existing live lock")
    ap.add_argument(
        "--fresh",
        action="store_true",
        help="Wipe approach dirs + pass --no-resume (hard restart)",
    )
    args = ap.parse_args()

    OUT.mkdir(parents=True, exist_ok=True)
    existing = read_lock_pid()
    if existing and pid_alive(existing) and not args.force:
        print(f"ALREADY_RUNNING pid={existing}")
        print(f"Poll: {PY if PY.exists() else 'python'} scripts/meta_approach_status.py")
        return 0

    if args.fresh:
        import shutil

        for name in APPROACHES:
            d = OUT / name
            if d.is_dir():
                shutil.rmtree(d, ignore_errors=True)
        for p in (
            OUT / "STATUS.json",
            OUT / "meta_approach_compare.json",
            OUT / "fig_meta_approach_compare.png",
            OUT / "bench.lock",
            OUT / "bench.pid",
        ):
            if p.is_file():
                p.unlink(missing_ok=True)
        ts = time.strftime("%Y%m%dT%H%M%S")
        for name in ("bench_stdout.log", "bench_stderr.log"):
            src = OUT / name
            if src.is_file() and src.stat().st_size > 0:
                src.rename(OUT / f"{name}.bak_{ts}")
        print(f"FRESH wipe under {OUT}", flush=True)

    man = write_repro_manifest(
        iters=args.iters, ckpt_every=args.ckpt_every, fresh=args.fresh, seed=args.seed
    )
    print(f"REPRO_MANIFEST={man}", flush=True)

    py = str(PY if PY.exists() else sys.executable)
    log = OUT / "bench_stdout.log"
    err = OUT / "bench_stderr.log"
    cmd = [
        py,
        "-u",
        str(SCRIPT),
        "--iters",
        str(args.iters),
        "--seed",
        str(args.seed),
        "--ckpt-every",
        str(args.ckpt_every),
        "--batch",
        "48",
        "--fit-steps",
        "24",
        "--pop-size",
        "12",
        "--out-dir",
        str(OUT),
        "--approaches",
        ",".join(APPROACHES),
    ]
    if args.fresh:
        cmd.append("--no-resume")

    creationflags = 0
    if os.name == "nt":
        creationflags = subprocess.CREATE_NEW_PROCESS_GROUP | subprocess.DETACHED_PROCESS  # type: ignore[attr-defined]

    with log.open("a", encoding="utf-8") as lo, err.open("a", encoding="utf-8") as er:
        lo.write(
            f"\n--- launch {time.strftime('%Y-%m-%dT%H:%M:%S')} "
            f"fresh={args.fresh} seed={args.seed} ---\n"
        )
        lo.write("CMD " + " ".join(cmd) + "\n")
        lo.flush()
        proc = subprocess.Popen(
            cmd,
            cwd=str(ROOT),
            stdout=lo,
            stderr=er,
            creationflags=creationflags,
            close_fds=True,
        )
    LOCK.write_text(str(proc.pid), encoding="utf-8")
    PIDFILE.write_text(str(proc.pid), encoding="utf-8")
    print(f"LAUNCHED pid={proc.pid}")
    print(f"log={log}")
    print(f"Poll: {py} scripts/meta_approach_status.py")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
