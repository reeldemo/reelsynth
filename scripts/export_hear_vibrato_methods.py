#!/usr/bin/env python3
"""Export side-by-side vibrato WAVs for DenoiseOpt method comparison.

Renders looped sinusoidal pitch vibrato (same family as bench_vibrato_spectrogram.py)
for wavetable / meta arms and optional signal-heal transfer champs.

Sources:
  - Fixed hear tiles [46, 21, 4, 49, 48] (seed 20260719)
  - Cycles for nobake / DualCosine / Ours from hear_presets .reelwt when present
    (Factory Lead–style bank), else refit hybrid_lstm champion
  - Other meta arms (random / cmaes / reinforce / aging_evo / tpe) via CPU refit
  - Transfer domains under signal_heal_transfer/*/hybrid_lstm/ when summaries exist

Factory Lead character: after vibrato playback, apply a light chorus+delay rack
matching hear_presets FX intent (chorus + short delay; no reverb).

Writes:
  brand/artifacts/hear_vibrato_methods/
    meta/
    transfer/<dataset>/
    manifest.json
    README.md

Prefer --device cpu when a long GPU search is running.
"""
from __future__ import annotations

import argparse
import json
import math
import struct
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

OUT_DIR = ROOT / "brand" / "artifacts" / "hear_vibrato_methods"
HEAR_PRESETS = META_DIR / "hear_presets"
TRANSFER_ROOT = ROOT / "brand" / "artifacts" / "signal_heal_transfer"

FIXED_TILES = [46, 21, 4, 49, 48]
SR = 44100
BASE_FREQ_HZ = 440.0  # A4 — match export_meta_hear_samples
DURATION_S = 3.0
VIBRATO_RATE_HZ = 5.0  # match bench_vibrato_spectrogram
VIBRATO_DEPTH = 0.03  # ±3% pitch

META_ARMS = (
    ("ours_hybrid", "hybrid_lstm", "Ours (hybrid GA–PPO)"),
    ("random", "random", "Random"),
    ("cmaes", "cmaes", "CMA-ES"),
    ("reinforce", "reinforce", "REINFORCE"),
    ("aging_evo", "aging_evo", "Aging evo"),
    ("tpe", "tpe", "TPE"),
)

REELWT_MAGIC = b"REELWT"
VARIANT_REELWT = {
    "nobake": "nobake",
    "dualcosine": "dualcosine",
    "ours_hybrid": "ours",
}


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


def render_vibrato(
    cycle: np.ndarray,
    *,
    sr: int,
    base_hz: float,
    duration_s: float,
    vib_rate_hz: float,
    vib_depth: float,
) -> np.ndarray:
    """Linear-interpolated wavetable playback with sinusoidal pitch vibrato."""
    table = np.asarray(cycle, dtype=np.float64).reshape(-1)
    n = len(table)
    if n < 2:
        return np.zeros(int(round(sr * duration_s)), dtype=np.float64)
    n_out = int(round(sr * duration_s))
    t = np.arange(n_out, dtype=np.float64) / float(sr)
    freq = base_hz * (1.0 + vib_depth * np.sin(2.0 * math.pi * vib_rate_hz * t))
    phase_inc = (n * freq) / float(sr)
    phase = np.cumsum(phase_inc) % n
    idx = np.floor(phase).astype(np.int64)
    frac = phase - idx
    a = table[idx]
    b = table[(idx + 1) % n]
    return a + frac * (b - a)


