# Changelog

All notable changes to ReelSynth are documented here.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Fixed

- **Held-note crackle** — widen VA/WT BLEP so saw/stack wrap cliffs are no longer near-vertical (was ~0.98 sample jump at A4); regressions cover Factory Lead mid/late sustain with FX. Bright saw overtones remain intentional; unintended wrap clicks are suppressed.
- **Design Result curve** — stack Result drawn only on the left 2D pane (distinct fill); right pane is layers-only; individual layer curves drag on both panes (Y=level, X=phase/WT)
- **Quant knobs** — dots snap to the selected layer curve (proximity hit + level/sign scale) on **both** Design panes; quantized edit polyline drawn through knobs for intuitive shaping
- **Right Layers pane** — follows selection (`Edit · Layer N · type`); selected curve gets fill + emphasis; dim siblings; Quant reshape when the selected layer is wavetable

### Changed

- Cleared workspace `cargo check` warnings (`-D warnings` clean for reelsynth / reelsynth-ui / reelsynth-app); Cursor **beforeShellExecution** hook blocks `git push` until compile stays clean
- **Settings** moved from floating modal window to a **Settings** dropdown in the top header navbar
- Removed Design pane animations (ambient waves, phase playhead scrub, idle repaint loops)
- README expanded with doc links and capability matrix
- **Design ↔ Compose** mode switch in header; Compose hides WT editor and osc column
- On-screen piano upgraded to **88 keys (A0–C8)** with horizontal scroll and scale-fold dimming

### Added

- `.cursor/hooks/` — `require-clean-compile.js` gates agent `git push` on `cargo check` with warnings denied
- **Compose mode** — header toggle switches from Design (sound engineering) to a mini-DAW layout: transport bar, multi-track arrangement, piano roll editor, scene grid, 88-key keyboard strip
- **Ableton-style clip editor** — thin clip strip + dominant piano roll; playable key column with QWERTY glyphs; unified live audition (keys / QWERTY / MIDI / pencil); transport ▶ voices scheduled notes; scenes collapsed by default
- User documentation pack: GETTING_STARTED, UI, WORKFLOW, FREE_STACK, SDK, REELDEMO_INTEGRATION
- [docs/README.md](docs/README.md) documentation index
- [AGENTS.md](AGENTS.md), [CONTRIBUTING.md](CONTRIBUTING.md) for agents and contributors
- `.cursor/skills/reelsynth-workflow/` — workflow skill for Cursor agents
- `scripts/bundle-docs-images.sh` — zip screenshots for GitHub Release upload
- Screenshot URLs via GitHub Release assets (not committed to repo)

## [0.1.0] - 2026-07-12

### Added

- Standalone egui app with realtime audio (cpal) and MIDI input (midir)
- Wavetable voice, filter, ADSR, LFO, mod matrix, FX chain
- `.reelwt` / `.reelpreset` native formats
- Import: Vital, WAV folder, Serum WT subset
- Export CLI: Vital, WAV, Serum, Ableton JSON, SFZ, MIDI, audio, reelpack
- Python PyO3 bindings for render and export
- Plugin UI shell (CLAP entry stub, no host I/O)

[Unreleased]: https://github.com/reeldemo/reelsynth/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/reeldemo/reelsynth/releases/tag/v0.1.0
