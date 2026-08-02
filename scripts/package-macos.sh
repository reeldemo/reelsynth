#!/usr/bin/env bash
# Stage a macOS .pkg: ReelSynth.app + VST3 + editor + Ableton config (postinstall).
# Usage: scripts/package-macos.sh [--version VER] [--target-dir DIR] [--arch ARCH] [--out DIR]
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION="$(awk -F'"' '/^version = / { print $2; exit }' "$ROOT/Cargo.toml")"
TARGET_DIR="$ROOT/target/release"
ARCH="$(uname -m)"
case "$ARCH" in arm64) ARCH="aarch64" ;; esac
OUT_DIR="$ROOT/dist"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --version) VERSION="$2"; shift 2 ;;
    --target-dir) TARGET_DIR="$2"; shift 2 ;;
    --arch) ARCH="$2"; shift 2 ;;
    --out) OUT_DIR="$2"; shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

APP_BIN="$TARGET_DIR/reelsynth-app"
EXPORT_BIN="$TARGET_DIR/reelsynth-export"
EDITOR_BIN="$TARGET_DIR/reelsynth-plugin-editor"
DLL="$TARGET_DIR/libreelsynth_plugin.dylib"
[[ -f "$DLL" ]] || DLL="$TARGET_DIR/reelsynth_plugin.dylib"

for f in "$APP_BIN" "$EXPORT_BIN" "$EDITOR_BIN" "$DLL"; do
  if [[ ! -f "$f" ]]; then
    echo "Missing required binary: $f" >&2
    echo "Build with: cargo build --release -p reelsynth-app -p reelsynth --bin reelsynth-export -p reelsynth-plugin" >&2
    exit 1
  fi
done

STAGE="$OUT_DIR/pkg-stage-macos-${ARCH}"
ROOTFS="$STAGE/root"
SCRIPTS="$STAGE/scripts"
PAYLOAD="$STAGE/payload"
rm -rf "$STAGE"
mkdir -p "$ROOTFS/Applications" \
  "$ROOTFS/Library/Audio/Plug-Ins/VST3" \
  "$ROOTFS/usr/local/bin" \
  "$SCRIPTS" \
  "$PAYLOAD"

# --- Standalone .app ---
APP="$ROOTFS/Applications/ReelSynth.app"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$APP_BIN" "$APP/Contents/MacOS/ReelSynth"
chmod +x "$APP/Contents/MacOS/ReelSynth"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>en</string>
  <key>CFBundleExecutable</key><string>ReelSynth</string>
  <key>CFBundleIdentifier</key><string>xyz.reelsynth.app</string>
  <key>CFBundleName</key><string>ReelSynth</string>
  <key>CFBundleDisplayName</key><string>ReelSynth</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# --- External editor .app (discovered by plugin) ---
EDITOR_APP="$ROOTFS/Applications/ReelSynth Editor.app"
mkdir -p "$EDITOR_APP/Contents/MacOS"
cp "$EDITOR_BIN" "$EDITOR_APP/Contents/MacOS/reelsynth-plugin-editor"
chmod +x "$EDITOR_APP/Contents/MacOS/reelsynth-plugin-editor"
cat > "$EDITOR_APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleExecutable</key><string>reelsynth-plugin-editor</string>
  <key>CFBundleIdentifier</key><string>xyz.reelsynth.editor</string>
  <key>CFBundleName</key><string>ReelSynth Editor</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>NSHighResolutionCapable</key><true/>
</dict>
</plist>
PLIST

# --- VST3 ---
BUNDLE="$ROOTFS/Library/Audio/Plug-Ins/VST3/ReelSynth.vst3"
mkdir -p "$BUNDLE/Contents/MacOS"
cp "$DLL" "$BUNDLE/Contents/MacOS/ReelSynth"
# Also drop editor beside plugin for discovery via current_exe parent
cp "$EDITOR_BIN" "$BUNDLE/Contents/MacOS/reelsynth-plugin-editor"
chmod +x "$BUNDLE/Contents/MacOS/ReelSynth" "$BUNDLE/Contents/MacOS/reelsynth-plugin-editor"
cat > "$BUNDLE/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleDevelopmentRegion</key><string>English</string>
  <key>CFBundleExecutable</key><string>ReelSynth</string>
  <key>CFBundleIdentifier</key><string>xyz.reelsynth.vst3</string>
  <key>CFBundleName</key><string>ReelSynth</string>
  <key>CFBundlePackageType</key><string>BNDL</string>
  <key>CFBundleShortVersionString</key><string>${VERSION}</string>
  <key>CFBundleVersion</key><string>${VERSION}</string>
</dict>
</plist>
PLIST

cp "$EXPORT_BIN" "$ROOTFS/usr/local/bin/reelsynth-export"
chmod +x "$ROOTFS/usr/local/bin/reelsynth-export"

# Payload copy of editor for postinstall into user Application Support
cp "$EDITOR_BIN" "$PAYLOAD/reelsynth-plugin-editor"
chmod +x "$PAYLOAD/reelsynth-plugin-editor"
# Bundle payload into the pkg root under a private path the postinstall can find
mkdir -p "$ROOTFS/Library/Application Support/ReelSynth/pkg-payload"
cp "$PAYLOAD/reelsynth-plugin-editor" \
  "$ROOTFS/Library/Application Support/ReelSynth/pkg-payload/reelsynth-plugin-editor"