def apply_factory_lead_fx(mono: np.ndarray, sr: int = SR) -> np.ndarray:
    """Light chorus + short delay approximating hear_presets Factory Lead FX rack."""
    x = np.asarray(mono, dtype=np.float64).reshape(-1)
    n = len(x)
    if n == 0:
        return x
    t = np.arange(n, dtype=np.float64) / float(sr)

    # Chorus: two modulated taps (rate≈0.8 Hz, depth≈0.35 → ~±3.5 ms), mix 0.22
    base_delay = int(round(0.012 * sr))
    depth_samp = 0.0035 * sr
    chorus = np.zeros(n, dtype=np.float64)
    for phase0, amp in ((0.0, 0.55), (2.1, 0.45)):
        delay = base_delay + depth_samp * np.sin(2.0 * math.pi * 0.8 * t + phase0)
        src = np.arange(n, dtype=np.float64) - delay
        src = np.clip(src, 0.0, n - 1.001)
        i0 = np.floor(src).astype(np.int64)
        frac = src - i0
        i1 = np.minimum(i0 + 1, n - 1)
        chorus += amp * (x[i0] * (1.0 - frac) + x[i1] * frac)
    wet_c = 0.22
    y = (1.0 - wet_c) * x + wet_c * chorus

    # Delay: 120 ms, feedback 0.28, mix 0.18
    d = int(round(0.120 * sr))
    if d >= n:
        return y
    out = y.copy()
    fb = 0.28
    mix = 0.18
    delayed = np.zeros(n, dtype=np.float64)
    delayed[d:] = y[:-d]
    # one-pole-ish feedback smear (cheap)
    delayed[d:] += fb * delayed[:-d]
    return (1.0 - mix) * out + mix * delayed


def read_reelwt_frame0(path: Path) -> np.ndarray:
    data = path.read_bytes()
    if len(data) < 16 or data[:6] != REELWT_MAGIC:
        raise ValueError(f"bad reelwt: {path}")
    _version, num_frames, frame_size = struct.unpack_from("<HII", data, 6)
    expected = 16 + num_frames * frame_size * 4
    if len(data) != expected:
        raise ValueError(f"reelwt size mismatch: {path}")
    samples = np.frombuffer(data, dtype="<f4", offset=16)
    return np.asarray(samples[:frame_size], dtype=np.float64).copy()


def find_preset_reelwt(tile: int, short: str) -> Path | None:
    """Locate hear_presets NN_tileK_{short}.reelwt for a fixed tile."""
    tag = VARIANT_REELWT.get(short, short)
    matches = sorted(HEAR_PRESETS.glob(f"*_*tile{tile}_{tag}.reelwt"))
    return matches[0] if matches else None


def vib_kwargs(args: argparse.Namespace) -> dict:
    return dict(
        sr=args.sr,
        base_hz=args.base_hz,
        duration_s=args.duration,
        vib_rate_hz=args.vib_rate,
        vib_depth=args.vib_depth,
    )


def render_method_wav(
    cycle: np.ndarray,
    path: Path,
    args: argparse.Namespace,
    *,
    with_fx: bool,
) -> None:
    audio = render_vibrato(cycle, **vib_kwargs(args))
    if with_fx:
        audio = apply_factory_lead_fx(audio, sr=args.sr)
    write_wav_mono(path, audio, sr=args.sr)


