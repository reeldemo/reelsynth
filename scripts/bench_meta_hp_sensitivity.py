#!/usr/bin/env python3
"""One-at-a-time ±50% HP sensitivity probe for DenoiseOpt hybrid meta-search.

LOCKED protocol (paper v8 / SDD W4):
  - Budget: 500 outer iters per config (NOT a full 5k re-search)
  - Design: OAT ±50% on key Table-2 HPs
  - Seed: 1902771841 (overnight_gpu_rl_arch.DEFAULT_SEED)
  - Cell protocol: sine+cliff FitCell, batch 48, fit_steps 24
  - Out: brand/artifacts/meta_hp_sensitivity/ only
  - NEVER write under brand/artifacts/meta_approach_compare/

Honesty label: sensitivity probe of local robustness under seed 1902771841,
not a second ranking campaign.

Usage (GPU venv):
  .venv_gpu/Scripts/python.exe scripts/bench_meta_hp_sensitivity.py
  .venv_gpu/Scripts/python.exe scripts/bench_meta_hp_sensitivity.py --aggregate-only
  .venv_gpu/Scripts/python.exe scripts/bench_meta_hp_sensitivity.py --configs default,pop_size_m50
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
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import torch
import torch.nn as nn

sys.path.insert(0, str(Path(__file__).resolve().parent))
import overnight_gpu_rl_arch as og  # noqa: E402
from denoise_arch_blocks import CELL_KINDS  # noqa: E402
from denoise_meta_evo import crossover_arch, crossover_hp  # noqa: E402

import bench_meta_approaches_5k as meta  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
META_ROOT = ROOT.parent / "denoise-opt-meta"
DEFAULT_OUT = ROOT / "brand" / "artifacts" / "meta_hp_sensitivity"
FORBIDDEN_OUT_MARKER = "meta_approach_compare"
DEFAULT_SEED = og.DEFAULT_SEED
DEFAULT_ITERS = 500
FIT_STEPS = 24
BATCH = 48

# Table-2 / tab:hyperparams defaults
DEFAULT_POP = 12
DEFAULT_PPO_CLIP = 0.2
DEFAULT_LR = 3e-3
DEFAULT_ENTROPY = 0.02
DEFAULT_GA_MUT2 = 0.5  # second-mutate probability after inherit (≥1 mutate always)


@dataclass(frozen=True)
class SensConfig:
    """One OAT (or baseline) sensitivity configuration."""

    id: str
    label: str
    pop_size: int = DEFAULT_POP
    ppo_clip: float = DEFAULT_PPO_CLIP
    lr: float = DEFAULT_LR
    entropy_coef: float = DEFAULT_ENTROPY
    ga_second_mutate_p: float = DEFAULT_GA_MUT2
    # Which HyperParams field is frozen to the Table-2 (perturbed) value after mutate.
    freeze_hp: str | None = None


def oat_grid() -> list[SensConfig]:
    """Baseline + OAT ±50% on locked knobs (design.md W4)."""
    return [
        SensConfig("default", "Table-2 default"),
        SensConfig("pop_size_m50", r"$n{-}50\%$", pop_size=max(2, int(round(DEFAULT_POP * 0.5)))),
        SensConfig("pop_size_p50", r"$n{+}50\%$", pop_size=int(round(DEFAULT_POP * 1.5))),
        SensConfig(
            "ppo_clip_m50",
            r"$\epsilon{-}50\%$",
            ppo_clip=DEFAULT_PPO_CLIP * 0.5,
            freeze_hp="ppo_clip",
        ),
        SensConfig(
            "ppo_clip_p50",
            r"$\epsilon{+}50\%$",
            ppo_clip=DEFAULT_PPO_CLIP * 1.5,
            freeze_hp="ppo_clip",
        ),
        SensConfig(
            "lr_m50",
            r"fit lr${-}50\%$",
            lr=DEFAULT_LR * 0.5,
            freeze_hp="lr",
        ),
        SensConfig(
            "lr_p50",
            r"fit lr${+}50\%$",
            lr=DEFAULT_LR * 1.5,
            freeze_hp="lr",
        ),
        SensConfig(
            "ga_mut2_m50",
            r"GA mut2${-}50\%$",
            ga_second_mutate_p=DEFAULT_GA_MUT2 * 0.5,
        ),
        SensConfig(
            "ga_mut2_p50",
            r"GA mut2${+}50\%$",
            ga_second_mutate_p=min(1.0, DEFAULT_GA_MUT2 * 1.5),
        ),
        SensConfig(
            "entropy_m50",
            r"entropy${-}50\%$",
            entropy_coef=DEFAULT_ENTROPY * 0.5,
            freeze_hp="entropy_coef",
        ),
        SensConfig(
            "entropy_p50",
            r"entropy${+}50\%$",
            entropy_coef=DEFAULT_ENTROPY * 1.5,
            freeze_hp="entropy_coef",
        ),
    ]


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def refuse_forbidden_out(out_dir: Path) -> None:
    resolved = out_dir.resolve()
    parts = {p.lower() for p in resolved.parts}
    if FORBIDDEN_OUT_MARKER in parts or FORBIDDEN_OUT_MARKER in str(resolved).replace("\\", "/"):
        raise SystemExit(
            f"REFUSED: sensitivity artifacts must NOT write under {FORBIDDEN_OUT_MARKER}/. "
            f"Got out_dir={out_dir}"
        )


def base_hp(cfg: SensConfig) -> og.HyperParams:
    return og.HyperParams(
        lr=float(cfg.lr),
        fit_steps=FIT_STEPS,
        batch=BATCH,
        entropy_coef=float(cfg.entropy_coef),
        ppo_clip=float(cfg.ppo_clip),
        adv_coef=0.05,
        reward_mode="vs_dualcosine",
    )


def apply_freeze(hp: og.HyperParams, cfg: SensConfig) -> og.HyperParams:
    """Re-assert the OAT-frozen Table-2 knob after mutate_hp / crossover."""
    if not cfg.freeze_hp:
        return hp
    h = og.HyperParams(**hp.to_dict())
    if cfg.freeze_hp == "lr":
        h.lr = float(cfg.lr)
    elif cfg.freeze_hp == "ppo_clip":
        h.ppo_clip = float(cfg.ppo_clip)
    elif cfg.freeze_hp == "entropy_coef":
        h.entropy_coef = float(cfg.entropy_coef)
    return h


def mutate_hp_frozen(hp: og.HyperParams, rng: random.Random, cfg: SensConfig) -> og.HyperParams:
    return apply_freeze(og.mutate_hp(hp, rng), cfg)


def save_json(path: Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    tmp = path.with_suffix(".tmp")
    tmp.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    tmp.replace(path)


def load_json(path: Path) -> dict[str, Any] | None:
    if not path.is_file():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except Exception:
        return None


def write_status(out_dir: Path, **extra: Any) -> None:
    payload = {
        "updated_at": utc_now(),
        "pid": os.getpid(),
        **extra,
    }
    save_json(out_dir / "STATUS.json", payload)
    # Sibling live pointer (does not touch meta_approach_compare)
    live = ROOT / "brand" / "artifacts" / "meta_hp_sensitivity_STATUS.json"
    try:
        save_json(live, payload)
    except Exception:
        pass


def run_config(
    cfg: SensConfig,
    *,
    iters: int,
    seed: int,
    device: torch.device,
    out_dir: Path,
    ckpt_every: int,
    resume: bool,
) -> dict[str, Any]:
    cfg_dir = out_dir / cfg.id
    cfg_dir.mkdir(parents=True, exist_ok=True)
    hist_path = cfg_dir / "history.jsonl"
    ckpt_path = cfg_dir / "checkpoint.json"
    log_path = cfg_dir / "run.log"
    summary_path = cfg_dir / "summary.json"

    def log(msg: str) -> None:
        line = f"{datetime.now().isoformat(timespec='seconds')} {msg}"
        print(line, flush=True)
        with log_path.open("a", encoding="utf-8") as f:
            f.write(line + "\n")

    ckpt = load_json(ckpt_path) if resume else None
    done_prev = int(ckpt.get("iters_done", 0)) if ckpt else 0
    if ckpt and done_prev >= iters and summary_path.is_file():
        log(f"SKIP {cfg.id} already complete iters_done={done_prev}")
        write_status(out_dir, phase="skip_complete", current=cfg.id, current_iter=done_prev)
        return json.loads(summary_path.read_text(encoding="utf-8"))

    rng = random.Random(seed + sum(ord(c) for c in cfg.id))
    torch.manual_seed(seed + len(cfg.id))
    if device.type == "cuda":
        torch.cuda.manual_seed_all(seed)

    baseline = og.dual_cosine_baseline(device, batch=128)
    nobake_ref = og.nobake_baseline(device, batch=128)

    start_it = 1
    champ_r = -1.0
    champ_raw = -1.0
    champ_cfg: og.ArchConfig | None = None
    champ_hp: og.HyperParams | None = None
    iters_since_improve = 0
    plateau_every = 500
    elapsed_prev = 0.0
    t0 = time.time()

    pop: list[og.Individual] = []
    policy: og.ActorCritic | None = None
    policy_opt: torch.optim.Optimizer | None = None
    buf = og.RolloutBuffer()
    last_good_policy = None
    plateau = og.PlateauAdaptState()
    hybrid_branches = ("ppo", "ga", "pbt", "nas", "combo")
    branch_best = {b: 0.0 for b in hybrid_branches}

    if ckpt and 0 < done_prev < iters:
        start_it = done_prev + 1
        champ_r = float(ckpt.get("champ_r", -1.0))
        champ_raw = float(ckpt.get("champ_raw", champ_r))
        elapsed_prev = float(ckpt.get("wall_s", 0.0))
        iters_since_improve = int(ckpt.get("iters_since_improve", 0))
        if ckpt.get("champ_cfg"):
            champ_cfg = og.ArchConfig.from_dict(ckpt["champ_cfg"])
        if ckpt.get("champ_hp"):
            champ_hp = og.HyperParams.from_dict(ckpt["champ_hp"])
        if ckpt.get("pop"):
            pop = [
                og.Individual(
                    og.ArchConfig.from_dict(p["cfg"]),
                    apply_freeze(og.HyperParams.from_dict(p["hp"]), cfg),
                    score=float(p["score"]),
                    age=int(p.get("age", 0)),
                )
                for p in ckpt["pop"]
            ]
        if ckpt.get("policy_path"):
            policy = og.ActorCritic().to(device)
            policy.load_state_dict(
                torch.load(ckpt["policy_path"], map_location=device, weights_only=True)
            )
            policy_opt = torch.optim.Adam(policy.parameters(), lr=3e-4)
            last_good_policy = og.snapshot_state_dict(policy)
        if ckpt.get("branch_best"):
            branch_best.update({k: float(v) for k, v in ckpt["branch_best"].items()})
        log(f"RESUME {cfg.id} from iter={start_it} champ={champ_r:.6f}")
    else:
        hist_path.write_text("", encoding="utf-8")
        log_path.write_text("", encoding="utf-8")

    if policy is None:
        policy = og.ActorCritic().to(device)
        policy_opt = torch.optim.Adam(policy.parameters(), lr=3e-4)
        last_good_policy = og.snapshot_state_dict(policy)

    if not pop:
        # Seed population at Table-2 (perturbed) HPs — OAT sensitivity of defaults.
        seed_hp = base_hp(cfg)
        pop = [
            og.Individual(og.random_arch(rng), og.HyperParams(**seed_hp.to_dict()), score=-1.0)
            for _ in range(cfg.pop_size)
        ]

    assert policy is not None and policy_opt is not None
    log(
        f"START config={cfg.id} iters={iters} seed={seed} device={device} "
        f"pop={cfg.pop_size} lr={cfg.lr} ppo_clip={cfg.ppo_clip} "
        f"entropy={cfg.entropy_coef} ga_mut2={cfg.ga_second_mutate_p} "
        f"freeze={cfg.freeze_hp} baseline_dual_cosine={baseline:.6f} "
        f"honesty=sensitivity_probe_not_5k_research"
    )
    write_status(
        out_dir,
        phase="running",
        current=cfg.id,
        current_iter=start_it - 1,
        target_iters=iters,
        config_label=cfg.label,
    )

    for it in range(start_it, iters + 1):
        branch = hybrid_branches[(it - 1) % len(hybrid_branches)]
        ind = pop[(it - 1) % len(pop)]
        arch, hp = ind.cfg, apply_freeze(ind.hp, cfg)
        state = og.arch_state_vec(arch, hp, device).unsqueeze(0)
        logits, value = policy(state)
        dist = og.categorical_from_logits(logits)
        action_t = dist.sample()
        action = int(action_t.item())
        logprob = dist.log_prob(action_t)

        if branch == "nas":
            trial_cfg = og.random_arch(rng, plateau)
            trial_hp = mutate_hp_frozen(hp, rng, cfg)
            proposal = "NAS_RANDOM"
        elif branch == "pbt":
            og.pbt_exploit_mutate(pop, rng, adapt=plateau)
            for p in pop:
                p.hp = apply_freeze(p.hp, cfg)
            ind = pop[(it - 1) % len(pop)]
            arch, hp = ind.cfg, apply_freeze(ind.hp, cfg)
            trial_cfg = og.mutate_arch(arch, action, rng, plateau)
            trial_hp = mutate_hp_frozen(hp, rng, cfg)
            proposal = "PBT_MUTATE_HP"
        elif branch == "ga":
            parent = max(pop, key=lambda x: x.score)
            if rng.random() < 0.6 and parent.score > -0.5:
                trial_cfg = crossover_arch(
                    arch,
                    parent.cfg,
                    rng,
                    ArchConfig=og.ArchConfig,
                    normalize_graph=og.normalize_graph,
                    ensure_trainable_ops=og.ensure_trainable_ops,
                    CELL_KINDS=CELL_KINDS,
                    ACTS=og.ACTS,
                )
                # ≥1 mutate after inherit; second mutate at Table-2 rate (OAT ±50%).
                trial_cfg = og.mutate_arch(trial_cfg, action, rng, plateau)
                if rng.random() < cfg.ga_second_mutate_p:
                    trial_cfg = og.mutate_arch(
                        trial_cfg, rng.randrange(og.N_ACTIONS), rng, plateau
                    )
                trial_hp = apply_freeze(
                    crossover_hp(hp, parent.hp, rng, HyperParams=og.HyperParams), cfg
                )
                proposal = "GA_CROSSOVER"
            else:
                trial_cfg = og.mutate_arch(arch, action, rng, plateau)
                if rng.random() < cfg.ga_second_mutate_p:
                    trial_cfg = og.mutate_arch(
                        trial_cfg, rng.randrange(og.N_ACTIONS), rng, plateau
                    )
                trial_hp = mutate_hp_frozen(hp, rng, cfg)
                proposal = "GA_MUTATE"
        elif branch == "combo":
            trial_cfg = og.mutate_arch(
                og.mutate_arch(arch, action, rng, plateau),
                rng.randrange(og.N_ACTIONS),
                rng,
                plateau,
            )
            trial_hp = mutate_hp_frozen(hp, rng, cfg)
            proposal = "COMBO"
        else:
            trial_cfg = og.mutate_arch(arch, action, rng, plateau)
            trial_hp = mutate_hp_frozen(hp, rng, cfg)
            proposal = "PPO_MUTATION"

        trial_cfg = meta.maybe_inject_recurrent(trial_cfg, rng, p=0.18)
        trial_hp = apply_freeze(trial_hp, cfg)
        # Enforce frozen PPO knobs used at update time as well.
        if cfg.freeze_hp == "ppo_clip":
            trial_hp.ppo_clip = float(cfg.ppo_clip)
        if cfg.freeze_hp == "entropy_coef":
            trial_hp.entropy_coef = float(cfg.entropy_coef)

        r_raw, r, _cell = meta.evaluate(
            trial_cfg,
            trial_hp,
            device,
            baseline=baseline,
            fit_steps_default=FIT_STEPS,
            batch_default=BATCH,
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
                clip_eps=float(cfg.ppo_clip if cfg.freeze_hp == "ppo_clip" else trial_hp.ppo_clip),
                entropy_coef=float(
                    cfg.entropy_coef
                    if cfg.freeze_hp == "entropy_coef"
                    else trial_hp.entropy_coef
                ),
                last_good=last_good_policy,
            )
            buf.clear()
            if og.params_finite(policy):
                last_good_policy = og.snapshot_state_dict(policy)

        if r >= ind.score:
            ind.cfg, ind.hp, ind.score = trial_cfg, trial_hp, r

        if r > champ_r:
            champ_r = r
            champ_raw = r_raw
            champ_cfg = trial_cfg
            champ_hp = trial_hp
            iters_since_improve = 0
            log(
                f"CHAMP config={cfg.id} iter={it} R={champ_r:.6f} raw={champ_raw:.6f} "
                f"proposal={proposal}"
            )
        else:
            iters_since_improve += 1

        if iters_since_improve >= plateau_every and plateau.level < 8:
            og.apply_plateau_adapt(plateau, pop, rng, it=it, max_level=8)
            for p in pop:
                p.hp = apply_freeze(p.hp, cfg)
            iters_since_improve = 0
            log(f"PLATEAU_ADAPT config={cfg.id} iter={it} level={plateau.level}")

        wall_s = elapsed_prev + (time.time() - t0)
        row = {
            "iter": it,
            "config": cfg.id,
            "proposal": proposal,
            "branch": branch,
            "residual": r_raw,
            "residual_scored": r,
            "champ": champ_r,
            "champ_raw": champ_raw,
            "baseline_dual_cosine": baseline,
            "wall_s": wall_s,
            "hp": trial_hp.to_dict(),
        }
        meta.append_hist(hist_path, row)
        write_status(
            out_dir,
            phase="running",
            current=cfg.id,
            current_iter=it,
            target_iters=iters,
            champ_r=champ_r,
            champ_raw=champ_raw,
        )

        if it % ckpt_every == 0 or it == iters:
            pol_path = cfg_dir / "policy.pt"
            torch.save(policy.state_dict(), pol_path)
            payload = {
                "config": cfg.id,
                "iters_done": it,
                "champ_r": champ_r,
                "champ_raw": champ_raw,
                "champ_cfg": champ_cfg.to_dict() if champ_cfg else None,
                "champ_hp": champ_hp.to_dict() if champ_hp else None,
                "wall_s": wall_s,
                "baseline_dual_cosine": baseline,
                "branch_best": branch_best,
                "iters_since_improve": iters_since_improve,
                "policy_path": str(pol_path),
                "pop": [
                    {
                        "cfg": p.cfg.to_dict(),
                        "hp": apply_freeze(p.hp, cfg).to_dict(),
                        "score": p.score,
                        "age": p.age,
                    }
                    for p in pop
                ],
                "knobs": {
                    "pop_size": cfg.pop_size,
                    "ppo_clip": cfg.ppo_clip,
                    "lr": cfg.lr,
                    "entropy_coef": cfg.entropy_coef,
                    "ga_second_mutate_p": cfg.ga_second_mutate_p,
                    "freeze_hp": cfg.freeze_hp,
                },
            }
            save_json(ckpt_path, payload)
            if it % max(50, ckpt_every) == 0:
                log(f"CKPT config={cfg.id} iter={it}/{iters} champ={champ_r:.6f}")

    wall_s = elapsed_prev + (time.time() - t0)
    summary = {
        "schema": "denoiseopt.meta_hp_sensitivity.config.v1",
        "config": cfg.id,
        "label": cfg.label,
        "honesty": "sensitivity_probe_500iters_OAT_not_full_5k_research",
        "iters": iters,
        "iters_done": iters,
        "seed": seed,
        "champ_r": champ_r,
        "champ_raw": champ_raw,
        "delta_r_vs_dual_cosine": (champ_raw - baseline) if champ_raw >= 0 else None,
        "delta_r_vs_default": None,  # filled at aggregate
        "baseline_dual_cosine": baseline,
        "wall_s": wall_s,
        "wall_h": wall_s / 3600.0,
        "knobs": {
            "pop_size": cfg.pop_size,
            "ppo_clip": cfg.ppo_clip,
            "lr": cfg.lr,
            "entropy_coef": cfg.entropy_coef,
            "ga_second_mutate_p": cfg.ga_second_mutate_p,
            "freeze_hp": cfg.freeze_hp,
            "fit_steps": FIT_STEPS,
            "batch": BATCH,
        },
        "champ_arch": champ_cfg.to_dict() if champ_cfg else None,
        "champ_hp": champ_hp.to_dict() if champ_hp else None,
        "history_path": str(hist_path),
        "finished_at": utc_now(),
        "complete": True,
    }
    save_json(summary_path, summary)
    log(
        f"DONE config={cfg.id} champ_r={champ_r:.6f} raw={champ_raw:.6f} "
        f"wall_h={wall_s/3600:.3f}"
    )
    write_status(out_dir, phase="config_done", current=cfg.id, current_iter=iters, champ_r=champ_r)
    return summary


def build_aggregate(out_dir: Path, configs: list[SensConfig], *, iters: int, seed: int) -> dict[str, Any]:
    rows: list[dict[str, Any]] = []
    default_raw: float | None = None
    default_summary = load_json(out_dir / "default" / "summary.json")
    if default_summary:
        default_raw = float(default_summary.get("champ_raw", default_summary.get("champ_r", -1)))

    for cfg in configs:
        summary = load_json(out_dir / cfg.id / "summary.json")
        ckpt = load_json(out_dir / cfg.id / "checkpoint.json")
        if summary is None and ckpt is None:
            rows.append(
                {
                    "config": cfg.id,
                    "label": cfg.label,
                    "complete": False,
                    "iters_done": 0,
                    "champ_r": None,
                    "champ_raw": None,
                    "delta_vs_default": None,
                    "knobs": {
                        "pop_size": cfg.pop_size,
                        "ppo_clip": cfg.ppo_clip,
                        "lr": cfg.lr,
                        "entropy_coef": cfg.entropy_coef,
                        "ga_second_mutate_p": cfg.ga_second_mutate_p,
                    },
                }
            )
            continue
        src = summary or {}
        if not src and ckpt:
            src = {
                "champ_r": ckpt.get("champ_r"),
                "champ_raw": ckpt.get("champ_raw"),
                "iters_done": ckpt.get("iters_done", 0),
                "baseline_dual_cosine": ckpt.get("baseline_dual_cosine"),
                "wall_s": ckpt.get("wall_s"),
                "complete": False,
            }
        champ_raw = float(src.get("champ_raw", src.get("champ_r") or -1))
        champ_r = float(src.get("champ_r", champ_raw))
        iters_done = int(src.get("iters_done", 0))
        complete = bool(src.get("complete", iters_done >= iters))
        delta = None
        if default_raw is not None and champ_raw >= 0:
            delta = champ_raw - default_raw
        rows.append(
            {
                "config": cfg.id,
                "label": cfg.label,
                "complete": complete,
                "iters_done": iters_done,
                "target_iters": iters,
                "champ_r": champ_r,
                "champ_raw": champ_raw,
                "delta_vs_default": delta,
                "baseline_dual_cosine": src.get("baseline_dual_cosine"),
                "wall_s": src.get("wall_s"),
                "wall_h": (float(src["wall_s"]) / 3600.0) if src.get("wall_s") is not None else None,
                "knobs": src.get("knobs")
                or {
                    "pop_size": cfg.pop_size,
                    "ppo_clip": cfg.ppo_clip,
                    "lr": cfg.lr,
                    "entropy_coef": cfg.entropy_coef,
                    "ga_second_mutate_p": cfg.ga_second_mutate_p,
                },
            }
        )
        if summary and default_raw is not None and delta is not None:
            summary["delta_r_vs_default"] = delta
            save_json(out_dir / cfg.id / "summary.json", summary)

    n_complete = sum(1 for r in rows if r.get("complete"))
    aggregate = {
        "schema": "denoiseopt.meta_hp_sensitivity.v1",
        "honesty": (
            "One-at-a-time ±50% sensitivity probe at 500 outer iters under seed "
            f"{seed}. Measures local robustness of Table-2 / tab:hyperparams knobs; "
            "does NOT re-prove the 5k meta-approach ranking."
        ),
        "seed": seed,
        "iters_per_config": iters,
        "fit_steps": FIT_STEPS,
        "batch": BATCH,
        "protocol": "sine+cliff FitCell matched to meta compare defaults",
        "design": "OAT_pm50",
        "default_champ_raw": default_raw,
        "n_configs": len(configs),
        "n_complete": n_complete,
        "all_complete": n_complete == len(configs),
        "table": rows,
        "built_at": utc_now(),
    }
    return aggregate


def write_table_tex(aggregate: dict[str, Any], out_tex: Path) -> None:
    lines = [
        r"% Auto-generated by scripts/bench_meta_hp_sensitivity.py — do not hand-edit.",
        r"\begin{table}[t]",
        r"  \centering",
        r"  \small",
        r"  \caption{One-at-a-time $\pm 50\%$ hyperparameter sensitivity probe "
        r"(500 outer iterations, seed \texttt{1902771841}, sine{+}cliff FitCell). "
        r"Champion absolute $R$ vs Table~\ref{tab:hyperparams} default. "
        r"\textbf{Sensitivity evidence only} --- not a full $5$k re-search per HP.}",
        r"  \label{tab:hp-sensitivity}",
        r"  \setlength{\tabcolsep}{4pt}",
        r"  \begin{tabular}{@{}lrrr@{}}",
        r"    \toprule",
        r"    Config & Champ $R$ & $\Delta R$ vs default & Iters \\",
        r"    \midrule",
    ]
    for row in aggregate.get("table", []):
        cid = str(row["config"]).replace("_", r"\_")
        if row.get("champ_raw") is None:
            lines.append(f"    \\texttt{{{cid}}} & --- & --- & {int(row.get('iters_done', 0))} \\\\")
            continue
        r = float(row["champ_raw"])
        d = row.get("delta_vs_default")
        d_s = f"{d:+.5f}" if d is not None else "---"
        done = int(row.get("iters_done", 0))
        mark = "" if row.get("complete") else r"\textsuperscript{*}"
        lines.append(f"    \\texttt{{{cid}}}{mark} & {r:.5f} & {d_s} & {done} \\\\")
    lines += [
        r"    \bottomrule",
        r"  \end{tabular}\\[0.3em]",
        r"  {\footnotesize $^{*}$Incomplete / checkpointed mid-run.}",
        r"\end{table}",
        "",
    ]
    out_tex.parent.mkdir(parents=True, exist_ok=True)
    out_tex.write_text("\n".join(lines), encoding="utf-8")


def write_figure(aggregate: dict[str, Any], out_png: Path, out_pdf: Path | None = None) -> None:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    rows = [r for r in aggregate.get("table", []) if r.get("champ_raw") is not None]
    if not rows:
        return
    labels = [r["config"] for r in rows]
    vals = [float(r["champ_raw"]) for r in rows]
    colors = ["#0072B2" if r["config"] == "default" else "#D55E00" for r in rows]
    fig, ax = plt.subplots(figsize=(9.5, 4.2))
    x = list(range(len(labels)))
    ax.bar(x, vals, color=colors, edgecolor="black", linewidth=0.4)
    default_raw = aggregate.get("default_champ_raw")
    if default_raw is not None:
        ax.axhline(float(default_raw), color="#0072B2", linestyle="--", linewidth=1.2, label="default")
    ax.set_xticks(x)
    ax.set_xticklabels(labels, rotation=35, ha="right", fontsize=8)
    ax.set_ylabel("Champion residual $R$")
    ax.set_title("HP ±50% OAT sensitivity (500 iters; not 5k re-search)")
    ax.set_ylim(min(0.7, min(vals) - 0.02), max(vals) + 0.01)
    ax.grid(True, axis="y", alpha=0.25)
    if default_raw is not None:
        ax.legend(frameon=False, fontsize=8)
    fig.tight_layout()
    out_png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out_png, dpi=220)
    if out_pdf is not None:
        fig.savefig(out_pdf)
    plt.close(fig)


def write_results_prose(aggregate: dict[str, Any], out_md: Path) -> None:
    rows = aggregate.get("table", [])
    default = next((r for r in rows if r["config"] == "default"), None)
    complete = [r for r in rows if r.get("complete") and r.get("champ_raw") is not None]
    incomplete = [r for r in rows if not r.get("complete")]
    lines = [
        "# HP ±50% sensitivity — Results snippet (Q3)",
        "",
        "> Honesty: **sensitivity probe** at 500 outer iterations (OAT ±50%), seed `1902771841`.",
        "> This does **not** re-prove the 5k meta-approach ranking.",
        "",
        f"- Protocol: sine+cliff FitCell, batch {BATCH}, fit_steps {FIT_STEPS}, hybrid GA–PPO loop.",
        f"- Configs complete: {aggregate.get('n_complete')}/{aggregate.get('n_configs')}.",
    ]
    if default and default.get("champ_raw") is not None:
        lines.append(f"- Default champ $R$: `{float(default['champ_raw']):.5f}`.")
    if complete:
        deltas = [
            (r["config"], float(r["delta_vs_default"]))
            for r in complete
            if r["config"] != "default" and r.get("delta_vs_default") is not None
        ]
        if deltas:
            worst = min(deltas, key=lambda t: t[1])
            best = max(deltas, key=lambda t: t[1])
            max_abs = max(abs(d) for _, d in deltas)
            lines.append(
                f"- Largest |ΔR| vs default among completed OAT arms: `{max_abs:.5f}` "
                f"(most negative: `{worst[0]}` ΔR=`{worst[1]:+.5f}`; "
                f"most positive: `{best[0]}` ΔR=`{best[1]:+.5f}`)."
            )
            lines.append(
                "- Interpretation for Q3: Table-2 defaults are **locally robust** under ±50% "
                "one-at-a-time shifts at this 500-iter budget when |ΔR| stays small relative "
                "to the DualCosine gap; remaining gaps are sensitivity, not a claim that the "
                "defaults are globally optimal."
            )
    if incomplete:
        lines.append(
            "- Incomplete configs (resume): "
            + ", ".join(f"`{r['config']}`@{r.get('iters_done', 0)}" for r in incomplete)
        )
        lines.append(
            "- Resume: `.venv_gpu/Scripts/python.exe scripts/bench_meta_hp_sensitivity.py` "
            "(checkpoints under each config dir)."
        )
    lines += [
        "",
        "## Paper pointer",
        "",
        "- Figure: `fig_meta_hp_sensitivity.png` / `.pdf`",
        "- Table: `tab_meta_hp_sensitivity.tex` (`tab:hp-sensitivity`)",
        "- Aggregate JSON: `meta_hp_sensitivity.json`",
        "",
    ]
    out_md.write_text("\n".join(lines), encoding="utf-8")


def write_results_tex_snippet(aggregate: dict[str, Any], out_tex: Path) -> None:
    """Short Results paragraph T6 can \\input without rewriting Methods."""
    rows = aggregate.get("table", [])
    default = next((r for r in rows if r["config"] == "default"), None)
    complete = [r for r in rows if r.get("complete") and r.get("champ_raw") is not None]
    d_r = float(default["champ_raw"]) if default and default.get("champ_raw") is not None else None
    max_abs = None
    if complete and d_r is not None:
        deltas = [
            abs(float(r["delta_vs_default"]))
            for r in complete
            if r["config"] != "default" and r.get("delta_vs_default") is not None
        ]
        if deltas:
            max_abs = max(deltas)
    n_c = int(aggregate.get("n_complete") or 0)
    n_t = int(aggregate.get("n_configs") or 0)
    cave = ""
    if n_c < n_t:
        cave = (
            f" At the time of writing, {n_c}/{n_t} OAT arms had finished; "
            r"incomplete arms are marked in Table~\ref{tab:hp-sensitivity}."
        )
    max_s = f"{max_abs:.4f}" if max_abs is not None else "TBD"
    def_s = f"{d_r:.4f}" if d_r is not None else "TBD"
    body = (
        "% Auto-generated HP sensitivity Results snippet (W4 / Q3). T6 may \\input this.\n"
        "\\paragraph{Hyperparameter sensitivity (Q3).}\n"
        "\\label{para:hp-sensitivity}\n"
        "RQ: how sensitive is hybrid champion $R$ to $\\pm 50\\%$ shifts of "
        "Table~\\ref{tab:hyperparams} knobs?\n"
        "We run a \\textbf{one-at-a-time sensitivity probe} (not a full $5$k re-search): "
        "$500$ outer iterations per config, seed \\texttt{1902771841}, "
        "sine{+}cliff FitCell matched to the meta-compare protocol "
        "(batch $48$, fit steps $24$).\n"
        "Perturbed knobs: population size $n$, PPO clip $\\epsilon$, fit Adam learning rate, "
        "GA second-mutate rate, and PPO entropy coefficient.\n"
        f"Table~\\ref{{tab:hp-sensitivity}} and Figure~\\ref{{fig:hp-sensitivity}} report "
        f"champion absolute $R$ versus the Table-2 default "
        f"(default champ $R\\approx {def_s}$; largest $|\\Delta R|$ among completed arms "
        f"$\\approx {max_s}$).\n"
        f"{cave}\n"
        "This is local robustness evidence under the stated budget and seed; "
        "the $5$k matched-budget ranking remains the primary approach comparison.\n"
    )
    out_tex.parent.mkdir(parents=True, exist_ok=True)
    out_tex.write_text(body.strip() + "\n", encoding="utf-8")


def write_repro(out_dir: Path, aggregate: dict[str, Any], *, seed: int, iters: int) -> None:
    manifest = {
        "schema": "denoiseopt.meta_hp_sensitivity.repro.v1",
        "seed": seed,
        "iters_per_config": iters,
        "fit_steps": FIT_STEPS,
        "batch": BATCH,
        "design": "OAT_pm50",
        "honesty": aggregate.get("honesty"),
        "script": "scripts/bench_meta_hp_sensitivity.py",
        "out_dir": str(out_dir),
        "forbidden": "brand/artifacts/meta_approach_compare/",
        "n_complete": aggregate.get("n_complete"),
        "n_configs": aggregate.get("n_configs"),
        "all_complete": aggregate.get("all_complete"),
        "built_at": utc_now(),
        "resume_cmd": (
            ".venv_gpu/Scripts/python.exe scripts/bench_meta_hp_sensitivity.py"
        ),
        "aggregate_only_cmd": (
            ".venv_gpu/Scripts/python.exe scripts/bench_meta_hp_sensitivity.py --aggregate-only"
        ),
    }
    save_json(out_dir / "REPRO_MANIFEST.json", manifest)
    md = f"""# Meta HP ±50% sensitivity — reproducibility

