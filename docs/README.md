# ReelSynth documentation

ReelSynth is an MIT-licensed wavetable synthesizer. This folder is the documentation index.

## Choose your path

| You are… | Start here |
|----------|------------|
| **New to the synth** — install, play, save your first sound | [GETTING_STARTED.md](GETTING_STARTED.md) |
| **Designing sounds + moving to a DAW** — MIDI, melody, export | [WORKFLOW.md](WORKFLOW.md) |
| **Using free tools only** — DAWs, Vital, no paid stack | [FREE_STACK.md](FREE_STACK.md) |
| **Using Reeldemo Studio** — agent compose → Ableton handoff | [REELDEMO_INTEGRATION.md](REELDEMO_INTEGRATION.md) |
| **Integrating in code** — Python, Rust, CLI, C FFI | [SDK.md](SDK.md) |
| **Automating the UI** — AgentSession + MCP tools | [AGENT_API.md](AGENT_API.md) |
| **Learning the UI** — regions, shortcuts, MIDI | [UI.md](UI.md) |

## Reference

| Topic | Doc |
|-------|-----|
| File formats (`.reelwt`, `.reelpreset`, `reelpack/`) | [FORMAT.md](FORMAT.md) |
| Export loss matrix (Vital, Serum, Ableton, …) | [INTEROP.md](INTEROP.md) |
| DenoiseOpt (label-free crackle denoise) | [WHITEPAPER_DENOISE_OPT.md](WHITEPAPER_DENOISE_OPT.md) · [arXiv-style paper](papers/denoise_opt/) |
| Serum `.fxp` byte layout | [SERUM_FXP.md](SERUM_FXP.md) |
| Code naming conventions | [NAMING.md](NAMING.md) |

## Screenshots

UI screenshots are **not stored in this repo** (keeps the clone lean). They ship as [GitHub Release assets](https://github.com/reeldemo/reelsynth/releases) tagged to the app version (e.g. `v0.1.0`).

Docs reference URLs like:

```
https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/01-full-window.png
```

See [CONTRIBUTING.md](../CONTRIBUTING.md) for capture and release upload steps.

## License

ReelSynth engine: **MIT**. Reeldemo Studio (agent compose, Ableton handoff) is commercial and documented separately in [REELDEMO_INTEGRATION.md](REELDEMO_INTEGRATION.md).

## Marketing docs

Go-to-market and landing-page funnel docs live in the [reeldemo.github.io](https://github.com/reeldemo/reeldemo.github.io/tree/main/docs) repo (`docs/GTM.md`), not here.
