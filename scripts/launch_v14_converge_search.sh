#!/usr/bin/env bash
# v14 hybrid FitCell-to-plateau meta-search (resume-safe). See denoiseopt-paper REPRO.md §B.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
CONFIG="${CONFIG:-brand/artifacts/phase2_experiments/config.json}"
DEVICE="${DEVICE:-cuda}"
SEED="${SEED:-1902771841}"
OUT="brand/artifacts/meta_approach_compare_v14_converge"
mkdir -p "brand/artifacts"
exec python3 scripts/bench_meta_approaches_5k.py \
  --protocol plateau \
  --approaches hybrid_lstm \
  --seed "$SEED" \
  --out-dir "$OUT" \
  --device "$DEVICE" \
  --ckpt-every 25 \
  "$@"
