"""Shared search-seed policy (Julian: ≥3 seeds, mean ± std on holdout)."""
from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Any

import torch

MIN_SEARCH_SEEDS = 3
DEFAULT_SEARCH_SEEDS = (1_902_771_841, 20_260_727_01, 20_260_727_02)


def load_config(path: Path) -> dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8"))


def resolve_search_seeds(cfg: dict[str, Any], override_csv: str) -> list[int]:
    if override_csv.strip():
        seeds = [int(s.strip()) for s in override_csv.split(",") if s.strip()]
    else:
        seeds = [int(s) for s in cfg.get("search_seeds", list(DEFAULT_SEARCH_SEEDS))]
    return seeds


def require_min_search_seeds(
    seeds: list[int],
    *,
    minimum: int | None = None,
    allow_fewer: bool = False,
    cfg: dict[str, Any] | None = None,
) -> None:
    if minimum is None:
        minimum = int((cfg or {}).get("min_search_seeds", MIN_SEARCH_SEEDS))
    if allow_fewer or len(seeds) >= minimum:
        return
    print(
        f"ERROR: need at least {minimum} search seeds (got {len(seeds)}: {seeds}). "
        f"Julian rule: experiments report holdout mean ± std across ≥3 search seeds.",
        file=sys.stderr,
    )
    raise SystemExit(2)


def mean_std(values: list[float]) -> tuple[float, float]:
    if not values:
        return float("nan"), float("nan")
    t = torch.tensor(values, dtype=torch.float64)
    std = float(t.std(unbiased=False).item()) if len(values) > 1 else 0.0
    return float(t.mean().item()), std


def format_mean_std(mean: float, std: float, digits: int = 5) -> str:
    return f"{mean:.{digits}f} ± {std:.{digits}f}"
