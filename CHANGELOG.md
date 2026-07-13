# Changelog

All notable changes to ReelSynth are documented here.

Format based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- User documentation pack: GETTING_STARTED, UI, WORKFLOW, FREE_STACK, SDK, REELDEMO_INTEGRATION
- [docs/README.md](docs/README.md) documentation index
- [AGENTS.md](AGENTS.md), [CONTRIBUTING.md](CONTRIBUTING.md) for agents and contributors
- `.cursor/skills/reelsynth-workflow/` — workflow skill for Cursor agents
- `scripts/bundle-docs-images.sh` — zip screenshots for GitHub Release upload
- Screenshot URLs via GitHub Release assets (not committed to repo)

### Changed

- README expanded with doc links and capability matrix

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
