#!/usr/bin/env python3
"""Compare meta-learning outer loops for DenoiseOpt (not PPO/GA/PBT rehashes).

Approaches (OA-citable; LSTM + xLSTM in searchable bake vocabulary):
  random       — uniform random arch/hp (control)
  cmaes        — CMA-ES over continuous arch+hp encoding (Hansen)
  reinforce    — vanilla policy gradient over discrete edit actions
  aging_evo    — regularized / aging evolution (Real et al.)
  tpe          — lightweight TPE-style Bayesian opt over discrete arch
  hybrid_lstm  — hybrid PPO+GA+PBT+NAS+combo with LSTM+xLSTM; co-tunes fit/PPO HPs
                 and reward shaping modes (abs_r / vs_dualcosine / vs_nobake / neglog_gap)
                 via PBT + plateau adapt. This matched compare is the sole GPU
                 experimentation vehicle (no concurrent overnight hybrid).

Each approach runs `--iters` evaluations (default 5000), search seed 1902771841,
batch/fit defaults matching overnight_gpu_rl_arch. Checkpoint/resume per approach.
"""
from __future__ import annotations

import argparse
import json
import math
import os
import random
import shutil
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import torch
import torch.nn as nn

sys.path.insert(0, str(Path(__file__).resolve().parent))
import overnight_gpu_rl_arch as og  # noqa: E402
from denoise_arch_blocks import BLOCKS, CELL_KINDS, MAX_GRAPH_LEN, MAX_SEARCH_DEPTH, MAX_WIDTH  # noqa: E402
from denoise_meta_evo import depth_mixture_bonus  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
META_ROOT = ROOT.parent / "denoise-opt-meta"
DEFAULT_SEED = og.DEFAULT_SEED
APPROACHES = ("random", "cmaes", "reinforce", "aging_evo", "tpe", "hybrid_lstm")

# Continuous CMA dim: depth, width, wet, lr_log, fit_steps, batch_idx, cell, act,
# moe, n_blocks + one logit per BLOCK + soft presence for key ops.
_OP_KEYS = ("mlp_seam", "dual_cosine", "fir3", "cycle_net", "hann_blend", "asym_wet")
CMA_DIM = 10 + len(BLOCKS) + len(_OP_KEYS)
RECURRENT_BLOCKS = ("lstm", "xlstm")


def _tb_writer(out_dir: Path):
    """Optional TensorBoard writer (torch.utils.tensorboard)."""
    try:
        from torch.utils.tensorboard import SummaryWriter

        tb_dir = out_dir / "tb"
        tb_dir.mkdir(parents=True, exist_ok=True)
        return SummaryWriter(log_dir=str(tb_dir))
    except Exception:
        return None


def tb_log(writer, approach: str, it: int, *, champ: float, residual: float, wall_s: float) -> None:
    if writer is None:
        return
    try:
        writer.add_scalar(f"{approach}/champ_R", champ, it)
        writer.add_scalar(f"{approach}/trial_R", residual, it)
        writer.add_scalar(f"{approach}/wall_h", wall_s / 3600.0, it)
        writer.flush()
    except Exception:
        pass


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def normalize_blocks(cfg: og.ArchConfig) -> list[str]:
    return list(og.normalize_graph(cfg.blocks, cfg.cell_kind))


def arch_uses_lstm(cfg: og.ArchConfig) -> bool:
    blocks = normalize_blocks(cfg)
    return any(b in blocks for b in RECURRENT_BLOCKS) or cfg.cell_kind in RECURRENT_BLOCKS


def arch_recurrent_flags(cfg: og.ArchConfig) -> dict[str, bool]:
    blocks = set(normalize_blocks(cfg))
    blocks.add(cfg.cell_kind)
    return {
        "lstm": "lstm" in blocks,
        "xlstm": "xlstm" in blocks,
        "any_recurrent": bool(blocks & set(RECURRENT_BLOCKS)),
    }


def maybe_inject_recurrent(cfg: og.ArchConfig, rng: random.Random, p: float = 0.2) -> og.ArchConfig:
    """With probability p, inject lstm and/or xlstm into the bake graph."""
    if rng.random() >= p:
        return cfg
    pick = rng.choice(["lstm", "xlstm", "both"])
    add: list[str] = []
    if pick in ("lstm", "both") and "lstm" not in cfg.blocks:
        add.append("lstm")
    if pick in ("xlstm", "both") and "xlstm" not in cfg.blocks:
        add.append("xlstm")
    if not add:
        return cfg
    cfg.blocks = og.normalize_graph(list(cfg.blocks) + add, cfg.cell_kind)
    return cfg


def evaluate(
    cfg: og.ArchConfig,
    hp: og.HyperParams,
    device: torch.device,
    *,
    baseline: float,
    fit_steps_default: int,
    batch_default: int,
) -> tuple[float, float, og.SeamCell]:
    """Fit + eval; return (residual_raw, residual_with_bonus, cell)."""
    cell = og.SeamCell(cfg).to(device)
    fit_steps = int(hp.fit_steps or fit_steps_default)
    batch = int(hp.batch or batch_default)
    r_fit, _ = og.fit_cell(
        cell,
        cfg.ops,
        device,
        steps=fit_steps,
        batch=batch,
        lr=hp.lr,
        adv_coef=hp.adv_coef if cfg.use_adv_aux else 0.0,
    )
    r_eval = og.eval_cell(cell, cfg.ops, device, batch=max(64, batch))
    residual_raw = og.finite_scalar(0.5 * r_fit + 0.5 * r_eval, 0.0)
    dmb = og.finite_scalar(
        depth_mixture_bonus(
            residual_raw,
            baseline,
            cfg.depth,
            len(cfg.blocks),
            cfg.moe_mode,
        ),
        0.0,
    )
    return residual_raw, residual_raw + dmb, cell


def append_hist(path: Path, row: dict[str, Any]) -> None:
    with path.open("a", encoding="utf-8") as f:
        f.write(json.dumps(row, separators=(",", ":")) + "\n")