- Honesty: **sensitivity probe** (500 iters × OAT ±50%), **not** a full 5k re-search per HP.
- Seed: `{seed}`
- FitCell protocol: sine+cliff, batch `{BATCH}`, fit_steps `{FIT_STEPS}` (matched to meta compare).
- Output root: `brand/artifacts/meta_hp_sensitivity/`
- **Forbidden:** do not wipe or write `brand/artifacts/meta_approach_compare/`.

## Status

- Configs complete: `{aggregate.get('n_complete')}/{aggregate.get('n_configs')}`
- All complete: `{aggregate.get('all_complete')}`

## Resume

```bash
.venv_gpu/Scripts/python.exe scripts/bench_meta_hp_sensitivity.py
```

Checkpoints live under each `{{config_id}}/checkpoint.json`. Completed configs are skipped.

## Aggregate / figures only

```bash
.venv_gpu/Scripts/python.exe scripts/bench_meta_hp_sensitivity.py --aggregate-only
```

## Artifacts

| File | Role |
|------|------|
| `results.json` / `meta_hp_sensitivity.json` | Aggregate table |
| `fig_meta_hp_sensitivity.png` / `.pdf` | Bar chart |
| `tab_meta_hp_sensitivity.tex` | Paper table |
| `RESULTS_SNIPPET.md` | Q3 prose for T6 |
| `{{config_id}}/summary.json` | Per-config champion |
"""
    (out_dir / "REPRO.md").write_text(md, encoding="utf-8")


def publish_to_paper(aggregate: dict[str, Any], out_dir: Path) -> list[Path]:
    paper_fig = META_ROOT / "paper" / "Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9" / "figures"
    paper_sub = META_ROOT / "paper" / "Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9" / "subsections"
    paper_fig.mkdir(parents=True, exist_ok=True)
    paper_sub.mkdir(parents=True, exist_ok=True)
    copied: list[Path] = []

    mapping = [
        (out_dir / "meta_hp_sensitivity.json", paper_fig / "meta_hp_sensitivity.json"),
        (out_dir / "fig_meta_hp_sensitivity.png", paper_fig / "fig_meta_hp_sensitivity.png"),
        (out_dir / "fig_meta_hp_sensitivity.pdf", paper_fig / "fig_meta_hp_sensitivity.pdf"),
        (out_dir / "tab_meta_hp_sensitivity.tex", paper_fig / "tab_meta_hp_sensitivity.tex"),
        (out_dir / "RESULTS_SNIPPET.md", paper_fig / "RESULTS_HP_SENSITIVITY.md"),
    ]
    for src, dst in mapping:
        if src.is_file():
            shutil.copy2(src, dst)
            copied.append(dst)

    snip = out_dir / "results_hp_sensitivity.tex"
    if snip.is_file():
        dst = paper_sub / "results_hp_sensitivity.tex"
        shutil.copy2(snip, dst)
        copied.append(dst)
    return copied


def write_figure_tex_include(out_tex: Path) -> None:
    body = r"""% Figure wrapper for HP sensitivity (W4).
