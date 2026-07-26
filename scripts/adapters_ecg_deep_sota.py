#!/usr/bin/env python3
"""Thin adapters for ECG Cycle-GAN + BeatDiff under DenoiseOpt wrap-R.

Honesty:
  - Clinical ECG restore (artifact removal / beat morph) ≠ prolonged wrap residual R.
  - Do NOT invent scores. Only emit R / R_blend when pretrained weights load and
    inference runs on L=256 cracked periods from MIT-BIH / PTB-XL boards.
  - Upstream weights live on Google Drive (Cycle-GAN, BeatDiff) or HF (lbedin/BeatDiff);
    this script probes those paths and records blockers in DEEP_SOTA JSON.

Clones (gitignored):
  brand/artifacts/signal_heal_transfer/external/
"""
from __future__ import annotations

import argparse
import json
import sys
import traceback
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import numpy as np
import torch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import overnight_gpu_rl_arch as og  # noqa: E402
from signal_heal.datasets import ensure_bundles  # noqa: E402

ART = ROOT / "brand" / "artifacts" / "signal_heal_transfer"
EXT = ART / "external"
WEIGHTS = EXT / "weights"
OUT = ART / "deep_sota_adapters"
CYCLEGAN_DIR = EXT / "Blind-ECG-Restoration-by-Operational-Cycle-GANs"
BEATDIFF_DIR = EXT / "BeatDiff"
HOLDOUT_SEED = 20260719

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


def find_cyclegan_weights() -> Path | None:
    names = [
        "model_weights_16NQ3.pth",
        "model_weights.pth",
        "G_basestyle.pth",
        "generator.pth",
    ]
    roots = [WEIGHTS / "cyclegan", CYCLEGAN_DIR, WEIGHTS]
    for root in roots:
        if not root.exists():
            continue
        for name in names:
            hit = root / name
            if hit.is_file() and hit.stat().st_size > 10_000:
                return hit
        for p in root.rglob("*.pth"):
            if p.stat().st_size > 10_000:
                return p
    return None


def try_load_cyclegan(weight: Path, device: torch.device) -> tuple[Any | None, str]:
    """Attempt to load Operational Cycle-GAN generator; return (model, note)."""
    # Prefer minimal arch (no pytorch_lightning); fall back to upstream module.
    try:
        from signal_heal.cyclegan_arch_minimal import CycleGAN_Unet_Generator  # type: ignore
        src = "cyclegan_arch_minimal"
    except Exception as e_min:
        if not CYCLEGAN_DIR.is_dir():
            return None, f"Cycle-GAN clone missing; minimal import failed: {e_min}"
        fastonn = EXT / "fastonn"
        for p in (CYCLEGAN_DIR, fastonn, CYCLEGAN_DIR / "Fastonn"):
            if p.is_dir():
                sys.path.insert(0, str(p if p.name != "Fastonn" else p.parent))
        try:
            from GAN_Arch_details import CycleGAN_Unet_Generator  # type: ignore
            src = "GAN_Arch_details"
        except Exception as e:
            return None, (
                f"import CycleGAN_Unet_Generator failed "
                f"(minimal={type(e_min).__name__}:{e_min}; upstream={type(e).__name__}:{e})"
            )
    try:
        net = CycleGAN_Unet_Generator()
        ckpt = torch.load(str(weight), map_location="cpu", weights_only=False)
        if isinstance(ckpt, dict) and "state_dict" in ckpt:
            ckpt = ckpt["state_dict"]
        net.load_state_dict(ckpt, strict=False)
        net.eval()
        net.to(device)
        return net, f"loaded {weight} via {src}"
    except Exception as e:
        return None, f"weight load failed: {type(e).__name__}: {e}"


