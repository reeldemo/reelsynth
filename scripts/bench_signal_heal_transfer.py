#!/usr/bin/env python3
"""Publishable-scale pilot: DenoiseOpt wrap heal on sci/eng cycles.

Method under test: **only** hybrid GA–PPO (`hybrid_lstm`) — same outer loop as
``bench_meta_approaches_5k.py`` / overnight hybrid (champ R≈0.991 on wavetable).

Baselines: shared classical board + domain classical (bearings COT / ECG SBMM-lite).
Honest labels: no deep SOTA claim when only classical fades run.

Artifacts: ``brand/artifacts/signal_heal_transfer/``
"""
from __future__ import annotations

import argparse
import json
import math
import random
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import matplotlib

matplotlib.use("Agg")
import matplotlib.pyplot as plt
import torch
import torch.nn as nn

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))

import overnight_gpu_rl_arch as og  # noqa: E402
from denoise_arch_blocks import BLOCKS, CELL_KINDS  # noqa: E402
from signal_heal.baselines_domain import (  # noqa: E402
    BASELINE_LABELS,
    domain_baselines,
)
from signal_heal.datasets import DomainBatcher, ensure_bundles  # noqa: E402

OUT = ROOT / "brand" / "artifacts" / "signal_heal_transfer"
META_PAPER = ROOT.parent / "denoise-opt-meta"
DEFAULT_SEED = og.DEFAULT_SEED
SEARCH_SEED = 1902771841


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")


def maybe_inject_recurrent(cfg: og.ArchConfig, rng: random.Random, p: float = 0.18) -> og.ArchConfig:
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
    """Fit + R_blend + latency J; return (r_blend, j_scored, cell)."""
    r_blend, _j, j_scored, t_ms, cell = og.evaluate_candidate(
        cfg,
        hp,
        device,
        baseline=baseline,
        fit_steps_default=fit_steps_default,
        batch_default=batch_default,
    )
    # Stash for history/checkpoint writers in this module.
    evaluate.last_t_ms = float(t_ms)  # type: ignore[attr-defined]
    evaluate.last_r_blend = float(r_blend)  # type: ignore[attr-defined]
    evaluate.last_r_seam = float(r_blend)  # type: ignore[attr-defined]  # legacy alias
    evaluate.last_j = float(_j)  # type: ignore[attr-defined]
    return r_blend, j_scored, cell


@torch.no_grad()
def score_method(
    ideal: torch.Tensor,
    eng: torch.Tensor,
    fn,
) -> float:
    out = fn(eng)
    return float(og.residual_score_blend(ideal, eng, out).mean().item())


