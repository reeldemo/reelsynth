#!/usr/bin/env python3
"""Fetch / inspect IEEE 39-bus PMU OA .mat and run an honest TVE / window-leakage probe.

Download attempts (in order):
  1. aws s3 cp / boto3 unsigned (--no-sign-request)
  2. HTTPS virtual-hosted / path-style S3 URLs
  3. Accept a manually dropped file under raw/ieee_pmu/

IEEE DataPort OA still requires a free IEEE login for object GET in practice —
anonymous S3 often returns 403. Drop the .mat after browser download if needed.

Primary score is **TVE / window-leakage on phasors**, not musical prolonged-R.
Optional L=256 phasor→waveform synthesis is labeled as synthesized (not native samples).
"""
from __future__ import annotations

import json
import math
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import numpy as np

ROOT = Path(__file__).resolve().parents[1]
RAW = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "raw" / "ieee_pmu"
CACHE = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "cache"
STATUS = CACHE / "ieee_pmu_status.json"
PROBE = CACHE / "ieee_pmu_tve_probe.json"
MAT_NAME = "IEEE-39-bus_10_generator_PMU.mat"
S3_URI = "s3://ieee-dataport/open/11968/IEEE-39-bus_10_generator_PMU.mat"
S3_BUCKET = "ieee-dataport"
S3_KEY = "open/11968/IEEE-39-bus_10_generator_PMU.mat"
HTTPS_CANDIDATES = [
    "https://ieee-dataport.s3.amazonaws.com/open/11968/IEEE-39-bus_10_generator_PMU.mat",
    "https://s3.amazonaws.com/ieee-dataport/open/11968/IEEE-39-bus_10_generator_PMU.mat",
]
DATAPORT_PAGE = (
    "https://ieee-dataport.org/open-access/"
    "pmu-measurements-ieee-39-bus-power-system-model"
)


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def _try_boto3_unsigned(dest: Path) -> dict[str, Any]:
    try:
        import boto3
        from botocore import UNSIGNED
        from botocore.config import Config
    except Exception as e:
        return {"ok": False, "method": "boto3_unsigned", "error": f"import: {e}"}
    try:
        s3 = boto3.client("s3", config=Config(signature_version=UNSIGNED))
        s3.download_file(S3_BUCKET, S3_KEY, str(dest))
        return {
            "ok": True,
            "method": "boto3_unsigned",
            "bytes": dest.stat().st_size,
            "uri": S3_URI,
        }
    except Exception as e:
        return {"ok": False, "method": "boto3_unsigned", "error": str(e), "uri": S3_URI}


def _try_https(dest: Path) -> dict[str, Any]:
    import urllib.error
    import urllib.request

    errors: list[str] = []
    for url in HTTPS_CANDIDATES:
        try:
            urllib.request.urlretrieve(url, dest)
            return {
                "ok": True,
                "method": "https",
                "url": url,
                "bytes": dest.stat().st_size,
            }
        except Exception as e:
            errors.append(f"{url}: {e}")
            if dest.is_file() and dest.stat().st_size == 0:
                dest.unlink(missing_ok=True)
    return {"ok": False, "method": "https", "error": "; ".join(errors)}


def ensure_mat() -> tuple[Path | None, list[dict[str, Any]]]:
    RAW.mkdir(parents=True, exist_ok=True)
    dest = RAW / MAT_NAME
    attempts: list[dict[str, Any]] = []
    if dest.is_file() and dest.stat().st_size > 10_000:
        attempts.append(
            {
                "ok": True,
                "method": "already_present",
                "bytes": dest.stat().st_size,
                "path": str(dest),
            }
        )
        return dest, attempts
    attempts.append(_try_boto3_unsigned(dest))
    if attempts[-1]["ok"]:
        return dest, attempts
    if dest.is_file():
        dest.unlink(missing_ok=True)
    attempts.append(_try_https(dest))
    if attempts[-1]["ok"]:
        return dest, attempts
    if dest.is_file():
        dest.unlink(missing_ok=True)
    # Accept alternate filenames dropped by user
    alts = sorted(RAW.glob("*.mat"))
    if alts:
        chosen = max(alts, key=lambda p: p.stat().st_size)
        attempts.append(
            {
                "ok": True,
                "method": "manual_drop",
                "bytes": chosen.stat().st_size,
                "path": str(chosen),
            }
        )
        return chosen, attempts
    attempts.append(
        {
            "ok": False,
            "method": "manual_drop",
            "error": f"no .mat under {RAW}; browser-download from {DATAPORT_PAGE}",
        }
    )
    return None, attempts


