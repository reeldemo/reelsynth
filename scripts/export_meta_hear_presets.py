#!/usr/bin/env python3
"""Export ReelSynth presets (.reelpreset + .reelwt) for meta-compare heal tiles.

Refits FitCell from the hybrid_lstm champion (same path as export_meta_hear_samples.py),
upsamples each 256-sample cycle into a small factory-sized wavetable bank, and writes
playable presets styled after factory_wt_lead + Factory Lead FX (chorus/delay/reverb).

Writes under:
  brand/artifacts/meta_approach_compare/hear_presets/
"""
from __future__ import annotations

import argparse
import json
import struct
import subprocess
import sys
from pathlib import Path

import numpy as np
import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
from plot_meta_heal_samples import (  # noqa: E402
    EVAL_SEED,
    META_DIR,
    SEARCH_SEED,
    load_holdout,
    refit_champ,
    score_batch,
)

OUT_DIR = META_DIR / "hear_presets"
# Match hear_samples / paper holdout ranking (seed 20260719).
FIXED_TILES = [46, 21, 4, 49, 48]
NUM_FRAMES = 16
FRAME_SIZE = 2048  # factory default
REELWT_MAGIC = b"REELWT"
REELWT_VERSION = 1

VARIANT_KEYS = (
    ("ours", "ours_healed", "Heal Ours"),
    ("nobake", "nobake", "Heal Nobake"),
    ("dualcosine", "dual_cosine", "Heal DualCosine"),
)


def peak_normalize(cycle: np.ndarray, peak: float = 0.95) -> np.ndarray:
    x = np.asarray(cycle, dtype=np.float64).reshape(-1)
    m = float(np.max(np.abs(x))) if x.size else 0.0
    if m > 1e-12:
        x = x * (peak / m)
    return x.astype(np.float32)


def resample_cycle_to_frame(cycle: np.ndarray, frame_size: int = FRAME_SIZE) -> np.ndarray:
    """Linear resample one period → frame (mirrors WavetableBank::set_frame_from_cycle)."""
    c = np.asarray(cycle, dtype=np.float64).reshape(-1)
    n = len(c)
    if n == 0:
        return np.zeros(frame_size, dtype=np.float32)
    out = np.empty(frame_size, dtype=np.float64)
    for i in range(frame_size):
        src = (i / frame_size) * n
        i0 = int(np.floor(src)) % n
        i1 = (i0 + 1) % n
        f = src - np.floor(src)
        out[i] = c[i0] * (1.0 - f) + c[i1] * f
    return out.astype(np.float32)


def write_reelwt(path: Path, cycle: np.ndarray, *, num_frames: int = NUM_FRAMES, frame_size: int = FRAME_SIZE) -> None:
    frame = resample_cycle_to_frame(peak_normalize(cycle), frame_size)
    frames = np.tile(frame, num_frames)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("wb") as f:
        f.write(REELWT_MAGIC)
        f.write(struct.pack("<H", REELWT_VERSION))
        f.write(struct.pack("<I", num_frames))
        f.write(struct.pack("<I", frame_size))
        f.write(frames.astype("<f4", copy=False).tobytes())


def read_reelwt_header(path: Path) -> dict:
    data = path.read_bytes()
    if len(data) < 16 or data[:6] != REELWT_MAGIC:
        raise ValueError(f"bad reelwt magic: {path}")
    version, num_frames, frame_size = struct.unpack_from("<HII", data, 6)
    expected = 16 + num_frames * frame_size * 4
    if len(data) != expected:
        raise ValueError(f"reelwt size mismatch: {path} got {len(data)} want {expected}")
    return {"version": version, "num_frames": num_frames, "frame_size": frame_size, "bytes": len(data)}


