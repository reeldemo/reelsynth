#!/usr/bin/env python3
"""Domain adapters: build (ideal, cracked) period batches for wrap/seam transfer.

Period length is fixed to overnight SeamCell ``N=256``.
Residual metric: ``R_blend = α·R_seam(ideal,out) + (1-α)·R_body(eng,out)`` with α=0.7
(``overnight_gpu_rl_arch.residual_score_blend``); search objective ``J = R_blend - λ·latency_norm``
(v10.1). Pure ``R_seam`` / whole-curve ``residual_score`` are debug-only.
"""
from __future__ import annotations

import json
import math
import zipfile
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any

import numpy as np
import torch

ROOT = Path(__file__).resolve().parents[2]
RAW = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "raw"
CACHE = ROOT / "brand" / "artifacts" / "signal_heal_transfer" / "cache"
PERIOD_L = 256
SEED = 1902771841


@dataclass
class DatasetBundle:
    name: str
    ideal: torch.Tensor  # [B, L]
    engine: torch.Tensor  # [B, L]
    meta: dict[str, Any]

    def to_device(self, device: torch.device) -> "DatasetBundle":
        return DatasetBundle(
            self.name,
            self.ideal.to(device).float(),
            self.engine.to(device).float(),
            self.meta,
        )

    def save(self, path: Path) -> None:
        path.parent.mkdir(parents=True, exist_ok=True)
        torch.save(
            {
                "name": self.name,
                "ideal": self.ideal.cpu(),
                "engine": self.engine.cpu(),
                "meta": self.meta,
            },
            path,
        )

    @classmethod
    def load(cls, path: Path) -> "DatasetBundle":
        blob = torch.load(path, map_location="cpu", weights_only=False)
        return cls(blob["name"], blob["ideal"], blob["engine"], blob["meta"])


def _resample_1d(y: np.ndarray, n: int, kind: str = "cubic") -> np.ndarray:
    """Resample a 1D segment to length n."""
    y = np.asarray(y, dtype=np.float64).ravel()
    if y.size < 4:
        y = np.pad(y, (0, max(0, 4 - y.size)), mode="edge")
    x_old = np.linspace(0.0, 1.0, num=y.size)
    x_new = np.linspace(0.0, 1.0, num=n)
    if kind == "linear":
        return np.interp(x_new, x_old, y).astype(np.float32)
    # cubic via numpy poly / scipy if available
    try:
        from scipy.interpolate import CubicSpline

        cs = CubicSpline(x_old, y, bc_type="not-a-knot")
        return cs(x_new).astype(np.float32)
    except Exception:
        return np.interp(x_new, x_old, y).astype(np.float32)


def _zscore(x: np.ndarray, eps: float = 1e-6) -> np.ndarray:
    mu = float(np.mean(x))
    sd = float(np.std(x))
    return ((x - mu) / max(sd, eps)).astype(np.float32)


def _inject_cliff(ideal: np.ndarray, rng: np.random.Generator) -> np.ndarray:
    """DenoiseOpt-style wrap cliff on a clean period (matches overnight make_batch)."""
    eng = ideal.copy()
    n = eng.shape[-1]
    w = 8
    cliff = (0.08 + 0.35 * rng.random()) * (1.0 - 2.0 * rng.random())
    for i in range(w):
        a = i / max(w - 1, 1)
        eng[i] = eng[i] + cliff * (1 - a)
        eng[n - w + i] = eng[n - w + i] - cliff * a
    noise = 0.02 * rng.standard_normal(n).astype(np.float32)
    noise[w:-w] *= 0.15
    return (eng + noise).astype(np.float32)


def _load_cwru_mat(path: Path) -> tuple[np.ndarray, float, float]:
    """Return DE vibration, RPM, Fs."""
    import scipy.io as sio

    mat = sio.loadmat(str(path))
    de = None
    rpm = None
    for k, v in mat.items():
        if k.startswith("__"):
            continue
        if k.endswith("_DE_time") or k.endswith("DE_time"):
            de = np.asarray(v, dtype=np.float64).ravel()
        if k == "RPM" or k.endswith("_RPM"):
            rpm = float(np.asarray(v).ravel()[0])
    if de is None:
        # fallback: largest 1d numeric
        cands = [
            (k, np.asarray(v, dtype=np.float64).ravel())
            for k, v in mat.items()
            if not k.startswith("__") and np.asarray(v).size > 1000
        ]
        if not cands:
            raise RuntimeError(f"no vibration channel in {path}")
        de = max(cands, key=lambda kv: kv[1].size)[1]
    if rpm is None or not math.isfinite(rpm) or rpm < 100:
        # Normal_0 / IR007_0 etc. are ~1797 rpm at 0 HP
        rpm = 1797.0
    # 12k drive-end / normal baseline files in this pilot
    fs = 12000.0
    return de, rpm, fs