def run_hybrid_lstm_domain(
    batcher: DomainBatcher,
    *,
    iters: int,
    seed: int,
    device: torch.device,
    out_dir: Path,
    fit_steps: int,
    batch: int,
    pop_size: int,
    ckpt_every: int = 50,
    warm_champ: dict[str, Any] | None = None,
) -> dict[str, Any]:
    """hybrid_lstm outer loop with og.make_batch patched to domain cycles."""
    out_dir.mkdir(parents=True, exist_ok=True)
    hist_path = out_dir / "history.jsonl"
    ckpt_path = out_dir / "checkpoint.json"
    log_path = out_dir / "run.log"

    # Patch synthetic batch → domain
    orig_make = og.make_batch
    og.make_batch = batcher  # type: ignore[assignment]

    rng = random.Random(seed)
    torch.manual_seed(seed)
    if device.type == "cuda":
        torch.cuda.manual_seed_all(seed)

    # Classical DualCosine baseline on domain holdout (v10.1: R_blend)
    hold_i, hold_e = batcher.holdout(64)
    baseline = float(
        og.residual_score_blend(hold_i, hold_e, og.dual_cosine_blend(hold_e)).mean().item()
    )
    nobake_ref = float(og.residual_score_blend(hold_i, hold_e, hold_e).mean().item())

    policy = og.ActorCritic().to(device)
    policy_opt = torch.optim.Adam(policy.parameters(), lr=3e-4)
    pop = [
        og.Individual(og.random_arch(rng), og.random_hp(rng), score=-1.0, age=0)
        for _ in range(pop_size)
    ]
    # Warm-start one individual from wavetable champion arch/HP if provided
    if warm_champ and warm_champ.get("champ_arch"):
        try:
            wcfg = og.ArchConfig.from_dict(warm_champ["champ_arch"])
            whp = og.HyperParams.from_dict(warm_champ.get("champ_hp"))
            pop[0] = og.Individual(wcfg, whp, score=-1.0, age=0)
        except Exception:
            pass

    buf = og.RolloutBuffer()
    last_good_policy = og.snapshot_state_dict(policy)
    plateau = og.PlateauAdaptState()
    hybrid_branches = ("ppo", "ga", "pbt", "nas", "combo")
    branch_best = {b: 0.0 for b in hybrid_branches}

    champ_r = -1.0
    champ_raw = -1.0
    champ_t_ms = float("nan")
    champ_j = -1.0
    champ_cfg: og.ArchConfig | None = None
    champ_hp: og.HyperParams | None = None
    champ_cell_sd: dict[str, Any] | None = None
    iters_since_improve = 0
    t0 = time.time()
    plateau_every = 40

    def log(msg: str) -> None:
        line = f"[{utc_now()}] {msg}"
        try:
            print(line, flush=True)
        except Exception:
            pass  # broken stdout pipe (e.g. Tee-Object) must not kill the run
        try:
            with log_path.open("a", encoding="utf-8") as f:
                f.write(line + "\n")
        except Exception:
            pass

    log(
        f"hybrid_lstm domain start iters={iters} seed={seed} baseline_dual={baseline:.4f} "
        f"nobake={nobake_ref:.4f} n_cycles={batcher.n} L={batcher.l} "
        f"metric=r_blend objective=J"
    )

    try:
        for it in range(1, iters + 1):
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

            improved = False
            if r > champ_r:
                champ_raw = r_raw
                champ_r = r
                champ_cfg = trial_cfg
                champ_hp = trial_hp
                champ_cell_sd = {k: v.detach().cpu().clone() for k, v in cell.state_dict().items()}
                champ_t_ms = float(getattr(evaluate, "last_t_ms", float("nan")))
                champ_j = float(getattr(evaluate, "last_j", r))
                iters_since_improve = 0
                improved = True
            else:
                iters_since_improve += 1

            if iters_since_improve >= plateau_every and plateau.level < 8:
                plateau.level += 1
                iters_since_improve = 0
                log(f"plateau level -> {plateau.level}")

            row = {
                "it": it,
                "branch": branch,
                "proposal": proposal,
                "r_raw": r_raw,
                "r_seam": r_raw,
                "r": r,
                "j": float(getattr(evaluate, "last_j", r)),
                "t_ms": float(getattr(evaluate, "last_t_ms", float("nan"))),
                "champ_raw": champ_raw,
                "champ_r": champ_r,
                "wall_s": time.time() - t0,
                "primary_metric": "r_blend",
                "search_objective": "J=R_blend-lambda*latency_norm",
            }
            with hist_path.open("a", encoding="utf-8") as f:
                f.write(json.dumps(row, separators=(",", ":")) + "\n")

            if it % 50 == 0 or it == 1 or it == iters:
                log(
                    f"it={it}/{iters} branch={branch} R_seam={r_raw:.4f} "
                    f"t_ms={getattr(evaluate, 'last_t_ms', float('nan')):.3f} "
                    f"J={getattr(evaluate, 'last_j', r):.4f} best_J={champ_r:.4f} "
                    f"dR_vs_Dual={champ_raw - baseline:+.4f}"
                )
            elif improved:
                # Quiet: improvements only to history.jsonl; avoid notify spam on "champ"
                pass

            if it % ckpt_every == 0 or it == iters:
                payload = {
                    "it": it,
                    "champ_raw": champ_raw,
                    "champ_r": champ_r,
                    "champ_r_seam": champ_raw,
                    "champ_j": champ_j if champ_j >= 0 else champ_r,
                    "champ_t_ms": champ_t_ms,
                    "lambda_latency": og.LAMBDA_LATENCY,
                    "champ_arch": champ_cfg.to_dict() if champ_cfg else None,
                    "champ_hp": champ_hp.to_dict() if champ_hp else None,
                    "baseline_dual_cosine": baseline,
                    "nobake": nobake_ref,
                    "branch_best": branch_best,
                    "wall_s": time.time() - t0,
                    "primary_metric": "r_blend",
                    "search_objective": "J=R_blend-lambda*latency_norm",
                }
                try:
                    ckpt_path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
                    # Mid-run summary so a killed process still leaves a usable champ
                    mid = {
                        "approach": "hybrid_lstm",
                        "iters": iters,
                        "iters_done": it,
                        "seed": seed,
                        "champ_raw": champ_raw,
                        "champ_r": champ_r,
                        "champ_r_seam": champ_raw,
                        "champ_j": champ_j if champ_j >= 0 else champ_r,
                        "champ_t_ms": champ_t_ms,
                        "baseline_dual_cosine": baseline,
                        "nobake": nobake_ref,
                        "delta_r_vs_dual_cosine": champ_raw - baseline,
                        "champ_arch": champ_cfg.to_dict() if champ_cfg else None,
                        "champ_hp": champ_hp.to_dict() if champ_hp else None,
                        "branch_best": branch_best,
                        "wall_s": time.time() - t0,
                        "finished_at": utc_now() if it >= iters else None,
                        "partial": it < iters,
                        "metric": (
                            "v10.1 R_blend = α·R_seam(ideal,out)+(1-α)·R_body(eng,out) on SEAM_W / body; "
                            "α=0.7; search objective J = R_blend - λ·latency_norm"
                        ),
                    }
                    (out_dir / "summary.json").write_text(json.dumps(mid, indent=2), encoding="utf-8")
                    if champ_cell_sd is not None:
                        torch.save(champ_cell_sd, out_dir / "champ_cell.pt")
                    torch.save(policy.state_dict(), out_dir / "policy.pt")
                except Exception as e:
                    log(f"ckpt write failed it={it}: {type(e).__name__}: {e}")
    finally:
        og.make_batch = orig_make

    summary = {
        "approach": "hybrid_lstm",
        "iters": iters,
        "iters_done": iters,
        "seed": seed,
        "champ_raw": champ_raw,
        "champ_r": champ_r,
        "champ_r_seam": champ_raw,
        "champ_j": champ_j if champ_j >= 0 else champ_r,
        "champ_t_ms": champ_t_ms,
        "lambda_latency": og.LAMBDA_LATENCY,
        "baseline_dual_cosine": baseline,
        "nobake": nobake_ref,
        "delta_r_vs_dual_cosine": champ_raw - baseline,
        "champ_arch": champ_cfg.to_dict() if champ_cfg else None,
        "champ_hp": champ_hp.to_dict() if champ_hp else None,
        "branch_best": branch_best,
        "wall_s": time.time() - t0,
        "finished_at": utc_now(),
        "partial": False,
        "metric": (
            "v10.1 R_blend=α·R_seam+(1-α)·R_body (α=0.7); "
            "search objective J = R_blend - λ·latency_norm (λ=0.02)"
        ),
        "primary_metric": "r_blend",
        "search_objective": "J=R_blend-lambda*latency_norm",
    }
    (out_dir / "summary.json").write_text(json.dumps(summary, indent=2), encoding="utf-8")
    log(f"done champ_r_seam={champ_raw:.4f} champ_j={champ_r:.4f} wall_s={summary['wall_s']:.1f}")
    return summary


