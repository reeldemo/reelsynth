# ReelSynth

Open-source wavetable synthesizer engine (MIT). Powers offline rendering in [Reeldemo](https://github.com/cap-jmk-launchpad/reeldemo-ableton) and targets VST3/AU in a future plugin build.

## Features

- **WavetableBank** — 256 frames × 2048 samples, linear + spectral crossfade
- **Voice** — multi-osc wavetable, ADSR, state-variable filter, modulation matrix
- **Import** — Vital `.vitaltable`, WAV single-cycle folders, Serum `.fxp` (wavetable subset v1)
- **Export** — Vital, WAV frames, Serum RSWT, Ableton param JSON, SFZ, MIDI, 24-bit audio, `reelpack/` bundle
- **Formats** — `.reelwt` (binary bank), `.reelpreset` (JSON patch) — see [docs/FORMAT.md](docs/FORMAT.md)

## Build

```bash
# Rust library + tests
cargo test

# Export CLI
cargo run --bin reelsynth-export -- --help

# Python wheel (PyO3)
maturin develop --features python
```

## Python API

```python
import reelsynth

audio = reelsynth.render_note(
    bank_path="data/wavetables/saw_morph.reelwt",
    freq=440.0,
    duration=1.0,
    patch_json='{"oscillators":[{"level":1.0,"position":0.0}],"filter":{"cutoff":1200}}',
    sample_rate=44100,
)
```

## CLI import (via Reeldemo wrapper)

```bash
python -m engine.reelsynth_import vital path/to/table.vitaltable
python -m engine.reelsynth_import wav path/to/cycles/
python -m engine.reelsynth_import serum path/to/patch.fxp
```

## Brand

Visual identity via [Majico](https://github.com/cap-jmk-launchpad/majico.xyz) — see [brand/BRAND.md](brand/BRAND.md) and [brand/MAJICO.md](brand/MAJICO.md).

Theme preview (requires display):

```bash
cargo run -p reelsynth-ui-theme --example smoke
```

## Plugin (Phase 5)

JUCE/VST3 scaffold lives in `plugin/` — shares the Rust core via static lib linkage (not yet wired).

## License

MIT — see [LICENSE](LICENSE). Reeldemo agent, text-to-wavetable, and compose integration are commercial and live in the Reeldemo repo.
