#!/usr/bin/env bash
# ReelSynth release helper — stage locally, publish on explicit approval.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

VERSION="$(awk -F'"' '/^version = / { print $2; exit }' Cargo.toml)"
TARGET="${CARGO_BUILD_TARGET:-$(rustc -vV | awk '/host: / { print $2 }')}"
OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
case "$OS" in
  darwin) OS="macos" ;;
esac
ARCH="$(uname -m)"
case "$ARCH" in
  arm64) ARCH="aarch64" ;;
esac
DIST="$ROOT/dist"
STAGE="$DIST/reelsynth-${VERSION}-${OS}-${ARCH}"
ARCHIVE="$DIST/reelsynth-${VERSION}-${OS}-${ARCH}.zip"

usage() {
  cat <<EOF
Usage: $(basename "$0") <command>

Commands:
  stage     Build release binaries and package dist/ artifacts (no publish)
  publish   Create GitHub release from staged artifacts (requires gh CLI)
  info      Print version, target triple, and artifact paths

Environment:
  VERSION   Override version (default: $VERSION from Cargo.toml)
  SKIP_BUILD=1   Skip cargo build (use existing target/release binaries)
EOF
}

build_release() {
  echo "==> Building release binaries (target: $TARGET)"
  cargo build --release -p reelsynth-app -p reelsynth --bin reelsynth-export
  cargo build --release -p reelsynth-plugin --bin reelsynth-plugin-editor 2>/dev/null || \
    echo "    (plugin editor optional — skipped if unavailable)"
}

stage() {
  if [[ "${SKIP_BUILD:-0}" != "1" ]]; then
    build_release
  fi

  rm -rf "$STAGE"
  mkdir -p "$STAGE/bin"

  cp "$ROOT/target/release/reelsynth-app" "$STAGE/bin/"
  cp "$ROOT/target/release/reelsynth-export" "$STAGE/bin/"
  cp "$ROOT/README.md" "$ROOT/LICENSE" "$STAGE/"

  if [[ -f "$ROOT/target/release/reelsynth-plugin-editor" ]]; then
    cp "$ROOT/target/release/reelsynth-plugin-editor" "$STAGE/bin/"
  fi

  cat >"$STAGE/RELEASE_NOTES.md" <<EOF
# ReelSynth v${VERSION}

## Standalone app
\`\`\`bash
./bin/reelsynth-app
\`\`\`

Keyboard: **Z S X D C V G B H N J M** (one octave) or click the on-screen piano.

## CLI export
\`\`\`bash
./bin/reelsynth-export --help
\`\`\`

## Platform
- OS: ${OS}
- Arch: ${ARCH}
- Rust target: ${TARGET}
EOF

  rm -f "$ARCHIVE"
  (cd "$DIST" && zip -qr "$(basename "$ARCHIVE")" "$(basename "$STAGE")")

  echo ""
  echo "Staged:"
  echo "  $STAGE"
  echo "  $ARCHIVE"
  ls -lh "$ARCHIVE"
}

publish() {
  if [[ ! -f "$ARCHIVE" ]]; then
    echo "No staged archive at $ARCHIVE — run: $(basename "$0") stage" >&2
    exit 1
  fi

  if ! command -v gh >/dev/null; then
    echo "gh CLI required for publish" >&2
    exit 1
  fi

  TAG="v${VERSION}"
  if gh release view "$TAG" >/dev/null 2>&1; then
    echo "Release $TAG already exists. Delete it first or bump VERSION." >&2
    exit 1
  fi

  gh release create "$TAG" \
    --title "ReelSynth ${VERSION}" \
    --notes-file "$STAGE/RELEASE_NOTES.md" \
    "$ARCHIVE"

  echo "Published: https://github.com/reeldemo/reelsynth/releases/tag/${TAG}"
}

info() {
  echo "version:  $VERSION"
  echo "target:   $TARGET"
  echo "stage:    $STAGE"
  echo "archive:  $ARCHIVE"
}

cmd="${1:-}"
case "$cmd" in
  stage) stage ;;
  publish) publish ;;
  info) info ;;
  -h|--help|help|"") usage ;;
  *) echo "Unknown command: $cmd" >&2; usage; exit 1 ;;
esac
