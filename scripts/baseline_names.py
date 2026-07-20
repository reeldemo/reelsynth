"""Canonical names for DenoiseOpt baselines / controls.

Manuscript name: No-bake (passthrough)
Meaning: unrepaired cracked engine scored vs ideal sibling; f(x)=x; no seam op.
Legacy JSON/code key: identity (still accepted when reading frozen artifacts).
"""
from __future__ import annotations

from typing import Any, Mapping

NO_BAKE_KEY = "no_bake"
NO_BAKE_LEGACY_KEYS = ("identity", "noop", "no_op", "passthrough")
NO_BAKE_ALIASES = (NO_BAKE_KEY,) + NO_BAKE_LEGACY_KEYS
NO_BAKE_DISPLAY = "No-bake (passthrough)"


def is_no_bake(name: str | None) -> bool:
    if not name:
        return False
    return str(name).strip().lower().replace("-", "_") in {
        "no_bake",
        "nobake",
        "identity",
        "noop",
        "no_op",
        "passthrough",
    }


def display_method(name: str) -> str:
    if is_no_bake(name):
        return NO_BAKE_DISPLAY
    return name


def get_method_block(d: Mapping[str, Any], *preferred: str) -> Any | None:
    """Fetch a method sub-dict, accepting no_bake ↔ identity aliases."""
    keys = list(preferred) if preferred else []
    if any(is_no_bake(k) for k in keys) or not keys:
        for k in NO_BAKE_ALIASES:
            if k not in keys:
                keys.append(k)
    for k in keys:
        if k in d:
            return d[k]
    return None
