#!/usr/bin/env python3
"""Poll meta-approach 5k comparison status (crash-safe; reads checkpoints).

Usage (from reelsynth root):
  .venv_gpu\\Scripts\\python.exe scripts\\meta_approach_status.py
  .venv_gpu\\Scripts\\python.exe scripts\\meta_approach_status.py --watch 30
  .venv_gpu\\Scripts\\python.exe scripts\\meta_approach_status.py --json

Reads (in order):
  brand/artifacts/meta_approach_compare/STATUS.json
  brand/artifacts/meta_approach_STATUS.json   (mirror)
  per-approach checkpoint.json / summary.json if STATUS missing
"""
from __future__ import annotations

import argparse
import json
import os
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_OUT = ROOT / "brand" / "artifacts" / "meta_approach_compare"
STATUS_LATEST = ROOT / "brand" / "artifacts" / "meta_approach_STATUS.json"
APPROACHES = ("random", "cmaes", "reinforce", "aging_evo", "tpe", "hybrid_lstm")


def pid_alive(pid: int | None) -> bool | None:
    if pid is None:
        return None
    try:
        if os.name == "nt":
            import ctypes

            k = ctypes.windll.kernel32
            handle = k.OpenProcess(0x1000, False, int(pid))  # PROCESS_QUERY_LIMITED_INFORMATION
            if handle:
                k.CloseHandle(handle)
                return True
            return False
        os.kill(int(pid), 0)
        return True
    except Exception:
        return False


def load_json(path: Path) -> dict | None:
    if not path.is_file():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def rebuild_from_checkpoints(out_dir: Path, target_iters: int = 5000) -> dict:
    rows = []
    for name in APPROACHES:
        ad = out_dir / name
        ckpt = load_json(ad / "checkpoint.json") or {}
        summary = load_json(ad / "summary.json") or {}
        done = int(ckpt.get("iters_done") or summary.get("iters_done") or 0)
        champ = summary.get("champ_raw", ckpt.get("champ_raw", ckpt.get("champ_r")))
        complete = bool(summary) and done >= target_iters
        hist = ad / "history.jsonl"
        hist_lines = 0
        if hist.is_file():
            try:
                hist_lines = sum(1 for line in hist.open(encoding="utf-8") if line.strip())
            except Exception:
                pass
        rows.append(
            {
                "approach": name,
                "iters_done": done,
                "target_iters": target_iters,
                "pct": round(100.0 * done / max(target_iters, 1), 2),
                "champ_r": champ,
                "lstm_in_champ": bool(
                    summary.get("lstm_in_champ", ckpt.get("champ_lstm", False))
                ),
                "xlstm_in_champ": bool(
                    summary.get("xlstm_in_champ", ckpt.get("champ_xlstm", False))
                ),
                "wall_s": float(summary.get("wall_s", ckpt.get("wall_s", 0.0)) or 0.0),
                "complete": complete,
                "history_lines": hist_lines,
                "has_checkpoint": (ad / "checkpoint.json").is_file(),
            }
        )
    n_done = sum(1 for r in rows if r["complete"])
    pid = None
    pid_file = out_dir / "bench.pid"
    if pid_file.is_file():
        try:
            pid = int(pid_file.read_text(encoding="utf-8").strip())
        except Exception:
            pid = None
    return {
        "schema": "denoiseopt.meta_approach_status.v1",
        "updated_at": datetime.now(timezone.utc).isoformat(),
        "phase": "rebuilt_from_checkpoints",
        "pid": pid,
        "out_dir": str(out_dir),
        "target_iters": target_iters,
        "approaches_planned": list(APPROACHES),
        "current_approach": None,
        "current_iter": None,
        "n_complete": n_done,
        "n_total": len(APPROACHES),
        "all_complete": n_done >= len(APPROACHES),
        "rows": rows,
    }