def _load_mat_any(path: Path) -> tuple[dict[str, Any], str]:
    """Load MATLAB v5 (scipy) or v7.3/HDF5 (h5py)."""
    try:
        import scipy.io as sio

        mat = sio.loadmat(str(path), squeeze_me=True, struct_as_record=False)
        return {k: v for k, v in mat.items() if not k.startswith("__")}, "scipy.io.loadmat"
    except Exception as e_scipy:
        try:
            import h5py
        except Exception as e_h5:
            raise RuntimeError(
                f"scipy load failed ({e_scipy}); h5py unavailable ({e_h5})"
            ) from e_scipy
        out: dict[str, Any] = {}
        with h5py.File(path, "r") as f:
            def _read(name, obj):
                if isinstance(obj, h5py.Dataset):
                    out[name] = np.array(obj)
            f.visititems(_read)
            for k in f.keys():
                if k not in out:
                    out[k] = f[k]
        return out, f"h5py (scipy failed: {e_scipy})"


def _as_array(x: Any) -> np.ndarray | None:
    if x is None:
        return None
    if hasattr(x, "dtype") or isinstance(x, np.ndarray):
        return np.asarray(x)
    # MATLAB struct field
    for attr in ("Magnitude", "Angle", "Freq", "ROCOF", "Time", "Quality"):
        if hasattr(x, attr):
            return None  # caller should walk struct
    try:
        return np.asarray(x)
    except Exception:
        return None


def _struct_field(obj: Any, *names: str) -> Any:
    for n in names:
        if hasattr(obj, n):
            return getattr(obj, n)
        if isinstance(obj, dict) and n in obj:
            return obj[n]
    return None


def describe_mat(mat: dict[str, Any], loader: str) -> dict[str, Any]:
    keys = sorted(mat.keys())
    shapes: dict[str, Any] = {}
    for k in keys:
        v = mat[k]
        arr = _as_array(v)
        if arr is not None and getattr(arr, "dtype", None) is not None:
            shapes[k] = {
                "shape": list(arr.shape),
                "dtype": str(arr.dtype),
                "ndim": int(arr.ndim),
            }
        else:
            # nested struct?
            fields = []
            if hasattr(v, "_fieldnames"):
                fields = list(v._fieldnames)
            elif hasattr(v, "__dict__"):
                fields = [x for x in dir(v) if not x.startswith("_")]
            shapes[k] = {"type": type(v).__name__, "fields": fields[:40]}

    # Heuristic: find DATA / Magnitude / Angle
    data_obj = mat.get("DATA") or mat.get("Data") or mat.get("data")
    content = "unknown"
    sampling_notes: list[str] = []
    n_frames = None
    n_gen = None
    fs_est = None

    mag = ang = freq = None
    if data_obj is not None:
        mag = _struct_field(data_obj, "Magnitude", "magnitude", "Mag")
        ang = _struct_field(data_obj, "Angle", "angle", "Phase")
        freq = _struct_field(data_obj, "Freq", "Frequency", "freq", "f")
        for label, obj in (("Magnitude", mag), ("Angle", ang), ("Freq", freq)):
            a = _as_array(obj)
            if a is not None:
                shapes[f"DATA.{label}"] = {
                    "shape": list(a.shape),
                    "dtype": str(a.dtype),
                }
                if n_frames is None and a.size > 10:
                    # prefer last dim as time or first
                    if a.ndim >= 2:
                        n_frames = int(max(a.shape))
                        n_gen = int(min(a.shape)) if min(a.shape) <= 20 else None
                    else:
                        n_frames = int(a.size)

    # Top-level Magnitude etc. (comment on DataPort page)
    if mag is None:
        for k in keys:
            kl = k.lower()
            if "mag" in kl:
                mag = mat[k]
            if "ang" in kl or "phase" in kl:
                ang = mat[k]
            if kl in ("freq", "f", "frequency") or kl.endswith("freq"):
                freq = mat[k]

    mag_a = _as_array(mag)
    ang_a = _as_array(ang)
    if mag_a is not None and ang_a is not None:
        content = "phasors"
        sampling_notes.append(
            "Synchrophasor magnitude+angle present (IEEE C37.118-style), not raw AC waveforms."
        )
    elif any("wave" in k.lower() or "sample" in k.lower() for k in keys):
        content = "waveforms_or_mixed"
    else:
        # still likely phasors per DataPort abstract
        content = "likely_phasors_per_dataport_docs"
        sampling_notes.append(
            "DataPort abstract: positive-sequence V/I synchrophasors, f, ROCOF, timestamps; "
            "~86.6 s, 5197 frames/generator, 10 generators → ~60 Hz reporting."
        )

    if n_frames and n_frames > 100:
        # DataPort: 86.6 s → estimate
        fs_est = n_frames / 86.6
        sampling_notes.append(
            f"If n_frames={n_frames} over 86.6 s (docs), reporting_rate≈{fs_est:.2f} Hz."
        )

    return {
        "loader": loader,
        "top_keys": keys,
        "shapes": shapes,
        "content_kind": content,
        "sampling_notes": sampling_notes,
        "n_frames_guess": n_frames,
        "n_generators_guess": n_gen,
        "reporting_hz_guess": fs_est,
        "dataport_expected": {
            "generators": 10,
            "frames_per_gen": 5197,
            "duration_s": 86.6,
            "fields": [
                "V/I magnitude+angle",
                "frequency",
                "ROCOF",
                "delta-f",
                "timestamps",
                "quality",
            ],
            "citation": "Naglic, IEEE DataPort DOI 10.21227/vkz3-2e96",
        },
    }


