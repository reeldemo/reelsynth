# ReelSynth

Open-source wavetable synthesizer engine (MIT). Powers offline rendering in [Reeldemo Studio](https://github.com/reeldemo/reeldemo-ableton) and targets VST3/AU in a future plugin build.

## Documentation

| I want to… | Read |
|------------|------|
| Install and play my first sound | [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) |
| Compose a melody and use it in a DAW | [docs/WORKFLOW.md](docs/WORKFLOW.md) |
| Use only free tools (Vital, LMMS, …) | [docs/FREE_STACK.md](docs/FREE_STACK.md) |
| Learn the UI | [docs/UI.md](docs/UI.md) |
| Integrate in code (Rust, Python, CLI) | [docs/SDK.md](docs/SDK.md) |
| Use with Reeldemo Studio + Ableton | [docs/REELDEMO_INTEGRATION.md](docs/REELDEMO_INTEGRATION.md) |

Full index: [docs/README.md](docs/README.md)

## Capability matrix (v0.1)

| Works today | Not yet |
|-------------|---------|
| Standalone app — live MIDI, piano, QWERTY | VST3 / AU / CLAP plugin (S7) |
| Save/load `.reelpreset` + `.reelwt` | In-app MIDI recording |
| Export to Vital, Serum, Ableton JSON, SFZ, `reelpack/` | Export your live performance as MIDI |
| Python + CLI offline render | |

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

# Standalone playable app
cargo run -p reelsynth-app --bin reelsynth-app

# Export CLI
cargo run --bin reelsynth-export -- --help

# Python wheel (PyO3)
maturin develop --features python
```

Keyboard: **Z S X D C V G B H N J M** (one octave) or click the on-screen piano. MIDI controller via header dropdown.

## Quick export

```bash
cargo run --bin reelsynth-export -- reelpack my_patch.reelpreset -o out/ \
  --targets vital,wav,serum,ableton,sfz,midi,audio
```

See [docs/WORKFLOW.md](docs/WORKFLOW.md) for the full DAW handoff workflow.

## Python API

```python
import reelsynth

audio = reelsynth.render_note_py(
    bank_path="data/wavetables/saw_morph.reelwt",
    freq=440.0,
    duration=1.0,
    patch_json='{"oscillators":[{"level":1.0,"position":0.0}],"filter":{"cutoff":1200}}',
    sample_rate=44100,
)
```

Full API: [docs/SDK.md](docs/SDK.md)

## Plugin (S6 shell)

Rust CLAP/VST3/AU + **egui** editor — see [plugin/README.md](plugin/README.md). UI spike only; no host audio/MIDI yet (S7).

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md) · [AGENTS.md](AGENTS.md) · [CHANGELOG.md](CHANGELOG.md)

## Brand

Visual identity via [Majico](https://github.com/cap-jmk-launchpad/majico.xyz) — see [brand/BRAND.md](brand/BRAND.md) and [brand/MAJICO.md](brand/MAJICO.md).

## License

MIT — see [LICENSE](LICENSE). Reeldemo agent, text-to-wavetable, and compose integration are commercial and live in the Reeldemo repo.
