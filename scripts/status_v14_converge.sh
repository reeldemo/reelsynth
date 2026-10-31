#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="${OUT:-$ROOT/brand/artifacts/meta_approach_compare_v14_converge}"
CKPT="$OUT/hybrid_lstm/checkpoint.json"
SUMMARY="$OUT/hybrid_lstm/summary.json"
PIDF="$OUT/bench.pid"
echo "v14 converge status: $OUT"
if [[ -f "$CKPT" ]]; then
  python3 - <<PY
import json
from pathlib import Path
ck = json.loads(Path("$CKPT").read_text())
print(f"  iters_done={ck.get('iters_done')} champ_R={ck.get('champ_raw', ck.get('champ_r'))}")
PY
else
  echo "  (no checkpoint yet)"
fi
if [[ -f "$SUMMARY" ]]; then
  echo "  summary: $SUMMARY"
fi
if [[ -f "$PIDF" ]]; then
  pid="$(cat "$PIDF")"
  if kill -0 "$pid" 2>/dev/null; then
    echo "  bench.pid=$pid (running)"
  else
    echo "  bench.pid=$pid (stale)"
  fi
fi
