# ReelSynth plugin

Shared **egui** editor (`reelsynth/ui`) — same Design UI as the standalone app. No JUCE.

**Hosts:** nih-plug **VST3** (Ableton Live) and **CLAP** (other DAWs). Live does not load CLAP.

## License

- Core / standalone: **MIT**
- This crate: **GPL-3.0-or-later** when linking nih-plug VST3 (`vst3-sys`). Don’t relicense the whole tree.

## Status

| Piece | State |
|-------|--------|
| Standalone | `app/` — egui + cpal + engine + Send to Ableton |
| VST3 / CLAP | `plugin/` — nih-plug instrument; Live QA + polish ongoing |
| Host pane | Slim egui panel — **Open Editor** launches external Design UI |
| External editor | `reelsynth-plugin-editor` — full Design UI over localhost IPC |
| In-host full UI | Not the goal — pane stays slim |
| JUCE scaffold | Retired |

Ableton install (Win/macOS): [docs/ABLETON.md](../docs/ABLETON.md) → `scripts/install-ableton.ps1` / `.sh`.

## Without a plugin

- Python: `maturin develop --features python`
- CLI: `cargo run --bin reelsynth-export -- --help`
- App: `cargo run -p reelsynth-app --bin reelsynth-app`
- Ableton Send: header **Ableton** → inbox + optional AbletonOSC