cat > "$SCRIPTS/postinstall" <<'POST'
#!/bin/bash
set -euo pipefail
# Write Ableton auto_editor config for the logged-in console user.
CONSOLE_USER="$(stat -f%Su /dev/console 2>/dev/null || true)"
if [[ -z "${CONSOLE_USER}" || "${CONSOLE_USER}" == "root" ]]; then
  CONSOLE_USER="${USER:-}"
fi
if [[ -z "${CONSOLE_USER}" || "${CONSOLE_USER}" == "root" ]]; then
  echo "postinstall: no console user; skipping per-user Ableton config"
  exit 0
fi
HOME_DIR="$(dscl . -read "/Users/${CONSOLE_USER}" NFSHomeDirectory 2>/dev/null | awk '{print $2}')"
if [[ -z "${HOME_DIR}" || ! -d "${HOME_DIR}" ]]; then
  echo "postinstall: cannot resolve home for ${CONSOLE_USER}"
  exit 0
fi
SUPPORT="${HOME_DIR}/Library/Application Support/ReelSynth"
BIN="${SUPPORT}/bin"
mkdir -p "${BIN}"
EDITOR_SRC="/Library/Application Support/ReelSynth/pkg-payload/reelsynth-plugin-editor"
EDITOR_DST="${BIN}/reelsynth-plugin-editor"
if [[ -f "${EDITOR_SRC}" ]]; then
  cp -f "${EDITOR_SRC}" "${EDITOR_DST}"
  chmod +x "${EDITOR_DST}"
  chown -R "${CONSOLE_USER}:staff" "${SUPPORT}" || true
fi
# Prefer Applications editor path (stable across updates)
EDITOR_APP="/Applications/ReelSynth Editor.app/Contents/MacOS/reelsynth-plugin-editor"
if [[ -x "${EDITOR_APP}" ]]; then
  EDITOR_PATH="${EDITOR_APP}"
else
  EDITOR_PATH="${EDITOR_DST}"
fi
cat > "${SUPPORT}/config.json" <<EOF
{
  "schema": "reelsynth-ableton-config-v1",
  "auto_editor": true,
  "editor_path": "${EDITOR_PATH}"
}
EOF
chown "${CONSOLE_USER}:staff" "${SUPPORT}/config.json" || true
echo "Ableton config -> ${SUPPORT}/config.json"
exit 0
POST
chmod +x "$SCRIPTS/postinstall"

PKG_ID="xyz.reelsynth.installer"
mkdir -p "$OUT_DIR"
OUT_DIR_ABS="$(cd "$OUT_DIR" && pwd)"
COMPONENT="$OUT_DIR_ABS/reelsynth-${VERSION}-macos-${ARCH}-component.pkg"
FINAL_PKG="$OUT_DIR_ABS/reelsynth-${VERSION}-macos-${ARCH}.pkg"

pkgbuild \
  --root "$ROOTFS" \
  --scripts "$SCRIPTS" \
  --identifier "$PKG_ID" \
  --version "$VERSION" \
  --install-location "/" \
  "$COMPONENT"

# Wrap with a simple distribution for Installer.app welcome/conclusion
DIST_XML="$STAGE/distribution.xml"
cat > "$DIST_XML" <<XML
<?xml version="1.0" encoding="utf-8"?>
<installer-gui-script minSpecVersion="2">
  <title>ReelSynth ${VERSION}</title>
  <organization>xyz.reelsynth</organization>
  <domains enable_localSystem="true"/>
  <options customize="never" require-scripts="false" rootVolumeOnly="true"/>
  <welcome file="welcome.html" mime-type="text/html"/>
  <conclusion file="conclusion.html" mime-type="text/html"/>
  <pkg-ref id="${PKG_ID}"/>
  <choices-outline>
    <line choice="default"/>
  </choices-outline>
  <choice id="default" visible="false">
    <pkg-ref id="${PKG_ID}"/>
  </choice>
  <pkg-ref id="${PKG_ID}" version="${VERSION}" onConclusion="none">${COMPONENT##*/}</pkg-ref>
</installer-gui-script>
XML

cat > "$STAGE/welcome.html" <<HTML
<html><body style="font-family:-apple-system,sans-serif;font-size:13px;">
<h2>Install ReelSynth</h2>
<p>This installs the standalone app, the Ableton VST3 plugin, and the external editor so Live can open the full Design UI automatically.</p>
<p><b>Unsigned build:</b> the first time you open the app, right-click → Open if Gatekeeper blocks it.</p>
</body></html>
HTML

cat > "$STAGE/conclusion.html" <<HTML
<html><body style="font-family:-apple-system,sans-serif;font-size:13px;">
<h2>Installed</h2>
<ol>
<li>Quit Ableton Live if it was open.</li>
<li>Preferences → Plug-ins → enable VST3 → Rescan.</li>
<li>Load <b>ReelSynth</b> on a MIDI track — the editor should open automatically.</li>
</ol>
<p>Standalone app: <code>/Applications/ReelSynth.app</code></p>
</body></html>
HTML

# productbuild needs component beside distribution or use --package-path
cp "$COMPONENT" "$STAGE/"
( cd "$STAGE" && productbuild \
  --distribution "$DIST_XML" \
  --resources "$STAGE" \
  --package-path "$STAGE" \
  "$FINAL_PKG" )

rm -f "$COMPONENT"
echo "Created $FINAL_PKG"
