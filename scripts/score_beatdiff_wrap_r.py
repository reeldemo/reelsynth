#!/usr/bin/env python3
"""Load BeatDiff Orbax prior without .zarray metadata and score wrap-R.

Drive ships TensorStore zstd chunks + msgpack PLACEHOLDERs but omits .zarray /
_CHECKPOINT_METADATA. Modern orbax restore fails; we reconstruct float32 leaves
by matching Flax init shapes, then run a short Heun denoise as an OOD wrap-R probe.
"""
from __future__ import annotations

import argparse
import json
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import numpy as np
import torch
import zstandard as zstd

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import overnight_gpu_rl_arch as og  # noqa: E402
from signal_heal.datasets import ensure_bundles  # noqa: E402

ART = ROOT / "brand" / "artifacts" / "signal_heal_transfer"
EXT = ART / "external"
BEATDIFF_REPO = EXT / "BeatDiff"
WEIGHTS = EXT / "weights" / "beatdiff"
PRIOR = WEIGHTS / "beatdiff_prior"
OUT = ART / "deep_sota_adapters"
HOLDOUT_SEED = 20260719
BEAT_L = 176
N_LEADS = 9

METRIC_FOOTNOTE = (
    "Clinical ECG restoration (Cycle-GAN / BeatDiff) optimizes different objectives "
    "(artifact removal, beat morph, inpainting) than DenoiseOpt prolonged wrap residual R. "
    "Even when weights load, wrap-R is an out-of-distribution transfer probe — not a "
    "claim that the method was designed for wavetable seam repair."
)


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def holdout_slice(ideal: torch.Tensor, engine: torch.Tensor, n: int = 64) -> tuple[torch.Tensor, torch.Tensor]:
    g = torch.Generator()
    g.manual_seed(HOLDOUT_SEED)
    perm = torch.randperm(ideal.shape[0], generator=g)
    h = perm[: min(n, ideal.shape[0])]
    return ideal[h], engine[h]


def score_pair(ideal: torch.Tensor, eng: torch.Tensor, out: torch.Tensor) -> dict[str, float]:
    return {
        "R": float(og.residual_score(ideal, out).mean().item()),
        "R_blend": float(og.residual_score_blend(ideal, eng, out).mean().item()),
        "no_bake_R": float(og.residual_score(ideal, eng).mean().item()),
    }


def best_step(prior: Path) -> int:
    best_s, best_loss = None, 1e9
    for step_dir in (prior / "model").iterdir():
        if not step_dir.is_dir() or not step_dir.name.isdigit():
            continue
        met = step_dir / "metrics" / "metrics"
        if not met.is_file():
            continue
        loss = float(json.loads(met.read_text(encoding="utf-8"))["loss/CV"])
        step = int(step_dir.name)
        if loss < best_loss:
            best_loss, best_s = loss, step
    if best_s is None:
        raise FileNotFoundError(f"no metrics under {prior/'model'}")
    return best_s


def load_zstd_f32(path: Path) -> np.ndarray:
    files = sorted(path.iterdir()) if path.is_dir() else [path]
    raw = b"".join(f.read_bytes() for f in files if f.is_file())
    if raw[:4] != b"\x28\xb5\x2f\xfd":
        raise ValueError(f"not zstd: {path}")
    dec = zstd.ZstdDecompressor().decompress(raw, max_output_size=50_000_000)
    if len(dec) % 4 != 0:
        raise ValueError(f"nbytes {len(dec)} not divisible by 4 at {path}")
    return np.frombuffer(dec, dtype=np.float32).copy()


def flatten_leaves(tree: Any, prefix: str = "") -> list[tuple[str, Any]]:
    out: list[tuple[str, Any]] = []
    if isinstance(tree, dict):
        for k, v in tree.items():
            out.extend(flatten_leaves(v, f"{prefix}.{k}" if prefix else str(k)))
    else:
        out.append((prefix, tree))
    return out


def unflatten_like(template: Any, flat: dict[str, np.ndarray], prefix: str = "") -> Any:
    if isinstance(template, dict):
        return {
            k: unflatten_like(v, flat, f"{prefix}.{k}" if prefix else str(k))
            for k, v in template.items()
        }
    key = prefix
    arr = flat[key]
    return np.asarray(arr, dtype=np.float32).reshape(template.shape)


