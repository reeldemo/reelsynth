#!/usr/bin/env bash
# Bundle annotated doc screenshots for GitHub Release upload.
# Usage: ./scripts/bundle-docs-images.sh [output_dir]
# Expects PNGs in ./docs-images-staging/ (see CONTRIBUTING.md).

set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
STAGING="${ROOT}/docs-images-staging"
OUT="${1:-${ROOT}/docs-images.zip}"

if [[ ! -d "$STAGING" ]]; then
  echo "Missing staging dir: $STAGING" >&2
  echo "Capture screenshots per CONTRIBUTING.md, then re-run." >&2
  exit 1
fi

cd "$STAGING"
zip -r "$OUT" . -x '*.DS_Store'
echo "Created $OUT ($(du -h "$OUT" | cut -f1))"
echo "Upload to GitHub Release tagged to Cargo.toml version."
