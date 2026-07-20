#!/usr/bin/env python3
"""Measure peak GPU memory during champion forward + FitCell search step.

Polls nvidia-smi while replaying overnight-like work (load favorite/champion,
forward, then one fit_cell). Overnight history.jsonl did not log peak VRAM;
this is an honest proxy labeled as such.

  .venv_gpu/Scripts/python.exe scripts/measure_peak_vram.py
"""
from __future__ import annotations

import argparse
import json
import subprocess
import sys
import threading
import time
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
import bench_inference_same_score as bib  # noqa: E402


class NvidiaSmiPoller:
    def __init__(self, interval_s: float = 0.15) -> None:
        self.interval_s = interval_s
        self.samples_mib: list[int] = []
        self._stop = threading.Event()
        self._thread: threading.Thread | None = None

    def start(self) -> None:
        self._stop.clear()
        self.samples_mib.clear()
        self._thread = threading.Thread(target=self._run, daemon=True)
        self._thread.start()

    def stop(self) -> None:
        self._stop.set()
        if self._thread is not None:
            self._thread.join(timeout=5.0)

    def _run(self) -> None:
        while not self._stop.is_set():
            try:
                out = subprocess.check_output(
                    [
                        "nvidia-smi",
                        "--query-gpu=memory.used",
                        "--format=csv,noheader,nounits",
                    ],
                    text=True,
                    stderr=subprocess.DEVNULL,
                )
                # multi-GPU: take max across devices
                vals = [int(float(x.strip())) for x in out.strip().splitlines() if x.strip()]
                if vals:
                    self.samples_mib.append(max(vals))
            except Exception:
                pass
            time.sleep(self.interval_s)

    def peak_mib(self) -> int | None:
        return max(self.samples_mib) if self.samples_mib else None


def resolve_fitted(path: Path | None) -> Path:
    if path is not None and path.exists():
        return path
    fav_meta = json.loads(
        (ROOT / "brand/artifacts/inference_bench/inference_bench.json").read_text(encoding="utf-8")
    )
    return Path(fav_meta["favorite"]["path"])


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--fitted", type=Path, default=None)
    ap.add_argument("--device", default="cuda")
    ap.add_argument("--batch", type=int, default=48)
    ap.add_argument("--fit-steps", type=int, default=24)
    ap.add_argument("--search-iters", type=int, default=8, help="Extra random FitCell trials")
    ap.add_argument(
        "--out",
        type=Path,
        default=ROOT / "brand/artifacts/peak_vram_replay.json",
    )
    args = ap.parse_args()

    if not torch.cuda.is_available():
        print("ERROR: CUDA required", file=sys.stderr)
        return 2

    device = torch.device(args.device)
    fitted = resolve_fitted(args.fitted)
    cfg, cell, residual_saved, arch = bib.load_fitted(fitted, device)
    hp = og.HyperParams(
        lr=3e-3,
        fit_steps=args.fit_steps,
        batch=args.batch,
        entropy_coef=0.01,
        ppo_clip=0.2,
    )

    # Baseline idle sample
    idle = NvidiaSmiPoller(0.2)
    idle.start()
    time.sleep(0.8)
    idle.stop()
    idle_peak = idle.peak_mib()

    torch.cuda.empty_cache()
    torch.cuda.reset_peak_memory_stats()
    poller = NvidiaSmiPoller(0.1)
    poller.start()
    t0 = time.time()

    # Champion forward passes
    ideal, eng = og.make_batch(args.batch, og.N, device)
    with torch.no_grad():
        for _ in range(20):
            out = og.apply_ops(eng, cell, cfg.ops)
            _ = og.residual_score(ideal, out)
    torch.cuda.synchronize()

    # Search-step proxy: refit champion + random arch trials (FitCell)
    r_fit, _ = og.fit_cell(cell, cfg.ops, device, steps=args.fit_steps, batch=args.batch, lr=hp.lr)
    rng = __import__("random").Random(1902771841)
    for _ in range(args.search_iters):
        trial_cfg = og.random_arch(rng)
        trial = og.SeamCell(trial_cfg).to(device)
        _rf, _ = og.fit_cell(
            trial,
            trial_cfg.ops,
            device,
            steps=args.fit_steps,
            batch=args.batch,
            lr=hp.lr,
        )
        del trial
        torch.cuda.empty_cache()

    torch.cuda.synchronize()
    elapsed = time.time() - t0
    poller.stop()

    torch_peak_gib = float(torch.cuda.max_memory_allocated() / (1024**3))
    smi_peak_mib = poller.peak_mib()
    smi_peak_gib = (smi_peak_mib / 1024.0) if smi_peak_mib is not None else None

    payload = {
        "protocol": "champion_forward_plus_fitcell_replay",
        "note": (
            "Overnight history.jsonl did not log peak VRAM. This measures nvidia-smi "
            "memory.used during favorite/champion forward + FitCell search-step replay "
            "on the same RTX 3090 geometry (batch/fit_steps matching overnight defaults). "
            "Labeled as replay proxy, not a byte-log of the multi-hour overnight process."
        ),
        "fitted_path": str(fitted),
        "residual_saved": residual_saved,
        "arch": arch,
        "batch": args.batch,
        "fit_steps": args.fit_steps,
        "search_iters": args.search_iters,
        "gpu": torch.cuda.get_device_name(0),
        "elapsed_sec": elapsed,
        "nvidia_smi_idle_peak_MiB": idle_peak,
        "nvidia_smi_peak_MiB": smi_peak_mib,
        "nvidia_smi_peak_GiB": smi_peak_gib,
        "torch_max_memory_allocated_GiB": torch_peak_gib,
        "fit_cell_residual": float(r_fit) if r_fit is not None else None,
    }
    args.out.parent.mkdir(parents=True, exist_ok=True)
    args.out.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(json.dumps(payload, indent=2))
    print(f"Wrote {args.out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