def reconstruct_params(step_dir: Path, template_params: dict) -> dict:
    """Map disk arrays into Flax param tree by leaf path + element count."""
    default = step_dir / "default"
    leaves = flatten_leaves(template_params)
    flat: dict[str, np.ndarray] = {}
    for path, arr in leaves:
        # Flax: params/params/DhariwalUnet_0/...
        # Disk: params.params.DhariwalUnet_0....
        disk_name = path.replace("/", ".")
        # template paths look like 'params.DhariwalUnet_0.Conv_0.kernel'
        candidates = [
            default / disk_name.replace("params.", "params.params.", 1)
            if disk_name.startswith("params.") and not disk_name.startswith("params.params.")
            else default / disk_name,
            default / disk_name,
        ]
        # Also try with params.params prefix always for network weights
        if "DhariwalUnet" in disk_name:
            tail = disk_name.split("DhariwalUnet_0.", 1)[-1]
            candidates.insert(0, default / f"params.params.DhariwalUnet_0.{tail}")
        hit = next((c for c in candidates if c.is_dir() or c.is_file()), None)
        if hit is None:
            raise FileNotFoundError(f"no disk array for leaf {path}; tried {candidates[:3]}")
        loaded = load_zstd_f32(hit)
        if loaded.size != int(np.prod(arr.shape)):
            raise ValueError(
                f"size mismatch {path}: disk={loaded.size} template={arr.shape} "
                f"prod={int(np.prod(arr.shape))} path={hit}"
            )
        flat[path] = loaded
    return unflatten_like(template_params, flat)


def build_model_and_params(step: int):
    import jax
    import jax.numpy as jnp
    from flax.training.train_state import TrainState
    from jax.tree_util import Partial as partial
    import optax
    from omegaconf import OmegaConf

    # Repo layout: BeatDiff/beat_net/beat_net/{unet_parts,variance_exploding_utils}.py
    sys.path.insert(0, str(BEATDIFF_REPO))
    from beat_net.beat_net.unet_parts import DenoiserNet
    from beat_net.beat_net.variance_exploding_utils import (
        heun_sampler,
        input_scaling,
        noise_scaling,
        output_scaling,
        skip_scaling,
    )

    cfg = OmegaConf.load(PRIOR / ".hydra" / "config.yaml")
    net_cfg, diffusion_cfg = cfg.model, cfg.diffusion
    model = DenoiserNet(
        skip_scaling=partial(skip_scaling, sigma_data=diffusion_cfg.sigma_data),
        output_scaling=partial(output_scaling, sigma_data=diffusion_cfg.sigma_data),
        input_scaling=partial(input_scaling, sigma_data=diffusion_cfg.sigma_data),
        noise_conditioning=noise_scaling,
        model_channels=net_cfg.model_channels,
        num_blocks=net_cfg.num_blocks,
        channel_mult=list(net_cfg.channel_mult),
        channel_mult_emb=net_cfg.channel_mult_emb,
        attn_resolutions=list(net_cfg.attn_resolutions),
        dropout_rate=net_cfg.dropout_rate,
        use_f_training=False,
        training=False,
        conditional=bool(net_cfg.conditional),
        embbed_position_in_signal=bool(net_cfg.embbed_position_in_signal),
        dtype=jnp.float32,
    )
    key = jax.random.PRNGKey(0)
    dummy_x = jnp.zeros((1, BEAT_L, N_LEADS), dtype=jnp.float32)
    dummy_sigma = jnp.ones((1,), dtype=jnp.float32)
    dummy_cls = jnp.zeros((1, 4), dtype=jnp.float32)
    variables = model.init(key, dummy_x, dummy_sigma, dummy_cls)
    template = variables["params"]
    step_dir = PRIOR / "model" / str(step)
    params = reconstruct_params(step_dir, template)
    # TrainState with identity EMA tx (inference only)
    state = TrainState.create(apply_fn=model.apply, params={"params": params}, tx=optax.ema(decay=1.0))
    return state, diffusion_cfg, heun_sampler, skip_scaling, output_scaling, input_scaling, noise_scaling


def resample_np(x: np.ndarray, new_len: int) -> np.ndarray:
    """x: [B, L] → [B, new_len] linear."""
    b, l = x.shape
    if l == new_len:
        return x.astype(np.float32)
    t_old = np.linspace(0.0, 1.0, l, dtype=np.float32)
    t_new = np.linspace(0.0, 1.0, new_len, dtype=np.float32)
    out = np.stack([np.interp(t_new, t_old, x[i]) for i in range(b)], axis=0)
    return out.astype(np.float32)


def to_beatdiff_batch(eng: torch.Tensor) -> tuple[np.ndarray, np.ndarray]:
    """Map L=256 mono periods → (B, 176, 9) + class features."""
    x = eng.detach().cpu().numpy().astype(np.float32)
    # per-sample peak normalize (BeatDiff global/per-lead style)
    peak = np.max(np.abs(x), axis=-1, keepdims=True).clip(1e-4, 10.0)
    x = x / peak
    x176 = resample_np(x, BEAT_L)
    # replicate mono into 9 leads (OOD vs true 12-lead morphology)
    multi = np.repeat(x176[:, :, None], N_LEADS, axis=2)
    # neutral adult-male-ish features
    feats = np.tile(np.array([1.0, 0.0, 0.0, 0.0], dtype=np.float32), (x.shape[0], 1))
    return multi, feats


def from_beatdiff_batch(y: np.ndarray, target_len: int) -> np.ndarray:
    """(B, 176, 9) → (B, L) using lead 0."""
    lead0 = y[:, :, 0]
    return resample_np(lead0, target_len)


