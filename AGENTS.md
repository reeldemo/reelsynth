# AGENTS.md — ReelSynth

Notes for Cursor agents and people hacking on this repo.

## What this is

MIT wavetable synth: Rust DSP, standalone egui app, PyO3, export CLI. VST3/CLAP plugin crate is separate (GPL when linking nih-plug). Studio agent work lives in `reeldemo-ableton` — [docs/REELDEMO_INTEGRATION.md](docs/REELDEMO_INTEGRATION.md).

## Doc map

| Audience | Start |
|----------|-------|
| Musicians | [docs/GETTING_STARTED.md](docs/GETTING_STARTED.md) → [docs/WORKFLOW.md](docs/WORKFLOW.md) |
| Free tools | [docs/FREE_STACK.md](docs/FREE_STACK.md) |
| Ableton | [docs/ABLETON.md](docs/ABLETON.md) |
| Developers | [docs/SDK.md](docs/SDK.md) |
| UI | [docs/UI.md](docs/UI.md) |
| Formats | [docs/FORMAT.md](docs/FORMAT.md), [docs/INTEROP.md](docs/INTEROP.md) |

Index: [docs/README.md](docs/README.md)

## Skills in this repo

| Skill | Path | When |
|-------|------|------|
| reelsynth-workflow | `.cursor/skills/reelsynth-workflow/SKILL.md` | How to use synth, export, DAW handoff |
| audit-reelsynth-ui | `.cursor/skills/audit-reelsynth-ui/SKILL.md` | Mockup parity / screenshot audit |

## Don’t mislead users

1. **Compose mode** — in-app clip edit, record, transport through the synth.
2. **`daw/midi/melody.mid`** — demo note until full SequenceProject SMF export.
3. **Plugin** — VST3 plays in Live with external Design UI; full UI inside Live’s pane is not the plan. Cite [docs/ABLETON.md](docs/ABLETON.md).
4. **Exports** to Vital/Serum/Ableton are lossy — [docs/INTEROP.md](docs/INTEROP.md).
5. **Canonical state** — `.reelpreset` + `.reelwt`; sequence data will embed in the patch schema.

## Build

```bash
cargo test
cargo run -p reelsynth-app --bin reelsynth-app
cargo run --bin reelsynth-export -- --help
maturin develop --features python
```

## Screenshots

Not in the repo. On [GitHub Releases](https://github.com/reeldemo/reelsynth/releases) tagged to the app version. Capture: [CONTRIBUTING.md](CONTRIBUTING.md). Docs use URLs like `…/releases/download/v0.1.0/<name>.png`.

## Layout

```
src/      DSP, export, import, ffi
app/      Standalone (cpal + midir)
ui/       Shared egui editor
plugin/   nih-plug VST3/CLAP + external editor
docs/     Musician + SDK docs
brand/    Design spec, mockups, audits
```

Sprint log: [brand/mockups/audits/IMPLEMENTATION_LOG.md](brand/mockups/audits/IMPLEMENTATION_LOG.md).

## Editing docs

- Keep musician vs developer tracks separate ([docs/README.md](docs/README.md)).
- Update CHANGELOG for user-visible doc or behavior changes.
- Re-capture release screenshots when header/center/WT layout changes.
- Don’t commit PNGs to main — release assets only.