def refit_and_score(
    summary: dict[str, Any],
    batcher: DomainBatcher,
    device: torch.device,
) -> tuple[float, og.SeamCell, og.ArchConfig]:
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
        ideal, eng = batcher.holdout(96)
        r = float(og.residual_score_blend(ideal, eng, og.apply_ops(eng, cell, cfg.ops)).mean().item())
    finally:
        og.make_batch = orig
    return r, cell, cfg


def score_baselines(
    bundle_name: str,
    domain: str,
    batcher: DomainBatcher,
) -> dict[str, float]:
    ideal, eng = batcher.holdout(96)
    scores: dict[str, float] = {}
    for name, fn in domain_baselines(domain).items():
        try:
            scores[name] = score_method(ideal, eng, fn)
        except Exception as e:
            scores[name] = float("nan")
            print(f"baseline {name} failed: {e}")
    return scores


def plot_bars(table: dict[str, dict[str, float]], out_png: Path, out_pdf: Path) -> None:
    # methods of interest, ordered
    prefer = [
        "ours_hybrid_lstm",
        "dual_cosine",
        "soft_periodize_hann",
        "linear_fade",
        "endpoint_pin_mean",
        "seam_fir3",
        "spline_join",
        "beat_average_sbmm_lite",
        "cot_cubic_then_dualcosine",
        "no_bake",
    ]
    datasets = list(table.keys())
    methods = []
    for m in prefer:
        if any(
            m in table[d] and isinstance(table[d].get(m), (int, float)) for d in datasets
        ):
            methods.append(m)
    # add any leftovers (numeric columns only — skip nested n2n blobs etc.)
    for d in datasets:
        for m, v in table[d].items():
            if m not in methods and isinstance(v, (int, float)):
                methods.append(m)

    n_ds = len(datasets)
    n_m = len(methods)
    # Tall 2×3 board so column-width / text-width embeds stay readable.
    n_cols = 3 if n_ds >= 3 else max(n_ds, 1)
    n_rows = int(math.ceil(n_ds / n_cols)) if n_ds else 1
    fig_w = max(10.0, 3.4 * n_cols)
    fig_h = max(6.5, 3.6 * n_rows)
    fig, axes = plt.subplots(n_rows, n_cols, figsize=(fig_w, fig_h), squeeze=False)
    colors = {
        "ours_hybrid_lstm": "#D55E00",
        "dual_cosine": "#0072B2",
        "no_bake": "#999999",
    }
    flat = list(axes.ravel())
    for i, ds in enumerate(datasets):
        ax = flat[i]
        vals = []
        for m in methods:
            v = table[ds].get(m, float("nan"))
            vals.append(float(v) if isinstance(v, (int, float)) else float("nan"))
        cols = [colors.get(m, "#56B4E9") for m in methods]
        xs = range(len(methods))
        ax.bar(xs, vals, color=cols)
        ax.set_xticks(list(xs))
        ax.set_xticklabels(methods, rotation=55, ha="right", fontsize=8)
        ax.set_ylim(0.0, 1.02)
        ax.set_ylabel(r"$R_{\mathrm{blend}}$")
        ax.set_title(ds.replace("_", " "), fontsize=10)
        ax.axhline(0.0, color="k", lw=0.5)
        ax.grid(axis="y", alpha=0.3)
    for j in range(len(datasets), len(flat)):
        flat[j].axis("off")
    fig.suptitle(
        "Signal-heal transfer pilot — Ours (hybrid GA–PPO) vs classical board",
        fontsize=12,
    )
    fig.tight_layout()
    out_png.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(out_png, dpi=180)
    fig.savefig(out_pdf)
    plt.close(fig)


