# Docs

MIT wavetable synth. Pick a starting point:

| If you want… | Open |
|--------------|------|
| Install, play, save a first sound | [GETTING_STARTED.md](GETTING_STARTED.md) |
| Melody + export into a DAW | [WORKFLOW.md](WORKFLOW.md) |
| Free DAWs / Vital only | [FREE_STACK.md](FREE_STACK.md) |
| Ableton Live (Send + VST3) | [ABLETON.md](ABLETON.md) |
| Reeldemo Studio → Ableton | [REELDEMO_INTEGRATION.md](REELDEMO_INTEGRATION.md) |
| Code (Python, Rust, CLI, FFI) | [SDK.md](SDK.md) |
| Headless UI / MCP | [AGENT_API.md](AGENT_API.md) |
| Regions, shortcuts, ReelAI | [UI.md](UI.md) |

## Reference

| Topic | Doc |
|-------|-----|
| `.reelwt`, `.reelpreset`, `reelpack/` | [FORMAT.md](FORMAT.md) |
| What export drops (Vital, Serum, Ableton, …) | [INTEROP.md](INTEROP.md) |
| ReelAI / wrap crackle | [UI.md § ReelAI](UI.md#reelai-seam-heal), [GETTING_STARTED.md](GETTING_STARTED.md#clean-wrap-crackle-with-reelai), [WHITEPAPER_DENOISE_OPT.md](WHITEPAPER_DENOISE_OPT.md), [paper notes](papers/denoise_opt/) |
| Serum `.fxp` bytes | [SERUM_FXP.md](SERUM_FXP.md) |
| Code naming | [NAMING.md](NAMING.md) |

## Screenshots

Not in the git tree — they live on [GitHub Releases](https://github.com/reeldemo/reelsynth/releases) next to the app version (e.g. `v0.1.0`):

```
https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/01-full-window.png
```

How to capture/upload: [CONTRIBUTING.md](../CONTRIBUTING.md).

## License note

Engine: MIT. Studio (agent compose, Ableton handoff) is commercial — [REELDEMO_INTEGRATION.md](REELDEMO_INTEGRATION.md).

GTM / landing funnel docs sit in [reeldemo.github.io](https://github.com/reeldemo/reeldemo.github.io/tree/main/docs), not here.