@torch.no_grad()
def cyclegan_infer(net: Any, eng: torch.Tensor) -> torch.Tensor | None:
    """Map L=256 cracked periods through generator; reshape as needed."""
    x = eng.unsqueeze(1)  # [B,1,L]
    # Upstream model was trained on ~4000-sample segments; try native L first,
    # then resample to 4000 and back if needed.
    try:
        y = net(x)
        if isinstance(y, (tuple, list)):
            y = y[0]
        y = y.squeeze(1)
        if y.shape[-1] == eng.shape[-1]:
            return y.float()
    except Exception:
        pass
    # Resample path
    try:
        import torch.nn.functional as F

        x4 = F.interpolate(x, size=4000, mode="linear", align_corners=False)
        y4 = net(x4)
        if isinstance(y4, (tuple, list)):
            y4 = y4[0]
        y4 = y4.squeeze(1)
        y = F.interpolate(y4.unsqueeze(1), size=eng.shape[-1], mode="linear", align_corners=False)
        return y.squeeze(1).float()
    except Exception as e:
        print(f"cyclegan infer fail: {e}")
        return None


def find_beatdiff_weights() -> Path | None:
    roots = [WEIGHTS / "beatdiff", BEATDIFF_DIR / "results", WEIGHTS]
    for root in roots:
        if not root.exists():
            continue
        for p in root.rglob("*.pt"):
            if p.stat().st_size > 50_000:
                return p
        for p in root.rglob("*.ckpt"):
            if p.stat().st_size > 50_000:
                return p
    return None


def try_hf_beatdiff(dest: Path) -> tuple[Path | None, str]:
    """Fetch author BeatDiff prior from HF. Stops cleanly if gated/unauthenticated."""
    dest.mkdir(parents=True, exist_ok=True)
    try:
        from huggingface_hub import hf_hub_download, list_repo_files
    except ImportError:
        return None, "huggingface_hub not installed"
    try:
        files = list_repo_files("lbedin/BeatDiff")
    except Exception as e:
        msg = str(e)
        gated = "401" in msg or "gated" in msg.lower() or "authenticated" in msg.lower()
        if gated:
            return None, (
                "HF_GATED_OR_AUTH_REQUIRED: repo lbedin/BeatDiff returns 401 without a token. "
                "User must run:  huggingface-cli login   "
                "then:  huggingface-cli download lbedin/BeatDiff "
                f"--local-dir {dest}"
            )
        return None, f"HF list_repo_files(lbedin/BeatDiff) failed: {type(e).__name__}: {e}"
    ckpts = [f for f in files if f.endswith((".pt", ".ckpt", ".pth", ".safetensors"))]
    # Also accept orbax / flax directory markers
    if not ckpts:
        orbaxish = [f for f in files if "checkpoint" in f.lower() or f.endswith((".msgpack", ".orbax"))]
        if not orbaxish and files:
            return None, f"HF repo listed but no weight files (n_files={len(files)}; sample={files[:12]})"
        if orbaxish:
            try:
                local = hf_hub_download("lbedin/BeatDiff", orbaxish[0], local_dir=str(dest))
                return Path(local), f"downloaded {orbaxish[0]}"
            except Exception as e:
                return None, f"HF download failed: {type(e).__name__}: {e}"
        return None, f"HF repo listed but no weight files (n_files={len(files)})"
    try:
        local = hf_hub_download("lbedin/BeatDiff", ckpts[0], local_dir=str(dest))
        return Path(local), f"downloaded {ckpts[0]}"
    except Exception as e:
        return None, f"HF download failed: {type(e).__name__}: {e}"


def try_gdown_cyclegan() -> str:
    out = WEIGHTS / "cyclegan"
    out.mkdir(parents=True, exist_ok=True)
    try:
        import gdown
    except ImportError:
        return "gdown not installed"
    url = "https://drive.google.com/drive/folders/1WPnskKwW_x2jtsSK-7RkBQvT1bXUF9JH"
    try:
        gdown.download_folder(url, output=str(out), quiet=False)
        return f"gdown folder -> {out} files={[p.name for p in out.rglob('*') if p.is_file()][:12]}"
    except Exception as e:
        return f"gdown Cycle-GAN failed: {type(e).__name__}: {e}"


