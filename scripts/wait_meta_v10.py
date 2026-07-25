#!/usr/bin/env python3
"""Poll hybrid_lstm v10 search until complete; print STATUS summary."""
from __future__ import annotations

import json
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
STATUS = ROOT / "brand" / "artifacts" / "meta_approach_compare_v10" / "STATUS.json"
CHAMP = ROOT / "brand" / "artifacts" / "meta_approach_compare_v10" / "hybrid_lstm" / "champ_cell.pt"
PID_FILE = ROOT / "brand" / "artifacts" / "meta_approach_compare_v10" / "bench.pid"


def alive(pid: int) -> bool:
    try:
        import os

        if sys.platform == "win32":
            import ctypes

            k = ctypes.windll.kernel32
            h = k.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
            if not h:
                return False
            k.CloseHandle(h)
            return True
        os.kill(pid, 0)
        return True
    except OSError:
        return False


def main() -> int:
    poll = float(sys.argv[1]) if len(sys.argv) > 1 else 60.0
    while True:
        st = json.loads(STATUS.read_text(encoding="utf-8"))
        row = (st.get("rows") or [{}])[0]
        pid = int(st.get("pid") or 0)
        print(
            f"{time.strftime('%H:%M:%S')} phase={st.get('phase')} "
            f"iter={st.get('current_iter')}/{st.get('target_iters')} "
            f"champ_r={row.get('champ_r')} complete={row.get('complete')} "
            f"all={st.get('all_complete')} pid_alive={alive(pid) if pid else False}",
            flush=True,
        )
        if st.get("all_complete") or row.get("complete"):
            print("DONE", "champ_exists=", CHAMP.is_file(), flush=True)
            return 0
        if pid and not alive(pid) and not CHAMP.is_file():
            print("DEAD_WITHOUT_CHAMP", flush=True)
            return 2
        if pid and not alive(pid) and CHAMP.is_file():
            # process exited; wait for STATUS to flip
            time.sleep(5)
            st2 = json.loads(STATUS.read_text(encoding="utf-8"))
            if st2.get("all_complete") or (st2.get("rows") or [{}])[0].get("complete"):
                print("DONE_AFTER_EXIT", flush=True)
                return 0
            print("EXITED_CHECK_STATUS", st2.get("phase"), flush=True)
            return 3
        time.sleep(poll)


if __name__ == "__main__":
    raise SystemExit(main())