def heal_wave_slots(num_frames: int = NUM_FRAMES) -> list[dict]:
    last = float(max(num_frames - 1, 0))
    slots = []
    for i in range(16):
        frame = (i / 15.0) * last if num_frames > 1 else 0.0
        label = ""
        if i == 0:
            label = "Heal"
        elif i == 7:
            label = "Mid"
        elif i == 15:
            label = "End"
        slots.append({"frame": float(frame), "label": label})
    return slots


def build_preset(
    *,
    name: str,
    wt_id: str,
    wt_filename: str,
    num_frames: int = NUM_FRAMES,
) -> dict:
    """factory_wt_lead Design + Factory Lead FX rack; WT points at sibling .reelwt."""
    last = float(max(num_frames - 1, 0))
    return {
        "schema": "reelsynth-preset-v2",
        "name": name,
        "wavetable_id": wt_id,
        # Sidecar hint for resolve_bank_for_preset (ignored by Patch serde).
        "wavetable_path": wt_filename,
        "oscillators": [
            {
                "type": "wavetable",
                "level": 0.9,
                "position": 0.0,
                "detune": 10.0,
                "unison": 3,
                "pan": 0.0,
                "wavetable_id": wt_id,
                "pulse_width": 0.5,
                "morph_a": 0.0,
                "morph_b": last,
                "morph_amount": 0.0,
                "warp_mode": "none",
                "warp_amount": 0.0,
                "fm_source": "none",
                "fm_ratio": 1.0,
                "fm_index": 0.0,
                "wave_quant": 16,
                "wave_slot": 0,
                "wave_slot_fine": 0.0,
                "wave_slots": heal_wave_slots(num_frames),
                "wave_layers": [],
                "stack_mode": "add",
            }
        ],
        "filter": {
            "type": "lowpass",
            "cutoff": 2800.0,
            "resonance": 0.45,
            "key_tracking": 0.65,
            "drive": 0.12,
        },
        "filter2": {
            "type": "lowpass",
            "cutoff": 5200.0,
            "resonance": 0.22,
            "key_tracking": 0.4,
            "drive": 0.0,
        },
        "envelope": {
            "attack": 0.01,
            "decay": 0.32,
            "sustain": 0.65,
            "release": 0.42,
        },
        "filter_envelope": {
            "attack": 0.02,
            "decay": 0.4,
            "sustain": 0.35,
            "release": 0.5,
        },
        "lfo": {
            "rate": 0.35,
            "depth": 0.08,
            "target": "osc1_position",
            "shape": "sine",
        },
        "lfo2": {
            "rate": 0.5,
            "depth": 0.0,
            "target": "wt_position",
            "shape": "sine",
        },
        "macros": [
            {"value": 0.5, "target": "filter_cutoff", "amount": 0.6},
            {"value": 0.5, "target": "osc1_position", "amount": 0.5},
            {"value": 0.5, "target": "osc1_fm_index", "amount": 0.4},
            {"value": 0.5, "target": "filter_resonance", "amount": 0.35},
        ],
        "mod_matrix": [
            {
                "source": "lfo1",
                "target": "osc1_position",
                "amount": 0.12,
                "enabled": True,
            },
            {
                "source": "velocity",
                "target": "osc1_level",
                "amount": 0.35,
                "enabled": True,
            },
            {
                "source": "filt_env",
                "target": "filter_cutoff",
                "amount": 0.22,
                "enabled": True,
            },
        ],
        # Factory Lead-style FX: chorus + short delay on, reverb bypassed.
        "effects": [
            {
                "effect_type": "chorus",
                "bypassed": False,
                "mix": 0.22,
                "rate": 0.8,
                "depth": 0.35,
                "time_ms": 280.0,
                "feedback": 0.32,
                "size": 0.68,
                "damping": 0.42,
                "drive": 0.35,
                "tone": 0.5,
                "threshold": -18.0,
                "ratio": 4.0,
                "attack": 0.01,
                "release": 0.12,
            },
            {
                "effect_type": "delay",
                "bypassed": False,
                "mix": 0.18,
                "rate": 0.8,
                "depth": 0.35,
                "time_ms": 120.0,
                "feedback": 0.28,
                "size": 0.68,
                "damping": 0.42,
                "drive": 0.35,
                "tone": 0.5,
                "threshold": -18.0,
                "ratio": 4.0,
                "attack": 0.01,
                "release": 0.12,
            },
            {
                "effect_type": "reverb",
                "bypassed": True,
                "mix": 0.28,
                "rate": 0.8,
                "depth": 0.35,
                "time_ms": 280.0,
                "feedback": 0.32,
                "size": 0.68,
                "damping": 0.42,
                "drive": 0.35,
                "tone": 0.5,
                "threshold": -18.0,
                "ratio": 4.0,
                "attack": 0.01,
                "release": 0.12,
            },
        ],
        "sub_level": 0.1,
        "noise_level": 0.0,
        "unison_stereo_spread": 0.85,
        "performance": {
            "root": 0,
            "scale": "major",
            "scale_behavior": "snap",
            "layout": "piano",
            "chord_set": "triads",
            "voicing": "close",
            "base_octave": 4,
            "arp": {
                "enabled": False,
                "input_mode": "single_note",
                "direction": "up",
                "rate": "quarter",
                "gate": 0.85,
                "octave_spread": 1,
                "latch": False,
            },
        },
        "sequence": {
            "bpm": 120.0,
            "time_sig_num": 4,
            "time_sig_den": 4,
            "loop_region": {"start_beats": 0.0, "end_beats": 16.0, "enabled": True},
            "tracks": [
                {"name": "Track 1", "mute": False, "solo": False, "arm": False, "clips": [], "target_osc": None},
                {"name": "Track 2", "mute": False, "solo": False, "arm": False, "clips": [], "target_osc": None},
                {"name": "Track 3", "mute": False, "solo": False, "arm": False, "clips": [], "target_osc": None},
                {"name": "Track 4", "mute": False, "solo": False, "arm": False, "clips": [], "target_osc": None},
            ],
            "scenes": [
                {"name": f"Scene {i}", "slots": [None, None, None, None]} for i in range(1, 9)
            ],
            "quantize": {"division": "sixteenth", "triplet": False},
        },
        "crackle": 0.0,
    }


