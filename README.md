# ReelSynth

MIT wavetable synth — Rust DSP, standalone egui app, Python bindings, export CLI. Also powers offline render in [Reeldemo Studio](https://github.com/reeldemo/reeldemo-ableton).

## Docs

| Goal | Doc |
|------|-----|
| Install and play | [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) |
| Melody + DAW handoff | [docs/WORKFLOW.md](docs/WORKFLOW.md) |
| Free tools only | [docs/FREE_STACK.md](docs/FREE_STACK.md) |
| Ableton Live | [docs/ABLETON.md](docs/ABLETON.md) |
| UI map | [docs/UI.md](docs/UI.md) |
| Rust / Python / CLI | [docs/SDK.md](docs/SDK.md) |
| Reeldemo Studio | [docs/REELDEMO_INTEGRATION.md](docs/REELDEMO_INTEGRATION.md) |

Index: [docs/README.md](docs/README.md)

## Download

Installers (recommended): [reeldemo.github.io/reelsynth](https://reeldemo.github.io/reelsynth/#download) — macOS `.pkg` / Windows setup `.exe` (app + VST3 + Ableton editor). Archives also on [GitHub Releases](https://github.com/reeldemo/reelsynth/releases/latest).

## What works (roughly v0.4.6)

| Yes | Not yet / partial |
|-----|-------------------|
| Standalone — MIDI, piano, QWERTY, Compose clips | Full SMF export of your Compose performance |
| Save/load `.reelpreset` + `.reelwt` | Perfect 1:1 Vital/Serum/Ableton round-trip |
| Export Vital, Serum, Ableton, SFZ, `reelpack/` | — |
| VST3 in Live (Win/macOS) + external Design UI | Full editor *inside* Live’s tiny pane |
| Unsigned Mac `.pkg` / Windows NSIS installers | Signed / notarized installers · AU |
| Python + CLI offline render | — |

## Features (engine)

- **WavetableBank** — 256 × 2048, linear + spectral crossfade
- **Voice** — multi-osc, ADSR, SVF, mod matrix
- **Import** — Vital `.vitaltable`, WAV cycle folders, Serum `.fxp` (WT subset)
- **Export** — Vital, WAV frames, Serum RSWT, Ableton map, SFZ, MIDI demo note, 24-bit stem, `reelpack/`
- **Formats** — [docs/FORMAT.md](docs/FORMAT.md)

## Build

```bash
cargo test
cargo run -p reelsynth-app --bin reelsynth-app
cargo run --bin reelsynth-export -- --help
maturin develop --features python
```

Keys: **Z S X D C V G B H N J M**, or the on-screen piano. MIDI via the header dropdown.

Ableton (Win/macOS): [docs/ABLETON.md](docs/ABLETON.md) — `scripts/install-ableton.ps1` / `.sh`.

## Quick export

```bash
cargo run --bin reelsynth-export -- reelpack my_patch.reelpreset -o out/ \
  --targets vital,wav,serum,ableton,sfz,midi,audio
```

DAW steps: [docs/WORKFLOW.md](docs/WORKFLOW.md).

## Python

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

## Plugin

nih-plug **VST3** (+ CLAP for other hosts). Live uses VST3; Design UI opens in a separate window over IPC. Details: [plugin/README.md](plugin/README.md), [docs/ABLETON.md](docs/ABLETON.md).

Plugin crate is GPL when linking VST3; core engine stays MIT.

## Contributing

[CONTRIBUTING.md](CONTRIBUTING.md) · [AGENTS.md](AGENTS.md) · [CHANGELOG.md](CHANGELOG.md)

## Brand

[Majico](https://github.com/cap-jmk-launchpad/majico.xyz) — [brand/BRAND.md](brand/BRAND.md), [brand/MAJICO.md](brand/MAJICO.md).

## License

MIT — [LICENSE](LICENSE). Agent compose / text-to-wavetable / Studio live in the Reeldemo commercial repo.
