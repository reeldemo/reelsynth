# ReelSynth plugin (Rust + egui)

**UI:** [egui](https://github.com/emilk/egui) via shared `reelsynth/ui` crate — same editor as the standalone app. **No JUCE.**

**Host:** S7 target — **nih-plug** VST3 (+ CLAP for other DAWs). Ableton Live requires **VST3** (not CLAP).

## License wall

- Core `reelsynth` / standalone app: **MIT**
- This `reelsynth-plugin` crate: becomes **GPL-3.0-or-later** when linking nih-plug VST3 (`vst3-sys`). Do not relicense the whole tree.

## Status

| Item | State |
|------|-------|
| Standalone app | `app/` — egui + cpal + `SynthEngine` + **Send to Ableton** bridge |
| Plugin shell | `plugin/` — CLAP entry stub + editor spike (`reelsynth-plugin-editor`) |
| nih-plug VST3/CLAP instrument | **S7 — in progress** (see `docs/sdd/specs/ableton-live-integration/`) |
| JUCE CMake scaffold | **Retired** — do not use |

## Offline / agent use (today)

No plugin required:

- PyO3: `maturin develop --features python`
- CLI export: `cargo run --bin reelsynth-export -- --help`
- Standalone UI: `cargo run -p reelsynth-app --bin reelsynth-app`
- Ableton bridge: header **Ableton** → inbox + optional AbletonOSC
