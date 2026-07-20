# AGENTS.md — ReelSynth

Guidance for Cursor agents and contributors working in this repo.

## What this repo is

MIT wavetable synthesizer: Rust DSP core, standalone egui app, Python/PyO3 bindings, export CLI. **Not** a loadable DAW plugin yet (S7 roadmap).

Commercial Reeldemo Studio integration lives in `reeldemo-ableton` — see [docs/REELDEMO_INTEGRATION.md](docs/REELDEMO_INTEGRATION.md).

## Doc map

| Audience | Start |
|----------|-------|
| Musicians | [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) → [docs/WORKFLOW.md](docs/WORKFLOW.md) |
| Free tools | [docs/FREE_STACK.md](docs/FREE_STACK.md) |
| Developers | [docs/SDK.md](docs/SDK.md) |
| UI layout | [docs/UI.md](docs/UI.md) |
| Formats | [docs/FORMAT.md](docs/FORMAT.md), [docs/INTEROP.md](docs/INTEROP.md) |

Index: [docs/README.md](docs/README.md)

## Agent skills in this repo

| Skill | Path | Use when |
|-------|------|----------|
| **reelsynth-workflow** | `.cursor/skills/reelsynth-workflow/SKILL.md` | User asks how to use synth, export, DAW handoff |
| **audit-reelsynth-ui** | `.cursor/skills/audit-reelsynth-ui/SKILL.md` | Visual parity vs mockups, screenshot audit |

## Hard constraints (do not mislead users)

1. **Compose mode** — in-app MIDI clip editing, recording, and transport playback of scheduled clip notes through the synth.
2. **Export `daw/midi/melody.mid`** — demo note until full `SequenceProject` SMF export lands.
3. **Plugin is UI-only** — no host audio/MIDI I/O until S7.
4. **Exports to Vital/Serum/Ableton are lossy** — cite [docs/INTEROP.md](docs/INTEROP.md).
5. **Canonical state** is `.reelpreset` + `.reelwt` — sequence data will embed in patch schema.

## Build commands

```bash
cargo test                          # core tests
cargo run -p reelsynth-app --bin reelsynth-app   # standalone
cargo run --bin reelsynth-export -- --help       # CLI
maturin develop --features python   # Python wheel
```

## Screenshot / release assets

- UI screenshots are **not** in the repo.
- Stored on [GitHub Releases](https://github.com/reeldemo/reelsynth/releases) tagged to `Cargo.toml` version.
- Capture process: [CONTRIBUTING.md](CONTRIBUTING.md)
- Docs reference: `https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/<name>.png`

## Code layout

```
src/           # DSP engine, export, import, ffi
app/           # Standalone (cpal + midir)
ui/            # egui editor (shared with plugin)
plugin/        # CLAP stub + editor spike
docs/          # User + SDK documentation
brand/         # Design spec, mockups, audits
```

## Sprint status

See [brand/mockups/audits/IMPLEMENTATION_LOG.md](brand/mockups/audits/IMPLEMENTATION_LOG.md). S6 plugin shell done; S7 host bindings next.

## When editing docs

- Keep musician and developer tracks separate ([docs/README.md](docs/README.md)).
- Update CHANGELOG.md for user-visible doc or behavior changes.
- Re-capture release screenshots when UI layout changes (header, center, WT editor).
- Do not commit PNGs to main — use release assets.
