"""Genetic algorithm operators for DenoiseOpt meta outer loop.

Literature-informed naming (not claimed SOTA):
  - Tournament selection + crossover + mutation (classic GA / Real aging-evo spirit)
  - Interleaved with PPO mutation proposals in overnight_gpu_rl_arch.py (ERL-inspired)

Operates on ArchConfig / HyperParams dict-compatible objects from the overnight script.
"""
from __future__ import annotations

import random
from typing import Any, Callable, Sequence, TypeVar

import denoise_arch_blocks as _blocks  # live mutable caps (plateau adapt)

T = TypeVar("T")

MIN_DEPTH_BIAS = 3  # prefer at least this depth when residual holds


def __getattr__(name: str):
    """Expose live caps so `from denoise_meta_evo import MAX_SEARCH_DEPTH` stays correct."""
    if name == "MAX_SEARCH_DEPTH":
        return _blocks.MAX_SEARCH_DEPTH
    if name == "MAX_GRAPH_LEN":
        return _blocks.MAX_GRAPH_LEN
    if name == "MAX_WIDTH":
        return _blocks.MAX_WIDTH
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")


def tournament_select(
    pop: Sequence[T],
    scores: Sequence[float],
    rng: random.Random,
    k: int = 3,
) -> T:
    """Fitness-proportionate-ish tournament (higher score wins)."""
    assert len(pop) == len(scores) and len(pop) >= 1
    k = max(1, min(k, len(pop)))
    idxs = rng.sample(range(len(pop)), k=k)
    best = max(idxs, key=lambda i: scores[i])
    return pop[best]


def crossover_lists(a: list[Any], b: list[Any], rng: random.Random) -> list[Any]:
    """One-point / uniform hybrid on variable-length lists (ops or blocks)."""
    if not a and not b:
        return []
    if not a:
        return list(b)
    if not b:
        return list(a)
    if rng.random() < 0.5:
        # uniform mix
        pool = list(dict.fromkeys(list(a) + list(b)))
        k = rng.randint(1, max(1, min(len(pool), _blocks.MAX_GRAPH_LEN if len(pool) > 3 else len(pool))))
        return rng.sample(pool, k=k) if len(pool) >= k else pool
    # one-point on longer parent
    cut = rng.randint(0, min(len(a), len(b)))
    child = list(a[:cut]) + list(b[cut:])
    # dedupe preserve order
    out: list[Any] = []
    for x in child:
        if x not in out:
            out.append(x)
    return out[: _blocks.MAX_GRAPH_LEN] if out else list(a[:1] or b[:1])


def crossover_arch(
    cfg_a: Any,
    cfg_b: Any,
    rng: random.Random,
    *,
    ArchConfig: type,
    normalize_graph: Callable,
    ensure_trainable_ops: Callable,
    CELL_KINDS: Sequence[str],
    ACTS: Sequence[str],
) -> Any:
    """Crossover two ArchConfig-like objects → child ArchConfig."""
    da, db = cfg_a.to_dict(), cfg_b.to_dict()
    depth = int(round(0.5 * (da["depth"] + db["depth"])))
    if rng.random() < 0.35:
        depth = max(da["depth"], db["depth"])  # depth-preferring bias
    depth = max(1, min(_blocks.MAX_SEARCH_DEPTH, depth + rng.choice([0, 0, 1])))
    width = rng.choice([da["width"], db["width"], int(round(0.5 * (da["width"] + db["width"])))])
    width = max(4, min(_blocks.MAX_WIDTH, width))
    act = rng.choice([da["act"], db["act"]])
    if act not in ACTS:
        act = rng.choice(list(ACTS))
    ops = crossover_lists(list(da.get("ops") or []), list(db.get("ops") or []), rng)
    ops = ensure_trainable_ops(ops)
    wet = float(max(0.05, min(0.95, 0.5 * (da["wet"] + db["wet"]) + rng.uniform(-0.05, 0.05))))
    fir_a = list(da.get("fir") or [0.25, 0.5, 0.25])
    fir_b = list(db.get("fir") or [0.25, 0.5, 0.25])
    n_fir = max(len(fir_a), len(fir_b), 5)
    fir = []
    for i in range(n_fir):
        va = fir_a[i] if i < len(fir_a) else 0.1
        vb = fir_b[i] if i < len(fir_b) else 0.1
        fir.append(0.5 * (va + vb) + rng.uniform(-0.05, 0.05))
    cell = rng.choice([da["cell_kind"], db["cell_kind"]])
    if cell not in CELL_KINDS:
        cell = rng.choice(list(CELL_KINDS))
    soft_a = list(da.get("soft_logits") or [])
    soft_b = list(db.get("soft_logits") or [])
    n_soft = max(len(soft_a), len(soft_b))
    soft = []
    for i in range(n_soft):
        va = soft_a[i] if i < len(soft_a) else 0.0
        vb = soft_b[i] if i < len(soft_b) else 0.0
        soft.append(0.5 * (va + vb) + rng.uniform(-0.2, 0.2))
    blocks = crossover_lists(list(da.get("blocks") or []), list(db.get("blocks") or []), rng)
    blocks = normalize_graph(blocks, cell)
    use_adv = bool(rng.choice([da.get("use_adv_aux", False), db.get("use_adv_aux", False)]))
    moe_mode = rng.choice(
        [
            da.get("moe_mode", "sequential"),
            db.get("moe_mode", "sequential"),
            "moe_parallel",
        ]
    )
    return ArchConfig(
        depth=depth,
        width=width,
        act=act,
        ops=ops,
        wet=wet,
        fir=fir[:5],
        cell_kind=cell,
        soft_logits=soft,
        blocks=blocks,
        use_adv_aux=use_adv,
        moe_mode=moe_mode,
    )