def try_gdown_beatdiff() -> str:
    out = WEIGHTS / "beatdiff"
    out.mkdir(parents=True, exist_ok=True)
    try:
        import gdown
    except ImportError:
        return "gdown not installed"
    url = "https://drive.google.com/drive/folders/1m2OvyYebvnirh1CraCrnSOyjihSkSkLG"
    try:
        gdown.download_folder(url, output=str(out), quiet=False)
        return f"gdown folder -> {out} files={[p.name for p in out.rglob('*') if p.is_file()][:12]}"
    except Exception as e:
        return f"gdown BeatDiff failed: {type(e).__name__}: {e}"


def run_cyclegan(domains: list[str], device: torch.device, fetch: bool) -> dict[str, Any]:
    notes: list[str] = []
    if fetch:
        notes.append(try_gdown_cyclegan())
    w = find_cyclegan_weights()
    row: dict[str, Any] = {
        "method": "cycle_gan_ecg",
        "status": "blocked",
        "metric_footnote": METRIC_FOOTNOTE,
        "weight": str(w) if w else None,
        "notes": notes,
        "scores": {},
    }
    if w is None:
        row["blocker"] = (
            "No pretrained Cycle-GAN .pth under external/weights or clone. "
            "Upstream publishes weights on Google Drive only; clone has Sample Outputs PNGs, not checkpoints."
        )
        return row
    net, note = try_load_cyclegan(w, device)
    notes.append(note)
    row["notes"] = notes
    if net is None:
        row["blocker"] = note
        return row
    bundles = ensure_bundles(force=False, n_periods=256)
    scored_any = False
    for name in domains:
        b = bundles.get(name)
        if b is None:
            row["scores"][name] = {"status": "bundle_missing"}
            continue
        ideal, eng = holdout_slice(b.ideal.to(device), b.engine.to(device))
        try:
            out = cyclegan_infer(net, eng)
        except Exception as e:
            row["scores"][name] = {"status": "infer_error", "error": str(e)}
            continue
        if out is None:
            row["scores"][name] = {"status": "infer_failed"}
            continue
        s = score_pair(ideal, eng, out)
        s["status"] = "scored"
        s["n_holdout"] = int(ideal.shape[0])
        row["scores"][name] = s
        scored_any = True
        print(f"Cycle-GAN {name}: R={s['R']:.4f} R_blend={s['R_blend']:.4f}", flush=True)
    row["status"] = "scored" if scored_any else "blocked"
    if not scored_any:
        row["blocker"] = "Weights present but inference failed on L=256 boards"
    return row


