# ReelSynth VST3/AU plugin scaffold (Phase 5)

JUCE wrapper planned to link the `reelsynth` Rust core as a static library. **Not required** for offline render, import, or Reeldemo agent integration — those use the PyO3 module or CLI today.

## Prerequisites (when wiring realtime)

| Tool | Purpose |
|------|---------|
| Rust toolchain | Build `reelsynth` static lib (`cargo build --release`) |
| CMake ≥ 3.22 | JUCE project generation |
| JUCE 7+ | VST3/AU shell (download or `git submodule`) |
| Corrosion or `cxx` | Rust ↔ C++ bridge (planned) |

macOS: Xcode command-line tools. Windows: Visual Studio 2022. Linux: `libasound2-dev`, `libfreetype6-dev`, `libx11-dev`.

## Planned layout

```
plugin/
  CMakeLists.txt      # JUCE + Corrosion/cargo fetch
  Source/
    PluginProcessor.h
    PluginProcessor.cpp
    ReelSynthVoice.h   # block-based voice (phase 5 — not implemented)
```

## Build steps (scaffold only — targets not linked yet)

```bash
# 1. Build Rust core (from repo root)
cargo build --release --no-default-features

# 2. Generate JUCE project (once CMakeLists wires FetchContent/Corrosion)
cmake -B plugin/build -DCMAKE_BUILD_TYPE=Release
cmake --build plugin/build --config Release

# 3. Install artifacts (paths vary by platform)
# macOS AU:  plugin/build/ReelSynth_artefacts/Release/AU/ReelSynth.component
# macOS VST3: plugin/build/ReelSynth_artefacts/Release/VST3/ReelSynth.vst3
```

## What works today

- **Offline render:** `maturin develop --features python` → `reelsynth.render_note_py`
- **CLI import:** `cargo run --bin gen_factory`
- **Export (parallel track):** `cargo run --bin reelsynth-export` when export module is merged

Realtime block processing needs a voice API refactor (polyphonic, per-block modulation). Phase 5 only; phases 1–4 do not require JUCE.

## Troubleshooting

- **Missing JUCE:** clone into `plugin/JUCE` or set `JUCE_PATH` when CMake is wired.
- **Rust link errors:** ensure `crate-type = ["staticlib", "rlib"]` is set before JUCE links the core.
- **Python vs plugin:** build separately — PyO3 `cdylib` and JUCE `staticlib` are different targets.
