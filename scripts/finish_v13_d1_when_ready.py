#!/usr/bin/env python3
"""When D1 multi-seed search finishes, re-aggregate and print paper-ready numbers."""
from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SEARCH = ROOT / "brand" / "artifacts" / "meta_approach_compare_v13_rblend"
SEEDS = ["1902771841", "2026072701", "2026072702"]
APPROACHES = ["hybrid_lstm", "random", "cmaes", "tpe", "aging_evo", "reinforce"]


def main() -> int:
    missing = []
    for seed in SEEDS:
        for ap in APPROACHES:
            p = SEARCH / seed / ap / "summary.json"
            if not p.is_file():
                missing.append(str(p.relative_to(SEARCH)))
    if missing:
        print(f"INCOMPLETE: {len(missing)} summaries missing")
        for m in missing[:12]:
            print(" ", m)
        if len(missing) > 12:
            print(f"  ... +{len(missing)-12} more")
        return 2

    sys.path.insert(0, str(ROOT / "scripts"))
    import aggregate_multiseed_stats as agg

    return agg.main()


if __name__ == "__main__":
    raise SystemExit(main())