def run_beatdiff(domains: list[str], device: torch.device, fetch: bool) -> dict[str, Any]:
    notes: list[str] = []
    if fetch:
        notes.append(try_gdown_beatdiff())
        hf_path, hf_note = try_hf_beatdiff(WEIGHTS / "beatdiff_hf")
        notes.append(hf_note)
        if hf_path is not None:
            notes.append(f"HF local={hf_path}")
    w = find_beatdiff_weights()
    row: dict[str, Any] = {
        "method": "beatdiff",
        "status": "blocked",
        "metric_footnote": METRIC_FOOTNOTE,
        "weight": str(w) if w else None,
        "notes": notes,
        "scores": {},
        "blocker": None,
    }
    # Detect incomplete Drive/Orbax stubs (gdown quota → ~18-byte placeholders).
    stub_tree = WEIGHTS / "beatdiff" / "beatdiff_prior"
    if stub_tree.is_dir():
        files = [p for p in stub_tree.rglob("*") if p.is_file()]
        big = [p for p in files if p.stat().st_size > 10_000]
        row["notes"].append(
            f"Drive tree present n_files={len(files)} n_gt10k={len(big)} "
            "(Orbax param shards; gdown FileURLRetrievalError left mostly empty stubs)"
        )
        if len(big) <= 1:
            row["blocker"] = (
                "BEATDIFF_WEIGHTS_INCOMPLETE: Google Drive folder "
                "https://drive.google.com/drive/folders/1m2OvyYebvnirh1CraCrnSOyjihSkSkLG "
                "listed but gdown cannot fetch Orbax shards (FileURLRetrievalError / quota). "
                "HF mirror lbedin/BeatDiff requires login (401 without token). "
                "USER ACTION: (1) huggingface-cli login  then  "
                "huggingface-cli download lbedin/BeatDiff "
                f"--local-dir {WEIGHTS / 'beatdiff_hf'}   "
                "OR (2) manually download the Drive folder in a browser to "
                f"{WEIGHTS / 'beatdiff'} and re-run this adapter. "
                "No wrap-R / native BeatDiff scores until a complete prior checkpoint lands."
            )
            row["status"] = "blocked_needs_user_auth_or_manual_drive"
            row["user_must"] = {
                "hf_login": "huggingface-cli login",
                "hf_download": (
                    f"huggingface-cli download lbedin/BeatDiff --local-dir {WEIGHTS / 'beatdiff_hf'}"
                ),
                "drive_url": "https://drive.google.com/drive/folders/1m2OvyYebvnirh1CraCrnSOyjihSkSkLG",
            }
            return row
    if w is None:
        row["blocker"] = (
            "No BeatDiff diffusion checkpoint under external/weights. "
            "HF lbedin/BeatDiff: 401 without token — run `huggingface-cli login` then "
            f"`huggingface-cli download lbedin/BeatDiff --local-dir {WEIGHTS / 'beatdiff_hf'}`. "
            "Drive prior+baselines: "
            "https://drive.google.com/drive/folders/1m2OvyYebvnirh1CraCrnSOyjihSkSkLG "
            "(browser download; gdown FileURLRetrievalError). "
            "Retrain from PhysioNet beat DB exceeds this session."
        )
        row["status"] = "blocked_needs_user_auth_or_manual_drive"
        row["user_must"] = {
            "hf_login": "huggingface-cli login",
            "hf_download": (
                f"huggingface-cli download lbedin/BeatDiff --local-dir {WEIGHTS / 'beatdiff_hf'}"
            ),
            "drive_url": "https://drive.google.com/drive/folders/1m2OvyYebvnirh1CraCrnSOyjihSkSkLG",
        }
        return row
    # Honest stop: loading a full BeatDiff sampling stack needs hydra configs +
    # trained prior; if a .pt exists we try a minimal torch.load smoke only.
    try:
        blob = torch.load(str(w), map_location="cpu", weights_only=False)
        keys = list(blob.keys())[:12] if isinstance(blob, dict) else [type(blob).__name__]
        row["notes"].append(f"torch.load ok keys/type={keys}")
        row["blocker"] = (
            f"Checkpoint found at {w} but BeatDiff inference requires the full "
            "hydra+JAX sampling stack (EMbeat_diff_denoising) + matching beat DB — "
            "not yet adapted to L=256 wrap-R in this thin adapter. No wrap-R scores emitted."
        )
        row["status"] = "weights_present_infer_not_wired"
    except Exception as e:
        row["blocker"] = f"checkpoint load failed: {type(e).__name__}: {e}"
    return row


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--device", default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--fetch-weights", action="store_true")
    ap.add_argument("--datasets", default="mitbih_ecg,ptbxl_ecg")
    ap.add_argument("--skip-cyclegan", action="store_true")
    ap.add_argument("--skip-beatdiff", action="store_true")
    args = ap.parse_args()
    OUT.mkdir(parents=True, exist_ok=True)
    device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")
    domains = [x.strip() for x in args.datasets.split(",") if x.strip()]

    report: dict[str, Any] = {
        "updated_at": utc_now(),
        "metric_footnote": METRIC_FOOTNOTE,
        "device": str(device),
        "external_clones": {
            "cyclegan": CYCLEGAN_DIR.is_dir(),
            "beatdiff": BEATDIFF_DIR.is_dir(),
        },
        "methods": {},
    }
    if not args.skip_cyclegan:
        report["methods"]["cycle_gan_ecg"] = run_cyclegan(domains, device, args.fetch_weights)
    if not args.skip_beatdiff:
        # BeatDiff fetch can run without GPU contention; prefer CPU device for load smoke
        report["methods"]["beatdiff"] = run_beatdiff(domains, torch.device("cpu"), args.fetch_weights)

    path = OUT / "smoke_report.json"
    path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, indent=2))
    print(f"wrote {path}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception:
        traceback.print_exc()
        raise SystemExit(1)
