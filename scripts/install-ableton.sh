#!/usr/bin/env bash
# Build (optional) and install ReelSynth for Ableton Live on macOS.
# Linux + Ableton is not supported.
set -euo pipefail

SKIP_BUILD=0
REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-build) SKIP_BUILD=1; shift ;;
    --repo) REPO_ROOT="$2"; shift 2 ;;
    *) echo "Unknown arg: $1"; exit 1 ;;
  esac
done

cd "$REPO_ROOT"
echo "ReelSynth Ableton installer (macOS)"
echo "Repo: $REPO_ROOT"

if [[ "$(uname -s)" != "Darwin" ]]; then
  echo "This script is for macOS only. Windows: scripts/install-ableton.ps1" >&2
  echo "Ableton Live is not officially supported on Linux." >&2
  exit 1
fi

if [[ "$SKIP_BUILD" -eq 0 ]]; then
  echo "Building release plugin + editor..."
  cargo build -p reelsynth-plugin --release
fi

DLL="$REPO_ROOT/target/release/libreelsynth_plugin.dylib"
EDITOR_SRC="$REPO_ROOT/target/release/reelsynth-plugin-editor"
# cdylib name may be reelsynth_plugin.dylib depending on crate-type naming
if [[ ! -f "$DLL" ]]; then
  DLL="$REPO_ROOT/target/release/reelsynth_plugin.dylib"
fi
if [[ ! -f "$DLL" ]]; then
  echo "Missing plugin dylib under target/release" >&2
  exit 1
fi
if [[ ! -f "$EDITOR_SRC" ]]; then
  echo "Missing $EDITOR_SRC" >&2
  exit 1
fi

SUPPORT="$HOME/Library/Application Support/ReelSynth"
BIN="$SUPPORT/bin"
mkdir -p "$BIN"
EDITOR_DST="$BIN/reelsynth-plugin-editor"
cp -f "$EDITOR_SRC" "$EDITOR_DST"
chmod +x "$EDITOR_DST"
echo "Editor -> $EDITOR_DST"

CFG="$SUPPORT/config.json"
cat > "$CFG" <<EOF
{
  "schema": "reelsynth-ableton-config-v1",
  "auto_editor": true,
  "editor_path": "$EDITOR_DST"
}
EOF
echo "Config -> $CFG (auto_editor=true)"

VST_ROOT="/Library/Audio/Plug-Ins/VST3"
if [[ ! -w "$VST_ROOT" ]]; then
  VST_ROOT="$HOME/Library/Audio/Plug-Ins/VST3"
fi
mkdir -p "$VST_ROOT"
BUNDLE="$VST_ROOT/ReelSynth.vst3"
rm -rf "$BUNDLE"
mkdir -p "$BUNDLE/Contents/MacOS"
cp -f "$DLL" "$BUNDLE/Contents/MacOS/ReelSynth"
# Minimal Info.plist so Live can identify the bundle
cat > "$BUNDLE/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>English</string>
  <key>CFBundleExecutable</key><string>ReelSynth</string>
  <key>CFBundleIdentifier</key><string>xyz.reelsynth.vst3</string>
  <key>CFBundleName</key><string>ReelSynth</string>
  <key>CFBundlePackageType</key><string>BNDL</string>
  <key>CFBundleVersion</key><string>0.3.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST
# Ad-hoc sign: unsigned linker-signed dylib in a .vst3 bundle fails Live's scanner
# ("code has no resources but signature indicates they must be present").
codesign --force --deep --sign - "$BUNDLE"
codesign --verify --deep --strict "$BUNDLE"
echo "VST3  -> $BUNDLE"

echo ""
echo "Done. In Ableton Live:"
echo "  1. Preferences → Plug-ins → enable VST3 → Rescan"
echo "  2. Load ReelSynth on a MIDI track — editor should open automatically"
echo "  Manual editor: $EDITOR_DST"
echo "Note: unsigned builds may need right-click → Open the first time (Gatekeeper)."
