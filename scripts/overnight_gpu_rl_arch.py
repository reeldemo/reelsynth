#!/usr/bin/env python3
"""
Overnight DenoiseOpt meta: PyTorch CUDA RL + searchable seam-operator networks.

Primary score: prolonged residual R in [0,1] (1=best), vs DualCosine baseline.
Saves unfitted (arch JSON) and fitted (weights+arch) under brand/artifacts/models/<run_id>/.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import random
import sys
import time
from dataclasses import asdict, dataclass, field
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import torch
import torch.nn as nn
import torch.nn.functional as F

ROOT = Path(__file__).resolve().parents[1]
META_ROOT = ROOT.parent / "denoise-opt-meta"
SEAM_W = 8
MLP_IN = SEAM_W * 2
N = 256
PROLONG = 16
DUAL_COSINE_BASELINE = 0.6944  # from CPU overnight sanity DualCosine bake

OPS = [
    "fade_pull",
    "polish",
    "pin",
    "dual_cosine",
    "classic",
    "soft_seam",
    "fir3",
    "mlp_seam",
]
N_ACTIONS = 8  # RL: mutate depth/width/act/ops/wet/fir/lr/reset


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


@dataclass
class ArchConfig:
    depth: int = 1
    width: int = 4
    act: str = "relu"  # relu|tanh|gelu
    ops: list[str] = field(default_factory=lambda: ["mlp_seam", "dual_cosine"])
    wet: float = 0.55
    fir: list[float] = field(default_factory=lambda: [0.25, 0.5, 0.25])
    cell_kind: str = "mlp"  # mlp|residual|gated

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


class SeamCell(nn.Module):
    """Searchable seam-window operator network (architecture cell)."""

    def __init__(self, cfg: ArchConfig):
        super().__init__()
        self.cfg = cfg
        h = max(1, min(16, cfg.width))
        d = max(1, min(3, cfg.depth))
        act = cfg.act
        layers: list[nn.Module] = []
        in_d = MLP_IN
        for i in range(d):
            out_d = h if i < d - 1 else MLP_IN
            layers.append(nn.Linear(in_d, out_d))
            if i < d - 1:
                if act == "tanh":
                    layers.append(nn.Tanh())
                elif act == "gelu":
                    layers.append(nn.GELU())
                else:
                    layers.append(nn.ReLU())
                if cfg.cell_kind == "gated":
                    layers.append(nn.Linear(out_d, out_d))
            in_d = out_d
        self.net = nn.Sequential(*layers)
        self.gate = nn.Parameter(torch.tensor(0.25))
        self.fir = nn.Parameter(torch.tensor(cfg.fir, dtype=torch.float32))
        self.wet = nn.Parameter(torch.tensor(cfg.wet, dtype=torch.float32))

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        # x: [B, MLP_IN]
        y = self.net(x)
        if self.cfg.cell_kind == "residual":
            y = x + torch.tanh(y) * self.gate
        elif self.cfg.cell_kind == "gated":
            g = torch.sigmoid(self.gate)
            y = g * y + (1 - g) * x
        else:
            y = x + torch.tanh(y) * self.gate
        return y


class RlPolicy(nn.Module):
    """Categorical policy over architecture/hyperparam edit actions."""

    def __init__(self, n_actions: int = N_ACTIONS, hidden: int = 64):
        super().__init__()
        self.net = nn.Sequential(
            nn.Linear(16, hidden),
            nn.Tanh(),
            nn.Linear(hidden, hidden),
            nn.Tanh(),
            nn.Linear(hidden, n_actions),
        )

    def forward(self, state: torch.Tensor) -> torch.Tensor:
        return self.net(state)


def make_batch(batch: int, n: int, device: torch.device) -> tuple[torch.Tensor, torch.Tensor]:
    """Synthetic wrap cycles: ideal continuous vs engine with seam cliff."""
    t = torch.linspace(0, 1, n, device=device).unsqueeze(0).expand(batch, -1)
    freqs = 1.0 + 3.0 * torch.rand(batch, 1, device=device)
    phase = 2 * math.pi * torch.rand(batch, 1, device=device)
    ideal = torch.sin(2 * math.pi * freqs * t + phase)
    ideal = ideal + 0.15 * torch.sin(4 * math.pi * freqs * t + phase * 0.7)
    # Engine: same but inject wrap discontinuity + high-freq crackle near seam
    eng = ideal.clone()
    cliff = (0.08 + 0.35 * torch.rand(batch, 1, device=device)) * (
        1.0 - 2.0 * torch.rand(batch, 1, device=device)
    )
    w = SEAM_W
    for i in range(w):
        a = i / max(w - 1, 1)
        eng[:, i] = eng[:, i] + cliff.squeeze(-1) * (1 - a)
        eng[:, -w + i] = eng[:, -w + i] - cliff.squeeze(-1) * a
    noise = 0.02 * torch.randn(batch, n, device=device)
    noise[:, w:-w] *= 0.15
    eng = eng + noise
    return ideal, eng


def pack_seam(frames: torch.Tensor) -> torch.Tensor:
    # [B, N] -> [B, MLP_IN]
    w = SEAM_W
    head = frames[:, :w]
    tail = frames[:, -w:]
    return torch.cat([head, tail], dim=1)


def write_seam(frames: torch.Tensor, y: torch.Tensor, wet: torch.Tensor) -> torch.Tensor:
    w = SEAM_W
    wet_c = wet.clamp(0.0, 1.0).view(-1, 1)
    head = frames[:, :w] * (1 - wet_c) + y[:, :w] * wet_c
    mid = frames[:, w:-w]
    tail = frames[:, -w:] * (1 - wet_c) + y[:, w:] * wet_c
    return torch.cat([head, mid, tail], dim=1)


def apply_fir3(frames: torch.Tensor, fir: torch.Tensor) -> torch.Tensor:
    # circular 3-tap near seam only (vectorized, autograd-safe)
    k = fir / (fir.abs().sum() + 1e-8)
    left = torch.roll(frames, 1, dims=1)
    right = torch.roll(frames, -1, dims=1)
    filtered = k[0] * left + k[1] * frames + k[2] * right
    w = SEAM_W + 2
    mask = frames.new_zeros(1, frames.shape[1])
    mask[:, :w] = 1.0
    mask[:, -w:] = 1.0
    return frames * (1.0 - mask) + filtered * mask


def dual_cosine_blend(frames: torch.Tensor) -> torch.Tensor:
    w = SEAM_W
    n = frames.shape[1]
    head_parts = []
    tail_parts = []
    for i in range(w):
        a = 0.5 - 0.5 * math.cos(math.pi * i / max(w, 1))
        head_parts.append(((1 - a) * frames[:, i] + a * frames[:, n - w + i]).unsqueeze(1))
        tail_parts.append(((1 - a) * frames[:, n - w + i] + a * frames[:, i]).unsqueeze(1))
    head = torch.cat(head_parts, dim=1)
    tail = torch.cat(tail_parts, dim=1)
    mid = frames[:, w : n - w]
    return torch.cat([head, mid, tail], dim=1)


def apply_ops(frames: torch.Tensor, cell: SeamCell, ops: list[str]) -> torch.Tensor:
    out = frames
    if "dual_cosine" in ops or "classic" in ops or "soft_seam" in ops:
        out = dual_cosine_blend(out)
    if "fir3" in ops:
        out = apply_fir3(out, cell.fir)
    if "mlp_seam" in ops or "fade_pull" in ops or "polish" in ops or "pin" in ops:
        x = pack_seam(out)
        y = cell(x)
        out = write_seam(out, y, cell.wet)
    return out


def prolong_tile(cycle: torch.Tensor, periods: int = PROLONG) -> torch.Tensor:
    return cycle.repeat(1, periods)


def residual_score(ideal: torch.Tensor, out: torch.Tensor) -> torch.Tensor:
    """R = clamp(1 - residual_rms / max(ideal_rms, eps), 0, 1); mean over batch."""
    idp = prolong_tile(ideal)
    otp = prolong_tile(out)
    resid = otp - idp
    residual_rms = resid.pow(2).mean(dim=1).sqrt()
    ideal_rms = idp.pow(2).mean(dim=1).sqrt().clamp_min(1e-6)
    r = (1.0 - residual_rms / ideal_rms).clamp(0.0, 1.0)
    return r


def arch_state_vec(cfg: ArchConfig, device: torch.device) -> torch.Tensor:
    op_bits = [1.0 if o in cfg.ops else 0.0 for o in OPS]
    act_id = {"relu": 0.0, "tanh": 0.5, "gelu": 1.0}.get(cfg.act, 0.0)
    cell_id = {"mlp": 0.0, "residual": 0.5, "gated": 1.0}.get(cfg.cell_kind, 0.0)
    vec = op_bits + [
        cfg.depth / 3.0,
        cfg.width / 16.0,
        act_id,
        cfg.wet,
        cell_id,
        abs(cfg.fir[1]),
        0.0,
        0.0,
    ]
    return torch.tensor(vec[:16], dtype=torch.float32, device=device)


def mutate_arch(cfg: ArchConfig, action: int, rng: random.Random) -> ArchConfig:
    c = ArchConfig(**cfg.to_dict())
    if action == 0:
        c.depth = max(1, min(3, c.depth + rng.choice([-1, 1])))
    elif action == 1:
        c.width = max(2, min(16, c.width + rng.choice([-2, -1, 1, 2])))
    elif action == 2:
        c.act = rng.choice(["relu", "tanh", "gelu"])
    elif action == 3:
        op = rng.choice(OPS)
        if op in c.ops and len(c.ops) > 1:
            c.ops = [x for x in c.ops if x != op]
        else:
            c.ops = list(dict.fromkeys(c.ops + [op]))
    elif action == 4:
        c.wet = float(max(0.05, min(0.95, c.wet + rng.uniform(-0.2, 0.2))))
    elif action == 5:
        c.fir = [rng.uniform(0.05, 0.6) for _ in range(3)]
    elif action == 6:
        c.cell_kind = rng.choice(["mlp", "residual", "gated"])
    else:
        c = ArchConfig(
            depth=rng.randint(1, 3),
            width=rng.choice([2, 4, 6, 8, 12, 16]),
            act=rng.choice(["relu", "tanh", "gelu"]),
            ops=rng.sample(OPS, k=rng.randint(2, 4)),
            wet=rng.uniform(0.2, 0.85),
            fir=[rng.uniform(0.1, 0.5) for _ in range(3)],
            cell_kind=rng.choice(["mlp", "residual", "gated"]),
        )
    if not (set(c.ops) & {"mlp_seam", "fade_pull", "polish", "pin", "fir3"}):
        c.ops = list(dict.fromkeys(list(c.ops) + ["mlp_seam"]))
    return c


def fit_cell(
    cell: SeamCell,
    ops: list[str],
    device: torch.device,
    steps: int = 24,
    batch: int = 32,
    lr: float = 3e-3,
) -> tuple[float, bool]:
    trainable_ops = {"mlp_seam", "fade_pull", "polish", "pin", "fir3"}
    can_train = bool(set(ops) & trainable_ops)
    opt = torch.optim.Adam(cell.parameters(), lr=lr) if can_train else None
    prev = None
    patience = 0
    last_r = 0.0
    converged = False
    for _ in range(steps):
        ideal, eng = make_batch(batch, N, device)
        out = apply_ops(eng, cell, ops)
        r = residual_score(ideal, out).mean()
        last_r = float(r.detach().item())
        if can_train and opt is not None:
            loss = 1.0 - r
            if loss.requires_grad:
                opt.zero_grad(set_to_none=True)
                loss.backward()
                opt.step()
        if prev is not None:
            rel = abs(prev - last_r) / max(abs(prev), 1e-6)
            if rel < 1e-4:
                patience += 1
                if patience >= 3:
                    converged = True
                    break
            else:
                patience = 0
        prev = last_r
        if not can_train:
            converged = True
            break
    return last_r, converged


@torch.no_grad()
def eval_cell(cell: SeamCell, ops: list[str], device: torch.device, batch: int = 64) -> float:
    ideal, eng = make_batch(batch, N, device)
    out = apply_ops(eng, cell, ops)
    return float(residual_score(ideal, out).mean().item())


@torch.no_grad()
def dual_cosine_baseline(device: torch.device, batch: int = 128) -> float:
    ideal, eng = make_batch(batch, N, device)
    out = dual_cosine_blend(eng)
    return float(residual_score(ideal, out).mean().item())


def save_unfitted(run_dir: Path, cfg: ArchConfig, tag: str) -> Path:
    d = run_dir / "unfitted"
    d.mkdir(parents=True, exist_ok=True)
    path = d / f"{tag}_arch.json"
    path.write_text(json.dumps({"architecture": cfg.to_dict(), "tag": tag}, indent=2), encoding="utf-8")
    return path


def save_fitted(
    run_dir: Path,
    cfg: ArchConfig,
    cell: SeamCell,
    policy: RlPolicy | None,
    residual: float,
    tag: str,
) -> Path:
    d = run_dir / "fitted"
    d.mkdir(parents=True, exist_ok=True)
    path = d / f"{tag}_fitted.pt"
    payload = {
        "architecture": cfg.to_dict(),
        "residual": residual,
        "cell_state_dict": cell.state_dict(),
        "policy_state_dict": policy.state_dict() if policy is not None else None,
        "tag": tag,
    }
    torch.save(payload, path)
    meta = d / f"{tag}_fitted.json"
    meta.write_text(
        json.dumps(
            {
                "architecture": cfg.to_dict(),
                "residual": residual,
                "weights_path": str(path),
                "tag": tag,
            },
            indent=2,
        ),
        encoding="utf-8",
    )
    return path


def log_line(log_path: Path, msg: str) -> None:
    line = f"{datetime.now().isoformat(timespec='seconds')} {msg}"
    print(line, flush=True)
    with log_path.open("a", encoding="utf-8") as f:
        f.write(line + "\n")


def append_history(history_path: Path, row: dict[str, Any]) -> None:
    """Append one JSONL record (dense learning-curve point)."""
    with history_path.open("a", encoding="utf-8") as f:
        f.write(json.dumps(row, separators=(",", ":")) + "\n")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--iters", type=int, default=270_000)
    ap.add_argument("--ckpt-every", type=int, default=500)
    ap.add_argument(
        "--history-every",
        type=int,
        default=1,
        help="Append one JSONL history row every N iters (default 1 = every iter).",
    )
    ap.add_argument("--batch", type=int, default=48)
    ap.add_argument("--fit-steps", type=int, default=20)
    ap.add_argument("--device", type=str, default="cuda")
    ap.add_argument("--run-id", type=str, default="")
    ap.add_argument("--max-hours", type=float, default=24.0)
    args = ap.parse_args()
    if args.history_every < 1:
        print("ERROR: --history-every must be >= 1", file=sys.stderr)
        return 2

    if args.device.startswith("cuda") and not torch.cuda.is_available():
        print("ERROR: CUDA requested but torch.cuda.is_available() is False", file=sys.stderr)
        return 2

    device = torch.device(args.device if torch.cuda.is_available() and args.device.startswith("cuda") else "cpu")
    gpu_name = torch.cuda.get_device_name(0) if device.type == "cuda" else "cpu"
    run_id = args.run_id or f"gpu-rl-arch-{utc_now()}"
    run_dir = ROOT / "brand" / "artifacts" / "models" / run_id
    run_dir.mkdir(parents=True, exist_ok=True)
    # mirror into meta repo
    meta_run = META_ROOT / "artifacts" / "models" / run_id
    meta_run.mkdir(parents=True, exist_ok=True)

    log_path = ROOT / "brand" / "artifacts" / f"overnight_gpu_rl_arch_{run_id}.log"
    ckpt_dir = run_dir / "checkpoints"
    ckpt_dir.mkdir(parents=True, exist_ok=True)

    rng = random.Random(0x0A172730)
    torch.manual_seed(0x0A172730)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(0x0A172730)

    history_path = run_dir / "history.jsonl"
    # Fresh history file for this run_id (run_id is unique per launch).
    if not history_path.exists():
        history_path.write_text("", encoding="utf-8")

    baseline = dual_cosine_baseline(device)
    # ETA / sizing note for launch log (paper-facing target may exceed wall clock).
    now_local = datetime.now().astimezone()
    log_line(
        log_path,
        f"START run_id={run_id} device={device} gpu={gpu_name} "
        f"torch={torch.__version__} cuda_available={torch.cuda.is_available()} "
        f"dual_cosine_baseline={baseline:.4f} target_iters={args.iters} "
        f"max_hours={args.max_hours} history_every={args.history_every} "
        f"history_path={history_path} local_start={now_local.isoformat(timespec='seconds')}",
    )
    (run_dir / "run_meta.json").write_text(
        json.dumps(
            {
                "run_id": run_id,
                "device": str(device),
                "gpu": gpu_name,
                "torch": torch.__version__,
                "cuda_available": torch.cuda.is_available(),
                "dual_cosine_baseline": baseline,
                "target_iters": args.iters,
                "max_hours": args.max_hours,
                "history_every": args.history_every,
                "history_path": str(history_path),
                "pid": os.getpid(),
                "started_at": utc_now(),
            },
            indent=2,
        ),
        encoding="utf-8",
    )

    policy = RlPolicy().to(device)
    policy_opt = torch.optim.Adam(policy.parameters(), lr=1e-3)
    cfg = ArchConfig()
    save_unfitted(run_dir, cfg, "init")
    save_unfitted(meta_run, cfg, "init")

    champion_r = -1.0
    champion_cfg = cfg
    champion_cell: SeamCell | None = None
    branch_best = {"rl": 0.0, "nas": 0.0, "combo": 0.0}
    t0 = time.time()
    max_sec = args.max_hours * 3600.0

    # Keep a live tensor on GPU so nvidia-smi shows residency even between iters
    keepalive = torch.zeros(1, device=device)

    for it in range(1, args.iters + 1):
        if time.time() - t0 > max_sec:
            log_line(log_path, f"STOP time budget reached at iter={it}")
            break

        branch = ("rl", "nas", "combo")[it % 3]
        state = arch_state_vec(cfg, device).unsqueeze(0)
        logits = policy(state)
        dist = torch.distributions.Categorical(logits=logits)
        action = int(dist.sample().item())
        logprob = dist.log_prob(torch.tensor(action, device=device))

        if branch == "nas":
            # evolutionary-ish random arch cell
            trial_cfg = ArchConfig(
                depth=rng.randint(1, 3),
                width=rng.choice([2, 4, 6, 8, 12, 16]),
                act=rng.choice(["relu", "tanh", "gelu"]),
                ops=rng.sample(OPS, k=rng.randint(2, 5)),
                wet=rng.uniform(0.15, 0.9),
                fir=[rng.uniform(0.05, 0.55) for _ in range(3)],
                cell_kind=rng.choice(["mlp", "residual", "gated"]),
            )
        elif branch == "combo":
            trial_cfg = mutate_arch(mutate_arch(cfg, action, rng), rng.randrange(N_ACTIONS), rng)
        else:
            trial_cfg = mutate_arch(cfg, action, rng)

        if it == 1 or it % args.ckpt_every == 1:
            save_unfitted(run_dir, trial_cfg, f"iter_{it:06d}")

        cell = SeamCell(trial_cfg).to(device)
        r_fit, converged = fit_cell(
            cell, trial_cfg.ops, device, steps=args.fit_steps, batch=args.batch
        )
        r_eval = eval_cell(cell, trial_cfg.ops, device, batch=max(64, args.batch))
        residual = 0.5 * r_fit + 0.5 * r_eval
        branch_best[branch] = max(branch_best[branch], residual)

        # REINFORCE with residual reward vs baseline
        advantage = residual - baseline
        loss_pi = -(logprob * advantage)
        loss_val = float(loss_pi.detach().item())
        policy_opt.zero_grad(set_to_none=True)
        loss_pi.backward()
        policy_opt.step()

        if it == 1 or (it % args.history_every == 0):
            tag = f"iter_{it:06d}"
            champ_now = residual if residual > champion_r else (champion_r if champion_r >= 0 else residual)
            append_history(
                history_path,
                {
                    "iter": it,
                    "t_sec": round(time.time() - t0, 6),
                    "residual": residual,
                    "champ": champ_now,
                    "branch": branch,
                    "branch_best_rl": branch_best["rl"],
                    "branch_best_nas": branch_best["nas"],
                    "branch_best_combo": branch_best["combo"],
                    "loss": loss_val,
                    "arch_id": tag,
                    "tag": tag,
                    "converged": converged,
                },
            )

        if residual > champion_r:
            champion_r = residual
            champion_cfg = trial_cfg
            champion_cell = cell
            cfg = trial_cfg  # climb
            save_fitted(run_dir, trial_cfg, cell, policy, residual, f"champion_iter_{it:06d}")
            save_fitted(meta_run, trial_cfg, cell, policy, residual, f"champion_iter_{it:06d}")
            log_line(
                log_path,
                f"NEW_CHAMPION iter={it} residual={residual:.4f} "
                f"delta_vs_dual={residual - baseline:+.4f} arch={trial_cfg.to_dict()}",
            )

        if it % 25 == 0 or it == 1:
            mem = torch.cuda.memory_allocated(device) / (1024**2) if device.type == "cuda" else 0.0
            elapsed = time.time() - t0
            rate = it / max(elapsed, 1e-6)
            log_line(
                log_path,
                f"progress {it}/{args.iters} branch={branch} residual={residual:.4f} "
                f"champ={champion_r:.4f} baseline={baseline:.4f} "
                f"converged={converged} gpu_mem_mb={mem:.1f} "
                f"iters_per_sec={rate:.2f} elapsed_h={elapsed/3600:.3f}",
            )
            # touch keepalive
            keepalive = keepalive + 0.0

        if it % args.ckpt_every == 0:
            ckpt = {
                "iter": it,
                "champion_residual": champion_r,
                "champion_arch": champion_cfg.to_dict(),
                "baseline_dual_cosine": baseline,
                "branch_best": branch_best,
                "elapsed_sec": time.time() - t0,
                "gpu": gpu_name,
                "pid": os.getpid(),
            }
            ckpt_path = ckpt_dir / f"ckpt_iter_{it:06d}.json"
            ckpt_path.write_text(json.dumps(ckpt, indent=2), encoding="utf-8")
            if champion_cell is not None:
                save_fitted(
                    run_dir, champion_cfg, champion_cell, policy, champion_r, f"ckpt_iter_{it:06d}"
                )
            # progress summary for parent
            summary = {
                **ckpt,
                "log_path": str(log_path),
                "run_dir": str(run_dir),
                "unfitted_dir": str(run_dir / "unfitted"),
                "fitted_dir": str(run_dir / "fitted"),
            }
            (ROOT / "brand" / "artifacts" / "overnight_gpu_rl_arch_latest.json").write_text(
                json.dumps(summary, indent=2), encoding="utf-8"
            )
            (META_ROOT / "artifacts" / "overnight_gpu_rl_arch_latest.json").write_text(
                json.dumps(summary, indent=2), encoding="utf-8"
            )
            log_line(log_path, f"CHECKPOINT iter={it} wrote {ckpt_path}")

    # final save
    if champion_cell is not None:
        save_fitted(run_dir, champion_cfg, champion_cell, policy, champion_r, "final_champion")
        save_fitted(meta_run, champion_cfg, champion_cell, policy, champion_r, "final_champion")
    final = {
        "run_id": run_id,
        "iters_done": it if args.iters else 0,
        "champion_residual": champion_r,
        "dual_cosine_baseline": baseline,
        "delta": champion_r - baseline,
        "gpu": gpu_name,
        "elapsed_sec": time.time() - t0,
        "run_dir": str(run_dir),
        "log_path": str(log_path),
    }
    (run_dir / "final_summary.json").write_text(json.dumps(final, indent=2), encoding="utf-8")
    log_line(log_path, f"DONE {json.dumps(final)}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
