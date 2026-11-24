#!/usr/bin/env python3
"""Build deterministic real-seam eval corpora for DenoiseOpt (AKWF + factory/FX).

Unlike overnight_gpu_rl_arch.make_batch (synthetic two-harmonic + cliff), this set uses
real waveforms with mis-set loop points and defined clean references.

Outputs:
  - real_seam_set.npz (ideal, engine float32 arrays [B, N])
  - real_seam_manifest.json (per-sample provenance + checksums)

Tensors match make_batch / holdout_batch.pt: N=256, no explicit sample rate in tensor
(DSP trains on normalized cycle vectors; source WAVs may be 44.1 kHz before resample).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import random
import sys
import urllib.request
import wave
import zipfile
from pathlib import Path
from typing import Any

import numpy as np
import torch
import torch.nn.functional as F

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402

N = int(og.N)
SEAM_W = int(og.SEAM_W)
PROLONG = int(og.PROLONG)
DEFAULT_SEED = 20_260_725
DEFAULT_OUT = ROOT / "brand" / "artifacts" / "real_seam_eval"
DEFAULT_AKWF_DIR = ROOT / "brand" / "artifacts" / "real_wt_cycles" / "oa_akwf"
FACTORY_FIXTURES = ROOT / "tests" / "fixtures" / "audio"
FACTORY_EXPORT = ROOT / "brand" / "artifacts" / "real_wt_cycles" / "reelsynth_export_cycles.json"

AKWF_PACK_URL = "https://www.adventurekid.se/aktf/AKWF-Complete.zip"
AKWF_LICENSE = "CC0 (Adventure Kid Waveforms)"

# Integer and fractional loop-offset ks (fractional → resample phase in [0, 1)).
AKWF_K_OFFSETS: tuple[float, ...] = (1.0, 3.0, 7.0, 13.0, 0.25, 0.5, 1.75)
EXPORT_K_OFFSETS: tuple[float, ...] = (2.0, 5.0, 11.0, 0.33, 1.5)


def sha256_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def sha256_tensor(t: torch.Tensor) -> str:
    arr = t.detach().cpu().numpy().astype(np.float32, copy=False)
    return sha256_bytes(arr.tobytes())


def resample_1d(cycle: torch.Tensor, length: int = N) -> torch.Tensor:
    if cycle.dim() == 1:
        cycle = cycle.unsqueeze(0)
    if cycle.shape[1] == length:
        return cycle
    x = cycle.unsqueeze(1)
    y = F.interpolate(x, size=length, mode="linear", align_corners=False)
    return y.squeeze(1)


def peak_normalize(cycle: torch.Tensor, eps: float = 1e-6) -> torch.Tensor:
    peak = cycle.abs().amax(dim=-1, keepdim=True).clamp_min(eps)
    return cycle / peak


def read_wav_mono(path: Path) -> torch.Tensor:
    with wave.open(str(path), "rb") as wf:
        nch = wf.getnchannels()
        sw = wf.getsampwidth()
        nframes = wf.getnframes()
        raw = wf.readframes(nframes)
    if sw == 2:
        import array

        arr = array.array("h")
        arr.frombytes(raw)
        samples = torch.tensor(arr, dtype=torch.float32) / 32768.0
    elif sw == 1:
        samples = (torch.tensor(list(raw), dtype=torch.float32) - 128.0) / 128.0
    else:
        raise ValueError(f"unsupported sampwidth {sw} in {path}")
    if nch > 1:
        samples = samples.view(-1, nch).mean(dim=1)
    return samples


def resample_with_fractional_offset(samples: torch.Tensor, *, frac_offset: float) -> torch.Tensor:
    """Linear resample entire wav to length N with sub-sample start phase in [0,1)."""
    t = torch.linspace(frac_offset, frac_offset + 1.0, N, device=samples.device)
    src_len = samples.shape[0]
    idx = t * (src_len - 1)
    i0 = torch.floor(idx).long().clamp(0, src_len - 1)
    i1 = (i0 + 1).clamp(0, src_len - 1)
    w = (idx - i0.float()).clamp(0.0, 1.0)
    return (1.0 - w) * samples[i0] + w * samples[i1]


def _interp_1d(xq: torch.Tensor, x: torch.Tensor, y: torch.Tensor) -> torch.Tensor:
    """Linear 1-D interp (x monotone); xq same shape as query grid."""
    xq = xq.clamp(x[0], x[-1])
    hi = torch.searchsorted(x, xq, right=True).clamp(1, x.numel() - 1)
    lo = hi - 1
    w = (xq - x[lo]) / (x[hi] - x[lo] + 1e-12)
    return (1.0 - w) * y[lo] + w * y[hi]


def misset_loop_from_cycle(cycle_n: torch.Tensor, k: float) -> tuple[torch.Tensor, torch.Tensor, dict[str, Any]]:
    """Ideal = correct period; engine = period mis-aligned by k samples (int roll or frac phase)."""
    ideal = cycle_n.clone()
    if float(k).is_integer() and int(k) % N != 0:
        ki = int(k) % N
        eng = torch.roll(ideal, ki, dims=-1)
        meta = {"offset_k": float(ki), "offset_kind": "integer_roll", "seam_index": 0}
    else:
        # Fractional: phase-shift the closed cycle via a shifted resample grid.
        phase = float(k) / float(N)
        t = torch.linspace(phase, phase + 1.0, N, device=cycle_n.device)
        base = torch.linspace(0, 1, N, device=cycle_n.device)
        y = ideal.squeeze(0)
        eng = _interp_1d(t, base, y).unsqueeze(0)
        meta = {"offset_k": float(k), "offset_kind": "fractional_phase", "seam_index": 0}
    return ideal, eng, meta


def nearest_zero_crossing(samples: torch.Tensor, idx: int) -> int:
    n = samples.shape[0]
    if samples[idx] == 0 or (idx > 0 and samples[idx - 1] * samples[idx] <= 0):
        return idx
    for d in range(1, min(n, 4096)):
        for j in (idx - d, idx + d):
            if 0 <= j < n:
                if j > 0 and samples[j - 1] * samples[j] <= 0:
                    return j
                if abs(samples[j].item()) < 1e-5:
                    return j
    return idx


def factory_loop_pair(
    samples: torch.Tensor, *, naive_cut: int, period: int
) -> tuple[torch.Tensor, torch.Tensor, dict[str, Any]]:
    """Reference: zero-crossing loop of length `period` resampled to N; engine: naive non-ZC cut."""
    n = samples.shape[0]
    period = max(64, min(period, n - 1))
    naive_cut = int(naive_cut) % (n - period)
    zc = nearest_zero_crossing(samples, naive_cut)
    ref_start = max(0, min(zc, n - period - 1))
    ref_seg = samples[ref_start : ref_start + period]
    eng_start = naive_cut
    eng_seg = samples[eng_start : eng_start + period]
    if eng_seg.shape[0] < period:
        pad = period - eng_seg.shape[0]
        eng_seg = torch.cat([eng_seg, samples[:pad]])
    ideal = peak_normalize(resample_1d(ref_seg.unsqueeze(0), N))
    eng = peak_normalize(resample_1d(eng_seg.unsqueeze(0), N))
    jump = float((eng[0, -1] - eng[0, 0]).abs().item())
    assert jump > 1e-6 or abs(ref_start - eng_start) > 2, "expected visible wrap discontinuity"
    meta = {
        "reference_type": "zero_crossing_loop",
        "engine_cut_type": "naive_non_zc",
        "ref_start_sample": int(ref_start),
        "eng_start_sample": int(eng_start),
        "period_source_samples": int(period),
        "wrap_jump_abs": jump,
        "seam_index": 0,
    }
    return ideal, eng, meta


def download_akwf_pack(dest: Path) -> tuple[Path, str]:
    dest.mkdir(parents=True, exist_ok=True)
    zip_path = dest / "AKWF-Complete.zip"
    if not zip_path.is_file():
        print(f"downloading {AKWF_PACK_URL} → {zip_path}")
        urllib.request.urlretrieve(AKWF_PACK_URL, zip_path)
    digest = sha256_file(zip_path)
    extract_dir = dest / "extracted"
    if not extract_dir.is_dir() or not any(extract_dir.rglob("*.wav")):
        extract_dir.mkdir(parents=True, exist_ok=True)
        with zipfile.ZipFile(zip_path, "r") as zf:
            zf.extractall(extract_dir)
    return extract_dir, digest


def resolve_akwf_dir(arg_path: Path | None) -> tuple[Path, dict[str, Any]]:
    meta: dict[str, Any] = {"license": AKWF_LICENSE}
    if arg_path is not None:
        if not arg_path.is_dir():
            raise SystemExit(f"--akwf-dir not found: {arg_path}")
        meta["source"] = "user_path"
        meta["path"] = str(arg_path)
        return arg_path, meta
    if DEFAULT_AKWF_DIR.is_dir() and list(DEFAULT_AKWF_DIR.glob("AKWF_*.wav")):
        meta["source"] = "repo_subset"
        meta["path"] = str(DEFAULT_AKWF_DIR)
        return DEFAULT_AKWF_DIR, meta
    staging = ROOT / "brand" / "artifacts" / "real_wt_cycles" / "_akwf_src"
    extract_dir, digest = download_akwf_pack(staging)
    meta.update(
        {
            "source": "downloaded_pack",
            "pack_url": AKWF_PACK_URL,
            "pack_sha256": digest,
            "extracted_dir": str(extract_dir),
        }
    )
    return extract_dir, meta


def load_factory_export_cycles(max_cycles: int) -> torch.Tensor | None:
    if not FACTORY_EXPORT.is_file():
        return None
    blob = json.loads(FACTORY_EXPORT.read_text(encoding="utf-8"))
    cycles = torch.tensor(blob["cycles"], dtype=torch.float32)
    if cycles.shape[1] != N:
        cycles = resample_1d(cycles, N)
    cycles = peak_normalize(cycles)
    if cycles.shape[0] > max_cycles:
        cycles = cycles[:max_cycles]
    return cycles


def build_akwf_entries(
    akwf_dir: Path, rng: random.Random, *, max_files: int
) -> tuple[list[torch.Tensor], list[torch.Tensor], list[dict[str, Any]]]:
    paths = sorted(akwf_dir.rglob("AKWF_*.wav"))
    if not paths:
        paths = sorted(akwf_dir.rglob("*.wav"))
    if not paths:
        raise SystemExit(f"no WAV files under {akwf_dir}")
    rng.shuffle(paths)
    paths = paths[:max_files]
    ideals, engines, manifest = [], [], []
    for p in paths:
        samples = read_wav_mono(p)
        base = peak_normalize(resample_1d(samples.unsqueeze(0), N)).squeeze(0)
        file_sha = sha256_file(p)
        for k in AKWF_K_OFFSETS:
            if k < 1.0:
                # fractional offset from source waveform
                shifted = resample_with_fractional_offset(samples.to(base.device), frac_offset=float(k) / len(samples))
                eng_v = peak_normalize(resample_1d(shifted.unsqueeze(0), N))
                ideal_v = peak_normalize(resample_1d(samples.unsqueeze(0), N))
                jump = float((eng_v[0, -1] - eng_v[0, 0]).abs().item())
                meta = {
                    "subset": "akwf",
                    "source_file": str(p.relative_to(ROOT)) if p.is_relative_to(ROOT) else str(p),
                    "source_sha256": file_sha,
                    "reference_type": "correct_period_resample",
                    "offset_k": float(k),
                    "offset_kind": "fractional_resample",
                    "seam_index": 0,
                    "wrap_jump_abs": jump,
                }
            else:
                ideal_v, eng_v, off = misset_loop_from_cycle(base.unsqueeze(0), k)
                jump = float((eng_v[0, -1] - eng_v[0, 0]).abs().item())
                meta = {
                    "subset": "akwf",
                    "source_file": str(p.relative_to(ROOT)) if p.is_relative_to(ROOT) else str(p),
                    "source_sha256": file_sha,
                    "reference_type": "correct_period_resample",
                    **off,
                    "wrap_jump_abs": jump,
                }
            ideals.append(ideal_v.squeeze(0))
            engines.append(eng_v.squeeze(0))
            manifest.append(meta)
    return ideals, engines, manifest


def build_factory_fx_entries(
    rng: random.Random,
    *,
    fixture_dir: Path | None,
    max_wav_loops: int,
    max_export_cycles: int,
) -> tuple[list[torch.Tensor], list[torch.Tensor], list[dict[str, Any]]]:
    ideals, engines, manifest = [], [], []

    # Loop cuts from mono factory/FX WAV fixtures in-repo.
    wav_dir = fixture_dir or FACTORY_FIXTURES
    wav_paths = sorted(wav_dir.glob("*.wav")) if wav_dir.is_dir() else []
    if not wav_paths:
        note = (
            "No factory/FX loop WAVs found in-repo; pass --factory-dir with mono loops. "
            f"Checked: {wav_dir}"
        )
    else:
        rng.shuffle(wav_paths)
        for p in wav_paths[:max_wav_loops]:
            samples = read_wav_mono(p)
            file_sha = sha256_file(p)
            # Period ≈ 0.25 s at native rate → mapped to N after resample.
            sr_guess = 44100
            period = max(128, min(samples.shape[0] // 2, int(sr_guess * 0.25)))
            for rep in range(3):
                naive = rng.randint(period // 4, min(samples.shape[0] - period - 1, period * 3))
                if samples[naive].abs() < 0.02:
                    naive += max(17, period // 9)  # push off trivial ZC
                ideal, eng, meta = factory_loop_pair(samples, naive_cut=naive, period=period)
                meta.update(
                    {
                        "subset": "factory_fx",
                        "source_file": str(p.relative_to(ROOT)) if p.is_relative_to(ROOT) else str(p),
                        "source_sha256": file_sha,
                        "loop_variant": rep,
                    }
                )
                ideals.append(ideal.squeeze(0))
                engines.append(eng.squeeze(0))
                manifest.append(meta)

    export = load_factory_export_cycles(max_export_cycles)
    if export is not None:
        export_sha = sha256_file(FACTORY_EXPORT)
        idxs = list(range(export.shape[0]))
        rng.shuffle(idxs)
        for i in idxs:
            base = export[i]
            for k in EXPORT_K_OFFSETS:
                ideal, eng, off = misset_loop_from_cycle(base.unsqueeze(0), k)
                jump = float((eng[0, -1] - eng[0, 0]).abs().item())
                manifest.append(
                    {
                        "subset": "factory_fx",
                        "source_file": str(FACTORY_EXPORT.relative_to(ROOT)),
                        "source_sha256": export_sha,
                        "cycle_index": int(i),
                        "reference_type": "reelsynth_export_correct_period",
                        **off,
                        "wrap_jump_abs": jump,
                    }
                )
                ideals.append(ideal.squeeze(0))
                engines.append(eng.squeeze(0))
    elif not wav_paths:
        raise SystemExit(note)
    return ideals, engines, manifest


def assemble_tensors(
    ideals: list[torch.Tensor], engines: list[torch.Tensor], manifest: list[dict[str, Any]]
) -> tuple[torch.Tensor, torch.Tensor, list[dict[str, Any]]]:
    if len(ideals) != len(engines) or not ideals:
        raise SystemExit("empty or mismatched corpus")
    ideal = torch.stack(ideals, dim=0).float()
    eng = torch.stack(engines, dim=0).float()
    for i, row in enumerate(manifest):
        row["sample_index"] = i
        row["ideal_sha256"] = sha256_tensor(ideal[i : i + 1])
        row["engine_sha256"] = sha256_tensor(eng[i : i + 1])
    return ideal, eng, manifest


def build_toy_synthetic() -> tuple[torch.Tensor, torch.Tensor, list[dict[str, Any]]]:
    """Tiny deterministic corpus for unit tests (no external files)."""
    t = torch.linspace(0, 1, N)
    base = torch.sin(2 * math.pi * 3.0 * t) + 0.2 * torch.sin(2 * math.pi * 7.0 * t)
    base = base / base.abs().max()
    ideal, eng, off = misset_loop_from_cycle(base.unsqueeze(0), 9.0)
    meta = {
        "subset": "toy",
        "source_file": "synthetic:toy_sine",
        "source_sha256": sha256_tensor(base.unsqueeze(0)),
        "reference_type": "correct_period",
        **off,
        "wrap_jump_abs": float((eng[0, -1] - eng[0, 0]).abs().item()),
    }
    return ideal, eng, [meta]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--seed", type=int, default=DEFAULT_SEED)
    ap.add_argument("--out-dir", type=Path, default=DEFAULT_OUT)
    ap.add_argument("--akwf-dir", type=Path, default=None, help="AKWF wav root (else repo subset or download pack)")
    ap.add_argument(
        "--factory-dir",
        type=Path,
        default=None,
        help="Optional extra factory/FX loop WAV directory",
    )
    ap.add_argument("--max-akwf-files", type=int, default=48)
    ap.add_argument("--max-wav-loops", type=int, default=12)
    ap.add_argument("--max-export-cycles", type=int, default=24)
    ap.add_argument("--toy-only", action="store_true", help="Write only synthetic toy pair (for tests)")
    args = ap.parse_args()

    rng = random.Random(args.seed)
    args.out_dir.mkdir(parents=True, exist_ok=True)

    if args.toy_only:
        ideal, eng, manifest = build_toy_synthetic()
        akwf_meta = {}
    else:
        akwf_dir, akwf_meta = resolve_akwf_dir(args.akwf_dir)
        i_list, e_list, m_list = build_akwf_entries(akwf_dir, rng, max_files=args.max_akwf_files)
        fi, fe, fm = build_factory_fx_entries(
            rng,
            fixture_dir=args.factory_dir,
            max_wav_loops=args.max_wav_loops,
            max_export_cycles=args.max_export_cycles,
        )
        ideal, eng, manifest = assemble_tensors(i_list + fi, e_list + fe, m_list + fm)

    np.savez_compressed(
        args.out_dir / "real_seam_set.npz",
        ideal=ideal.numpy(),
        engine=eng.numpy(),
        subset=np.array([r.get("subset", "") for r in manifest]),
        N=np.array([N], dtype=np.int32),
        SEAM_W=np.array([SEAM_W], dtype=np.int32),
        PROLONG=np.array([PROLONG], dtype=np.int32),
        seed=np.array([args.seed], dtype=np.int64),
    )
    torch.save(
        {
            "ideal": ideal.cpu(),
            "engine": eng.cpu(),
            "seed": args.seed,
            "N": N,
            "SEAM_W": SEAM_W,
            "PROLONG": PROLONG,
        },
        args.out_dir / "real_seam_set.pt",
    )

    doc = {
        "schema": "denoiseopt.real_seam_set.v1",
        "seed": args.seed,
        "n_samples": int(ideal.shape[0]),
        "tensor_shapes": {"ideal": list(ideal.shape), "engine": list(eng.shape)},
        "N": N,
        "SEAM_W": SEAM_W,
        "PROLONG": PROLONG,
        "primary_metric": "r_blend",
        "akwf_provenance": akwf_meta,
        "factory_fx_note": (
            "Loop reference: zero_crossing_loop on WAV fixtures; "
            "reelsynth_export cycles use correct_period vs integer/fractional mis-set loop. "
            "Engine on WAVs: naive_non_zc cut at deterministic off-crossing points."
        ),
        "samples": manifest,
    }
    (args.out_dir / "real_seam_manifest.json").write_text(json.dumps(doc, indent=2), encoding="utf-8")
    n_akwf = sum(1 for r in manifest if r.get("subset") == "akwf")
    n_ff = sum(1 for r in manifest if r.get("subset") == "factory_fx")
    print(
        json.dumps(
            {
                "out_dir": str(args.out_dir),
                "n_samples": int(ideal.shape[0]),
                "akwf": n_akwf,
                "factory_fx": n_ff,
                "manifest": str(args.out_dir / "real_seam_manifest.json"),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())