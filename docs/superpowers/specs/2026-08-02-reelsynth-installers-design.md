# ReelSynth Mac + Windows installers — design

**Linear:** [REE-5](https://linear.app/reeldemoio/issue/REE-5/ship-mac-windows-installers-app-vst3)  
**Date:** 2026-08-02  
**Status:** Approved for implementation

## Goal

Ship familiar installers so users get standalone ReelSynth **and** Ableton-ready VST3 (plus external editor/config) without running install scripts by hand.

## Decisions

| Topic | Choice |
|-------|--------|
| Contents | Standalone app + VST3 + plugin editor + `auto_editor` config |
| Signing (v1) | Ad-hoc `codesign` on Mac bundles (required for Live VST3 scan); no Developer ID / notarization yet. Document Gatekeeper / SmartScreen |
| Mac format | `.pkg` (Installer.app wizard) |
| Windows format | NSIS `.exe` setup wizard |
| Win privileges | Prefer system VST3; fall back to per-user VST3 if no admin |
| Packaging | Custom scripts (`pkgbuild` / NSIS), not cargo-packager |
| Distribution | Website CTAs → installers; GitHub Releases also keep zip/tar.gz |
| Linux | Archive only (no Ableton installer) |

## Install map

### macOS (`.pkg`)

| Component | Destination |
|-----------|-------------|
| `ReelSynth.app` | `/Applications/ReelSynth.app` |
| VST3 | `/Library/Audio/Plug-Ins/VST3/ReelSynth.vst3` (user Library fallback if needed) |
| Editor + `config.json` | `~/Library/Application Support/ReelSynth/` (postinstall for console user) |
| CLI (optional component) | `/usr/local/bin/reelsynth-export` |

### Windows (NSIS)

| Component | Destination |
|-----------|-------------|
| App + Start Menu | `%ProgramFiles%\ReelSynth\` (or user Program Files if no admin) |
| VST3 | `%CommonProgramFiles%\VST3\ReelSynth.vst3`; fallback `%USERPROFILE%\Documents\VST3\` |
| Editor + config | `%LOCALAPPDATA%\ReelSynth\` |
| CLI (optional) | same install dir |

Finish page: quit Live if open → Rescan VST3 → load ReelSynth; unsigned note uses System Settings → Privacy & Security → Open Anyway (Installer HTML must avoid raw UTF-8 arrows — use HTML entities).


## Build & release

On tag `v*`:

1. `cargo build --release` for app, export, **plugin cdylib**, plugin editor  
2. Stage payload  
3. Mac → `scripts/package-macos.sh` → `reelsynth-<ver>-macos-<arch>.pkg`  
4. Win → `scripts/package-windows.ps1` + `installer/windows/reelsynth.nsi` → `reelsynth-<ver>-windows-x86_64-setup.exe`  
5. Keep existing zip/tar.gz archives  
6. Upload all to GitHub Release  

Signing hooks deferred (see Follow-up).

## Website

[reeldemo.github.io/reelsynth](https://reeldemo.github.io/reelsynth/) download cards point at installer assets (`.pkg` / `-setup.exe`); Linux stays tar.gz; “All release assets” still links GitHub for archives.

## Docs

- `GETTING_STARTED.md` — website installer first; cargo for developers  
- Unsigned install notes  
- Ableton: installer already placed plugin; Rescan only  
- Keep `scripts/install-ableton.*` for contributors  

## Follow-up: developer certificates

After unsigned v1:

**macOS:** Apple Developer Program → Developer ID Application (+ Installer if needed) → notarize with `notarytool` → staple → CI secrets.

**Windows:** Azure Trusted Signing **or** OV/EV CA cert → sign setup.exe in CI.

Spawn child issues from REE-5 when ready; keep unsigned artifacts until signed replace them.

## Out of scope (v1)

- Code signing / notarization  
- AU plugin  
- Universal macOS binary (ship per-arch pkgs)  