def crossover_hp(hp_a: Any, hp_b: Any, rng: random.Random, *, HyperParams: type) -> Any:
    da, db = hp_a.to_dict(), hp_b.to_dict()
    return HyperParams(
        lr=float(10 ** (0.5 * (math_log10(da["lr"]) + math_log10(db["lr"])))),
        fit_steps=int(round(0.5 * (da["fit_steps"] + db["fit_steps"]))),
        batch=int(rng.choice([da["batch"], db["batch"]])),
        entropy_coef=float(0.5 * (da["entropy_coef"] + db["entropy_coef"])),
        ppo_clip=float(rng.choice([da["ppo_clip"], db["ppo_clip"]])),
        adv_coef=float(0.5 * (da["adv_coef"] + db["adv_coef"])),
    )


def math_log10(x: float) -> float:
    import math

    return math.log10(max(x, 1e-8))


def ga_generation(
    pop: list[Any],
    rng: random.Random,
    *,
    mutate_arch: Callable,
    mutate_hp: Callable,
    ArchConfig: type,
    HyperParams: type,
    normalize_graph: Callable,
    ensure_trainable_ops: Callable,
    CELL_KINDS: Sequence[str],
    ACTS: Sequence[str],
    n_actions: int,
    elite_frac: float = 0.2,
    crossover_frac: float = 0.5,
) -> dict[str, float]:
    """In-place GA step: elites kept; rest replaced by crossover+mutate offspring.

    Returns stats for logging (GA_CROSSOVER / GA_MUTATE counts).
    """
    ranked = sorted(pop, key=lambda ind: ind.score, reverse=True)
    n = len(ranked)
    n_elite = max(1, int(n * elite_frac))
    scores = [ind.score for ind in ranked]
    n_cross = 0
    n_mut = 0
    for i in range(n_elite, n):
        parent_a = tournament_select(ranked[: max(n_elite * 2, 2)], scores[: max(n_elite * 2, 2)], rng, k=3)
        parent_b = tournament_select(ranked[: max(n_elite * 2, 2)], scores[: max(n_elite * 2, 2)], rng, k=3)
        if rng.random() < crossover_frac:
            child_cfg = crossover_arch(
                parent_a.cfg,
                parent_b.cfg,
                rng,
                ArchConfig=ArchConfig,
                normalize_graph=normalize_graph,
                ensure_trainable_ops=ensure_trainable_ops,
                CELL_KINDS=CELL_KINDS,
                ACTS=ACTS,
            )
            child_hp = crossover_hp(parent_a.hp, parent_b.hp, rng, HyperParams=HyperParams)
            n_cross += 1
        else:
            child_cfg = ArchConfig(**parent_a.cfg.to_dict())
            child_hp = HyperParams(**parent_a.hp.to_dict())
        # Always mutate a bit (Real aging-evo spirit: explore after inherit)
        child_cfg = mutate_arch(child_cfg, rng.randrange(n_actions), rng)
        if rng.random() < 0.5:
            child_cfg = mutate_arch(child_cfg, rng.randrange(n_actions), rng)
        child_hp = mutate_hp(child_hp, rng)
        n_mut += 1
        victim = ranked[i]
        victim.cfg = child_cfg
        victim.hp = child_hp
        victim.age = 0
        victim.score = 0.5 * (parent_a.score + parent_b.score)
    return {"ga_crossover": float(n_cross), "ga_mutate": float(n_mut), "ga_elites": float(n_elite)}


def depth_mixture_bonus(
    residual: float,
    baseline: float,
    depth: int,
    n_blocks: int,
    moe_mode: str,
    *,
    hold_margin: float = 0.02,
) -> float:
    """Small additive bonus when residual holds above baseline — rewards depth & mixtures.

    Primary score remains residual; this is a light search bias only.
    """
    if residual < baseline + hold_margin:
        return 0.0
    depth_term = 0.002 * max(0, depth - (MIN_DEPTH_BIAS - 1)) / max(_blocks.MAX_SEARCH_DEPTH, 1)
    mix_term = 0.0015 * max(0, n_blocks - 1) / max(_blocks.MAX_GRAPH_LEN, 1)
    moe_term = 0.001 if moe_mode == "moe_parallel" and n_blocks >= 2 else 0.0
    return depth_term + mix_term + moe_term
