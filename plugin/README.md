# ReelSynth plugin (Rust + egui)

**UI:** [egui](https://github.com/emilk/egui) via shared `reelsynth/ui` crate — same editor as the standalone app. **No JUCE.**

**Host:** Custom Rust CLAP + VST3 + AU bindings (MIT). Planned for S6 public release.

## Status

| Item | State |
|------|--------|
| Standalone app | `app/` — egui + cpal + `SynthEngine` |
| Plugin shell | `plugin/` — CLAP entry stub + editor spike (`reelsynth-plugin-editor`) |
| JUCE CMake scaffold | **Retired** — do not use |

## Planned layout (S6)

```
plugin/
  Cargo.toml
  src/
    lib.rs
    clap_entry.rs
    vst3_entry.rs
    au_entry.rs          # macOS
    editor.rs            # egui editor (shared with app/)
```

## Build (when implemented)

```bash
cargo build -p reelsynth-plugin --release
```

Artifacts: `.clap`, `.vst3`, `.component` (AU) — see `docs/UI.md`.

## Offline / agent use (today)

No plugin required:

- PyO3: `maturin develop --features python`
- CLI export: `cargo run --bin reelsynth-export -- --help`
- Standalone UI: `cargo run -p reelsynth-app --bin reelsynth-ui`