def save_ckpt(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(".tmp")
    tmp.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    tmp.replace(path)


def load_ckpt(path: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def last_hist_row(path: Path) -> dict[str, Any] | None:
    """O(1)-ish tail read of the latest history.jsonl record."""
    if not path.is_file():
        return None
    try:
        size = path.stat().st_size
        if size <= 0:
            return None
        with path.open("rb") as f:
            f.seek(max(0, size - 65536))
            chunk = f.read().decode("utf-8", errors="ignore")
        lines = [ln for ln in chunk.splitlines() if ln.strip()]
        if not lines:
            return None
        return json.loads(lines[-1])
    except Exception:
        return None


STATUS_NAME = "STATUS.json"
STATUS_LATEST = ROOT / "brand" / "artifacts" / "meta_approach_STATUS.json"


def write_status(
    out_dir: Path,
    *,
    phase: str,
    target_iters: int,
    approaches: list[str],
    current: str | None = None,
    current_iter: int | None = None,
    pid: int | None = None,
    extra: dict[str, Any] | None = None,
) -> Path:
    """Crash-safe aggregate status from history tails + checkpoints (live iters)."""
    rows: list[dict[str, Any]] = []
    for name in approaches:
        ad = out_dir / name
        ckpt = load_ckpt(ad / "checkpoint.json") or {}
        summary = load_ckpt(ad / "summary.json") or {}
        hist = ad / "history.jsonl"
        last = last_hist_row(hist) or {}
        hist_iter = int(last.get("iter") or 0)
        done = max(
            hist_iter,
            int(ckpt.get("iters_done") or 0),
            int(summary.get("iters_done") or 0),
        )
        champ_val = last.get(
            "champ_raw",
            summary.get("champ_raw", ckpt.get("champ_raw", ckpt.get("champ_r", float("nan")))),
        )
        try:
            champ = float(champ_val)
        except (TypeError, ValueError):
            champ = float("nan")
        complete = bool(summary) and done >= target_iters
        rows.append(
            {
                "approach": name,
                "iters_done": done,
                "target_iters": target_iters,
                "pct": round(100.0 * done / max(target_iters, 1), 2),
                "champ_r": None if champ != champ else champ,
                "lstm_in_champ": bool(
                    last.get(
                        "lstm_in_champ",
                        summary.get("lstm_in_champ", ckpt.get("champ_lstm", False)),
                    )
                ),
                "xlstm_in_champ": bool(
                    last.get(
                        "xlstm_in_champ",
                        summary.get("xlstm_in_champ", ckpt.get("champ_xlstm", False)),
                    )
                ),
                "wall_s": float(
                    last.get("wall_s", summary.get("wall_s", ckpt.get("wall_s", 0.0))) or 0.0
                ),
                "complete": complete,
                "history_lines": hist_iter,
                "has_checkpoint": (ad / "checkpoint.json").is_file(),
            }
        )
    n_done = sum(1 for r in rows if r["complete"])
    payload: dict[str, Any] = {
        "schema": "denoiseopt.meta_approach_status.v1",
        "updated_at": datetime.now(timezone.utc).isoformat(),
        "phase": phase,
        "pid": pid if pid is not None else os.getpid(),
        "out_dir": str(out_dir),
        "target_iters": target_iters,
        "approaches_planned": approaches,
        "current_approach": current,
        "current_iter": current_iter,
        "n_complete": n_done,
        "n_total": len(approaches),
        "all_complete": n_done >= len(approaches) and len(approaches) > 0,
        "rows": rows,
    }
    if extra:
        payload.update(extra)
    out_dir.mkdir(parents=True, exist_ok=True)
    path = out_dir / STATUS_NAME
    save_ckpt(path, payload)
    try:
        STATUS_LATEST.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(path, STATUS_LATEST)
    except Exception:
        pass
    return path


# --- CMA-ES (lightweight, no external deps) ---------------------------------


class TinyCMA:
    """Hansen-style CMA-ES skeleton (diagonal covariance) for continuous encoding."""

    def __init__(self, dim: int, rng: random.Random, sigma: float = 0.35):
        self.dim = dim
        self.rng = rng
        self.mean = [0.0] * dim
        self.sigma = sigma
        self.c_diag = [1.0] * dim
        self.pc = [0.0] * dim
        self.generation = 0
        self.lam = max(4, 4 + int(3 * math.log(dim)))
        self.mu = max(1, self.lam // 2)
        w = [math.log(self.mu + 0.5) - math.log(i + 1) for i in range(self.mu)]
        s = sum(w)
        self.weights = [x / s for x in w]
        self.mueff = 1.0 / sum(x * x for x in self.weights)
        self.cc = (4.0 + self.mueff / dim) / (dim + 4.0 + 2.0 * self.mueff / dim)
        self.c1 = 2.0 / ((dim + 1.3) ** 2 + self.mueff)
        self.cmu = min(
            1.0 - self.c1,
            2.0 * (self.mueff - 2.0 + 1.0 / self.mueff) / ((dim + 2.0) ** 2 + self.mueff),
        )
        self.cs = (self.mueff + 2.0) / (dim + self.mueff + 5.0)
        self.damps = 1.0 + 2.0 * max(0.0, math.sqrt((self.mueff - 1.0) / (dim + 1.0)) - 1.0) + self.cs
        self.ps = [0.0] * dim
        self.chi_n = math.sqrt(dim) * (1.0 - 1.0 / (4.0 * dim) + 1.0 / (21.0 * dim * dim))

    def ask(self) -> list[list[float]]:
        samples: list[list[float]] = []
        for _ in range(self.lam):
            z = [self.rng.gauss(0.0, 1.0) for _ in range(self.dim)]
            x = [
                self.mean[i] + self.sigma * math.sqrt(max(1e-8, self.c_diag[i])) * z[i]
                for i in range(self.dim)
            ]
            samples.append(x)
        return samples

    def tell(self, xs: list[list[float]], fitness: list[float]) -> None:
        # Maximize fitness → sort descending
        order = sorted(range(len(xs)), key=lambda i: fitness[i], reverse=True)
        old_mean = list(self.mean)
        self.mean = [0.0] * self.dim
        for w, idx in zip(self.weights, order[: self.mu]):
            for i in range(self.dim):
                self.mean[i] += w * xs[idx][i]
        # Evolution paths (diagonal)
        for i in range(self.dim):
            y = (self.mean[i] - old_mean[i]) / max(self.sigma, 1e-8)
            self.ps[i] = (1.0 - self.cs) * self.ps[i] + math.sqrt(
                self.cs * (2.0 - self.cs) * self.mueff
            ) * y / math.sqrt(max(1e-8, self.c_diag[i]))
            self.pc[i] = (1.0 - self.cc) * self.pc[i] + math.sqrt(
                self.cc * (2.0 - self.cc) * self.mueff
            ) * y
            # Rank-mu + rank-one on diagonal
            c_update = self.c1 * self.pc[i] * self.pc[i]
            for w, idx in zip(self.weights, order[: self.mu]):
                zi = (xs[idx][i] - old_mean[i]) / max(self.sigma, 1e-8)
                c_update += self.cmu * w * zi * zi
            self.c_diag[i] = (1.0 - self.c1 - self.cmu) * self.c_diag[i] + c_update
            self.c_diag[i] = max(1e-8, min(1e2, self.c_diag[i]))
        ps_norm = math.sqrt(sum(p * p for p in self.ps))
        self.sigma *= math.exp((self.cs / self.damps) * (ps_norm / self.chi_n - 1.0))
        self.sigma = max(1e-3, min(2.0, self.sigma))
        self.generation += 1

    def state_dict(self) -> dict[str, Any]:
        return {
            "mean": self.mean,
            "sigma": self.sigma,
            "c_diag": self.c_diag,
            "pc": self.pc,
            "ps": self.ps,
            "generation": self.generation,
        }

    def load_state_dict(self, d: dict[str, Any]) -> None:
        self.mean = list(d["mean"])
        self.sigma = float(d["sigma"])
        self.c_diag = list(d["c_diag"])
        self.pc = list(d["pc"])
        self.ps = list(d["ps"])
        self.generation = int(d.get("generation", 0))


def decode_cma(vec: list[float], rng: random.Random) -> tuple[og.ArchConfig, og.HyperParams]:
    def sig(x: float) -> float:
        return 1.0 / (1.0 + math.exp(-max(-20.0, min(20.0, x))))

    depth = 1 + int(sig(vec[0]) * (MAX_SEARCH_DEPTH - 1))
    width_choices = [4, 6, 8, 12, 16, 24, 32, 40, 48]
    width = width_choices[min(len(width_choices) - 1, int(sig(vec[1]) * len(width_choices)))]
    wet = 0.05 + 0.9 * sig(vec[2])
    lr = 10 ** (-4.0 + 2.0 * sig(vec[3]))
    fit_steps = [16, 20, 24, 32, 40, 48][min(5, int(sig(vec[4]) * 6))]
    batch = [32, 48, 64][min(2, int(sig(vec[5]) * 3))]
    cell = CELL_KINDS[min(len(CELL_KINDS) - 1, int(sig(vec[6]) * len(CELL_KINDS)))]
    act = og.ACTS[min(len(og.ACTS) - 1, int(sig(vec[7]) * len(og.ACTS)))]
    moe = "moe_parallel" if sig(vec[8]) > 0.55 else "sequential"
    n_extra = int(sig(vec[9]) * (MAX_GRAPH_LEN - 1))
    block_scores = list(enumerate(vec[10 : 10 + len(BLOCKS)]))
    block_scores.sort(key=lambda t: t[1], reverse=True)
    chosen = [BLOCKS[i] for i, _ in block_scores[: max(1, n_extra + 1)]]
    if cell not in chosen:
        chosen = [cell] + chosen
    blocks = og.normalize_graph(chosen, cell)
    ops = ["mlp_seam", "dual_cosine"]
    for j, name in enumerate(_OP_KEYS):
        if sig(vec[10 + len(BLOCKS) + j]) > 0.45 and name not in ops:
            ops.append(name)
    ops = og.ensure_trainable_ops(ops)
    cfg = og.ArchConfig(
        depth=depth,
        width=min(MAX_WIDTH, width),
        act=act,
        ops=ops,
        wet=wet,
        fir=[0.2, 0.5, 0.2, 0.1, 0.1],
        cell_kind=cell,
        soft_logits=[rng.uniform(-0.5, 0.5) for _ in og.OPS],
        blocks=blocks,
        use_adv_aux=False,
        moe_mode=moe,
    )
    hp = og.HyperParams(lr=lr, fit_steps=fit_steps, batch=batch)
    return cfg, hp


# --- TPE (lightweight) ------------------------------------------------------


class TinyTPE:
    """Tree-structured Parzen Estimator spirit over discrete categorical choices."""

    def __init__(self, rng: random.Random, gamma: float = 0.2):
        self.rng = rng
        self.gamma = gamma
        self.obs: list[tuple[dict[str, Any], float]] = []

    def _sample_prior(self) -> dict[str, Any]:
        cell = self.rng.choice(CELL_KINDS)
        return {
            "cell_kind": cell,
            "depth": self.rng.randint(1, MAX_SEARCH_DEPTH),
            "width": self.rng.choice([4, 6, 8, 12, 16, 24, 32, 40, 48]),
            "act": self.rng.choice(list(og.ACTS)),
            "moe_mode": self.rng.choice(["sequential", "moe_parallel"]),
            "blocks": og.random_block_graph(self.rng, cell, max_extra=3),
            "ops_k": self.rng.randint(2, 6),
            "wet": self.rng.uniform(0.1, 0.95),
            "lr_exp": self.rng.uniform(-4.0, -2.0),
            "fit_steps": self.rng.choice([16, 20, 24, 32, 40, 48]),
            "batch": self.rng.choice([32, 48, 64]),
            "use_lstm": self.rng.random() < 0.28,
            "use_xlstm": self.rng.random() < 0.28,
        }

    def ask(self) -> dict[str, Any]:
        if len(self.obs) < 12:
            return self._sample_prior()
        scores = sorted(r for _, r in self.obs)
        cut = scores[max(0, int((1.0 - self.gamma) * (len(scores) - 1)))]
        good = [x for x, r in self.obs if r >= cut]
        bad = [x for x, r in self.obs if r < cut] or good
        # Sample categorical from good with small epsilon from bad/prior
        def pick(key: str, choices: list[Any]) -> Any:
            g_vals = [g[key] for g in good if key in g]
            if self.rng.random() < 0.15 or not g_vals:
                return self.rng.choice(choices)
            # frequency in good
            from collections import Counter

            c = Counter(tuple(v) if isinstance(v, list) else v for v in g_vals)
            items, weights = zip(*c.items())
            # unhash list blocks separately
            if key == "blocks":
                return self.rng.choice(g_vals)
            idx = self.rng.choices(range(len(items)), weights=weights, k=1)[0]
            return items[idx]

        cell = pick("cell_kind", list(CELL_KINDS))
        sample = {
            "cell_kind": cell,
            "depth": pick("depth", list(range(1, MAX_SEARCH_DEPTH + 1))),
            "width": pick("width", [4, 6, 8, 12, 16, 24, 32, 40, 48]),
            "act": pick("act", list(og.ACTS)),
            "moe_mode": pick("moe_mode", ["sequential", "moe_parallel"]),
            "blocks": pick("blocks", [og.random_block_graph(self.rng, cell, max_extra=3)]),
            "ops_k": pick("ops_k", list(range(2, 7))),
            "wet": float(pick("wet", [self.rng.uniform(0.1, 0.95)])),
            "lr_exp": float(pick("lr_exp", [self.rng.uniform(-4.0, -2.0)])),
            "fit_steps": pick("fit_steps", [16, 20, 24, 32, 40, 48]),
            "batch": pick("batch", [32, 48, 64]),
            "use_lstm": pick("use_lstm", [True, False]),
            "use_xlstm": pick("use_xlstm", [True, False]),
        }
        # Mild mutation vs bad attractor
        if bad and self.rng.random() < 0.2:
            b = self.rng.choice(bad)
            for k in ("depth", "width", "act"):
                if self.rng.random() < 0.3:
                    sample[k] = b[k]
        return sample

    def tell(self, sample: dict[str, Any], score: float) -> None:
        self.obs.append((sample, float(score)))

    def state_dict(self) -> dict[str, Any]:
        return {"obs": [{"x": x, "r": r} for x, r in self.obs[-2000:]]}

    def load_state_dict(self, d: dict[str, Any]) -> None:
        self.obs = [(o["x"], float(o["r"])) for o in d.get("obs", [])]


def tpe_to_arch(sample: dict[str, Any], rng: random.Random) -> tuple[og.ArchConfig, og.HyperParams]:
    cell = sample["cell_kind"]
    blocks = list(sample.get("blocks") or [cell])
    extra: list[str] = []
    if sample.get("use_lstm") and "lstm" not in blocks:
        extra.append("lstm")
    if sample.get("use_xlstm") and "xlstm" not in blocks:
        extra.append("xlstm")
    blocks = og.normalize_graph(blocks + extra, cell)
    k = int(sample.get("ops_k", 3))
    ops = og.ensure_trainable_ops(rng.sample(og.OPS, k=min(k, len(og.OPS))))
    cfg = og.ArchConfig(
        depth=int(sample["depth"]),
        width=min(MAX_WIDTH, int(sample["width"])),
        act=str(sample["act"]),
        ops=ops,
        wet=float(sample["wet"]),
        fir=[rng.uniform(0.05, 0.55) for _ in range(5)],
        cell_kind=cell,
        soft_logits=[rng.uniform(-1.0, 1.0) for _ in og.OPS],
        blocks=blocks,
        use_adv_aux=False,
        moe_mode=str(sample.get("moe_mode", "sequential")),
    )
    hp = og.HyperParams(
        lr=10 ** float(sample["lr_exp"]),
        fit_steps=int(sample["fit_steps"]),
        batch=int(sample["batch"]),
    )
    return cfg, hp


# --- Aging evolution --------------------------------------------------------


def aging_step(
    pop: list[og.Individual],
    rng: random.Random,
    device: torch.device,
    baseline: float,
    fit_steps: int,
    batch: int,
) -> tuple[og.ArchConfig, og.HyperParams, float, float, og.SeamCell]:
    for ind in pop:
        ind.age += 1
    # Tournament parent among non-oldest
    if len(pop) >= 3:
        cand = rng.sample(pop, k=min(3, len(pop)))
        parent = max(cand, key=lambda x: x.score)
    else:
        parent = max(pop, key=lambda x: x.score)
    action = rng.randrange(og.N_ACTIONS)
    child_cfg = og.mutate_arch(parent.cfg, action, rng, None)
    child_cfg = maybe_inject_recurrent(child_cfg, rng, p=0.22)
    child_hp = og.mutate_hp(parent.hp, rng)
    r_raw, r, cell = evaluate(
        child_cfg, child_hp, device, baseline=baseline, fit_steps_default=fit_steps, batch_default=batch
    )
    child = og.Individual(cfg=child_cfg, hp=child_hp, score=r, age=0)
    # Kill oldest (aging / regularized evolution)
    oldest_i = max(range(len(pop)), key=lambda i: pop[i].age)
    pop[oldest_i] = child
    return child_cfg, child_hp, r_raw, r, cell


# --- REINFORCE --------------------------------------------------------------


def reinforce_update(
    policy: og.ActorCritic,
    opt: torch.optim.Optimizer,
    logprob: torch.Tensor,
    reward: float,
    baseline_ema: float,
) -> float:
    adv = reward - baseline_ema
    loss = -(logprob * adv)
    if not torch.isfinite(loss).item():
        return baseline_ema
    opt.zero_grad(set_to_none=True)
    loss.backward()
    nn.utils.clip_grad_norm_(policy.parameters(), 1.0)
    opt.step()
    return 0.9 * baseline_ema + 0.1 * reward


# --- Approach runners -------------------------------------------------------


def run_approach(
    name: str,
    *,
    iters: int,
    seed: int,
    device: torch.device,
    out_dir: Path,
    fit_steps: int,
    batch: int,
    pop_size: int,
    ckpt_every: int,
    resume: bool,
    all_approaches: list[str] | None = None,
) -> dict[str, Any]:
    approach_dir = out_dir / name
    approach_dir.mkdir(parents=True, exist_ok=True)
    hist_path = approach_dir / "history.jsonl"
    ckpt_path = approach_dir / "checkpoint.json"
    log_path = approach_dir / "run.log"
    planned = all_approaches or [name]

    def refresh_status(phase: str, it: int | None = None) -> None:
        write_status(
            out_dir,
            phase=phase,
            target_iters=iters,
            approaches=planned,
            current=name,
            current_iter=it,
            pid=os.getpid(),
        )

    tb = _tb_writer(out_dir)
    rng = random.Random(seed + sum(ord(c) for c in name))
    torch.manual_seed(seed + len(name))
    if device.type == "cuda":
        torch.cuda.manual_seed_all(seed)

    baseline = og.dual_cosine_baseline(device, batch=128)
    nobake_ref = og.nobake_baseline(device, batch=128)
    start_it = 1
    champ_r = -1.0
    champ_raw = -1.0
    champ_cfg: og.ArchConfig | None = None
    champ_hp: og.HyperParams | None = None
    champ_lstm = False
    champ_xlstm = False
    iters_since_improve = 0
    plateau_every = 500
    t0 = time.time()
    elapsed_prev = 0.0

    # Approach-specific state
    pop: list[og.Individual] = []
    cma: TinyCMA | None = None
    tpe: TinyTPE | None = None
    policy: og.ActorCritic | None = None
    policy_opt: torch.optim.Optimizer | None = None
    re_base = 0.0
    cur_cfg = og.random_arch(rng)
    cur_hp = og.random_hp(rng)
    cur_score = -1.0
    pending_cma: list[list[float]] = []
    pending_fit: list[float] = []

    ckpt = load_ckpt(ckpt_path) if resume else None
    done_prev = int(ckpt.get("iters_done", 0)) if ckpt else 0
    if ckpt and done_prev >= iters:
        summary_path = approach_dir / "summary.json"
        if summary_path.is_file():
            print(f"SKIP {name} already complete iters_done={done_prev}", flush=True)
            refresh_status("skip_complete", done_prev)
            return json.loads(summary_path.read_text(encoding="utf-8"))
    if ckpt and 0 < done_prev < iters:
        start_it = done_prev + 1
        champ_r = float(ckpt.get("champ_r", -1.0))
        champ_raw = float(ckpt.get("champ_raw", champ_r))
        champ_lstm = bool(ckpt.get("champ_lstm", False))
        champ_xlstm = bool(ckpt.get("champ_xlstm", False))
        elapsed_prev = float(ckpt.get("wall_s", 0.0))
        cur_score = float(ckpt.get("cur_score", -1.0))
        if ckpt.get("champ_cfg"):
            champ_cfg = og.ArchConfig.from_dict(ckpt["champ_cfg"])
            flags = arch_recurrent_flags(champ_cfg)
            champ_lstm = flags["lstm"]
            champ_xlstm = flags["xlstm"]
        if ckpt.get("champ_hp"):
            champ_hp = og.HyperParams.from_dict(ckpt["champ_hp"])
        if ckpt.get("cur_cfg"):
            cur_cfg = og.ArchConfig.from_dict(ckpt["cur_cfg"])
        if ckpt.get("cur_hp"):
            cur_hp = og.HyperParams.from_dict(ckpt["cur_hp"])
        re_base = float(ckpt.get("re_base", 0.0))
        if name == "aging_evo" and ckpt.get("pop"):
            pop = [
                og.Individual(
                    og.ArchConfig.from_dict(p["cfg"]),
                    og.HyperParams.from_dict(p["hp"]),
                    score=float(p["score"]),
                    age=int(p.get("age", 0)),
                )
                for p in ckpt["pop"]
            ]
        if name == "hybrid_lstm" and ckpt.get("pop"):
            pop = [
                og.Individual(
                    og.ArchConfig.from_dict(p["cfg"]),
                    og.HyperParams.from_dict(p["hp"]),
                    score=float(p["score"]),
                    age=int(p.get("age", 0)),
                )
                for p in ckpt["pop"]
            ]
        if name == "cmaes" and ckpt.get("cma"):
            cma = TinyCMA(CMA_DIM, rng)
            cma.load_state_dict(ckpt["cma"])
        if name == "tpe" and ckpt.get("tpe"):
            tpe = TinyTPE(rng)
            tpe.load_state_dict(ckpt["tpe"])
        if name in ("reinforce", "hybrid_lstm") and ckpt.get("policy_path"):
            policy = og.ActorCritic().to(device)
            policy.load_state_dict(
                torch.load(ckpt["policy_path"], map_location=device, weights_only=True)
            )
            policy_opt = torch.optim.Adam(policy.parameters(), lr=3e-4)
        msg = f"RESUME {name} from iter={start_it} champ={champ_r:.6f}"
        print(msg, flush=True)
        with log_path.open("a", encoding="utf-8") as f:
            f.write(msg + "\n")
        refresh_status("resume", start_it - 1)
    else:
        hist_path.write_text("", encoding="utf-8")
        refresh_status("start", 0)

    if name == "aging_evo" and not pop:
        pop = [
            og.Individual(og.random_arch(rng), og.random_hp(rng), score=-1.0, age=i)
            for i in range(pop_size)
        ]
    if name == "cmaes" and cma is None:
        cma = TinyCMA(CMA_DIM, rng)
    if name == "tpe" and tpe is None:
        tpe = TinyTPE(rng)
    if name in ("reinforce", "hybrid_lstm") and policy is None:
        policy = og.ActorCritic().to(device)
        policy_opt = torch.optim.Adam(policy.parameters(), lr=3e-4)

    def log(msg: str) -> None:
        line = f"{datetime.now().isoformat(timespec='seconds')} {msg}"
        print(line, flush=True)
        with log_path.open("a", encoding="utf-8") as f:
            f.write(line + "\n")

    log(
        f"START approach={name} iters={iters} seed={seed} device={device} "
        f"baseline_dual_cosine={baseline:.6f} nobake={nobake_ref:.6f} "
        f"blocks_has_lstm={'lstm' in BLOCKS} blocks_has_xlstm={'xlstm' in BLOCKS} "
        f"hp_reward_sweep=on plateau_every={plateau_every}"
    )

    # Hybrid: reuse overnight branch rotation with LSTM vocabulary already live
    hybrid_branches = ("ppo", "ga", "pbt", "nas", "combo")
    branch_best = {b: 0.0 for b in hybrid_branches}
    buf = og.RolloutBuffer() if name == "hybrid_lstm" else None
    last_good_policy = og.snapshot_state_dict(policy) if policy is not None else None
    plateau = og.PlateauAdaptState()
    if name == "hybrid_lstm" and not pop:
        pop = [
            og.Individual(og.random_arch(rng), og.random_hp(rng), score=-1.0, age=0)
            for _ in range(pop_size)
        ]

    for it in range(start_it, iters + 1):
        proposal = name
        if name == "random":
            trial_cfg = maybe_inject_recurrent(og.random_arch(rng), rng, p=0.2)
            trial_hp = og.random_hp(rng)
            r_raw, r, cell = evaluate(
                trial_cfg,
                trial_hp,
                device,
                baseline=baseline,
                fit_steps_default=fit_steps,
                batch_default=batch,
            )
        elif name == "cmaes":
            assert cma is not None
            if not pending_cma:
                pending_cma = cma.ask()
                pending_fit = []
            vec = pending_cma[len(pending_fit)]
            trial_cfg, trial_hp = decode_cma(vec, rng)
            r_raw, r, cell = evaluate(
                trial_cfg,
                trial_hp,
                device,
                baseline=baseline,
                fit_steps_default=fit_steps,
                batch_default=batch,
            )
            pending_fit.append(r)
            if len(pending_fit) >= len(pending_cma):
                cma.tell(pending_cma, pending_fit)
                pending_cma, pending_fit = [], []
            proposal = f"cmaes_gen{cma.generation}"
        elif name == "tpe":
            assert tpe is not None
            sample = tpe.ask()
            trial_cfg, trial_hp = tpe_to_arch(sample, rng)
            r_raw, r, cell = evaluate(
                trial_cfg,
                trial_hp,
                device,
                baseline=baseline,
                fit_steps_default=fit_steps,
                batch_default=batch,
            )
            tpe.tell(sample, r)
            proposal = "tpe"
        elif name == "aging_evo":
            trial_cfg, trial_hp, r_raw, r, cell = aging_step(
                pop, rng, device, baseline, fit_steps, batch
            )
            proposal = "aging_evo"
        elif name == "reinforce":
            assert policy is not None and policy_opt is not None
            state = og.arch_state_vec(cur_cfg, cur_hp, device).unsqueeze(0)
            logits, _ = policy(state)
            dist = og.categorical_from_logits(logits)
            action_t = dist.sample()
            action = int(action_t.item())
            logprob = dist.log_prob(action_t)
            trial_cfg = og.mutate_arch(cur_cfg, action, rng, None)
            trial_cfg = maybe_inject_recurrent(trial_cfg, rng, p=0.18)
            trial_hp = og.mutate_hp(cur_hp, rng)
            r_raw, r, cell = evaluate(
                trial_cfg,
                trial_hp,
                device,
                baseline=baseline,
                fit_steps_default=fit_steps,
                batch_default=batch,
            )
            reward = og.finite_scalar(
                og.shaped_reward(
                    r,
                    mode=getattr(trial_hp, "reward_mode", "vs_dualcosine"),
                    r_dualcosine=baseline,
                    r_nobake=nobake_ref,
                ),
                0.0,
            )
            re_base = reinforce_update(policy, policy_opt, logprob, reward, re_base)
            if r >= cur_score:
                cur_cfg, cur_hp, cur_score = trial_cfg, trial_hp, r
            proposal = f"reinforce_a{action}"
        elif name == "hybrid_lstm":
            assert policy is not None and policy_opt is not None and buf is not None
            branch = hybrid_branches[(it - 1) % len(hybrid_branches)]
            ind = pop[(it - 1) % len(pop)]
            cfg, hp = ind.cfg, ind.hp
            state = og.arch_state_vec(cfg, hp, device).unsqueeze(0)
            logits, value = policy(state)
            dist = og.categorical_from_logits(logits)
            action_t = dist.sample()
            action = int(action_t.item())
            logprob = dist.log_prob(action_t)
            if branch == "nas":
                trial_cfg = og.random_arch(rng, plateau)
                trial_hp = og.mutate_hp(hp, rng)
                proposal = "NAS_RANDOM"
            elif branch == "pbt":
                # Exploit/mutate HPs + reward_mode (near-ceiling sweep lives here)
                og.pbt_exploit_mutate(pop, rng)
                ind = pop[(it - 1) % len(pop)]
                cfg, hp = ind.cfg, ind.hp
                trial_cfg = og.mutate_arch(cfg, action, rng, plateau)
                trial_hp = og.mutate_hp(hp, rng)
                proposal = "PBT_MUTATE_HP"
            elif branch == "ga":
                parent = max(pop, key=lambda x: x.score)
                from denoise_meta_evo import crossover_arch, crossover_hp

                if rng.random() < 0.6 and parent.score > -0.5:
                    trial_cfg = crossover_arch(
                        cfg,
                        parent.cfg,
                        rng,
                        ArchConfig=og.ArchConfig,
                        normalize_graph=og.normalize_graph,
                        ensure_trainable_ops=og.ensure_trainable_ops,
                        CELL_KINDS=CELL_KINDS,
                        ACTS=og.ACTS,
                    )
                    trial_cfg = og.mutate_arch(trial_cfg, action, rng, plateau)
                    trial_hp = crossover_hp(hp, parent.hp, rng, HyperParams=og.HyperParams)
                    proposal = "GA_CROSSOVER"
                else:
                    trial_cfg = og.mutate_arch(cfg, action, rng, plateau)
                    trial_hp = og.mutate_hp(hp, rng)
                    proposal = "GA_MUTATE"
            elif branch == "combo":
                trial_cfg = og.mutate_arch(
                    og.mutate_arch(cfg, action, rng, plateau),
                    rng.randrange(og.N_ACTIONS),
                    rng,
                    plateau,
                )
                trial_hp = og.mutate_hp(hp, rng)
                proposal = "COMBO"
            else:
                trial_cfg = og.mutate_arch(cfg, action, rng, plateau)
                trial_hp = og.mutate_hp(hp, rng)
                proposal = "PPO_MUTATION"
            trial_cfg = maybe_inject_recurrent(trial_cfg, rng, p=0.18)
            r_raw, r, cell = evaluate(
                trial_cfg,
                trial_hp,
                device,
                baseline=baseline,
                fit_steps_default=fit_steps,
                batch_default=batch,
            )
            branch_best[branch] = max(branch_best[branch], r_raw)
            reward = og.finite_scalar(
                og.shaped_reward(
                    r,
                    mode=getattr(trial_hp, "reward_mode", "vs_dualcosine"),
                    r_dualcosine=baseline,
                    r_nobake=nobake_ref,
                ),
                0.0,
            )
            buf.states.append(state.squeeze(0).detach())
            buf.actions.append(action)
            buf.logprobs.append(logprob.detach())
            buf.rewards.append(reward)
            buf.values.append(value.squeeze().detach())
            buf.dones.append(False)
            if len(buf) >= 16:
                og.ppo_update(
                    policy,
                    policy_opt,
                    buf,
                    device,
                    clip_eps=trial_hp.ppo_clip,
                    entropy_coef=trial_hp.entropy_coef,
                    last_good=last_good_policy,
                )
                buf.clear()
                if og.params_finite(policy):
                    last_good_policy = og.snapshot_state_dict(policy)
            if r >= ind.score:
                ind.cfg, ind.hp, ind.score = trial_cfg, trial_hp, r
            # Plateau adapt: deepen + crazier mixes when champ stalls
            if iters_since_improve >= plateau_every and plateau.level < 8:
                ev = og.apply_plateau_adapt(plateau, pop, rng, it=it, max_level=8)
                iters_since_improve = 0
                log(f"PLATEAU_ADAPT approach={name} iter={it} level={ev.get('level')}")
        else:
            raise ValueError(name)

        if r > champ_r:
            champ_r = r
            champ_raw = r_raw
            champ_cfg = trial_cfg
            champ_hp = trial_hp
            flags = arch_recurrent_flags(trial_cfg)
            champ_lstm = flags["lstm"]
            champ_xlstm = flags["xlstm"]
            iters_since_improve = 0
            log(
                f"CHAMP approach={name} iter={it} R={champ_r:.6f} raw={champ_raw:.6f} "
                f"lstm={champ_lstm} xlstm={champ_xlstm} "
                f"reward_mode={getattr(trial_hp, 'reward_mode', None)}"
            )
        else:
            iters_since_improve += 1

        trial_flags = arch_recurrent_flags(trial_cfg)
        row = {
            "iter": it,
            "approach": name,
            "proposal": proposal,
            "residual": r_raw,
            "residual_scored": r,
            "champ": champ_r,
            "champ_raw": champ_raw,
            "lstm_in_trial": trial_flags["lstm"],
            "xlstm_in_trial": trial_flags["xlstm"],
            "lstm_in_champ": champ_lstm,
            "xlstm_in_champ": champ_xlstm,
            "baseline_dual_cosine": baseline,
            "baseline_nobake": nobake_ref,
            "reward_mode": getattr(trial_hp, "reward_mode", None),
            "wall_s": elapsed_prev + (time.time() - t0),
            "arch": trial_cfg.to_dict(),
            "hp": trial_hp.to_dict(),
        }
        append_hist(hist_path, row)
        tb_log(
            tb,
            name,
            it,
            champ=champ_raw if champ_raw >= 0 else champ_r,
            residual=r_raw,
            wall_s=elapsed_prev + (time.time() - t0),
        )
        # Live STATUS every iter so dashboards see progress between checkpoints.
        refresh_status("running", it)

        if it % ckpt_every == 0 or it == iters:
            payload: dict[str, Any] = {
                "approach": name,
                "iters_done": it,
                "champ_r": champ_r,
                "champ_raw": champ_raw,
                "champ_lstm": champ_lstm,
                "champ_xlstm": champ_xlstm,
                "champ_cfg": champ_cfg.to_dict() if champ_cfg else None,
                "champ_hp": champ_hp.to_dict() if champ_hp else None,
                "cur_cfg": cur_cfg.to_dict(),
                "cur_hp": cur_hp.to_dict(),
                "cur_score": cur_score,
                "re_base": re_base,
                "wall_s": elapsed_prev + (time.time() - t0),
                "baseline_dual_cosine": baseline,
                "branch_best": branch_best if name == "hybrid_lstm" else None,
            }
            if name == "aging_evo":
                payload["pop"] = [
                    {"cfg": p.cfg.to_dict(), "hp": p.hp.to_dict(), "score": p.score, "age": p.age}
                    for p in pop
                ]
            if name == "hybrid_lstm":
                payload["pop"] = [
                    {"cfg": p.cfg.to_dict(), "hp": p.hp.to_dict(), "score": p.score, "age": p.age}
                    for p in pop
                ]
            if cma is not None:
                payload["cma"] = cma.state_dict()
            if tpe is not None:
                payload["tpe"] = tpe.state_dict()
            if policy is not None:
                pol_path = approach_dir / "policy.pt"
                torch.save(policy.state_dict(), pol_path)
                payload["policy_path"] = str(pol_path)
            save_ckpt(ckpt_path, payload)
            refresh_status("running", it)
            if it % max(50, ckpt_every) == 0:
                log(f"CKPT approach={name} iter={it}/{iters} champ={champ_r:.6f}")

    wall_s = elapsed_prev + (time.time() - t0)
    summary = {
        "approach": name,
        "iters": iters,
        "iters_done": iters,
        "seed": seed,
        "champ_r": champ_r,
        "champ_raw": champ_raw,
        "delta_r_vs_dual_cosine": champ_raw - baseline if champ_raw >= 0 else None,
        "baseline_dual_cosine": baseline,
        "wall_s": wall_s,
        "wall_h": wall_s / 3600.0,
        "lstm_in_champ": champ_lstm,
        "xlstm_in_champ": champ_xlstm,
        "champ_arch": champ_cfg.to_dict() if champ_cfg else None,
        "champ_hp": champ_hp.to_dict() if champ_hp else None,
        "history_path": str(hist_path),
        "finished_at": utc_now(),
    }
    (approach_dir / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    log(
        f"DONE approach={name} champ_r={champ_r:.6f} wall_h={wall_s/3600:.3f} "
        f"lstm={champ_lstm} xlstm={champ_xlstm}"
    )
    refresh_status("approach_done", iters)
    if tb is not None:
        try:
            tb.close()
        except Exception:
            pass
    return summary


def write_meta_table_tex(aggregate: dict[str, Any], out_tex: Path) -> None:
    labels = {
        "random": "Random NAS",
        "cmaes": "Cont.\\ CMA-ES",
        "reinforce": "Arch REINFORCE",
        "aging_evo": "Aging evolution",
        "tpe": "TPE Bayes NAS",
        "hybrid_lstm": "Ours (hybrid GA--PPO)",
    }
    rows: list[str] = []
    for row in aggregate.get("table", []):
        m = row["method"]
        name = labels.get(m, m)
        r = float(row["champ_r"])
        d = float(row["delta_r_vs_dual_cosine"])
        wh = float(row["wall_h"])
        lstm = "yes" if row.get("lstm_in_champ") else "no"
        xlstm = "yes" if row.get("xlstm_in_champ") else "no"
        rows.append(f"    {name} & {r:.5f} & ${d:+.5f}$ & {wh:.2f} & {lstm} & {xlstm} \\\\")
    body = "\n".join(rows) if rows else "    \\textit{(pending)} & -- & -- & -- & -- & -- \\\\"
    tex = (
        "\\begin{table}[t]\n"
        "  \\centering\n"
        "  \\caption{Matched $5$k-budget outer-loop comparison (search seed "
        "\\texttt{1902771841}; LSTM+xLSTM in bake vocab; reward-mode + HP co-tune in Ours). "
        "Champion $R$ vs ideal sibling; $\\Delta R$ vs DualCosine is one classical gap only. "
        "Wall-h: per-method CUDA wall time. LSTM?/xLSTM?: whether the champion graph "
        "included that block.}\n"
        "  \\label{tab:meta-approaches}\n"
        "  \\setlength{\\tabcolsep}{3pt}\n"
        "  \\small\n"
        "  \\begin{tabular}{@{}lrrrcc@{}}\n"
        "    \\toprule\n"
        "    Method & Champ $R$@5k & $\\Delta R$ vs DualCosine & Wall-h & LSTM? & xLSTM? \\\\\n"
        "    \\midrule\n"
        f"{body}\n"
        "    \\bottomrule\n"
        "  \\end{tabular}\n"
        "\\end{table}\n"
    )
    out_tex.write_text(tex, encoding="utf-8")


DISPLAY_NAMES = {
    "random": "Random NAS",
    "cmaes": "Cont. CMA-ES",
    "reinforce": "Arch REINFORCE",
    "aging_evo": "Aging evolution",
    "tpe": "TPE Bayes NAS",
    "hybrid_lstm": "Ours (hybrid GA–PPO)",
}

def plot_compare(aggregate: dict[str, Any], out_png: Path) -> None:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    styles = {
        "random": {"color": "#000000", "marker": "o", "linestyle": "-"},
        "cmaes": {"color": "#0072B2", "marker": "s", "linestyle": "-"},
        "reinforce": {"color": "#009E73", "marker": "^", "linestyle": "-"},
        "aging_evo": {"color": "#E69F00", "marker": "D", "linestyle": "-"},
        "tpe": {"color": "#CC79A7", "marker": "v", "linestyle": "-"},
        "hybrid_lstm": {"color": "#D55E00", "marker": "P", "linestyle": "-"},
    }
    fig, ax = plt.subplots(figsize=(5.6, 3.5))
    max_x = 5000
    for name, info in aggregate.get("approaches", {}).items():
        hist = Path(info.get("history_path", ""))
        if not hist.is_file():
            continue
        xs, ys = [], []
        best = -1.0
        with hist.open(encoding="utf-8") as f:
            for line in f:
                if not line.strip():
                    continue
                row = json.loads(line)
                it = int(row["iter"])
                if it > max_x:
                    break
                c = float(row.get("champ", row.get("residual", 0.0)))
                if c > best:
                    best = c
                xs.append(it)
                ys.append(best)
        st = styles.get(name, {"color": "#666666", "marker": "x", "linestyle": "-"})
        markevery = max(1, len(xs) // 12) if xs else 1
        ax.plot(
            xs,
            ys,
            label=DISPLAY_NAMES.get(name, name.replace("_", " ")),
            color=st["color"],
            marker=st["marker"],
            linestyle=st["linestyle"],
            linewidth=1.6,
            markersize=4.5,
            markevery=markevery,
        )
    base = aggregate.get("baseline_dual_cosine")
    if base is not None:
        ax.axhline(float(base), color="#999999", linestyle="--", linewidth=1.3, label="DualCosine")
    ax.set_xlim(0, max_x)
    ax.set_xlabel("Outer iteration")
    ax.set_ylabel("Champion residual $R$")
    ax.set_title("Meta-learning approach comparison (5k budget)")
    ax.legend(loc="lower right", fontsize=7, frameon=False)
    ax.grid(True, alpha=0.25)
    fig.tight_layout()
    out_png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out_png, dpi=220)
    plt.close(fig)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--iters", type=int, default=5000)
    ap.add_argument("--seed", type=int, default=DEFAULT_SEED)
    ap.add_argument("--batch", type=int, default=48)
    ap.add_argument("--fit-steps", type=int, default=24)
    ap.add_argument("--pop-size", type=int, default=12)
    ap.add_argument("--ckpt-every", type=int, default=25)
    ap.add_argument("--device", type=str, default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument(
        "--approaches",
        type=str,
        default=",".join(APPROACHES),
        help="Comma-separated subset of: " + ",".join(APPROACHES),
    )
    ap.add_argument(
        "--no-resume",
        action="store_true",
        help="Deprecated; ignored unless --force-fresh is also set.",
    )
    ap.add_argument(
        "--force-fresh",
        action="store_true",
        help="Wipe checkpoints/history and start from scratch.",
    )
    ap.add_argument(
        "--out-dir",
        type=Path,
        default=ROOT / "brand" / "artifacts" / "meta_approach_compare",
    )
    args = ap.parse_args()

    device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")
    out_dir = args.out_dir
    out_dir.mkdir(parents=True, exist_ok=True)
    names = [a.strip() for a in args.approaches.split(",") if a.strip()]
    for n in names:
        if n not in APPROACHES:
            raise SystemExit(f"Unknown approach {n!r}; choose from {APPROACHES}")

    # Protect long runs: --no-resume alone must not wipe progress.
    allow_fresh = bool(args.force_fresh)
    if args.no_resume and not allow_fresh:
        print(
            "WARN: --no-resume ignored without --force-fresh; resuming from checkpoints",
            flush=True,
        )
    resume = not allow_fresh

    print(f"lstm in BLOCKS: {'lstm' in BLOCKS}", flush=True)
    print(f"xlstm in BLOCKS: {'xlstm' in BLOCKS}", flush=True)
    print(f"device={device} iters={args.iters} approaches={names} resume={resume}", flush=True)
    write_status(
        out_dir,
        phase="launch",
        target_iters=args.iters,
        approaches=names,
        current=None,
        current_iter=0,
        pid=os.getpid(),
        extra={"seed": args.seed, "device": str(device)},
    )
    # Durable PID file for crash survival / poll
    (out_dir / "bench.pid").write_text(str(os.getpid()), encoding="utf-8")

    summaries: list[dict[str, Any]] = []
    for name in names:
        summaries.append(
            run_approach(
                name,
                iters=args.iters,
                seed=args.seed,
                device=device,
                out_dir=out_dir,
                fit_steps=args.fit_steps,
                batch=args.batch,
                pop_size=args.pop_size,
                ckpt_every=args.ckpt_every,
                resume=resume,
                all_approaches=names,
            )
        )

    baseline = summaries[0]["baseline_dual_cosine"] if summaries else None
    aggregate = {
        "schema": "denoiseopt.meta_approach_compare.v1",
        "publishable": True,
        "seed": args.seed,
        "iters": args.iters,
        "batch": args.batch,
        "fit_steps": args.fit_steps,
        "pop_size": args.pop_size,
        "device": str(device),
        "baseline_dual_cosine": baseline,
        "lstm_in_search_vocab": True,
        "xlstm_in_search_vocab": True,
        "reward_modes": list(getattr(og, "REWARD_MODES", ())),
        "blocks": list(BLOCKS),
        "approaches": {s["approach"]: s for s in summaries},
        "table": [
            {
                "method": s["approach"],
                "champ_r": s["champ_raw"],
                "delta_r_vs_dual_cosine": s["delta_r_vs_dual_cosine"],
                "wall_h": s["wall_h"],
                "lstm_in_champ": s["lstm_in_champ"],
                "xlstm_in_champ": s.get("xlstm_in_champ", False),
            }
            for s in summaries
        ],
        "created_at": utc_now(),
    }
    agg_path = out_dir / "meta_approach_compare.json"
    agg_path.write_text(json.dumps(aggregate, indent=2), encoding="utf-8")

    paper_fig = META_ROOT / "paper" / "v7" / "figures"
    paper_fig.mkdir(parents=True, exist_ok=True)
    shutil.copy2(agg_path, paper_fig / "meta_approach_compare.json")

    png = paper_fig / "fig_meta_approach_compare.png"
    plot_compare(aggregate, png)
    shutil.copy2(png, out_dir / "fig_meta_approach_compare.png")
    write_meta_table_tex(aggregate, paper_fig / "meta_approaches_table.tex")
    write_status(
        out_dir,
        phase="all_complete",
        target_iters=args.iters,
        approaches=names,
        current=None,
        current_iter=args.iters,
        pid=os.getpid(),
        extra={"aggregate": str(agg_path), "figure": str(png)},
    )
    print(json.dumps(aggregate["table"], indent=2), flush=True)
    print(f"Wrote {agg_path}", flush=True)
    print(f"Wrote {png}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