\begin{figure}[t]
  \centering
  \includegraphics[width=\linewidth]{figures/fig_meta_hp_sensitivity.png}
  \caption{One-at-a-time $\pm 50\%$ hyperparameter sensitivity of hybrid champion $R$
  at a $500$-iteration budget (seed \texttt{1902771841}). Blue bar / dashed line: Table~\ref{tab:hyperparams} default.
  This is a \textbf{sensitivity probe}, not a full $5$k re-search per HP.}
  \label{fig:hp-sensitivity}
\end{figure}
"""
    out_tex.write_text(body.strip() + "\n", encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--iters", type=int, default=DEFAULT_ITERS)
    ap.add_argument("--seed", type=int, default=DEFAULT_SEED)
    ap.add_argument("--ckpt-every", type=int, default=25)
    ap.add_argument("--device", type=str, default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--out-dir", type=Path, default=DEFAULT_OUT)
    ap.add_argument(
        "--configs",
        type=str,
        default="",
        help="Comma-separated subset of config ids (default: full OAT grid).",
    )
    ap.add_argument("--force-fresh", action="store_true", help="Wipe config dirs and restart.")
    ap.add_argument(
        "--aggregate-only",
        action="store_true",
        help="Rebuild aggregate/figures/paper copies from summaries; no search.",
    )
    args = ap.parse_args()

    refuse_forbidden_out(args.out_dir)
    out_dir: Path = args.out_dir
    out_dir.mkdir(parents=True, exist_ok=True)
    (out_dir / "bench.pid").write_text(str(os.getpid()), encoding="utf-8")

    grid = oat_grid()
    by_id = {c.id: c for c in grid}
    if args.configs.strip():
        names = [x.strip() for x in args.configs.split(",") if x.strip()]
        for n in names:
            if n not in by_id:
                raise SystemExit(f"Unknown config {n!r}; choose from {sorted(by_id)}")
        configs = [by_id[n] for n in names]
    else:
        configs = grid

    device = torch.device(
        args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu"
    )

    if not args.aggregate_only:
        if args.force_fresh:
            for cfg in configs:
                cfg_dir = out_dir / cfg.id
                if cfg_dir.is_dir():
                    shutil.rmtree(cfg_dir)
        print(
            f"device={device} iters={args.iters} seed={args.seed} "
            f"configs={[c.id for c in configs]} out={out_dir}",
            flush=True,
        )
        write_status(
            out_dir,
            phase="launch",
            target_iters=args.iters,
            configs=[c.id for c in configs],
            seed=args.seed,
            device=str(device),
        )
        def checkpoint_publish(tag: str) -> dict[str, Any]:
            """Rebuild aggregate + paper copies after each config (crash-safe)."""
            agg = build_aggregate(out_dir, grid, iters=args.iters, seed=args.seed)
            save_json(out_dir / "results.json", agg)
            save_json(out_dir / "meta_hp_sensitivity.json", agg)
            write_table_tex(agg, out_dir / "tab_meta_hp_sensitivity.tex")
            write_figure(
                agg,
                out_dir / "fig_meta_hp_sensitivity.png",
                out_dir / "fig_meta_hp_sensitivity.pdf",
            )
            write_figure_tex_include(out_dir / "fig_meta_hp_sensitivity.tex")
            write_results_prose(agg, out_dir / "RESULTS_SNIPPET.md")
            write_results_tex_snippet(agg, out_dir / "results_hp_sensitivity.tex")
            write_repro(out_dir, agg, seed=args.seed, iters=args.iters)
            copied_local = publish_to_paper(agg, out_dir)
            write_status(
                out_dir,
                phase="all_complete" if agg.get("all_complete") else "partial",
                target_iters=args.iters,
                n_complete=agg.get("n_complete"),
                n_configs=agg.get("n_configs"),
                last_publish=tag,
                copied=[str(p) for p in copied_local],
            )
            print(
                f"PUBLISH tag={tag} n_complete={agg.get('n_complete')}/{agg.get('n_configs')}",
                flush=True,
            )
            return agg

        for cfg in configs:
            run_config(
                cfg,
                iters=args.iters,
                seed=args.seed,
                device=device,
                out_dir=out_dir,
                ckpt_every=args.ckpt_every,
                resume=not args.force_fresh,
            )
            checkpoint_publish(cfg.id)

    # Final aggregate (also used for --aggregate-only).
    aggregate = build_aggregate(out_dir, grid, iters=args.iters, seed=args.seed)
    save_json(out_dir / "results.json", aggregate)
    save_json(out_dir / "meta_hp_sensitivity.json", aggregate)
    write_table_tex(aggregate, out_dir / "tab_meta_hp_sensitivity.tex")
    write_figure(
        aggregate,
        out_dir / "fig_meta_hp_sensitivity.png",
        out_dir / "fig_meta_hp_sensitivity.pdf",
    )
    write_figure_tex_include(out_dir / "fig_meta_hp_sensitivity.tex")
    write_results_prose(aggregate, out_dir / "RESULTS_SNIPPET.md")
    write_results_tex_snippet(aggregate, out_dir / "results_hp_sensitivity.tex")
    write_repro(out_dir, aggregate, seed=args.seed, iters=args.iters)
    copied = publish_to_paper(aggregate, out_dir)
    write_status(
        out_dir,
        phase="all_complete" if aggregate.get("all_complete") else "partial",
        target_iters=args.iters,
        n_complete=aggregate.get("n_complete"),
        n_configs=aggregate.get("n_configs"),
        copied=[str(p) for p in copied],
    )
    print(json.dumps({"n_complete": aggregate.get("n_complete"), "n_configs": aggregate.get("n_configs")}, indent=2))
    for p in copied:
        print(f"copied {p}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
