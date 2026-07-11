# JUCE VST3/AU plugin scaffold (Phase 5)

This directory will host a JUCE wrapper that links the `reelsynth` Rust core as a static library.

## Planned layout

```
plugin/
  CMakeLists.txt      # JUCE + Corrosion/cargo fetch
  Source/
    PluginProcessor.h
    PluginProcessor.cpp
    ReelSynthVoice.h   # calls reelsynth::render_note (realtime path TBD)
```

## Build (not yet wired)

```bash
# Future:
cmake -B build -DCMAKE_BUILD_TYPE=Release
cmake --build build
```

Realtime playback requires a block-based voice API (phase 5); offline render uses the PyO3 / CLI path today.