def export_meta(args: argparse.Namespace, device: torch.device) -> dict:
    ideal_b, eng_b, hold_note = load_holdout(device)
    tiles = [int(t) for t in args.tiles]
    for t in tiles:
        if t < 0 or t >= eng_b.shape[0]:
            raise SystemExit(f"tile {t} out of range 0..{eng_b.shape[0]-1}")

    dual_b = og.dual_cosine_blend(eng_b)
    meta_dir = args.out_dir / "meta"
    meta_dir.mkdir(parents=True, exist_ok=True)

    # Baseline cycles: prefer hear_presets reelwt (Factory Lead banks)
    baseline_cycles: dict[str, dict[int, np.ndarray]] = {
        "nobake": {},
        "dualcosine": {},
        "ours_hybrid": {},
    }
    baseline_source: dict[str, str] = {}
    for short in ("nobake", "dualcosine", "ours_hybrid"):
        used_preset = 0
        for tile in tiles:
            p = find_preset_reelwt(tile, short)
            if p is not None and not args.force_refit_cycles:
                baseline_cycles[short][tile] = read_reelwt_frame0(p)
                used_preset += 1
        if used_preset == len(tiles):
            baseline_source[short] = "hear_presets.reelwt"
        else:
            baseline_source[short] = "refit_hybrid_lstm"

    need_hybrid = (
        any(baseline_source[s] != "hear_presets.reelwt" for s in baseline_cycles)
        or any(a == "hybrid_lstm" for _, a, _ in META_ARMS)
    )
    hybrid_ours_b = None
    hybrid_meta = None
    if need_hybrid or args.include_meta_arms:
        summary_path = args.meta_dir / "hybrid_lstm" / "summary.json"
        if not summary_path.is_file():
            raise SystemExit(f"missing champion summary: {summary_path}")
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        print(f"refitting hybrid_lstm on {device} …")
        cfg, cell, train_r, fit_meta = refit_champ(summary, device)
        hybrid_meta = {
            "champ_raw": summary.get("champ_raw"),
            "delta_r_vs_dual_cosine": summary.get("delta_r_vs_dual_cosine"),
            "refit": fit_meta,
            "train_r_last": train_r,
            "champ_arch": cfg.to_dict(),
        }
        with torch.no_grad():
            hybrid_ours_b = og.apply_ops(eng_b, cell, cfg.ops)
        for tile in tiles:
            if tile not in baseline_cycles["nobake"]:
                baseline_cycles["nobake"][tile] = eng_b[tile].detach().cpu().numpy()
            if tile not in baseline_cycles["dualcosine"]:
                baseline_cycles["dualcosine"][tile] = dual_b[tile].detach().cpu().numpy()
            if tile not in baseline_cycles["ours_hybrid"]:
                baseline_cycles["ours_hybrid"][tile] = hybrid_ours_b[tile].detach().cpu().numpy()

    # Fill any remaining gaps from tensors
    for tile in tiles:
        baseline_cycles["nobake"].setdefault(tile, eng_b[tile].detach().cpu().numpy())
        baseline_cycles["dualcosine"].setdefault(tile, dual_b[tile].detach().cpu().numpy())
        if hybrid_ours_b is not None:
            baseline_cycles["ours_hybrid"].setdefault(
                tile, hybrid_ours_b[tile].detach().cpu().numpy()
            )

    arm_cycles: dict[str, torch.Tensor | None] = {"ours_hybrid": hybrid_ours_b}
    arm_info: dict[str, dict] = {}
    if hybrid_meta is not None:
        arm_info["ours_hybrid"] = hybrid_meta

    if args.include_meta_arms:
        for short, approach, display in META_ARMS:
            if short == "ours_hybrid":
                continue
            summary_path = args.meta_dir / approach / "summary.json"
            if not summary_path.is_file():
                print(f"skip meta arm {approach}: no summary")
                continue
            summary = json.loads(summary_path.read_text(encoding="utf-8"))
            if not summary.get("champ_arch"):
                print(f"skip meta arm {approach}: empty champ_arch")
                continue
            print(f"refitting {approach} on {device} …")
            cfg, cell, train_r, fit_meta = refit_champ(summary, device)
            with torch.no_grad():
                healed = og.apply_ops(eng_b, cell, cfg.ops)
            arm_cycles[short] = healed
            arm_info[short] = {
                "approach_code": approach,
                "approach_display": display,
                "champ_raw": summary.get("champ_raw"),
                "delta_r_vs_dual_cosine": summary.get("delta_r_vs_dual_cosine"),
                "refit": fit_meta,
                "train_r_last": train_r,
            }

    written: list[str] = []
    samples: list[dict] = []
    with_fx = not args.dry_cycle_only

    for rank, tile in enumerate(tiles, start=1):
        ideal = ideal_b[tile : tile + 1]
        eng = eng_b[tile : tile + 1]
        dual = dual_b[tile : tile + 1]
        files: dict[str, str] = {}
        scores: dict[str, float] = {
            "no_bake": score_batch(ideal, eng),
            "dual_cosine": score_batch(ideal, dual),
        }
        if hybrid_ours_b is not None:
            scores["ours_hybrid"] = score_batch(ideal, hybrid_ours_b[tile : tile + 1])

        # Classical + Ours from baseline (prefer presets)
        for short in ("nobake", "dualcosine", "ours_hybrid"):
            cycle = baseline_cycles[short][tile]
            name = f"{rank:02d}_tile{tile}_{short}.wav"
            path = meta_dir / name
            render_method_wav(cycle, path, args, with_fx=with_fx)
            files[short] = name
            written.append(str(path.resolve()))
            print(f"wrote {path.resolve()}")

        # Other meta arms
        for short, approach, _display in META_ARMS:
            if short == "ours_hybrid":
                continue
            tens = arm_cycles.get(short)
            if tens is None:
                continue
            scores[short] = score_batch(ideal, tens[tile : tile + 1])
            cycle = tens[tile].detach().cpu().numpy()
            name = f"{rank:02d}_tile{tile}_{short}.wav"
            path = meta_dir / name
            render_method_wav(cycle, path, args, with_fx=with_fx)
            files[short] = name
            written.append(str(path.resolve()))
            print(f"wrote {path.resolve()}")

        samples.append(
            {
                "sample_index": rank,
                "tile_index": int(tile),
                "eval_seed": EVAL_SEED,
                "wrap_abs": float((eng[0, 0] - eng[0, -1]).abs().item()),
                "R": scores,
                "files": files,
                "paper_heal_tile": tile == 46,
                "cycle_source": dict(baseline_source),
            }
        )

    return {
        "holdout_source": hold_note,
        "arms": arm_info,
        "samples": samples,
        "absolute_paths": written,
        "factory_lead_fx": with_fx,
        "baseline_cycle_source": baseline_source,
    }


