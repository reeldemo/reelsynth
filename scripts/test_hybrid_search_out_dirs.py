#!/usr/bin/env python3
"""Plateau hybrid search must use per-seed out dirs (holdout_eval layout)."""
from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))

from phase2_experiments.run_hybrid_search import hybrid_search_out_dir  # noqa: E402


def _skip_if_summary_exists(out_dir: Path) -> bool:
    return (out_dir / "hybrid_lstm" / "summary.json").is_file()


def test_plateau_seeds_use_distinct_out_dirs() -> None:
    cfg = json.loads(
        (ROOT / "brand/artifacts/phase2_experiments/config.json").read_text(encoding="utf-8")
    )
    plateau_seeds = [int(s) for s in cfg["gpu_plateau_search_seeds"][:2]]
    assert len(plateau_seeds) == 2
    a, b = plateau_seeds
    out_a = hybrid_search_out_dir(a, protocol="plateau", root=ROOT)
    out_b = hybrid_search_out_dir(b, protocol="plateau", root=ROOT)
    assert out_a != out_b
    assert out_a.name == f"search_{a}"
    assert out_b.name == f"search_{b}"
    holdout_base = ROOT / "brand/artifacts/phase2_experiments"
    assert out_a == holdout_base / f"search_{a}"
    assert out_b == holdout_base / f"search_{b}"


def test_first_seed_summary_does_not_skip_second_seed(tmp_path: Path) -> None:
    """Old shared meta_approach_compare_v14_converge dir made seed2 skip after seed1."""
    seed1, seed2 = 2026072701, 2026072702
    legacy_shared = tmp_path / "meta_approach_compare_v14_converge"
    (legacy_shared / "hybrid_lstm").mkdir(parents=True)
    (legacy_shared / "hybrid_lstm" / "summary.json").write_text("{}", encoding="utf-8")

    def legacy_out(_seed: int) -> Path:
        return legacy_shared

    assert _skip_if_summary_exists(legacy_out(seed2)), "old layout: seed2 wrongly skips"

    out1 = hybrid_search_out_dir(seed1, protocol="plateau", root=tmp_path)
    out2 = hybrid_search_out_dir(seed2, protocol="plateau", root=tmp_path)
    (out1 / "hybrid_lstm").mkdir(parents=True)
    (out1 / "hybrid_lstm" / "summary.json").write_text("{}", encoding="utf-8")

    assert _skip_if_summary_exists(out1)
    assert not _skip_if_summary_exists(out2), "seed2 must still run after seed1 finishes"


def main() -> int:
    test_plateau_seeds_use_distinct_out_dirs()
    import tempfile

    with tempfile.TemporaryDirectory() as td:
        test_first_seed_summary_does_not_skip_second_seed(Path(td))
    print("OK test_hybrid_search_out_dirs")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