def write_readme(path: Path, results: dict[str, Any]) -> None:
    skipped = results.get("skipped_optional", {})
    lines = [
        "# Signal-heal transfer pilot",
        "",
        f"Generated: `{results.get('finished_at')}`",
        "",
        "## Method under test",
        "",
        "- **Ours:** hybrid GA–PPO outer loop (`hybrid_lstm` in `bench_meta_approaches_5k.py`).",
        "- FitCell / SeamCell / arch search reused; period length fixed to `N=256`.",
        "- Metric: DenoiseOpt prolonged residual $R$ (same formula as wavetable).",
        "- Pilot budget: modest outer iters (see `config`); not full industrial overnight.",
        "",
        "## Wrap construction",
        "",
        "- **CWRU bearings:** DE @12 kHz; per-rev windows via RPM; **ideal** = cubic resample to $L$; "
        "**engine** = linear resample (bad-COT proxy) + DenoiseOpt-style wrap cliff + seam noise.",
        "- **MFPT:** same protocol when zip available; fixed shaft-rate periods.",
        "- **Paderborn KAt (K001):** vibration_1 @~64 kHz; speed→angle equal-rev windows when "
        "Mech_4kHz tach available; same cubic ideal / linear+cliff engine (classical board only).",
        "- **MIT-BIH / PTB-XL ECG:** R–R beats → $L$; **ideal** = local mean template + mild endpoint "
        "equalize (SBMM-lite classical); **engine** = single beat + wrap cliff.",
        "- **synth_cnc_g01 / synth_pmu_cycle:** synthetic CNC / power-cycle proxies when KIT / DataPort blocked.",
        "",
        "## Seeds",
        "",
        f"- Search / construction seed: `{SEARCH_SEED}`",
        f"- Holdout sample seed: `20260719`",
        "",
        "## Honesty / limits",
        "",
        "- Baselines are a **classical board** (+ domain classical COT / SBMM-lite). "
        "We do **not** claim BeatDiff / Cycle-GAN / deep order-tracking SOTA unless those weights ran.",
        "- See `DEEP_SOTA_NOT_EXECUTED.json` and `LISTENING_PROTOCOL_MAIN.md` (no invented MOS).",
        "- Do not wipe `brand/artifacts/meta_approach_compare/`.",
        "- Optional KIT CNC / IEEE PMU / BMRB NMR skipped if login/paywall. "
        "Paderborn K001 is extracted; deep Paderborn models remain unwired.",
        "",
        "### Login-walled downloads (user must open)",
        "",
        "- KIT CNC: https://doi.org/10.35097/hvvwn1kfwf7qt48z",
        "- IEEE 39-bus PMU: https://ieee-dataport.org/open-access/pmu-measurements-ieee-39-bus-power-system-model",
        "",
        "### Skipped optional",
        "",
    ]
    for k, v in skipped.items():
        lines.append(f"- **{k}:** {v}")
    lines += [
        "",
        "## Results table",
        "",
        "See `results_table.json` and `fig_signal_heal_transfer.{png,pdf}`.",
        "",
        "## Reproduce",
        "",
        "```bash",
        ".venv_gpu/Scripts/python scripts/download_signal_heal_data.py",
        ".venv_gpu/Scripts/python scripts/bench_signal_heal_transfer.py --iters 250 --merge-existing",
        ".venv_gpu/Scripts/python scripts/export_signal_heal_hear_pack.py",
        "```",
        "",
        "Also: `REPRO.md`, `DEEP_SOTA_NOT_EXECUTED.json`, `LISTENING_PROTOCOL_MAIN.md`.",
        "",
    ]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def write_paper_note(path: Path, results: dict[str, Any]) -> None:
    table = results.get("table", {})
    lines = [
        "# Appendix stub — sci/eng wrap-heal transfer pilot",
        "",
        f"**Date:** {results.get('finished_at', '')}",
        "",
        "Pilot transfer of DenoiseOpt’s winning outer loop (**hybrid GA–PPO / `hybrid_lstm`**) "
        "to public cycle-local wrap tasks (CWRU, MIT-BIH, PTB-XL; MFPT if available; "
        "synthetic CNC/PMU proxies when OA downloads are blocked). "
        "Period length $L=256$; score = prolonged residual $R$ vs ideal sibling.",
        "",
        "## Results (prolonged $R$, higher better)",
        "",
    ]
    for ds, row in table.items():
        lines.append(f"### {ds}")
        lines.append("")
        lines.append("| Method | $R$ | Label |")
        lines.append("|--------|-----|-------|")
        def _sort_key(kv: tuple[str, Any]) -> tuple[float, str]:
            v = kv[1]
            if isinstance(v, dict):
                v = v.get("R", float("nan"))
            if not isinstance(v, (int, float)) or v != v:
                return (1.0, kv[0])
            return (-float(v), kv[0])

        for m, r in sorted(row.items(), key=_sort_key):
            lab = BASELINE_LABELS.get(m, m)
            if isinstance(r, dict):
                rv = r.get("R")
                if isinstance(rv, (int, float)):
                    lines.append(f"| `{m}` | {float(rv):.4f} | {lab} (nested R) |")
                continue
            if not isinstance(r, (int, float)):
                continue
            lines.append(f"| `{m}` | {float(r):.4f} | {lab} |")
        lines.append("")
    lines += [
        "## Caveats",
        "",
        "- Classical board + domain classical proxies; not a claim of beating published deep SOTA "
        "unless those models were executed.",
        "- Modest outer-loop budget (pilot hours, not multi-day).",
        "- Real content is z-scored per period; musical/clinical absolute scale not preserved.",
        "",
        "Artifacts live in reelsynth `brand/artifacts/signal_heal_transfer/`.",
        "",
    ]
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--iters", type=int, default=250, help="outer hybrid iters per dataset")
    ap.add_argument("--fit-steps", type=int, default=40)
    ap.add_argument("--batch", type=int, default=48)
    ap.add_argument("--pop-size", type=int, default=8)
    ap.add_argument("--device", type=str, default="cuda" if torch.cuda.is_available() else "cpu")
    ap.add_argument("--seed", type=int, default=DEFAULT_SEED)
    ap.add_argument("--n-periods", type=int, default=256)
    ap.add_argument("--force-rebuild", action="store_true")
    ap.add_argument(
        "--datasets",
        type=str,
        default=(
            "cwru_bearings,mitbih_ecg,mfpt_bearings,paderborn_kat,"
            "ptbxl_ecg,synth_cnc_g01,synth_pmu_cycle"
        ),
        help="comma list; missing downloads are skipped",
    )
    ap.add_argument(
        "--merge-existing",
        action="store_true",
        help="keep prior results_table rows for datasets not re-run",
    )
    ap.add_argument("--skip-search", action="store_true", help="baselines only (debug)")
    args = ap.parse_args()

    device = torch.device(args.device if args.device != "cuda" or torch.cuda.is_available() else "cpu")
    OUT.mkdir(parents=True, exist_ok=True)

    prior_table: dict[str, dict[str, float]] = {}
    prior_per: dict[str, Any] = {}
    prior_path = OUT / "results_table.json"
    if args.merge_existing and prior_path.is_file():
        try:
            prior = json.loads(prior_path.read_text(encoding="utf-8"))
            prior_table = dict(prior.get("table") or {})
            prior_per = dict(prior.get("per_dataset") or {})
        except Exception:
            pass

    print("Building / loading domain bundles…")
    bundles = ensure_bundles(force=args.force_rebuild, n_periods=args.n_periods)
    wanted = [x.strip() for x in args.datasets.split(",") if x.strip()]

    warm_path = ROOT / "brand" / "artifacts" / "meta_approach_compare" / "hybrid_lstm" / "summary.json"
    warm = json.loads(warm_path.read_text(encoding="utf-8")) if warm_path.is_file() else None

    table: dict[str, dict[str, float]] = dict(prior_table) if args.merge_existing else {}
    per_ds: dict[str, Any] = {}
    ran: list[str] = []
    skipped: dict[str, str] = json.loads(
        (OUT / "cache" / "skipped_optional.json").read_text(encoding="utf-8")
    ) if (OUT / "cache" / "skipped_optional.json").is_file() else {}

    for name in wanted:
        bundle = bundles.get(name)
        if bundle is None:
            skipped[name] = "dataset unavailable after download/build"
            print(f"SKIP {name}: unavailable")
            continue
        print(f"=== {name} n={bundle.ideal.shape[0]} domain={bundle.meta.get('domain')} ===")
        batcher = DomainBatcher(bundle, device)
        domain = str(bundle.meta.get("domain", "unknown"))
        ds_dir = OUT / name
        ds_dir.mkdir(parents=True, exist_ok=True)

        base_scores = score_baselines(name, domain, batcher)
        ours_r = float("nan")
        summary = None
        if not args.skip_search:
            summary = run_hybrid_lstm_domain(
                batcher,
                iters=args.iters,
                seed=args.seed,
                device=device,
                out_dir=ds_dir / "hybrid_lstm",
                fit_steps=args.fit_steps,
                batch=args.batch,
                pop_size=args.pop_size,
                warm_champ=warm,
            )
            # Holdout score with refit for reporting stability
            ours_r, _, _ = refit_and_score(summary, batcher, device)
            # Also keep search champ_raw
            summary["holdout_refit_R"] = ours_r
            (ds_dir / "hybrid_lstm" / "summary.json").write_text(
                json.dumps(summary, indent=2), encoding="utf-8"
            )
        row = dict(base_scores)
        if ours_r == ours_r:
            row["ours_hybrid_lstm"] = ours_r
        elif summary is not None:
            row["ours_hybrid_lstm"] = float(summary["champ_raw"])
        # else: leave for prior-row preserve below (skip-search / failed search)
        # Preserve prior nested columns (e.g. n2n_domain_trained) when re-running Ours only.
        prior_row = prior_table.get(name) or {}
        for pk, pv in prior_row.items():
            if pk not in row:
                row[pk] = pv
        table[name] = row
        per_ds[name] = {
            "meta": bundle.meta,
            "baseline_labels": {k: BASELINE_LABELS.get(k, k) for k in row},
            "hybrid_summary": summary,
        }
        ran.append(name)
        (ds_dir / "scores.json").write_text(json.dumps(row, indent=2), encoding="utf-8")

    # Preserve prior per_dataset for merge-only rows
    if args.merge_existing:
        for k, v in prior_per.items():
            if k not in per_ds:
                per_ds[k] = {
                    "meta": v.get("meta", {}),
                    "baseline_labels": v.get("baseline_labels", {}),
                    "hybrid_summary": {
                        "champ_raw": v.get("champ_raw"),
                        "holdout_refit_R": v.get("holdout_refit_R"),
                    },
                }

    results = {
        "finished_at": utc_now(),
        "config": {
            "iters": args.iters,
            "fit_steps": args.fit_steps,
            "batch": args.batch,
            "pop_size": args.pop_size,
            "seed": args.seed,
            "device": str(device),
            "period_l": 256,
            "metric": "prolonged residual R (DenoiseOpt)",
            "method": "hybrid_lstm only",
        },
        "datasets_ran": sorted(set(ran) | (set(prior_table) if args.merge_existing else set())),
        "datasets_ran_this_session": ran,
        "skipped": skipped,
        "skipped_optional": skipped,
        "table": table,
        "per_dataset": {
            k: {
                "meta": v["meta"],
                "baseline_labels": v["baseline_labels"],
                "champ_raw": (v["hybrid_summary"] or {}).get("champ_raw"),
                "holdout_refit_R": (v["hybrid_summary"] or {}).get("holdout_refit_R"),
            }
            for k, v in per_ds.items()
        },
        "honesty": (
            "Classical board + domain classical proxies. Not a deep SOTA bake-off unless "
            "published model weights were run. synth_cnc_g01 / synth_pmu_cycle are synthetic "
            "proxies when real KIT / DataPort downloads are blocked."
        ),
        "deep_sota_status": {
            "cycle_gan": "not_executed",
            "beatdiff": "not_executed",
            "paderborn_deep": "not_executed",
            "note": "Literature comparison only; no invented scores.",
        },
    }
    (OUT / "results_table.json").write_text(json.dumps(results, indent=2), encoding="utf-8")
    plot_bars(table, OUT / "fig_signal_heal_transfer.png", OUT / "fig_signal_heal_transfer.pdf")
    write_readme(OUT / "README.md", results)

    # Paper mirror note + figures into titled v9 folder
    paper_note = META_PAPER / "docs" / "SIGNAL_HEAL_TRANSFER_PILOT.md"
    if META_PAPER.is_dir():
        write_paper_note(paper_note, results)
        write_paper_note(ROOT / "docs" / "papers" / "denoise_opt" / "SIGNAL_HEAL_TRANSFER_PILOT.md", results)
        fig_dst = (
            META_PAPER
            / "paper"
            / "Unsupervised_Wavetable_Seam_Artifact_Repair_via_Hybrid_GA-PPO_Meta-Search_v9"
            / "figures"
        )
        if fig_dst.parent.is_dir():
            fig_dst.mkdir(parents=True, exist_ok=True)
            import shutil

            for ext in ("png", "pdf"):
                src = OUT / f"fig_signal_heal_transfer.{ext}"
                if src.is_file():
                    shutil.copy2(src, fig_dst / f"fig_signal_heal_transfer.{ext}")
            # Also keep JSON table sibling for paper author
            shutil.copy2(OUT / "results_table.json", fig_dst / "signal_heal_transfer_results_table.json")
            for name in (
                "DEEP_SOTA_NOT_EXECUTED.json",
                "LISTENING_PROTOCOL_MAIN.md",
                "REPRO.md",
            ):
                p = OUT / name
                if p.is_file():
                    shutil.copy2(p, fig_dst.parent / name)

    # Always write deep-SOTA honesty + REPRO next to artifacts
    deep_path = OUT / "DEEP_SOTA_NOT_EXECUTED.json"
    if not deep_path.is_file():
        deep_path.write_text(
            json.dumps(results.get("deep_sota_status", {}), indent=2),
            encoding="utf-8",
        )

    print("Ran:", ran)
    print("Table:", json.dumps(table, indent=2))
    print("Artifacts:", OUT)
    return 0 if ran else 1


if __name__ == "__main__":
    raise SystemExit(main())
