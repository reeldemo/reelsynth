#!/usr/bin/env python3
"""Author deep model on Paderborn KAt — Al Firdausi & Ahmad 2022 CNN.

Honesty:
  - This is a published *fault classifier* (4-class N/B/IR/OR), not a wrap denoiser.
  - Do NOT invent wrap-R by feeding L=256 periods through the classifier head.
  - Native metric: segment-level class predictions on vibration_1 windows (len=516).
  - With only healthy K001 extracted, we report healthy→Normal hit-rate (not multi-class acc).

Citation:
  Al Firdausi, M. & Ahmad, S. (2022). Concise convolutional neural network model
  for fault detection. Communications in Science and Technology, 7(1), 62–72.
  Code+weights: https://github.com/mdzalfirdausi/CNN-for-Paderborn-Bearing-Dataset
"""
from __future__ import annotations

import json
import sys
import traceback
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import numpy as np
import torch
import torch.nn as nn

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

from signal_heal.datasets import _paderborn_channels  # noqa: E402

ART = ROOT / "brand" / "artifacts" / "signal_heal_transfer"
EXT = ART / "external"
WEIGHT = EXT / "weights" / "paderborn_cnn" / "model.pth"
K001 = ART / "raw" / "paderborn" / "K001"
OUT = ART / "deep_sota_adapters"
SEG_LEN = 516  # matches author model.pth flatten=16000 with 2×MaxPool1d(2)
CLASS_MAP = {0: "N", 1: "B", 2: "IR", 3: "OR"}


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


class CNN_1D_2L(nn.Module):
    """Reconstructed from author model.pth + gui_core/model.py CNN_1D_2L(500).

    Weights require input length 516 (gui comment says 500; state_dict linear
    in_features=16000 ⇒ L=516 with MaxPool1d(2) after each conv block).
    """

    def __init__(self) -> None:
        super().__init__()
        self.layer1 = nn.Sequential(
            nn.Conv1d(1, 64, kernel_size=9),
            nn.BatchNorm1d(64),
            nn.ReLU(),
            nn.MaxPool1d(2),
        )
        self.layer2 = nn.Sequential(
            nn.Conv1d(64, 128, kernel_size=5),
            nn.BatchNorm1d(128),
            nn.ReLU(),
            nn.MaxPool1d(2),
        )
        self.linear1 = nn.Linear(16000, 4)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        x = self.layer1(x)
        x = self.layer2(x)
        return self.linear1(x.flatten(1))


def load_author_cnn(weight: Path, device: torch.device) -> tuple[CNN_1D_2L | None, str]:
    if not weight.is_file() or weight.stat().st_size < 10_000:
        return None, f"missing weight {weight}"
    try:
        sd = torch.load(str(weight), map_location="cpu", weights_only=False)
        net = CNN_1D_2L()
        net.load_state_dict(sd, strict=True)
        net.eval()
        net.to(device)
        return net, f"loaded {weight} as CNN_1D_2L (L={SEG_LEN})"
    except Exception as e:
        return None, f"load failed: {type(e).__name__}: {e}"


@torch.no_grad()
def score_k001_healthy(
    net: CNN_1D_2L,
    device: torch.device,
    *,
    max_mats: int = 8,
    max_segments: int = 256,
) -> dict[str, Any]:
    mats = sorted(K001.glob("*.mat"))[:max_mats]
    if not mats:
        return {"status": "blocked", "blocker": f"no .mat under {K001}"}

    preds: list[int] = []
    used_files: list[str] = []
    for mat_path in mats:
        chans = _paderborn_channels(mat_path)
        if chans is None:
            continue
        vib, *_ = chans
        vib = np.asarray(vib, dtype=np.float64).ravel()
        if vib.size < SEG_LEN * 2:
            continue
        # Author-style: fixed segments; z-score each window
        n_seg = min(vib.size // SEG_LEN, max(1, max_segments // max(len(mats), 1)))
        segs = []
        for i in range(n_seg):
            w = vib[i * SEG_LEN : (i + 1) * SEG_LEN]
            w = (w - w.mean()) / (w.std() + 1e-8)
            segs.append(w)
        if not segs:
            continue
        x = torch.from_numpy(np.stack(segs, 0)).float().unsqueeze(1).to(device)
        logits = net(x)
        pred = logits.argmax(dim=1).cpu().tolist()
        preds.extend(int(p) for p in pred)
        used_files.append(mat_path.name)
        if len(preds) >= max_segments:
            break

    if not preds:
        return {"status": "blocked", "blocker": "no vibration segments extracted from K001"}

    arr = np.asarray(preds, dtype=np.int64)
    counts = {CLASS_MAP[i]: int((arr == i).sum()) for i in range(4)}
    n = int(arr.size)
    healthy_hit = float((arr == 0).mean())  # class 0 = Normal
    return {
        "status": "scored_native_healthy_only",
        "metric": "healthy_K001_segment_Normal_rate",
        "metric_note": (
            "Author model is 4-class fault diagnosis (N/B/IR/OR). Only healthy K001 "
            "is extracted here — report fraction of segments predicted Normal. "
            "Not multi-class accuracy; wrap-R not applicable (classifier ≠ denoise)."
        ),
        "n_segments": n,
        "n_mats": len(used_files),
        "files": used_files,
        "class_counts": counts,
        "healthy_Normal_rate": healthy_hit,
        "mode_class": CLASS_MAP[int(np.bincount(arr, minlength=4).argmax())],
        "seg_len": SEG_LEN,
        "wrap_R": None,
        "wrap_R_blocker": (
            "Refused: fault-classifier head cannot honestly produce DenoiseOpt wrap-R. "
            "Native classification metric only."
        ),
        "citation": {
            "paper": "Al Firdausi & Ahmad, Comm. Sci. Tech. 7(1):62–72, 2022",
            "github": "https://github.com/mdzalfirdausi/CNN-for-Paderborn-Bearing-Dataset",
            "weight": str(weight_rel()),
        },
    }


def weight_rel() -> Path:
    return WEIGHT


def main() -> int:
    device = torch.device("cuda" if torch.cuda.is_available() else "cpu")
    OUT.mkdir(parents=True, exist_ok=True)
    net, note = load_author_cnn(WEIGHT, device)
    report: dict[str, Any] = {
        "updated_at": utc_now(),
        "method": "paderborn_kat_deep_alfirdausi_cnn",
        "device": str(device),
        "weight": str(WEIGHT) if WEIGHT.is_file() else None,
        "load_note": note,
        "honesty": (
            "Author published deep model on Paderborn = fault classifier. "
            "SeamN2N / Ours wrap-R remain separate. Do not rename classification as denoise."
        ),
    }
    if net is None:
        report["status"] = "blocked"
        report["blocker"] = note
    else:
        report["status"] = "weights_loaded"
        report["native"] = score_k001_healthy(net, device)
        report["status"] = report["native"].get("status", "weights_loaded")

    path = OUT / "paderborn_deep_report.json"
    path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, indent=2))
    print(f"wrote {path}")
    return 0 if report.get("status", "").startswith("scored") else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception:
        traceback.print_exc()
        raise SystemExit(1)
