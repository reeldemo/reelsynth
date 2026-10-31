#!/usr/bin/env python3
"""Kill-and-resume smoke for v14 plateau protocol checkpoints."""
from __future__ import annotations

import json
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BENCH = ROOT / "scripts/bench_meta_approaches_5k.py"


def run_bench(out: Path, iters: int, extra: list[str] | None = None) -> subprocess.CompletedProcess[str]:
    cmd = [
        sys.executable,
        str(BENCH),
        "--protocol",
        "plateau",
        "--approaches",
        "hybrid_lstm",
        "--seed",
        "424242",
        "--iters",
        str(iters),
        "--batch",
        "12",
        "--fit-steps",
        "4",
        "--pop-size",
        "4",
        "--ckpt-every",
        "2",
        "--proposals-per-iter",
        "2",
        "--device",
        "cpu",
        "--out-dir",
        str(out),
        *(extra or []),
    ]
    return subprocess.run(cmd, cwd=str(ROOT), capture_output=True, text=True, check=False)


def main() -> int:
    with tempfile.TemporaryDirectory() as td:
        out = Path(td) / "v14_smoke"
        r1 = run_bench(out, iters=2)
        if r1.returncode != 0:
            print(r1.stdout, r1.stderr, file=sys.stderr)
            return r1.returncode
        ckpt = out / "hybrid_lstm" / "checkpoint.json"
        assert ckpt.is_file(), "missing checkpoint after partial run"
        done1 = int(json.loads(ckpt.read_text())["iters_done"])
        assert done1 == 2, done1
        r2 = run_bench(out, iters=4)
        if r2.returncode != 0:
            print(r2.stdout, r2.stderr, file=sys.stderr)
            return r2.returncode
        done2 = int(json.loads(ckpt.read_text())["iters_done"])
        assert done2 == 4, f"expected resume to 4, got {done2}"
        hist = (out / "hybrid_lstm" / "history.jsonl").read_text(encoding="utf-8").strip().splitlines()
        assert len(hist) >= 4, f"expected >=4 history rows, got {len(hist)}"
    print("OK test_v14_plateau_resume")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