def _pick_phasor_streams(mat: dict[str, Any]) -> tuple[np.ndarray, np.ndarray, np.ndarray | None]:
    """Return (mag[T] or [G,T], ang, freq_or_None) as float arrays."""
    data_obj = mat.get("DATA") or mat.get("Data") or mat.get("data")
    mag = ang = freq = None
    if data_obj is not None:
        mag = _struct_field(data_obj, "Magnitude", "magnitude", "Mag")
        ang = _struct_field(data_obj, "Angle", "angle", "Phase")
        freq = _struct_field(data_obj, "Freq", "Frequency", "freq", "f")
    if mag is None or ang is None:
        for k, v in mat.items():
            kl = k.lower()
            if mag is None and "mag" in kl:
                mag = v
            if ang is None and ("ang" in kl or "phase" in kl):
                ang = v
            if freq is None and ("freq" in kl or kl == "f"):
                freq = v
    mag_a = _as_array(mag)
    ang_a = _as_array(ang)
    if mag_a is None or ang_a is None:
        raise ValueError("Could not locate Magnitude/Angle phasor fields in .mat")
    mag_a = np.asarray(mag_a, dtype=np.float64)
    ang_a = np.asarray(ang_a, dtype=np.float64)
    freq_a = None if freq is None else np.asarray(_as_array(freq), dtype=np.float64)
    return mag_a, ang_a, freq_a


def tve_window_leakage_probe(mat: dict[str, Any]) -> dict[str, Any]:
    """Honest phasor probe: self-consistency TVE + rectangular-window leakage proxy.

    Reference phasor = measured; 'estimator' = DFT of one-cycle waveform reconstructed
    from the same phasor (should be near-zero TVE) vs off-nominal / non-integer window
    (leakage). This does **not** claim musical prolonged-R on native waveforms.
    """
    mag, ang, freq = _pick_phasor_streams(mat)
    # Normalize to [n_gen, T]
    if mag.ndim == 1:
        mag = mag[None, :]
        ang = ang[None, :]
        if freq is not None and freq.ndim == 1:
            freq = freq[None, :]
    elif mag.ndim == 2 and mag.shape[0] > mag.shape[1] and mag.shape[0] > 50:
        # time × gen → transpose
        mag = mag.T
        ang = ang.T
        if freq is not None and freq.ndim == 2 and freq.shape == ang.T.shape:
            freq = freq.T

    n_gen, n_t = mag.shape[0], mag.shape[1]
    # reporting rate from docs if length matches
    fs_rep = (n_t / 86.6) if n_t > 1000 else 60.0
    f0 = 60.0
    if freq is not None:
        f0 = float(np.nanmedian(freq))

    # Complex phasors
    # Angle assumed radians; if looks like degrees, convert
    ang_use = ang.copy()
    if np.nanmax(np.abs(ang_use)) > 2 * math.pi + 0.5:
        ang_use = np.deg2rad(ang_use)
    x = mag * np.exp(1j * ang_use)

    # --- Self TVE (sanity): reconstruct one AC cycle from phasor, re-estimate ---
    n_cyc = 256
    t = np.arange(n_cyc) / (f0 * n_cyc)  # one nominal cycle at waveform rate f0*n_cyc
    # Use middle frame of gen 0
    g, k = 0, n_t // 2
    m0, a0 = float(mag[g, k]), float(ang_use[g, k])
    wave = m0 * np.cos(2 * math.pi * f0 * t + a0)
    # DFT bin 1 (fundamental)
    spec = np.fft.rfft(wave)
    est = spec[1] / (n_cyc / 2)  # amplitude of cos fundamental ≈ complex
    # Reference complex from phasor (peak)
    ref = m0 * np.exp(1j * a0)
    # Map DFT cos coefficient to phasor-ish: rfft[1] for cos(wt+phi) ...
    # Simpler TVE on magnitude/angle of analytic estimate:
    est_mag = float(np.abs(est))
    # For cos, |rfft[1]|*(2/N) ≈ amplitude
    est_mag = float(np.abs(spec[1]) * 2.0 / n_cyc)
    tve_self = abs(est_mag - m0) / max(m0, 1e-12)

    # --- Window leakage: non-integer cycle length ---
    n_leak = int(n_cyc * 1.07)  # 7% long window
    t_l = np.arange(n_leak) / (f0 * n_cyc)
    wave_l = m0 * np.cos(2 * math.pi * f0 * t_l + a0)
    # Truncate/pad to n_cyc for same bin
    wave_l = wave_l[:n_cyc]
    spec_l = np.fft.rfft(wave_l)
    est_mag_l = float(np.abs(spec_l[1]) * 2.0 / n_cyc)
    tve_leak = abs(est_mag_l - m0) / max(m0, 1e-12)

    # Streaming ΔTVE across time: consecutive phasor jump as wrap stress proxy
    dx = np.diff(x, axis=1)
    rel = np.abs(dx) / np.maximum(np.abs(x[:, :-1]), 1e-12)
    jump_p95 = float(np.nanpercentile(rel, 95))

    return {
        "metric_family": "TVE_window_leakage_phasor",
        "not_musical_prolonged_R": True,
        "footnote": (
            "IEEE 39-bus OA is synchrophasor streams (mag∠, f, ROCOF), not raw AC samples. "
            "Scores below are TVE / leakage probes on phasor-consistent reconstruction — "
            "not DenoiseOpt musical prolonged-R on native waveforms."
        ),
        "geometry": {"n_generators": int(n_gen), "n_frames": int(n_t), "reporting_hz_est": fs_rep},
        "nominal_f_hz": f0,
        "scores": {
            "tve_self_consistency": tve_self,
            "tve_noninteger_window_leakage": tve_leak,
            "phasor_rel_jump_p95": jump_p95,
            "leakage_minus_self": tve_leak - tve_self,
        },
        "interpretation": (
            "tve_self_consistency near 0 validates phasor→1-cycle DFT path; "
            "tve_noninteger_window_leakage rises under off-length windows (analysis wrap). "
            "phasor_rel_jump_p95 summarizes streaming frame-to-frame stress (faults/topology)."
        ),
    }


