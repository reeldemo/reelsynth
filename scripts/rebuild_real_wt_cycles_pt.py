#!/usr/bin/env python3
"""Rebuild cycles.pt from Factory+FX export JSON + oa_akwf WAVs (v10 ~1280)."""
from __future__ import annotations

import sys
from pathlib import Path

import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import real_wt_wrap_protocol as rwp  # noqa: E402

ART = ROOT / "brand" / "artifacts" / "real_wt_cycles"
device = torch.device("cpu")

factory = rwp.load_reelsynth_export(ART / "reelsynth_export_cycles.json", device)
akwf = rwp.load_oa_wav_cycles(ART / "oa_akwf", device, n_max=1280)
if akwf is None:
    raise SystemExit("no AKWF cycles under oa_akwf/")

blob = {
    "reelsynth_factory": factory.cpu(),
    "oa_instrument": akwf.cpu(),
    "meta": {
        "protocol": "paper_v10.1",
        "factory_source": "reelsynth_export_cycles.json",
        "factory_n": int(factory.shape[0]),
        "akwf_source": "oa_akwf/*.wav",
        "akwf_n": int(akwf.shape[0]),
        "L": int(factory.shape[1]),
    },
}
out = ART / "cycles.pt"
torch.save(blob, out)
print(f"wrote {out} factory={factory.shape} akwf={akwf.shape}")
