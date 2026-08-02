# ReelSynth installers Implementation Plan

> **For agentic workers:** Implement task-by-task. Steps use checkbox syntax.

**Goal:** Ship unsigned Mac `.pkg` + Windows NSIS installers (app + VST3 + Ableton) on GitHub Releases; website CTAs point at installers.

**Architecture:** Custom `scripts/package-macos.sh` + `scripts/package-windows.ps1` / `installer/windows/reelsynth.nsi`, wired into `.github/workflows/release.yml`. Archives kept; website uses installer `data-asset` names.

**Tech Stack:** pkgbuild/productbuild, NSIS, GitHub Actions, reeldemo.github.io

**Linear:** REE-5

## Tasks

- [x] Design spec `docs/superpowers/specs/2026-08-02-reelsynth-installers-design.md`
- [x] `scripts/package-macos.sh` + postinstall Ableton config
- [x] `installer/windows/reelsynth.nsi` + `scripts/package-windows.ps1`
- [x] Extend `release.yml` (full plugin build + installers)
- [x] Docs / CHANGELOG / version bump 0.4.2
- [x] Website download cards → `.pkg` / `-setup.exe`
- [ ] Push, tag `v0.4.2`, verify CI release assets
