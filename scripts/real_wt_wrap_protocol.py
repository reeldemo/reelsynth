#!/usr/bin/env python3
"""Wrap-discontinuity protocol for real / exported wavetable cycles (Phase F1).

1. Load mono cycle or extract one period (L=256 resample/crop).
2. Ideal = endpoint-matched / closed-seam reference.
3. Apply open-wrap cliff ±U(0.08,0.43) over SEAM_W=8.
4. Score methods with R, SNR/SDR, wrap-jump, edge RMSE.

Primary: true ReelSynth-exported factory frames (`--source reelsynth_export`).
Secondary: external OA WAV cycles (`--source oa_files`).
Tertiary smoke: procedural stand-ins (demoted; not claim tables).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
import sys
import wave
from pathlib import Path

import torch
import torch.nn.functional as F

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
import overnight_gpu_rl_arch as og  # noqa: E402
import bench_classical_vs_ai as cav  # noqa: E402
import bench_sota_matrix as bsm  # noqa: E402
import metrics_snr_sdr as msm  # noqa: E402
from baselines.poly_seam_fitter import fit_poly_seam  # noqa: E402
from baselines.endpoint_pin import endpoint_pin  # noqa: E402

L = og.N
SEAM_W = og.SEAM_W
PROTOCOL_SEED = 20260719
ART = ROOT / "brand" / "artifacts" / "real_wt_cycles"
V6_FIG = ROOT.parent / "denoise-opt-meta" / "paper" / "v6" / "figures"


def resample_to_l(cycle: torch.Tensor, length: int = L) -> torch.Tensor:
    """cycle: [B, T] or [T] → [B, length] linear resample."""
    if cycle.dim() == 1:
        cycle = cycle.unsqueeze(0)
    if cycle.shape[1] == length:
        return cycle
    x = cycle.unsqueeze(1)  # [B,1,T]
    y = F.interpolate(x, size=length, mode="linear", align_corners=False)
    return y.squeeze(1)


def close_seam_ideal(cycle: torch.Tensor) -> torch.Tensor:
    """Endpoint-match reference: remove linear wrap trend (closed seam)."""
    n = cycle.shape[1]
    t = torch.linspace(0, 1, n, device=cycle.device).unsqueeze(0)
    delta = cycle[:, -1:] - cycle[:, :1]
    return cycle - delta * t


def apply_open_wrap_cliff(
    ideal: torch.Tensor, *, cliff: torch.Tensor | None = None, seed: int | None = None
) -> tuple[torch.Tensor, torch.Tensor]:
    """Apply synthetic open-wrap cliff + seam-boosted noise (same as make_batch)."""
    batch, n = ideal.shape
    device = ideal.device
    if seed is not None:
        g = torch.Generator(device="cpu")
        g.manual_seed(seed)
        if cliff is None:
            cliff_cpu = (0.08 + 0.35 * torch.rand(batch, 1, generator=g)) * (
                1.0 - 2.0 * torch.rand(batch, 1, generator=g)
            )
            cliff = cliff_cpu.to(device)
            noise_cpu = 0.02 * torch.randn(batch, n, generator=g)
            noise = noise_cpu.to(device)
        else:
            noise = 0.02 * torch.randn(batch, n, device=device)
    else:
        if cliff is None:
            cliff = (0.08 + 0.35 * torch.rand(batch, 1, device=device)) * (
                1.0 - 2.0 * torch.rand(batch, 1, device=device)
            )
        noise = 0.02 * torch.randn(batch, n, device=device)
    eng = ideal.clone()
    w = SEAM_W
    for i in range(w):
        a = i / max(w - 1, 1)
        eng[:, i] = eng[:, i] + cliff.squeeze(-1) * (1 - a)
        eng[:, -w + i] = eng[:, -w + i] - cliff.squeeze(-1) * a
    noise[:, w:-w] *= 0.15
    eng = eng + noise
    return ideal, eng


def factory_reelsynth_cycles(device: torch.device, n_cycles: int = 24) -> torch.Tensor:
    """TERTIARY smoke only: procedural factory-like shapes (not claim primary)."""
    t = torch.linspace(0, 1, L, device=device)
    cycles = []
    for i in range(n_cycles):
        kind = i % 6
        phase = (i * 0.37) % 1.0
        tp = (t + phase) % 1.0
        if kind == 0:
            c = 2.0 * tp - 1.0
        elif kind == 1:
            c = torch.where(tp < 0.5, torch.ones_like(tp), -torch.ones_like(tp))
        elif kind == 2:
            c = 1.0 - 4.0 * (tp - 0.5).abs()
        elif kind == 3:
            width = 0.15 + 0.2 * ((i * 0.13) % 1.0)
            c = torch.where(tp < width, torch.ones_like(tp), -0.3 * torch.ones_like(tp))
        elif kind == 4:
            c = 0.7 * (2.0 * tp - 1.0) + 0.3 * torch.sin(2 * math.pi * tp)
        else:
            a = (i % 5) / 4.0
            saw = 2.0 * tp - 1.0
            sq = torch.where(tp < 0.5, torch.ones_like(tp), -torch.ones_like(tp))
            c = (1 - a) * saw + a * sq
        c = c + 0.08 * torch.sin(4 * math.pi * tp + i)
        c = c / (c.abs().max().clamp_min(1e-6))
        cycles.append(c)
    return torch.stack(cycles, dim=0)


def oa_instrument_cycles(device: torch.device, n_cycles: int = 20) -> torch.Tensor:
    """TERTIARY smoke: procedural additive harmonics (not OA claim)."""
    t = torch.linspace(0, 1, L, device=device)
    cycles = []
    specs = [
        (1.0, 0.5, 0.25, 0.12, 0.06),
        (1.0, 0.35, 0.18, 0.0, 0.0),
        (1.0, 0.6, 0.4, 0.25, 0.15),
        (1.0, 0.2, 0.4, 0.1, 0.05),
        (1.0, 0.8, 0.1, 0.3, 0.05),
    ]
    for i in range(n_cycles):
        amps = specs[i % len(specs)]
        phase = 2 * math.pi * ((i * 0.19) % 1.0)
        c = torch.zeros_like(t)
        for k, a in enumerate(amps, start=1):
            if a == 0:
                continue
            c = c + a * torch.sin(2 * math.pi * k * t + phase * (0.3 * k))
        env = torch.exp(-1.5 * t * (0.5 + 0.5 * ((i * 0.11) % 1.0)))
        c = c * (0.55 + 0.45 * env)
        c = c / (c.abs().max().clamp_min(1e-6))
        cycles.append(c)
    return torch.stack(cycles, dim=0)


def load_reelsynth_export(path: Path, device: torch.device) -> torch.Tensor:
    """Load true factory-export JSON from export_reelsynth_wt_cycles."""
    blob = json.loads(path.read_text(encoding="utf-8"))
    cycles = torch.tensor(blob["cycles"], dtype=torch.float32, device=device)
    if cycles.dim() != 2:
        raise ValueError(f"bad cycles shape {tuple(cycles.shape)}")
    if cycles.shape[1] != L:
        cycles = resample_to_l(cycles, L)
    # peak normalize each
    peaks = cycles.abs().amax(dim=1, keepdim=True).clamp_min(1e-6)
    return cycles / peaks


def _read_wav_mono(path: Path) -> torch.Tensor:
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


def load_oa_wav_cycles(
    directory: Path,
    device: torch.device,
    *,
    n_max: int = 24,
    glob_pat: str = "*.wav",
) -> torch.Tensor | None:
    """Load OA single-cycle WAVs; fixed-window resample to L=256; peak-normalize."""
    paths = sorted(directory.rglob(glob_pat) if "**" in glob_pat else directory.glob(glob_pat))
    if not paths:
        paths = sorted(directory.rglob("*.wav"))
    if not paths:
        return None
    cycles = []
    for p in paths[:n_max]:
        try:
            samples = _read_wav_mono(p)
        except Exception as exc:  # noqa: BLE001
            print(f"skip {p.name}: {exc}")
            continue
        if samples.numel() < 8:
            continue
        c = resample_to_l(samples.unsqueeze(0), L).squeeze(0)
        c = c / c.abs().max().clamp_min(1e-6)
        cycles.append(c)
    if not cycles:
        return None
    return torch.stack(cycles, dim=0).to(device)


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def try_load_optional(name: str, device: torch.device):
    ckpt = ROOT / "brand" / "artifacts" / "n2n_seam_baselines" / f"{name}.pt"
    if not ckpt.is_file():
        return None
    try:
        from baselines import n2n_seam, seq_seam_lstm, seq_seam_cnn1d

        blob = torch.load(ckpt, map_location=device, weights_only=False)
        if name.startswith("n2n"):
            m = n2n_seam.SeamN2N.from_state(blob["state_dict"], device)
        elif "lstm" in name:
            m = seq_seam_lstm.SeamLSTM.from_state(blob["state_dict"], device)
        else:
            m = seq_seam_cnn1d.SeamCNN1D.from_state(blob["state_dict"], device)
        m.eval()
        return lambda eng: m(eng)
    except Exception as exc:  # noqa: BLE001
        print(f"skip {name}: {exc}")
        return None


@torch.no_grad()
def score_corpus(
    cycles: torch.Tensor,
    *,
    device: torch.device,
    label: str,
) -> dict:
    closed = close_seam_ideal(cycles.to(device))
    ideal, eng = apply_open_wrap_cliff(closed, seed=PROTOCOL_SEED)

    methods: list[tuple[str, callable]] = [
        ("no_bake", lambda x: x),
        ("dual_cosine", og.dual_cosine_blend),
        ("seam_fir3", cav.seam_fir3),
        ("poly_seam_d3", lambda x: fit_poly_seam(x, degree=3, seam_w=SEAM_W)),
        ("endpoint_pin_mean", lambda x: endpoint_pin(x, seam_w=SEAM_W, mode="mean")),
    ]
    neural_fn, neural_meta, _, _ = bsm.load_neural_favorite(device)
    methods.append(("neural_favorite", neural_fn))
    for extra in ("n2n_corrupt_corrupt", "n2n_sibling_supervised", "seq_lstm", "seq_cnn1d"):
        fn = try_load_optional(extra, device)
        if fn is not None:
            methods.append((extra, fn))

    rows = {}
    for name, fn in methods:
        out = fn(eng)
        r = og.residual_score(ideal, out)
        sec = msm.secondary_metrics(ideal, out, periods=int(og.PROLONG), seam_w=SEAM_W)
        rows[name] = {
            "n": int(cycles.shape[0]),
            "R_mean": float(r.mean().item()),
            "R_std": float(r.std(unbiased=False).item()),
            **sec,
        }
    if "no_bake" in rows:
        rows["identity"] = rows["no_bake"]  # legacy alias
    return {
        "label": label,
        "n_cycles": int(cycles.shape[0]),
        "L": L,
        "SEAM_W": SEAM_W,
        "protocol_seed": PROTOCOL_SEED,
        "favorite_meta": neural_meta,
        "methods": rows,
        "nomenclature": {"no_bake": "passthrough unrepaired engine; legacy key identity"},
    }


def unit_test_bit_comparable(device: torch.device) -> None:
    torch.manual_seed(0)
    ideal0, _eng0 = og.make_batch(4, L, device)
    ideal1, eng1 = apply_open_wrap_cliff(ideal0, seed=123)
    assert torch.allclose(ideal0, ideal1)
    assert (eng1[:, :SEAM_W] - ideal1[:, :SEAM_W]).abs().mean() > 0.01
    print("unit_test_bit_comparable: ok")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument(
        "--source",
        choices=("all", "reelsynth_export", "oa_files", "procedural"),
        default="all",
    )
    ap.add_argument(
        "--export-json",
        type=Path,
        default=ART / "reelsynth_export_cycles.json",
    )
    ap.add_argument(
        "--oa-dir",
        type=Path,
        default=ART / "oa_akwf",
    )
    ap.add_argument("--n-oa", type=int, default=24)
    ap.add_argument("--n-procedural", type=int, default=24)
    args = ap.parse_args()
    device = torch.device(args.device)

    unit_test_bit_comparable(device)
    ART.mkdir(parents=True, exist_ok=True)

    blob: dict = {
        "meta": {
            "primary": "reelsynth_export",
            "secondary": "oa_akwf",
            "tertiary_smoke": "procedural_standin",
            "no_librispeech": True,
            "no_musdb": True,
            "protocol": "close_seam_ideal + open_wrap_cliff SEAM_W=8",
            "edge_rmse_locked": True,
        }
    }

    # PRIMARY
    if args.source in ("all", "reelsynth_export"):
        if not args.export_json.is_file():
            raise SystemExit(
                f"missing export JSON {args.export_json}; "
                "run: cargo run -p reelsynth --release --bin export_reelsynth_wt_cycles"
            )
        rs = load_reelsynth_export(args.export_json, device)
        assert rs.shape[0] >= 20, f"need ≥20 export cycles, got {rs.shape[0]}"
        # SHA256 of export JSON for SAMPLE_LICENSES
        blob["meta"]["reelsynth_export_sha256"] = sha256_file(args.export_json)
        blob["meta"]["reelsynth_export_n"] = int(rs.shape[0])
        blob["reelsynth_export_primary"] = score_corpus(
            rs, device=device, label="reelsynth_export_primary"
        )
        # Keep demoted procedural under explicit key if requested
        if args.source == "all":
            proc = factory_reelsynth_cycles(device, args.n_procedural)
            blob["procedural_standin"] = score_corpus(
                proc, device=device, label="procedural_standin_tertiary"
            )

    # SECONDARY OA files
    if args.source in ("all", "oa_files"):
        oa = load_oa_wav_cycles(args.oa_dir, device, n_max=args.n_oa)
        if oa is None or oa.shape[0] < 20:
            raise SystemExit(
                f"need ≥20 OA WAV cycles under {args.oa_dir}, "
                f"got {0 if oa is None else oa.shape[0]}"
            )
        licenses = []
        for p in sorted(args.oa_dir.rglob("*.wav"))[: oa.shape[0]]:
            licenses.append({"file": p.name, "sha256": sha256_file(p)})
        blob["meta"]["oa_file_hashes"] = licenses
        blob["oa_akwf_secondary"] = score_corpus(oa, device=device, label="oa_akwf_secondary")

    if args.source == "procedural":
        proc = factory_reelsynth_cycles(device, args.n_procedural)
        oa_proc = oa_instrument_cycles(device, 20)
        blob["meta"]["primary"] = "procedural_standin"
        blob["procedural_standin"] = score_corpus(proc, device=device, label="procedural")
        blob["oa_instrument_procedural"] = score_corpus(
            oa_proc, device=device, label="oa_procedural_tertiary"
        )

    out_local = ART / "real_wt_matrix.json"
    out_local.write_text(json.dumps(blob, indent=2), encoding="utf-8")
    print(f"wrote {out_local}")
    if V6_FIG.is_dir() or V6_FIG.parent.is_dir():
        V6_FIG.mkdir(parents=True, exist_ok=True)
        (V6_FIG / "real_wt_matrix.json").write_text(json.dumps(blob, indent=2), encoding="utf-8")
        print(f"wrote {V6_FIG / 'real_wt_matrix.json'}")

    for key in (
        "reelsynth_export_primary",
        "oa_akwf_secondary",
        "procedural_standin",
    ):
        block = blob.get(key)
        if not block:
            continue
        m = block["methods"]
        print(
            f"{block['label']}: fav_R={m['neural_favorite']['R_mean']:.4f} "
            f"dc_R={m['dual_cosine']['R_mean']:.4f} "
            f"nobake_R={m.get('no_bake', m.get('identity', {})).get('R_mean', float('nan')):.4f} "
            f"poly_R={m.get('poly_seam_d3', {}).get('R_mean', float('nan')):.4f}"
        )


if __name__ == "__main__":
    main()