def export_transfer(args: argparse.Namespace, device: torch.device) -> dict:
    try:
        from signal_heal.datasets import DomainBatcher, ensure_bundles  # noqa: WPS433
    except Exception as exc:  # pragma: no cover
        print(f"transfer skip: cannot import signal_heal ({exc})")
        return {"datasets": {}, "absolute_paths": [], "note": str(exc)}

    bundles = ensure_bundles(force=False)
    out: dict = {"datasets": {}, "absolute_paths": []}
    with_fx = not args.dry_cycle_only
    n_per = int(args.transfer_n_samples)

    for name, bundle in bundles.items():
        if bundle is None:
            continue
        summary_path = TRANSFER_ROOT / name / "hybrid_lstm" / "summary.json"
        if not summary_path.is_file():
            continue
        summary = json.loads(summary_path.read_text(encoding="utf-8"))
        if not summary.get("champ_arch"):
            continue
        print(f"transfer {name}: refit on {device} …")
        batcher = DomainBatcher(bundle, device)
        cfg = og.ArchConfig.from_dict(summary["champ_arch"])
        hp = summary.get("champ_hp") or {}
        cell = og.SeamCell(cfg).to(device)
        orig = og.make_batch
        og.make_batch = batcher  # type: ignore[assignment]
        try:
            torch.manual_seed(SEARCH_SEED)
            if device.type == "cuda":
                torch.cuda.manual_seed_all(SEARCH_SEED)
            og.fit_cell(
                cell,
                cfg.ops,
                device,
                steps=int(hp.get("fit_steps", 48)),
                batch=int(hp.get("batch", 48)),
                lr=float(hp.get("lr", 3e-3)),
                adv_coef=float(hp.get("adv_coef", 0.0)),
            )
            cell.eval()
        finally:
            og.make_batch = orig

        ideal, eng = batcher.holdout(64, seed=EVAL_SEED)
        with torch.no_grad():
            dual = og.dual_cosine_blend(eng)
            ours = og.apply_ops(eng, cell, cfg.ops)
            wrap = (eng[:, 0] - eng[:, -1]).abs().detach().cpu().numpy()
        order = list(np.argsort(-wrap))
        picked = order[:n_per]
        ds_dir = args.out_dir / "transfer" / name
        ds_dir.mkdir(parents=True, exist_ok=True)
        files: list[str] = []
        for rank, ti in enumerate(picked, start=1):
            for tag, tens in (("engine", eng), ("dualcosine", dual), ("ours", ours)):
                cycle = tens[ti].detach().cpu().numpy()
                fname = f"{rank:02d}_tile{ti}_{tag}.wav"
                path = ds_dir / fname
                render_method_wav(cycle, path, args, with_fx=with_fx)
                files.append(fname)
                out["absolute_paths"].append(str(path.resolve()))
                print(f"wrote {path.resolve()}")
        out["datasets"][name] = {
            "dir": str((ds_dir).relative_to(ROOT)).replace("\\", "/"),
            "files": files,
            "champ_raw": summary.get("champ_raw"),
            "holdout_refit_R": summary.get("holdout_refit_R"),
            "picked_tiles": [int(x) for x in picked],
        }
    return out