def build_cwru(
    *,
    n_periods: int = 256,
    period_l: int = PERIOD_L,
    seed: int = SEED,
    raw_dir: Path | None = None,
) -> DatasetBundle | None:
    raw_dir = raw_dir or (RAW / "cwru")
    mats = sorted(raw_dir.glob("*.mat"))
    if not mats:
        return None
    rng = np.random.default_rng(seed)
    ideals: list[np.ndarray] = []
    engines: list[np.ndarray] = []
    sources: list[str] = []

    for mat_path in mats:
        de, rpm, fs = _load_cwru_mat(mat_path)
        spr = fs * 60.0 / rpm  # samples per revolution
        spr_i = int(round(spr))
        if spr_i < 32:
            continue
        # Use overlapping windows of ~1 rev
        max_start = max(0, de.size - 2 * spr_i)
        if max_start < spr_i:
            continue
        n_take = min(80, max(8, max_start // max(spr_i // 2, 1)))
        starts = rng.choice(np.arange(0, max_start, max(spr_i // 4, 1)), size=min(n_take, max_start), replace=False)
        for s in starts:
            s = int(s)
            seg = de[s : s + spr_i]
            if seg.size < spr_i // 2:
                continue
            # Ideal: cubic angle-domain resample (many-ppr / smooth COT proxy)
            ideal = _zscore(_resample_1d(seg, period_l, kind="cubic"))
            # Bad COT sibling: linear resample of same rev (classical order-track error proxy)
            bad_cot = _zscore(_resample_1d(seg, period_l, kind="linear"))
            # Engine = bad COT + DenoiseOpt cliff (combined wrap artifact)
            eng = _inject_cliff(bad_cot, rng)
            # Keep content-matched ideal from cubic
            ideals.append(ideal)
            engines.append(eng)
            sources.append(mat_path.name)
            if len(ideals) >= n_periods:
                break
        if len(ideals) >= n_periods:
            break

    if len(ideals) < 16:
        return None
    ideal_t = torch.from_numpy(np.stack(ideals[:n_periods], axis=0))
    eng_t = torch.from_numpy(np.stack(engines[:n_periods], axis=0))
    return DatasetBundle(
        name="cwru_bearings",
        ideal=ideal_t,
        engine=eng_t,
        meta={
            "domain": "bearings",
            "files": sorted({Path(s).name for s in sources}),
            "n": int(ideal_t.shape[0]),
            "period_l": period_l,
            "wrap": (
                "Per-rev windows from CWRU DE @12 kHz; ideal=cubic resample to L; "
                "engine=linear (bad-COT proxy) + DenoiseOpt-style wrap cliff+noise."
            ),
            "fs_hz": 12000.0,
            "citation": "Case Western Reserve University Bearing Data Center",
            "seed": seed,
        },
    )


def _paderborn_channels(mat_path: Path) -> tuple[np.ndarray, np.ndarray, np.ndarray, np.ndarray] | None:
    """Return (vib, t_vib, speed_rpm, t_mech) from a Paderborn KAt .mat measurement."""
    import scipy.io as sio

    try:
        mat = sio.loadmat(str(mat_path), squeeze_me=True, struct_as_record=False)
    except Exception:
        return None
    keys = [k for k in mat if not k.startswith("__")]
    if not keys:
        return None
    obj = mat[keys[0]]
    y = getattr(obj, "Y", None)
    x = getattr(obj, "X", None)
    if y is None or x is None:
        return None
    vib = None
    speed = None
    for ch in np.atleast_1d(y):
        name = str(getattr(ch, "Name", "") or "")
        data = np.asarray(getattr(ch, "Data", []), dtype=np.float64).ravel()
        if name == "vibration_1" and data.size > 1000:
            vib = data
        elif name == "speed" and data.size > 100:
            speed = data
    if vib is None:
        return None
    t_vib = None
    t_mech = None
    for ch in np.atleast_1d(x):
        raster = str(getattr(ch, "Raster", "") or "")
        data = np.asarray(getattr(ch, "Data", []), dtype=np.float64).ravel()
        if "HostService" in raster and data.size == vib.size:
            t_vib = data
        elif "Mech" in raster and speed is not None and data.size == speed.size:
            t_mech = data
    if t_vib is None:
        # ~64 kHz HostService fallback from literature / observed Δt
        t_vib = np.arange(vib.size, dtype=np.float64) / 64000.0
    if speed is None:
        # Healthy K001 nominal ~900–1500 rpm; use 1500 as last-resort constant
        speed = np.full(max(16, vib.size // 16), 1500.0, dtype=np.float64)
        t_mech = np.linspace(float(t_vib[0]), float(t_vib[-1]), speed.size)
    if t_mech is None:
        t_mech = np.linspace(float(t_vib[0]), float(t_vib[-1]), speed.size)
    return vib, t_vib, speed, t_mech


def _angle_rev_starts(angle: np.ndarray) -> np.ndarray:
    """Indices where cumulative shaft angle crosses successive 2π boundaries."""
    revs = angle / (2.0 * math.pi)
    crosses = np.where(np.diff(np.floor(revs)) > 0)[0] + 1
    return crosses.astype(np.int64)


def build_paderborn(
    *,
    n_periods: int = 256,
    period_l: int = PERIOD_L,
    seed: int = SEED,
    raw_dir: Path | None = None,
) -> DatasetBundle | None:
    """Paderborn KAt K001: vibration_1 @~64 kHz; speed→angle COT when available.

    Ideal = cubic equal-angle resample of one shaft rev; engine = linear resample
    (bad-COT proxy) + DenoiseOpt wrap cliff. Deep Paderborn pipelines are separate
    (see DEEP_SOTA_NOT_EXECUTED.json) — this only builds the classical wrap board.
    """
    raw_dir = raw_dir or (RAW / "paderborn")
    mats = sorted(raw_dir.rglob("*.mat"))
    if not mats:
        return None

    rng = np.random.default_rng(seed + 31)
    ideals: list[np.ndarray] = []
    engines: list[np.ndarray] = []
    sources: list[str] = []
    used_angle = 0
    used_fixed = 0
    fs_seen: list[float] = []

    for mat_path in mats:
        chans = _paderborn_channels(mat_path)
        if chans is None:
            continue
        vib, t_vib, speed, t_mech = chans
        span = float(t_vib[-1] - t_vib[0])
        if span <= 0 or vib.size < 4096:
            continue
        fs = float((vib.size - 1) / span)
        fs_seen.append(fs)

        # Upsample / interpolate mechanical RPM onto vibration timebase
        rpm_i = np.interp(t_vib, t_mech, speed).astype(np.float64)
        rpm_i = np.clip(rpm_i, 60.0, 6000.0)
        omega = rpm_i * (2.0 * math.pi / 60.0)  # rad/s
        dt = np.diff(t_vib, prepend=t_vib[0])
        dt[0] = dt[1] if dt.size > 1 else (1.0 / max(fs, 1.0))
        angle = np.cumsum(omega * dt)

        starts = _angle_rev_starts(angle)
        if starts.size >= 3:
            # Pair consecutive 2π crossings as one revolution
            rev_bounds = list(zip(starts[:-1], starts[1:]))
            if len(rev_bounds) > 4:
                # Drop first/last incomplete-ish edges
                rev_bounds = rev_bounds[1:-1]
            take_n = min(max(40, n_periods // max(len(mats), 1)), len(rev_bounds))
            pick = rng.choice(len(rev_bounds), size=take_n, replace=False)
            for bi in pick:
                a, b = rev_bounds[int(bi)]
                if b - a < 64:
                    continue
                seg = vib[int(a) : int(b)]
                ideal = _zscore(_resample_1d(seg, period_l, kind="cubic"))
                bad = _zscore(_resample_1d(seg, period_l, kind="linear"))
                eng = _inject_cliff(bad, rng)
                ideals.append(ideal)
                engines.append(eng)
                sources.append(mat_path.name)
                used_angle += 1
                if len(ideals) >= n_periods:
                    break
        else:
            # Fixed samples/rev from mean RPM (no reliable tach crossings)
            rpm_mean = float(np.median(speed))
            spr = int(round(fs * 60.0 / max(rpm_mean, 1.0)))
            if spr < 64:
                continue
            max_start = vib.size - 2 * spr
            if max_start <= 0:
                continue
            starts_f = np.arange(0, max_start, spr)
            take_n = min(max(40, n_periods // max(len(mats), 1)), starts_f.size)
            for s in rng.choice(starts_f, size=take_n, replace=False):
                seg = vib[int(s) : int(s) + spr]
                ideal = _zscore(_resample_1d(seg, period_l, kind="cubic"))
                bad = _zscore(_resample_1d(seg, period_l, kind="linear"))
                eng = _inject_cliff(bad, rng)
                ideals.append(ideal)
                engines.append(eng)
                sources.append(mat_path.name)
                used_fixed += 1
                if len(ideals) >= n_periods:
                    break
        if len(ideals) >= n_periods:
            break

    if len(ideals) < 16:
        return None
    return DatasetBundle(
        name="paderborn_kat",
        ideal=torch.from_numpy(np.stack(ideals[:n_periods], 0)),
        engine=torch.from_numpy(np.stack(engines[:n_periods], 0)),
        meta={
            "domain": "bearings",
            "files": sorted({Path(s).name for s in sources}),
            "n": min(n_periods, len(ideals)),
            "period_l": period_l,
            "wrap": (
                "Paderborn K001 vibration_1 (~64 kHz); shaft angle from Mech_4kHz speed "
                "when available (equal-angle rev windows); ideal=cubic resample to L; "
                "engine=linear (bad-COT) + DenoiseOpt wrap cliff+noise."
            ),
            "fs_hz": float(np.median(fs_seen)) if fs_seen else 64000.0,
            "citation": "Paderborn KAt Bearing Data Center (Lessmeier et al., PHME 2016); CC BY-NC 4.0",
            "seed": seed,
            "bearing_code": "K001",
            "n_angle_revs": used_angle,
            "n_fixed_rpm_revs": used_fixed,
            "label": (
                "classical_board_plus_bad_cot — not a published deep Paderborn denoise "
                "reimplementation (deep SOTA still blocked)"
            ),
            "requested_n": n_periods,
        },
    )


def build_mfpt(
    *,
    n_periods: int = 256,
    period_l: int = PERIOD_L,
    seed: int = SEED,
    raw_dir: Path | None = None,
) -> DatasetBundle | None:
    """MFPT fixed 25 Hz shaft → exact samples/rev when available."""
    raw_dir = raw_dir or (RAW / "mfpt")
    zips = list(raw_dir.glob("*.zip"))
    mat_files = list(raw_dir.rglob("*.mat"))
    if not mat_files and zips:
        zpath = zips[0]
        extract = raw_dir / "_extracted"
        extract.mkdir(parents=True, exist_ok=True)
        try:
            with zipfile.ZipFile(zpath, "r") as zf:
                zf.extractall(extract)
            mat_files = list(extract.rglob("*.mat"))
        except Exception as e:
            return None
    if not mat_files:
        return None

    import scipy.io as sio

    rng = np.random.default_rng(seed + 7)
    ideals: list[np.ndarray] = []
    engines: list[np.ndarray] = []
    # MFPT baseline rate often 97656 Hz; shaft 25 Hz → spr ≈ 3906
    for mat_path in mat_files[:24]:
        try:
            mat = sio.loadmat(str(mat_path))
        except Exception:
            continue
        # Prefer nested 'bearing' struct used by MFPT
        sig = None
        fs = 97656.0
        rpm = 25.0 * 60.0
        for k, v in mat.items():
            if k.startswith("__"):
                continue
            arr = np.asarray(v)
            # Structured (1,1) bearing record: fields sr/gs/load/rate
            if arr.dtype.names and arr.size >= 1:
                try:
                    st = arr.flat[0]
                    names = st.dtype.names or ()
                    if "gs" in names:
                        sig = np.asarray(st["gs"], dtype=np.float64).ravel()
                    if "sr" in names:
                        fs = float(np.asarray(st["sr"]).ravel()[0])
                    if "rate" in names:
                        # rate is shaft Hz in MFPT baseline files
                        rpm = float(np.asarray(st["rate"]).ravel()[0]) * 60.0
                except Exception:
                    pass
            elif arr.dtype == object and arr.size == 1:
                try:
                    st = arr.flat[0]
                    if hasattr(st, "dtype") and st.dtype.names:
                        names = st.dtype.names
                        if "gs" in names:
                            sig = np.asarray(st["gs"], dtype=np.float64).ravel()
                        if "sr" in names:
                            fs = float(np.asarray(st["sr"]).ravel()[0])
                        if "rate" in names:
                            rpm = float(np.asarray(st["rate"]).ravel()[0]) * 60.0
                except Exception:
                    pass
            elif arr.ndim >= 1 and arr.size > 5000 and np.issubdtype(arr.dtype, np.number):
                flat = arr.astype(np.float64).ravel()
                if sig is None or flat.size > sig.size:
                    sig = flat
        if sig is None or sig.size < 8000:
            continue
        spr = int(round(fs * 60.0 / rpm))
        if spr < 64:
            continue
        max_start = sig.size - 2 * spr
        if max_start <= 0:
            continue
        starts = np.arange(0, max_start, spr)
        if starts.size == 0:
            continue
        take = min(max(80, n_periods // 3), starts.size)
        for s in rng.choice(starts, size=take, replace=False):
            seg = sig[int(s) : int(s) + spr]
            ideal = _zscore(_resample_1d(seg, period_l, kind="cubic"))
            bad = _zscore(_resample_1d(seg, period_l, kind="linear"))
            eng = _inject_cliff(bad, rng)
            ideals.append(ideal)
            engines.append(eng)
            if len(ideals) >= n_periods:
                break
        if len(ideals) >= n_periods:
            break
    if len(ideals) < 16:
        return None
    if len(ideals) < n_periods:
        # Soft fail: return what we have but mark under-target in meta
        pass
    return DatasetBundle(
        name="mfpt_bearings",
        ideal=torch.from_numpy(np.stack(ideals[:n_periods], 0)),
        engine=torch.from_numpy(np.stack(engines[:n_periods], 0)),
        meta={
            "domain": "bearings",
            "n": min(n_periods, len(ideals)),
            "period_l": period_l,
            "wrap": "MFPT shaft-rate periods; cubic ideal vs linear+cliff engine.",
            "citation": "MFPT Fault Data Sets (Society for Machinery Failure Prevention Technology)",
            "seed": seed,
            "label": "classical_board_plus_bad_cot — not a published deep SOTA reimplementation",
            "requested_n": n_periods,
        },
    )


def build_mitbih(
    *,
    n_periods: int = 256,
    period_l: int = PERIOD_L,
    seed: int = SEED,
    raw_dir: Path | None = None,
) -> DatasetBundle | None:
    raw_dir = raw_dir or (RAW / "mitdb")
    try:
        import wfdb
    except ImportError:
        return None
    recs = sorted({p.stem for p in raw_dir.glob("*.dat")})
    if not recs:
        return None
    rng = np.random.default_rng(seed + 11)
    beats: list[np.ndarray] = []
    for rec in recs:
        try:
            sig, fields = wfdb.rdsamp(str(raw_dir / rec))
            ann = wfdb.rdann(str(raw_dir / rec), "atr")
        except Exception:
            continue
        x = np.asarray(sig[:, 0], dtype=np.float64)
        # Normal beat peaks
        peaks = [i for i, sym in zip(ann.sample, ann.symbol) if sym in ("N", "L", "R", "e", "j")]
        for a, b in zip(peaks[:-1], peaks[1:]):
            if b - a < 40 or b - a > 500:
                continue
            seg = x[a:b]
            beats.append(_zscore(_resample_1d(seg, period_l, kind="cubic")))
        if len(beats) >= n_periods * 3:
            break
    if len(beats) < 32:
        return None
    beats_arr = np.stack(beats, axis=0)
    # SBMM-lite template: mean of random neighborhood
    ideals: list[np.ndarray] = []
    engines: list[np.ndarray] = []
    idx = rng.permutation(len(beats_arr))
    for i in idx[:n_periods]:
        # template from local window
        lo = max(0, int(i) - 8)
        hi = min(len(beats_arr), int(i) + 9)
        template = beats_arr[lo:hi].mean(axis=0).astype(np.float32)
        # Clean sibling: mild endpoint equalize (clinical-safe classical morph prior)
        ideal = template.copy()
        w = 8
        target = 0.5 * (ideal[0] + ideal[-1])
        for j in range(w):
            a = j / max(w - 1, 1)
            ideal[j] = (1 - a) * target + a * ideal[j]
            ideal[-1 - j] = (1 - a) * target + a * ideal[-1 - j]
        # Cracked: single beat with natural join mismatch + cliff
        eng = _inject_cliff(beats_arr[int(i)], rng)
        ideals.append(_zscore(ideal))
        engines.append(eng)
    return DatasetBundle(
        name="mitbih_ecg",
        ideal=torch.from_numpy(np.stack(ideals, 0)),
        engine=torch.from_numpy(np.stack(engines, 0)),
        meta={
            "domain": "ecg",
            "records": recs,
            "n": len(ideals),
            "period_l": period_l,
            "wrap": (
                "R–R normal beats resampled to L; ideal=local mean template with mild "
                "endpoint equalize (SBMM-lite classical); engine=single beat + wrap cliff."
            ),
            "fs_hz": 360.0,
            "citation": "MIT-BIH Arrhythmia Database (PhysioNet, ODC-By 1.0)",
            "seed": seed,
            "baseline_note": (
                "ECG baselines are classical board + beat-average / spline join — "
                "not Cycle-GAN / BeatDiff SOTA (those need trained weights)."
            ),
        },
    )


# Explicit PTB-XL wrap-board policy (document in meta + PTBXL_POLICY.md).
PTBXL_POLICY = {
    "source": "PhysioNet ptb-xl 1.0.3",
    "preferred_subset": "records500 (*_hr, 500 Hz)",
    "fallback_subset": "records100 (*_lr, 100 Hz)",
    "lead_index": 0,
    "lead_name": "I",
    "period_l": PERIOD_L,
    "board_n_periods": 256,
    "peak_thr_frac_of_max": 0.55,
    "min_peak_distance_s": 0.28,
    "rr_min_s": 0.22,
    "rr_max_s": 1.8,
    "template_neighbors": 8,
    "endpoint_equalize_w": 8,
    "pool_mode": "scan_all_available_records_then_sample",
    "note": (
        "Clinical multi-lead / diagnosis restore ≠ wrap-R; this board is L=256 "
        "prolonged residual R only (lead I)."
    ),
}


def build_ptbxl(
    *,
    n_periods: int = 256,
    period_l: int = PERIOD_L,
    seed: int = SEED,
    raw_dir: Path | None = None,
    prefer_hz: int = 500,
    max_pool_beats: int | None = None,
) -> DatasetBundle | None:
    """PTB-XL lead-I beat windows under PTBXL_POLICY.

    Prefer records500 (500 Hz `_hr`); fall back to `_lr`. Scans all available
    records for the beat pool (no early stop at n*4), then samples ``n_periods``.
    """
    raw_dir = raw_dir or (RAW / "ptbxl")
    # Prefer flattened raw/ptbxl; also accept nested AWS sync tree beside it.
    aws_nested = RAW / "ptbxl_aws" / "records500"
    try:
        import wfdb
    except ImportError:
        return None
    search_roots = [raw_dir]
    if aws_nested.is_dir():
        search_roots.append(aws_nested)
    hr: list[Path] = []
    lr: list[Path] = []
    seen: set[str] = set()
    for root in search_roots:
        for p in sorted(root.rglob("*_hr.hea")):
            if p.stem not in seen:
                hr.append(p)
                seen.add(p.stem)
        for p in sorted(root.rglob("*_lr.hea")):
            if p.stem not in seen:
                lr.append(p)
                seen.add(p.stem)
    if prefer_hz >= 500 and hr:
        heas = hr
        subset = "records500 (*_hr, 500 Hz)"
        default_fs = 500.0
    elif lr:
        heas = lr
        subset = "records100 (*_lr, 100 Hz) fallback"
        default_fs = 100.0
    elif hr:
        heas = hr
        subset = "records500 (*_hr, 500 Hz)"
        default_fs = 500.0
    else:
        return None
    rng = np.random.default_rng(seed + 23)
    beats: list[np.ndarray] = []
    used: list[str] = []
    fs_seen: list[float] = []
    # Cap only if explicitly requested; default = scan all downloaded records.
    pool_cap = max_pool_beats if max_pool_beats is not None else 10**9
    for hea in heas:
        if len(beats) >= pool_cap:
            break
        rec = hea.with_suffix("")  # path without .hea
        try:
            sig, fields = wfdb.rdsamp(str(rec))
        except Exception:
            continue
        fs = float(fields.get("fs", default_fs))
        fs_seen.append(fs)
        # Policy: lead index 0 = lead I only (not multi-lead restore).
        x = np.asarray(sig[:, int(PTBXL_POLICY["lead_index"])], dtype=np.float64)
        xz = _zscore(x)
        thr = float(PTBXL_POLICY["peak_thr_frac_of_max"]) * float(np.max(xz))
        min_dist = max(12, int(float(PTBXL_POLICY["min_peak_distance_s"]) * fs))
        peaks: list[int] = []
        i = 5
        while i < xz.size - 5:
            if xz[i] >= thr and xz[i] >= xz[i - 1] and xz[i] >= xz[i + 1]:
                if not peaks or i - peaks[-1] >= min_dist:
                    peaks.append(i)
                i += max(4, min_dist // 2)
            else:
                i += 1
        rr_min = max(16, int(float(PTBXL_POLICY["rr_min_s"]) * fs))
        rr_max = int(float(PTBXL_POLICY["rr_max_s"]) * fs)
        for a, b in zip(peaks[:-1], peaks[1:]):
            if b - a < rr_min or b - a > rr_max:
                continue
            beats.append(_zscore(_resample_1d(x[a:b], period_l, kind="cubic")))
            if len(beats) >= pool_cap:
                break
        used.append(hea.stem)
    if len(beats) < 32:
        return None
    beats_arr = np.stack(beats, axis=0)
    ideals: list[np.ndarray] = []
    engines: list[np.ndarray] = []
    idx = rng.permutation(len(beats_arr))
    neigh = int(PTBXL_POLICY["template_neighbors"])
    eq_w = int(PTBXL_POLICY["endpoint_equalize_w"])
    for i in idx[:n_periods]:
        lo = max(0, int(i) - neigh)
        hi = min(len(beats_arr), int(i) + neigh + 1)
        template = beats_arr[lo:hi].mean(axis=0).astype(np.float32)
        ideal = template.copy()
        target = 0.5 * (ideal[0] + ideal[-1])
        for j in range(eq_w):
            a = j / max(eq_w - 1, 1)
            ideal[j] = (1 - a) * target + a * ideal[j]
            ideal[-1 - j] = (1 - a) * target + a * ideal[-1 - j]
        eng = _inject_cliff(beats_arr[int(i)], rng)
        ideals.append(_zscore(ideal))
        engines.append(eng)
    fs_meta = float(np.median(fs_seen)) if fs_seen else default_fs
    policy = {
        **PTBXL_POLICY,
        "board_n_periods": int(n_periods),
        "period_l": int(period_l),
        "n_records_available": len(heas),
        "n_records_used": len(used),
        "n_beats_pool": int(beats_arr.shape[0]),
        "subset_resolved": subset,
    }
    return DatasetBundle(
        name="ptbxl_ecg",
        ideal=torch.from_numpy(np.stack(ideals, 0)),
        engine=torch.from_numpy(np.stack(engines, 0)),
        meta={
            "domain": "ecg",
            "records": used,
            "n_records_files": len(used),
            "n_beats_pool": int(beats_arr.shape[0]),
            "n": len(ideals),
            "period_l": period_l,
            "wrap": (
                f"PTB-XL {subset} lead-I (idx 0) R–R windows resampled to L; "
                "ideal=local mean template + mild endpoint equalize; engine=beat+cliff. "
                "Pool=all available downloaded records (explicit PTBXL_POLICY)."
            ),
            "fs_hz": fs_meta,
            "citation": "PTB-XL (PhysioNet, CC BY 4.0)",
            "seed": seed,
            "subset": subset,
            "policy": policy,
            "baseline_note": (
                "Classical board + SBMM-lite / spline only. "
                "Cycle-GAN / BeatDiff clinical restore ≠ wrap-R (see DEEP_SOTA)."
            ),
        },
    )


def build_synth_cnc(
    *,
    n_periods: int = 256,
    period_l: int = PERIOD_L,
    seed: int = SEED,
) -> DatasetBundle:
    """Synthetic closed G01 toolpath: sharp corners vs G2-ish rounded sibling.

    Proxy when KIT CNC DOI is login-walled. Scores path residual under tiled loops.
    """
    rng = np.random.default_rng(seed + 41)
    ideals: list[np.ndarray] = []
    engines: list[np.ndarray] = []
    for _ in range(n_periods):
        # Closed square-ish contour in angle domain: 4 corners + edge noise
        t = np.linspace(0.0, 1.0, period_l, endpoint=False)
        # Ideal: rounded (raised-cosine corner blend) radial path error signal
        corners = np.array([0.0, 0.25, 0.5, 0.75])
        ideal = 0.15 * np.sin(2 * np.pi * t)  # base contour component
        for c in corners:
            d = np.minimum(np.abs(t - c), 1.0 - np.abs(t - c))
            # Smooth G2-ish bump (wide)
            ideal = ideal + 0.35 * np.exp(-0.5 * (d / 0.04) ** 2)
        ideal = _zscore(ideal.astype(np.float32))
        # Engine: sharp G01 corners (narrow spikes) + wrap cliff
        eng = 0.15 * np.sin(2 * np.pi * t)
        for c in corners:
            d = np.minimum(np.abs(t - c), 1.0 - np.abs(t - c))
            eng = eng + 0.55 * np.exp(-0.5 * (d / 0.012) ** 2)
        eng = _inject_cliff(_zscore(eng.astype(np.float32)), rng)
        # Mild content jitter so periods are not identical
        jitter = 0.02 * rng.standard_normal(period_l).astype(np.float32)
        ideals.append(_zscore(ideal + 0.5 * jitter))
        engines.append(eng + 0.25 * jitter)
    return DatasetBundle(
        name="synth_cnc_g01",
        ideal=torch.from_numpy(np.stack(ideals, 0)),
        engine=torch.from_numpy(np.stack(engines, 0)),
        meta={
            "domain": "cnc",
            "n": n_periods,
            "period_l": period_l,
            "wrap": (
                "Synthetic closed G01 contour residual: ideal=wide G2-ish corner blend; "
                "engine=sharp corners + DenoiseOpt wrap cliff. KIT CNC real data not fetched."
            ),
            "citation": "Synthetic CAD proxy per SIGNAL_HEALING_DATASETS.md CNC paragraph",
            "seed": seed,
            "label": "synthetic_pilot — not KIT multimodal CNC recordings",
        },
    )


def build_synth_pmu(
    *,
    n_periods: int = 256,
    period_l: int = PERIOD_L,
    seed: int = SEED,
) -> DatasetBundle:
    """Synthetic power-cycle / PMU window wrap (IEEE 1159-style harmonics).

    Proxy when IEEE DataPort PMU OA requires account login.
    """
    rng = np.random.default_rng(seed + 59)
    ideals: list[np.ndarray] = []
    engines: list[np.ndarray] = []
    for _ in range(n_periods):
        t = np.linspace(0.0, 1.0, period_l, endpoint=False)
        # Fundamental + odd harmonics (clean AC cycle)
        a3 = 0.08 + 0.04 * rng.random()
        a5 = 0.04 + 0.03 * rng.random()
        phase = 2 * np.pi * rng.random()
        ideal = (
            np.sin(2 * np.pi * t + phase)
            + a3 * np.sin(6 * np.pi * t + phase)
            + a5 * np.sin(10 * np.pi * t + phase)
        )
        ideal = _zscore(ideal.astype(np.float32))
        # Engine: same content with abrupt phase/step at wrap (window cliff) + noise
        eng = ideal.copy()
        eng = _inject_cliff(eng, rng)
        # Extra mid-cycle notch (capacitor-switch / window splice proxy)
        if rng.random() < 0.35:
            k = int(rng.integers(period_l // 4, 3 * period_l // 4))
            eng[k : k + 3] *= 0.2
        ideals.append(ideal)
        engines.append(eng)
    return DatasetBundle(
        name="synth_pmu_cycle",
        ideal=torch.from_numpy(np.stack(ideals, 0)),
        engine=torch.from_numpy(np.stack(engines, 0)),
        meta={
            "domain": "power",
            "n": n_periods,
            "period_l": period_l,
            "wrap": (
                "Synthetic 1-cycle voltage with harmonics; ideal=clean AC; "
                "engine=wrap cliff (+ optional notch). IEEE 39-bus PMU OA not fetched."
            ),
            "citation": "Synthetic IEEE 1159 / C37.118-style cycle proxy",
            "seed": seed,
            "label": "synthetic_pilot — not DataPort PMU recordings",
        },
    )


def build_ieee_pmu_real(
    *,
    n_periods: int = 256,
    period_l: int = PERIOD_L,
    seed: int = SEED,
    raw_dir: Path | None = None,
) -> DatasetBundle | None:
    """IEEE 39-bus OA PMU → L=256 periods from **phasor-synthesized** AC cycles.

    Native content is synchrophasors (mag∠ / f / ROCOF), not oscillography.
    Ideal = one clean cycle from measured phasor; engine = same + DenoiseOpt cliff.
    Meta labels this as phasor-synth — do **not** claim native waveform prolonged-R.
    Prefer ``scripts/fetch_and_probe_ieee_pmu.py`` TVE probe for honest PMU scoring.
    """
    raw_dir = raw_dir or (RAW / "ieee_pmu")
    mats = sorted(raw_dir.glob("*.mat"))
    if not mats:
        return None
    mat_path = max(mats, key=lambda p: p.stat().st_size)

    # Prefer shared loader from fetch script; fall back inline
    try:
        sys_path_note = None
        import importlib.util

        probe_path = ROOT / "scripts" / "fetch_and_probe_ieee_pmu.py"
        spec = importlib.util.spec_from_file_location("fetch_ieee_pmu", probe_path)
        if spec and spec.loader:
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
            mat, loader = mod._load_mat_any(mat_path)
            mag, ang, freq = mod._pick_phasor_streams(mat)
        else:
            raise RuntimeError("importlib failed")
    except Exception as e:
        sys_path_note = str(e)
        import scipy.io as sio

        blob = sio.loadmat(str(mat_path), squeeze_me=True, struct_as_record=False)
        mat = {k: v for k, v in blob.items() if not k.startswith("__")}
        data = mat.get("DATA") or mat.get("Data")
        if data is None:
            return None
        mag = np.asarray(getattr(data, "Magnitude"), dtype=np.float64)
        ang = np.asarray(getattr(data, "Angle"), dtype=np.float64)
        freq = None
        if hasattr(data, "Freq"):
            freq = np.asarray(getattr(data, "Freq"), dtype=np.float64)
        loader = f"scipy-fallback ({sys_path_note})"

    if mag.ndim == 1:
        mag = mag[None, :]
        ang = ang[None, :]
    elif mag.ndim == 2 and mag.shape[0] > mag.shape[1] and mag.shape[0] > 50:
        mag, ang = mag.T, ang.T
        if freq is not None and freq.ndim == 2 and freq.shape[0] > freq.shape[1]:
            freq = freq.T
    if np.nanmax(np.abs(ang)) > 2 * math.pi + 0.5:
        ang = np.deg2rad(ang)

    rng = np.random.default_rng(seed + 71)
    n_gen, n_t = int(mag.shape[0]), int(mag.shape[1])
    f0 = float(np.nanmedian(freq)) if freq is not None else 60.0
    ideals: list[np.ndarray] = []
    engines: list[np.ndarray] = []
    # Sample frames across gens/time
    idxs = rng.choice(n_t, size=min(n_periods, n_t), replace=n_t < n_periods)
    gens = rng.integers(0, n_gen, size=len(idxs))
    t = np.linspace(0.0, 1.0, period_l, endpoint=False)
    for g, k in zip(gens, idxs):
        m0 = float(mag[int(g), int(k)])
        a0 = float(ang[int(g), int(k)])
        # One AC cycle synthesized from phasor (footnote in meta)
        ideal = _zscore((m0 * np.cos(2 * math.pi * t + a0)).astype(np.float32))
        eng = _inject_cliff(ideal, rng)
        ideals.append(ideal)
        engines.append(eng)
    while len(ideals) < n_periods:
        ideals.append(ideals[len(ideals) % max(1, len(idxs))])
        engines.append(engines[len(engines) % max(1, len(idxs))])

    return DatasetBundle(
        name="ieee_pmu_real",
        ideal=torch.from_numpy(np.stack(ideals[:n_periods], 0)),
        engine=torch.from_numpy(np.stack(engines[:n_periods], 0)),
        meta={
            "domain": "power",
            "n": n_periods,
            "period_l": period_l,
            "label": (
                "phasor_synthesized_cycles — NOT native OA oscillography; "
                "prefer TVE probe in cache/ieee_pmu_tve_probe.json"
            ),
            "wrap": (
                "Ideal=1-cycle AC reconstructed from measured synchrophasor; "
                "engine=+DenoiseOpt wrap cliff. Musical prolonged-R here is a protocol "
                "compatibility board only."
            ),
            "content_kind": "phasors_synthesized_to_waveform",
            "mat_path": str(mat_path),
            "loader": loader,
            "n_generators": n_gen,
            "n_frames": n_t,
            "nominal_f_hz": f0,
            "citation": "Naglic, IEEE DataPort DOI 10.21227/vkz3-2e96",
            "seed": seed,
            "s3_uri": "s3://ieee-dataport/open/11968/IEEE-39-bus_10_generator_PMU.mat",
        },
    )


def build_kit_cnc_real(
    *,
    n_periods: int = 256,
    period_l: int = PERIOD_L,
    seed: int = SEED,
    raw_dir: Path | None = None,
) -> DatasetBundle | None:
    """KIT multimodal CNC (DOI 10.35097/hvvwn1kfwf7qt48z) — stub until files dropped.

    Expect extracted tree under ``raw/kit_cnc/`` (see kit_cnc_README.txt):
    synced ``.mat`` / controller ``.json`` / force-accel ``.mat`` / NC ``.nc`` / CAD ``.stp``.
    """
    raw_dir = raw_dir or (RAW / "kit_cnc")
    raw_dir.mkdir(parents=True, exist_ok=True)
    mats = list(raw_dir.rglob("*.mat"))
    jsons = list(raw_dir.rglob("*.json"))
    ncs = list(raw_dir.rglob("*.nc")) + list(raw_dir.rglob("*.NC"))
    if not mats and not jsons and not ncs:
        raise FileNotFoundError(
            "KIT CNC files not found. Drop extracted RADAR4KIT archive contents into "
            f"{raw_dir} (see kit_cnc_README.txt). DOI: "
            "https://doi.org/10.35097/hvvwn1kfwf7qt48z — until then use synth_cnc_g01."
        )
    # Minimal wrap: prefer accel/force channel from first usable .mat
    rng = np.random.default_rng(seed + 83)
    ideals: list[np.ndarray] = []
    engines: list[np.ndarray] = []
    used = None
    for mat_path in sorted(mats)[:40]:
        try:
            import scipy.io as sio

            blob = sio.loadmat(str(mat_path), squeeze_me=True, struct_as_record=False)
        except Exception:
            continue
        # Largest 1-D numeric ≥ period_l*4 as vib/force proxy
        best = None
        best_n = 0
        for k, v in blob.items():
            if k.startswith("__"):
                continue
            try:
                a = np.asarray(v, dtype=np.float64).ravel()
            except Exception:
                continue
            if a.size > best_n and a.size >= period_l * 4:
                best, best_n = a, a.size
        if best is None:
            continue
        used = str(mat_path)
        step = max(1, best.size // (n_periods * 2))
        for i in range(0, best.size - period_l, step):
            seg = best[i : i + period_l]
            if seg.size < period_l:
                break
            ideal = _zscore(seg.astype(np.float32))
            # Sharp-corner proxy: emphasize local peaks + cliff
            eng = ideal.copy()
            eng = _inject_cliff(eng, rng)
            ideals.append(ideal)
            engines.append(eng)
            if len(ideals) >= n_periods:
                break
        if len(ideals) >= n_periods:
            break
    if len(ideals) < 8:
        raise FileNotFoundError(
            f"KIT drop present but no usable long 1-D channels found under {raw_dir} "
            f"(mats={len(mats)}, json={len(jsons)}, nc={len(ncs)}). "
            "Expect synced sensor .mat with force/accel time series."
        )
    while len(ideals) < n_periods:
        ideals.append(ideals[len(ideals) % len(ideals)])
        engines.append(engines[len(engines) % len(engines)])
    return DatasetBundle(
        name="kit_cnc_real",
        ideal=torch.from_numpy(np.stack(ideals[:n_periods], 0)),
        engine=torch.from_numpy(np.stack(engines[:n_periods], 0)),
        meta={
            "domain": "cnc",
            "n": n_periods,
            "period_l": period_l,
            "wrap": (
                "KIT real sensor/controller streams: ideal=z-scored segment; "
                "engine=+wrap cliff (G01 corner / loop residual proxy)."
            ),
            "citation": "Ströbel et al. 2025, DOI 10.35097/hvvwn1kfwf7qt48z (CC BY 4.0)",
            "source_mat": used,
            "seed": seed,
            "label": "kit_cnc_real",
        },
    )


def try_optional_probe() -> dict[str, str]:
    """Document skipped optional datasets (real KIT / PMU / Paderborn / NMR)."""
    ieee_mat = list((RAW / "ieee_pmu").glob("*.mat")) if (RAW / "ieee_pmu").is_dir() else []
    kit_dir = RAW / "kit_cnc"
    kit_has = False
    if kit_dir.is_dir():
        kit_has = any(kit_dir.rglob("*.mat")) or any(kit_dir.rglob("*.json")) or any(
            kit_dir.rglob("*.nc")
        )
    base = {
        "kit_cnc_real": (
            "ready — files detected under raw/kit_cnc/"
            if kit_has
            else (
                "awaiting drop — extract KIT CNC into "
                "brand/artifacts/signal_heal_transfer/raw/kit_cnc/ "
                "(see kit_cnc_README.txt); DOI https://doi.org/10.35097/hvvwn1kfwf7qt48z; "
                "synth_cnc_g01 remains the scored proxy"
            )
        ),
        "ieee_pmu_real": (
            "mat present — run scripts/fetch_and_probe_ieee_pmu.py + build_ieee_pmu_real"
            if ieee_mat
            else (
                "S3 URI known (s3://ieee-dataport/open/11968/IEEE-39-bus_10_generator_PMU.mat) "
                "but anonymous --no-sign-request / HTTPS GET return 403; drop .mat into "
                "raw/ieee_pmu/ after free IEEE DataPort login; synth_pmu_cycle remains proxy; "
                "content is phasors → prefer TVE probe over musical R"
            )
        ),
        "paderborn_kat": (
            "extracted K001 via repo UnRAR.exe — classical wrap board available as "
            "paderborn_kat; deep Paderborn pipelines still unwired (see DEEP_SOTA)"
        ),
        "bmrb_nmr": "skipped — BMRB FID needs per-entry API hunt; defer to follow-up",
        "mfpt_bearings": (
            "skipped if zip missing — official + Figshare + data-acoustics HTML/timeout/404"
        ),
        "deep_sota_cyclegan_beatdiff": (
            "not executed — no trained Cycle-GAN / BeatDiff weights under residual protocol"
        ),
    }
    skip_path = CACHE / "skipped_optional.json"
    if skip_path.is_file():
        try:
            base.update(json.loads(skip_path.read_text(encoding="utf-8")))
        except Exception:
            pass
    # Re-assert live filesystem truth over stale cache for KIT / IEEE
    if ieee_mat:
        base["ieee_pmu_real"] = (
            f"mat present ({ieee_mat[0].name}, {ieee_mat[0].stat().st_size} B) — "
            "phasors; TVE probe / phasor-synth board"
        )
        base["ieee_pmu"] = base["ieee_pmu_real"]
    else:
        base["ieee_pmu_real"] = (
            "S3 URI known but anonymous GET 403; drop "
            "IEEE-39-bus_10_generator_PMU.mat into raw/ieee_pmu/; "
            "synth_pmu_cycle proxy still scored"
        )
        base["ieee_pmu"] = base["ieee_pmu_real"]
    if kit_has:
        base["kit_cnc_real"] = "files detected under raw/kit_cnc/ — build_kit_cnc_real available"
        base["kit_cnc"] = base["kit_cnc_real"]
    else:
        base["kit_cnc_real"] = (
            "awaiting user drop under raw/kit_cnc/ (kit_cnc_README.txt); "
            "synth_cnc_g01 proxy still scored"
        )
        base["kit_cnc"] = base["kit_cnc_real"]
    return base


def ensure_bundles(
    *,
    force: bool = False,
    n_periods: int = 256,
) -> dict[str, DatasetBundle | None]:
    CACHE.mkdir(parents=True, exist_ok=True)
    out: dict[str, DatasetBundle | None] = {}
    builders = {
        "cwru_bearings": lambda: build_cwru(n_periods=n_periods),
        "mfpt_bearings": lambda: build_mfpt(n_periods=n_periods),
        "paderborn_kat": lambda: build_paderborn(n_periods=n_periods),
        "mitbih_ecg": lambda: build_mitbih(n_periods=n_periods),
        "ptbxl_ecg": lambda: build_ptbxl(n_periods=n_periods),
        "synth_cnc_g01": lambda: build_synth_cnc(n_periods=n_periods),
        "synth_pmu_cycle": lambda: build_synth_pmu(n_periods=n_periods),
        "ieee_pmu_real": lambda: build_ieee_pmu_real(n_periods=n_periods),
        "kit_cnc_real": lambda: build_kit_cnc_real(n_periods=n_periods),
    }
    for name, fn in builders.items():
        cache_path = CACHE / f"{name}.pt"
        meta_path = CACHE / f"{name}_meta.json"
        if cache_path.is_file() and not force:
            try:
                out[name] = DatasetBundle.load(cache_path)
                continue
            except Exception:
                pass
        try:
            bundle = fn()
        except FileNotFoundError as e:
            out[name] = None
            if name in ("kit_cnc_real", "ieee_pmu_real"):
                # leave skip message; do not raise
                (CACHE / f"{name}_stub_error.txt").write_text(str(e), encoding="utf-8")
            continue
        except Exception as e:
            out[name] = None
            if name in ("kit_cnc_real", "ieee_pmu_real"):
                (CACHE / f"{name}_stub_error.txt").write_text(repr(e), encoding="utf-8")
            continue
        out[name] = bundle
        if bundle is not None:
            bundle.save(cache_path)
            meta_path.write_text(json.dumps(bundle.meta, indent=2), encoding="utf-8")
    skip = try_optional_probe()
    if out.get("mfpt_bearings") is not None:
        skip.pop("mfpt_bearings", None)
    if out.get("ptbxl_ecg") is not None:
        skip.pop("ptbxl_ecg", None)
    if out.get("paderborn_kat") is not None:
        # Classical board built; deep pipelines remain blocked elsewhere.
        skip["paderborn_kat"] = (
            "K001 extracted; classical paderborn_kat board cached — deep SOTA still not executed"
        )
    if out.get("ieee_pmu_real") is not None:
        skip["ieee_pmu_real"] = (
            "phasor-synth L=256 board cached — see meta label; TVE probe is preferred score"
        )
        skip.pop("ieee_pmu", None)
    if out.get("kit_cnc_real") is not None:
        skip["kit_cnc_real"] = "kit_cnc_real board cached from dropped files"
        skip.pop("kit_cnc", None)
    (CACHE / "skipped_optional.json").write_text(json.dumps(skip, indent=2), encoding="utf-8")
    return out


class DomainBatcher:
    """Sample (ideal, engine) minibatches; drop-in for og.make_batch."""

    def __init__(self, bundle: DatasetBundle, device: torch.device):
        self.bundle = bundle.to_device(device)
        self.device = device
        self.n = int(self.bundle.ideal.shape[0])
        self.l = int(self.bundle.ideal.shape[1])

    def __call__(self, batch: int, n: int, device: torch.device) -> tuple[torch.Tensor, torch.Tensor]:
        # n ignored (fixed L); resample index with replacement
        idx = torch.randint(0, self.n, (batch,), device=device)
        return self.bundle.ideal[idx], self.bundle.engine[idx]

    def holdout(self, n: int = 64, seed: int = 20260719) -> tuple[torch.Tensor, torch.Tensor]:
        g = torch.Generator(device="cpu")
        g.manual_seed(seed)
        idx = torch.randperm(self.n, generator=g)[: min(n, self.n)]
        return self.bundle.ideal[idx.to(self.device)], self.bundle.engine[idx.to(self.device)]