def find_bench_pids() -> list[int]:
    pids: list[int] = []
    try:
        if os.name == "nt":
            import subprocess

            out = subprocess.check_output(
                [
                    "powershell",
                    "-NoProfile",
                    "-Command",
                    "Get-CimInstance Win32_Process -Filter \"Name='python.exe'\" | "
                    "Where-Object { $_.CommandLine -match 'bench_meta_approaches_5k' } | "
                    "Select-Object -ExpandProperty ProcessId",
                ],
                text=True,
                stderr=subprocess.DEVNULL,
            )
            for line in out.splitlines():
                line = line.strip()
                if line.isdigit():
                    pids.append(int(line))
        else:
            import subprocess

            out = subprocess.check_output(["pgrep", "-f", "bench_meta_approaches_5k"], text=True)
            pids = [int(x) for x in out.split() if x.isdigit()]
    except Exception:
        pass
    return pids


def format_status(st: dict) -> str:
    lines = []
    pid = st.get("pid")
    live = find_bench_pids()
    if live and (pid is None or pid not in live):
        pid = live[0]
        st = dict(st)
        st["pid"] = pid
        st["pids_live"] = live
    alive = pid_alive(pid)
    if live:
        alive = True
    alive_s = {True: "ALIVE", False: "DEAD", None: "?"}.get(alive, "?")
    lines.append("=== DenoiseOpt meta-approach status ===")
    lines.append(f"updated:  {st.get('updated_at')}")
    lines.append(f"phase:    {st.get('phase')}")
    lines.append(f"pid:      {pid} ({alive_s})" + (f"  all={live}" if live else ""))
    lines.append(f"out_dir:  {st.get('out_dir')}")
    lines.append(
        f"progress: {st.get('n_complete')}/{st.get('n_total')} complete"
        f"  | current={st.get('current_approach')} iter={st.get('current_iter')}"
    )
    lines.append("")
    lines.append(
        f"{'approach':12} {'done':>8} {'pct':>7} {'champ_R':>9} {'lstm':>5} {'xlstm':>5} {'ckpt':>4} {'state':>10}"
    )
    lines.append("-" * 70)
    for r in st.get("rows", []):
        champ = r.get("champ_r")
        champ_s = f"{champ:.5f}" if isinstance(champ, (int, float)) else "-"
        state = "DONE" if r.get("complete") else ("RUN" if r.get("iters_done") else "PEND")
        if st.get("current_approach") == r.get("approach") and state == "RUN":
            state = "ACTIVE"
        lines.append(
            f"{r.get('approach', '?'):12} "
            f"{r.get('iters_done', 0):>4}/{r.get('target_iters', 0):<4} "
            f"{r.get('pct', 0):>6.1f}% "
            f"{champ_s:>9} "
            f"{'Y' if r.get('lstm_in_champ') else 'n':>5} "
            f"{'Y' if r.get('xlstm_in_champ') else 'n':>5} "
            f"{'Y' if r.get('has_checkpoint') else 'n':>4} "
            f"{state:>10}"
        )
    lines.append("")
    if st.get("all_complete"):
        lines.append("ALL COMPLETE")
    else:
        lines.append("IN PROGRESS (resume-safe via per-approach checkpoint.json)")
    return "\n".join(lines)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--out-dir",
        type=Path,
        default=DEFAULT_OUT,
        help="Benchmark artifact root",
    )
    ap.add_argument("--json", action="store_true", help="Print raw JSON")
    ap.add_argument(
        "--watch",
        type=float,
        default=0.0,
        help="If >0, re-poll every N seconds",
    )
    ap.add_argument("--target-iters", type=int, default=5000)
    args = ap.parse_args()

    def once() -> dict:
        st = load_json(args.out_dir / "STATUS.json") or load_json(STATUS_LATEST)
        if st is None:
            st = rebuild_from_checkpoints(args.out_dir, args.target_iters)
        return st

    while True:
        st = once()
        if args.json:
            print(json.dumps(st, indent=2))
        else:
            print(format_status(st))
            print()
            print("Poll again:")
            print(
                r"  .venv_gpu\Scripts\python.exe scripts\meta_approach_status.py"
            )
            print(f"  # or: Get-Content {STATUS_LATEST}")
        if args.watch <= 0:
            return 0 if not st.get("all_complete") else 0
        time.sleep(args.watch)


if __name__ == "__main__":
    raise SystemExit(main())