def run_heun_denoise(eng: torch.Tensor, state, diffusion_cfg, heun_sampler, n_steps: int = 20) -> torch.Tensor:
    """OOD wrap probe: single-step BeatDiff denoise of the cracked mono period.

    Maps L=256→176, tiles to 9 leads, applies DenoiserNet at moderate σ (no
    full Heun from pure noise — that collapses wrap-R to ~0 and only measures
    unconditional prior draw quality).
    """
    import jax.numpy as jnp

    multi, feats = to_beatdiff_batch(eng)
    x = jnp.asarray(multi)
    labels = jnp.asarray(feats)
    # Moderate noise level: wrap cliffs are small vs full σ_max=80 prior range
    sigma = jnp.full((x.shape[0],), 0.5, dtype=jnp.float32)
    y = state.apply_fn(state.params, x, sigma, labels)
    y = np.asarray(y)
    restored = from_beatdiff_batch(y, eng.shape[-1])
    eng_np = eng.detach().cpu().numpy()
    scale = (
        np.max(np.abs(eng_np), axis=-1, keepdims=True)
        / np.max(np.abs(restored), axis=-1, keepdims=True).clip(1e-6)
    )
    restored = restored * scale
    return torch.from_numpy(np.asarray(restored)).float()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--datasets", default="mitbih_ecg,ptbxl_ecg")
    ap.add_argument("--n-steps", type=int, default=20)
    ap.add_argument("--step", type=int, default=0, help="Orbax step; 0=best by loss/CV")
    args = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)

    row: dict[str, Any] = {
        "method": "beatdiff",
        "status": "blocked",
        "metric_footnote": METRIC_FOOTNOTE,
        "download_source": (
            "https://drive.google.com/drive/folders/1m2OvyYebvnirh1CraCrnSOyjihSkSkLG "
            "(subfolder beatdiff_prior=1QN6mZXnBpYJFxwUNYV5PXbkhd4HYw3Xh; curl uc?export=download&confirm=t)"
        ),
        "notes": [],
        "scores": {},
        "blocker": None,
    }

    if not (PRIOR / "model").is_dir():
        row["blocker"] = f"prior missing at {PRIOR}"
        (OUT / "beatdiff_report.json").write_text(json.dumps(row, indent=2), encoding="utf-8")
        print(json.dumps(row, indent=2))
        return 1

    step = args.step or best_step(PRIOR)
    row["orbax_step"] = step
    row["notes"].append(f"reconstructed hydra config at {PRIOR/'.hydra'/'config.yaml'}")
    row["notes"].append(
        f"Drive listed 637 files; zstd shards load as float32; "
        f"modern orbax .zarray restore skipped (manual leaf load)"
    )

    try:
        state, diffusion_cfg, heun_sampler, *_ = build_model_and_params(step)
        row["notes"].append("params reconstructed + TrainState built")
        row["load_ok"] = True
    except Exception as e:
        row["load_ok"] = False
        row["blocker"] = f"load failed: {type(e).__name__}: {e}"
        import traceback

        row["traceback"] = traceback.format_exc()
        (OUT / "beatdiff_report.json").write_text(json.dumps(row, indent=2), encoding="utf-8")
        print(json.dumps(row, indent=2))
        return 1

    bundles = ensure_bundles(force=False, n_periods=256)
    domains = [x.strip() for x in args.datasets.split(",") if x.strip()]
    scored = False
    for name in domains:
        b = bundles.get(name)
        if b is None:
            row["scores"][name] = {"status": "bundle_missing"}
            continue
        ideal, eng = holdout_slice(b.ideal.cpu(), b.engine.cpu())
        try:
            out = run_heun_denoise(eng, state, diffusion_cfg, heun_sampler, n_steps=args.n_steps)
            s = score_pair(ideal, eng, out)
            s["status"] = "scored"
            s["n_holdout"] = int(ideal.shape[0])
            s["protocol"] = (
                f"beatdiff_onestep_sigma0.5_L{BEAT_L}x{N_LEADS}_mono_tile_"
                f"step{step}_holdout_{HOLDOUT_SEED}"
            )
            row["scores"][name] = s
            scored = True
            print(f"BeatDiff {name}: R={s['R']:.4f} R_blend={s['R_blend']:.4f}", flush=True)
        except Exception as e:
            import traceback

            row["scores"][name] = {
                "status": "infer_error",
                "error": f"{type(e).__name__}: {e}",
                "traceback": traceback.format_exc(),
            }
            print(f"BeatDiff {name} FAIL {e}", flush=True)

    row["status"] = "scored" if scored else "blocked"
    if not scored and row["blocker"] is None:
        row["blocker"] = "load ok but inference failed on all domains"
    row["updated_at"] = utc_now()
    path = OUT / "beatdiff_report.json"
    path.write_text(json.dumps(row, indent=2), encoding="utf-8")
    print(json.dumps(row, indent=2))
    print(f"wrote {path}")
    return 0 if scored else 2


if __name__ == "__main__":
    raise SystemExit(main())