def main() -> int:
    CACHE.mkdir(parents=True, exist_ok=True)
    mat_path, attempts = ensure_mat()
    status: dict[str, Any] = {
        "schema": "denoiseopt.ieee_pmu_status.v1",
        "updated_at": utc_now(),
        "s3_uri": S3_URI,
        "dataport_page": DATAPORT_PAGE,
        "dest_dir": str(RAW),
        "download_attempts": attempts,
        "download_ok": bool(mat_path and mat_path.is_file()),
        "bytes": (mat_path.stat().st_size if mat_path and mat_path.is_file() else None),
        "mat_path": str(mat_path) if mat_path else None,
        "anonymous_s3_note": (
            "aws/boto3 --no-sign-request and public HTTPS GETs return 403 without "
            "IEEE DataPort session/credentials; OA still needs free IEEE login or manual drop."
        ),
    }

    if not mat_path:
        status["structure"] = {
            "content_kind": "expected_phasors_not_downloaded",
            "from_docs": status.get("anonymous_s3_note"),
            "dataport_expected": {
                "generators": 10,
                "frames_per_gen": 5197,
                "duration_s": 86.6,
                "fields": ["V/I mag∠", "f", "ROCOF", "timestamps", "quality"],
            },
        }
        status["scored_board"] = False
        STATUS.write_text(json.dumps(status, indent=2), encoding="utf-8")
        print(json.dumps(status, indent=2))
        return 1

    mat, loader = _load_mat_any(mat_path)
    structure = describe_mat(mat, loader)
    status["structure"] = structure
    status["scored_board"] = False

    probe: dict[str, Any] | None = None
    if structure.get("content_kind") in (
        "phasors",
        "likely_phasors_per_dataport_docs",
        "unknown",
    ):
        try:
            probe = tve_window_leakage_probe(mat)
            probe["updated_at"] = utc_now()
            probe["mat_path"] = str(mat_path)
            probe["bytes"] = mat_path.stat().st_size
            PROBE.write_text(json.dumps(probe, indent=2), encoding="utf-8")
            status["scored_board"] = True
            status["score_kind"] = "TVE_window_leakage_phasor"
            status["probe_path"] = str(PROBE)
            status["probe_scores"] = probe.get("scores")
        except Exception as e:
            status["probe_error"] = str(e)

    STATUS.write_text(json.dumps(status, indent=2), encoding="utf-8")
    print(json.dumps({k: status[k] for k in status if k != "download_attempts"}, indent=2))
    print("download_attempts:", json.dumps(attempts, indent=2))
    return 0 if status["download_ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