def write_preset(path: Path, preset: dict) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(preset, indent=2) + "\n", encoding="utf-8")


def verify_pair(preset_path: Path, wt_path: Path) -> None:
    preset = json.loads(preset_path.read_text(encoding="utf-8"))
    if preset.get("schema") not in ("reelsynth-preset-v1", "reelsynth-preset-v2"):
        raise ValueError(f"bad schema in {preset_path}")
    if not preset.get("oscillators"):
        raise ValueError(f"no oscillators in {preset_path}")
    hdr = read_reelwt_header(wt_path)
    if hdr["num_frames"] < 1 or hdr["frame_size"] < 2:
        raise ValueError(f"degenerate bank: {wt_path}")
    # Engine roundtrip: Patch::from_json + WavetableBank::read_file via reelsynth-export.
    tmp = preset_path.parent / "_verify_export"
    tmp.mkdir(parents=True, exist_ok=True)
    out_wav = tmp / f"{preset_path.stem}_probe.vitaltable"
    cmd = [
        "cargo",
        "run",
        "-q",
        "--bin",
        "reelsynth-export",
        "--",
        "vital",
        str(preset_path),
        "-o",
        str(out_wav),
    ]
    proc = subprocess.run(cmd, cwd=str(ROOT), capture_output=True, text=True)
    if proc.returncode != 0:
        raise RuntimeError(
            f"reelsynth-export failed for {preset_path}:\n"
            f"{proc.stdout}\n{proc.stderr}"
        )


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--approach", type=str, default="hybrid_lstm")
    ap.add_argument("--device", type=str, default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--meta-dir", type=Path, default=META_DIR)
    ap.add_argument("--out-dir", type=Path, default=OUT_DIR)
    ap.add_argument("--tiles", type=int, nargs="+", default=FIXED_TILES)
    ap.add_argument("--num-frames", type=int, default=NUM_FRAMES)
    ap.add_argument("--frame-size", type=int, default=FRAME_SIZE)
    ap.add_argument("--skip-rust-verify", action="store_true")
    ap.add_argument("--ours-only", action="store_true", help="Skip nobake/dualcosine A/B presets")
    args = ap.parse_args()

    device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")
    summary_path = args.meta_dir / args.approach / "summary.json"
    if not summary_path.is_file():
        raise SystemExit(f"missing champion summary: {summary_path}")
    summary = json.loads(summary_path.read_text(encoding="utf-8"))

    print(f"refitting champion from {summary_path} on {device} …")
    cfg, cell, train_r, fit_meta = refit_champ(summary, device)
    ideal_b, eng_b, hold_note = load_holdout(device)
    tiles = [int(t) for t in args.tiles]
    for t in tiles:
        if t < 0 or t >= eng_b.shape[0]:
            raise SystemExit(f"tile {t} out of range 0..{eng_b.shape[0]-1}")

    dual_b = og.dual_cosine_blend(eng_b)
    with torch.no_grad():
        ours_b = og.apply_ops(eng_b, cell, cfg.ops)

    args.out_dir.mkdir(parents=True, exist_ok=True)
    entries: list[dict] = []
    written: list[str] = []
    variants_spec = VARIANT_KEYS[:1] if args.ours_only else VARIANT_KEYS

    for rank, idx in enumerate(tiles, start=1):
        ideal = ideal_b[idx : idx + 1]
        eng = eng_b[idx : idx + 1]
        dual = dual_b[idx : idx + 1]
        ours = ours_b[idx : idx + 1]
        wrap_abs = float((eng[0, 0] - eng[0, -1]).abs().item())
        scores = {
            "no_bake": score_batch(ideal, eng),
            "dual_cosine": score_batch(ideal, dual),
            "ours_hybrid": score_batch(ideal, ours),
        }
        cycles = {
            "ours": ours[0].detach().cpu().numpy(),
            "nobake": eng[0].detach().cpu().numpy(),
            "dualcosine": dual[0].detach().cpu().numpy(),
        }
        files: dict[str, dict[str, str]] = {}
        for short, _score_key, display in variants_spec:
            stem = f"{rank:02d}_tile{idx}_{short}"
            wt_name = f"{stem}.reelwt"
            preset_name = f"{stem}.reelpreset"
            wt_path = args.out_dir / wt_name
            preset_path = args.out_dir / preset_name
            write_reelwt(
                wt_path,
                cycles[short],
                num_frames=args.num_frames,
                frame_size=args.frame_size,
            )
            preset = build_preset(
                name=f"{display} · tile {idx}",
                wt_id=stem,
                wt_filename=wt_name,
                num_frames=args.num_frames,
            )
            write_preset(preset_path, preset)
            files[short] = {"reelpreset": preset_name, "reelwt": wt_name}
            written.extend([str(preset_path.resolve()), str(wt_path.resolve())])
            print(f"wrote {preset_path.resolve()}")
            print(f"wrote {wt_path.resolve()}")
            if not args.skip_rust_verify:
                verify_pair(preset_path, wt_path)
                print(f"verified {stem}")

        entries.append(
            {
                "sample_index": rank,
                "tile_index": int(idx),
                "eval_seed": EVAL_SEED,
                "wrap_abs": wrap_abs,
                "R": scores,
                "files": files,
                "paper_heal_tile": idx == 46,
            }
        )

    # Drop verify scratch
    verify_dir = args.out_dir / "_verify_export"
    if verify_dir.is_dir():
        for p in verify_dir.glob("*"):
            p.unlink()
        verify_dir.rmdir()

    manifest = {
        "schema": "denoiseopt.meta_hear_presets.v1",
        "approach_code": args.approach,
        "approach_display": "Ours (hybrid GA–PPO)",
        "eval_seed": EVAL_SEED,
        "search_seed": SEARCH_SEED,
        "holdout_source": hold_note,
        "bank": {"num_frames": args.num_frames, "frame_size": args.frame_size},
        "preset_style": "factory_wt_lead Design + Factory Lead FX (chorus/delay on, reverb bypassed)",
        "champ_raw_from_summary": summary.get("champ_raw"),
        "delta_r_vs_dual_cosine_summary": summary.get("delta_r_vs_dual_cosine"),
        "refit": fit_meta,
        "train_r_last": train_r,
        "champ_arch": cfg.to_dict(),
        "samples": entries,
        "absolute_paths": written,
        "note": (
            "Each .reelwt is the L=256 heal/engine cycle linearly resampled to "
            f"{args.frame_size}-sample frames × {args.num_frames} (identical frames). "
            "Open the .reelpreset in reelsynth-app; sibling .reelwt resolves via "
            "wavetable_id / wavetable_path."
        ),
    }
    man_path = args.out_dir / "manifest.json"
    man_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")

    readme = f"""# Meta-compare hear presets

Playable **ReelSynth** presets for the wrap-seam heal tiles (holdout seed **{EVAL_SEED}**),
from the **Ours (hybrid GA–PPO)** / `{args.approach}` champion in
`brand/artifacts/meta_approach_compare/`.

Each preset is a real Design patch (filter, ADSR, mod matrix, chorus + delay) pointing at a
sidecar `.reelwt` bank built from the healed / cracked / DualCosine period.

## Open in reelsynth-app

1. Build & launch:
   ```bash
   cargo run -p reelsynth-app --bin reelsynth-app
   ```
2. **File → Open Preset…** (or the app’s Open Preset control) and choose a
   `*.reelpreset` in this folder — keep the matching `*.reelwt` **next to it**
   (same directory; same stem).
3. Play notes from the keyboard / MIDI. Compare:
   - `*_ours.*` — healed FitCell (hybrid_lstm champion)
   - `*_nobake.*` — cracked engine wrap (A/B)
   - `*_dualcosine.*` — DualCosine classical bake (A/B)

Bank geometry: **{args.num_frames} frames × {args.frame_size} samples** (cycle upsampled from L=256).
Preset Design: **factory_wt_lead**-style WT lead + **Factory Lead** FX (chorus/delay on, reverb bypassed).

## Samples

| # | Tile | Ours | Nobake | DualCosine |
|---|------|------|--------|------------|
"""
    for e in entries:
        flag = " (paper heal)" if e["paper_heal_tile"] else ""
        f = e["files"]

        def cell(key: str) -> str:
            if key not in f:
                return "—"
            return f"`{f[key]['reelpreset']}` + `{f[key]['reelwt']}`"

        readme += (
            f"| {e['sample_index']} | {e['tile_index']}{flag} | "
            f"{cell('ours')} | {cell('nobake')} | {cell('dualcosine')} |\n"
        )
    readme += f"""
See `manifest.json` for absolute R scores and wrap magnitudes.

Absolute paths (this machine at generation time) are listed in `manifest.json` → `absolute_paths`.

Rebuild:

```bash
.venv_gpu/Scripts/python.exe scripts/export_meta_hear_presets.py --approach hybrid_lstm
```

Skip engine load check (faster):

```bash
.venv_gpu/Scripts/python.exe scripts/export_meta_hear_presets.py --skip-rust-verify
```
"""
    (args.out_dir / "README.md").write_text(readme, encoding="utf-8")
    print(f"wrote {man_path.resolve()}")
    print(f"wrote {(args.out_dir / 'README.md').resolve()}")
    print(json.dumps({"n_files": len(written), "tiles": tiles}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
