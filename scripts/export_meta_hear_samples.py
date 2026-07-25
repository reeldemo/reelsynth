#!/usr/bin/env python3
"""Export audible WAV demos for DenoiseOpt meta-compare heal (Ours hybrid GA–PPO).

Refits FitCell from the hybrid_lstm champion (same path as plot_meta_heal_samples.py),
scores holdout seed 20260719 tiles, and writes short looped wavetable clips at a fixed
pitch so wrap seams are audible.

Writes under:
  brand/artifacts/meta_approach_compare/hear_samples/
"""
from __future__ import annotations

import argparse
import json
import sys
import wave
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

OUT_DIR = META_DIR / "hear_samples"
SR = 44100
FREQ_HZ = 440.0  # A4
DURATION_S = 3.0
N_SAMPLES = 5


def write_wav_mono(path: Path, samples: np.ndarray, sr: int = SR) -> None:
    x = np.asarray(samples, dtype=np.float64)
    peak = float(np.max(np.abs(x))) if x.size else 0.0
    if peak > 1e-12:
        x = x / peak * 0.89
    pcm = np.clip(x * 32767.0, -32768, 32767).astype(np.int16)
    path.parent.mkdir(parents=True, exist_ok=True)
    with wave.open(str(path), "wb") as wf:
        wf.setnchannels(1)
        wf.setsampwidth(2)
        wf.setframerate(sr)
        wf.writeframes(pcm.tobytes())


def render_wavetable(cycle: np.ndarray, *, sr: int, freq_hz: float, duration_s: float) -> np.ndarray:
    """Linear-interpolated wavetable playback of one period cycle."""
    table = np.asarray(cycle, dtype=np.float64).reshape(-1)
    n = len(table)
    n_out = int(round(sr * duration_s))
    inc = (n * freq_hz) / float(sr)
    phase = (np.arange(n_out, dtype=np.float64) * inc) % n
    idx = np.floor(phase).astype(np.int64)
    frac = phase - idx
    a = table[idx]
    b = table[(idx + 1) % n]
    return a + frac * (b - a)


def pick_tiles(eng: torch.Tensor, n: int, prefer: int = 46) -> list[int]:
    wrap = (eng[:, 0] - eng[:, -1]).abs().detach().cpu().numpy()
    order = list(np.argsort(-wrap))
    picked: list[int] = []
    if prefer < eng.shape[0]:
        picked.append(int(prefer))
    for i in order:
        ii = int(i)
        if ii not in picked:
            picked.append(ii)
        if len(picked) >= n:
            break
    return picked[:n]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--approach", type=str, default="hybrid_lstm")
    ap.add_argument("--device", type=str, default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--meta-dir", type=Path, default=META_DIR)
    ap.add_argument("--out-dir", type=Path, default=OUT_DIR)
    ap.add_argument("--n-samples", type=int, default=N_SAMPLES)
    ap.add_argument("--sr", type=int, default=SR)
    ap.add_argument("--freq", type=float, default=FREQ_HZ)
    ap.add_argument("--duration", type=float, default=DURATION_S)
    args = ap.parse_args()

    device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")
    summary_path = args.meta_dir / args.approach / "summary.json"
    if not summary_path.is_file():
        raise SystemExit(f"missing champion summary: {summary_path}")
    summary = json.loads(summary_path.read_text(encoding="utf-8"))

    print(f"refitting champion from {summary_path} on {device} …")
    cfg, cell, train_r, fit_meta = refit_champ(summary, device)
    ideal_b, eng_b, hold_note = load_holdout(device)
    tiles = pick_tiles(eng_b, args.n_samples, prefer=46)

    dual_b = og.dual_cosine_blend(eng_b)
    with torch.no_grad():
        ours_b = og.apply_ops(eng_b, cell, cfg.ops)

    args.out_dir.mkdir(parents=True, exist_ok=True)
    entries: list[dict] = []
    written: list[str] = []

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
        variants = {
            "nobake": eng[0].detach().cpu().numpy(),
            "dualcosine": dual[0].detach().cpu().numpy(),
            "ours_healed": ours[0].detach().cpu().numpy(),
        }
        files = {}
        for key, cycle in variants.items():
            name = f"{rank:02d}_tile{idx}_{key}.wav"
            path = args.out_dir / name
            audio = render_wavetable(cycle, sr=args.sr, freq_hz=args.freq, duration_s=args.duration)
            write_wav_mono(path, audio, sr=args.sr)
            files[key] = name
            written.append(str(path.resolve()))
            print(f"wrote {path.resolve()}")

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

    manifest = {
        "schema": "denoiseopt.meta_hear_samples.v1",
        "approach_code": args.approach,
        "approach_display": "Ours (hybrid GA–PPO)",
        "eval_seed": EVAL_SEED,
        "search_seed": SEARCH_SEED,
        "holdout_source": hold_note,
        "sample_rate": args.sr,
        "freq_hz": args.freq,
        "duration_s": args.duration,
        "playback": (
            "Each WAV is linear-interpolated wavetable playback of one 256-sample cycle "
            f"at {args.freq} Hz for {args.duration}s ({args.sr} Hz mono PCM16). "
            "Compare nobake (cracked wrap) vs dualcosine vs ours_healed."
        ),
        "champ_raw_from_summary": summary.get("champ_raw"),
        "delta_r_vs_dual_cosine_summary": summary.get("delta_r_vs_dual_cosine"),
        "refit": fit_meta,
        "train_r_last": train_r,
        "champ_arch": cfg.to_dict(),
        "samples": entries,
        "absolute_paths": written,
        "note": (
            "Cell weights are refit from champion arch+HP (meta suite does not persist "
            "fitted state_dict). Same FitCell path as fig_meta_heal_samples."
        ),
    }
    man_path = args.out_dir / "manifest.json"
    man_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")

    readme = f"""# Meta-compare hear samples

Audible demos of wrap-seam heal using the **Ours (hybrid GA–PPO)** / `hybrid_lstm` champion
from `brand/artifacts/meta_approach_compare/`.

## Playback

- Sample rate: **{args.sr} Hz**, mono PCM16
- Pitch: **{args.freq} Hz (A4)** via linear wavetable interpolation of each 256-sample cycle
- Duration: **{args.duration} s** per clip
- Holdout seed: **{EVAL_SEED}** (paper heal figure); search/refit seed: **{SEARCH_SEED}**

Open the `*_nobake.wav` vs `*_dualcosine.wav` vs `*_ours_healed.wav` files in any audio player.
Cracked (nobake) clips should click/buzz at the wrap; healed Ours should sound smoother.

## Samples

| # | Tile | Files |
|---|------|-------|
"""
    for e in entries:
        files = ", ".join(e["files"].values())
        flag = " (paper heal tile)" if e["paper_heal_tile"] else ""
        readme += (
            f"| {e['sample_index']} | {e['tile_index']}{flag} | `{files}` |\n"
        )
    readme += f"""
See `manifest.json` for absolute R scores and wrap magnitudes.

Rebuild:

```bash
.venv_gpu/Scripts/python.exe scripts/export_meta_hear_samples.py --approach hybrid_lstm
```
"""
    (args.out_dir / "README.md").write_text(readme, encoding="utf-8")
    print(f"wrote {man_path.resolve()}")
    print(f"wrote {(args.out_dir / 'README.md').resolve()}")
    print(json.dumps({"n_wavs": len(written), "tiles": tiles}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