def write_readme(args: argparse.Namespace, manifest: dict) -> None:
    meta = manifest.get("meta", {})
    samples = meta.get("samples") or []
    lines = [
        "# Hear vibrato method comparison",
        "",
        "Side-by-side **looped vibrato** WAVs for DenoiseOpt methods (wavetable meta + optional transfer).",
        "",
        "## Playback",
        "",
        f"- Sample rate: **{args.sr} Hz**, mono PCM16",
        f"- Base pitch: **{args.base_hz} Hz (A4)** with sinusoidal vibrato "
        f"**{args.vib_rate} Hz**, depth **±{100 * args.vib_depth:.1f}%** "
        f"(same family as `scripts/bench_vibrato_spectrogram.py`)",
        f"- Duration: **{args.duration} s**",
        f"- Factory Lead FX post: **{'on (chorus+delay approx)' if not args.dry_cycle_only else 'off (dry cycle)'}**",
        f"- Holdout / eval seed: **{EVAL_SEED}**; refit seed: **{SEARCH_SEED}**",
        "",
        "## How to compare",
        "",
        "1. Open `meta/` clips for the same tile (e.g. `01_tile46_*`).",
        "2. Play **nobake → dualcosine → ours_hybrid** (then other meta arms if present).",
        "3. Listen for wrap/seam clicks under pitch motion; Ours should be smoother.",
        "4. Optional: `transfer/<dataset>/` for engine / DualCosine / Ours on non-WT domains.",
        "",
        "Any audio player works (VLC, foobar, Windows Media Player). For A/B, loop or scrub the same region.",
        "",
        "## Meta samples",
        "",
        "| # | Tile | Files |",
        "|---|------|-------|",
    ]
    for e in samples:
        flag = " (paper heal)" if e.get("paper_heal_tile") else ""
        flist = ", ".join(f"`{v}`" for v in e.get("files", {}).values())
        lines.append(f"| {e['sample_index']} | {e['tile_index']}{flag} | {flist} |")

    xfer = manifest.get("transfer", {}).get("datasets") or {}
    if xfer:
        lines += ["", "## Transfer domains", ""]
        for ds, info in xfer.items():
            lines.append(f"- `{ds}/` — {len(info.get('files') or [])} WAVs "
                         f"(tiles {info.get('picked_tiles')})")

    lines += [
        "",
        "## Rebuild",
        "",
        "```bash",
        ".venv_gpu/Scripts/python.exe scripts/export_hear_vibrato_methods.py --device cpu",
        "```",
        "",
        "Skip other meta arms / transfer:",
        "",
        "```bash",
        ".venv_gpu/Scripts/python.exe scripts/export_hear_vibrato_methods.py "
        "--device cpu --no-meta-arms --no-transfer",
        "```",
        "",
        "See `manifest.json` for absolute paths and cycle R scores.",
        "",
    ]
    (args.out_dir / "README.md").write_text("\n".join(lines), encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--device", type=str, default="cpu", help="Prefer cpu when GPU search is busy")
    ap.add_argument("--meta-dir", type=Path, default=META_DIR)
    ap.add_argument("--out-dir", type=Path, default=OUT_DIR)
    ap.add_argument("--tiles", type=int, nargs="+", default=FIXED_TILES)
    ap.add_argument("--sr", type=int, default=SR)
    ap.add_argument("--base-hz", type=float, default=BASE_FREQ_HZ)
    ap.add_argument("--vib-rate", type=float, default=VIBRATO_RATE_HZ)
    ap.add_argument("--vib-depth", type=float, default=VIBRATO_DEPTH)
    ap.add_argument("--duration", type=float, default=DURATION_S)
    ap.add_argument("--include-meta-arms", action=argparse.BooleanOptionalAction, default=True)
    ap.add_argument("--transfer", action=argparse.BooleanOptionalAction, default=True)
    ap.add_argument("--transfer-n-samples", type=int, default=2)
    ap.add_argument(
        "--dry-cycle-only",
        action="store_true",
        help="Skip Factory Lead chorus/delay post (pure vibrato cycle)",
    )
    ap.add_argument(
        "--force-refit-cycles",
        action="store_true",
        help="Ignore hear_presets .reelwt; always use refit tensors",
    )
    args = ap.parse_args()

    if args.device == "cuda" and not torch.cuda.is_available():
        args.device = "cpu"
    device = torch.device(args.device)
    print(f"device={device} out={args.out_dir}")

    args.out_dir.mkdir(parents=True, exist_ok=True)
    meta_block = export_meta(args, device)
    transfer_block: dict = {"datasets": {}, "absolute_paths": []}
    if args.transfer:
        transfer_block = export_transfer(args, device)

    all_paths = list(meta_block.get("absolute_paths") or []) + list(
        transfer_block.get("absolute_paths") or []
    )
    method_map: dict[str, list[str]] = {}
    for p in all_paths:
        stem = Path(p).stem
        # ..._method at end after tile id
        parts = stem.split("_")
        # e.g. 01_tile46_ours_hybrid → ours_hybrid; 01_tile46_nobake → nobake
        if len(parts) >= 3 and parts[1].startswith("tile"):
            method = "_".join(parts[2:])
        else:
            method = stem
        method_map.setdefault(method, []).append(p)

    manifest = {
        "schema": "denoiseopt.hear_vibrato_methods.v1",
        "eval_seed": EVAL_SEED,
        "search_seed": SEARCH_SEED,
        "sample_rate": args.sr,
        "base_hz": args.base_hz,
        "duration_s": args.duration,
        "vibrato_rate_hz": args.vib_rate,
        "vibrato_depth": args.vib_depth,
        "factory_lead_fx": not args.dry_cycle_only,
        "playback": (
            f"Linear wavetable vibrato at {args.base_hz} Hz, rate={args.vib_rate} Hz, "
            f"depth=±{100 * args.vib_depth:.1f}%, {args.duration}s @ {args.sr} Hz mono PCM16. "
            + (
                "Factory Lead–inspired chorus+delay applied after vibrato."
                if not args.dry_cycle_only
                else "Dry cycle (no FX post)."
            )
        ),
        "method_to_wavs": method_map,
        "meta": meta_block,
        "transfer": transfer_block,
        "absolute_paths": all_paths,
        "n_wavs": len(all_paths),
    }
    man_path = args.out_dir / "manifest.json"
    man_path.write_text(json.dumps(manifest, indent=2) + "\n", encoding="utf-8")
    write_readme(args, manifest)
    print(f"wrote {man_path.resolve()}")
    print(f"wrote {(args.out_dir / 'README.md').resolve()}")
    print(json.dumps({"n_wavs": len(all_paths), "tiles": list(args.tiles)}, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
