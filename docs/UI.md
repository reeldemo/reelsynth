# ReelSynth UI reference

The standalone app (`reelsynth-app`) uses a fixed **1280×880** layout (see `ui/src/layout.rs`). Regions match [brand/mockups/COMPONENT_SPEC.md](../brand/mockups/COMPONENT_SPEC.md).

![Full window with numbered regions](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/01-full-window.png)

| # | Region | Purpose |
|---|--------|---------|
| 1 | Header | Open/Save preset, WT menu, MIDI device, Piano toggle, status |
| 2 | Oscillator column (left rail) | Per-osc level, pan, detune, unison, FM |
| 3 | Center column | Filter, ADSR, LFO, mod matrix, FX rack |
| 4 | WT editor | Position strip, 2D waveform, 3D surface, morph A/B |
| 5 | Scope strip | Live osc → filter → FX → out |
| 6 | Piano (optional) | On-screen keyboard, 3 octaves from C3 |

---

## Header

![Header detail](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/02-header-midi-save.png)

| Control | Action |
|---------|--------|
| **Open** | Load `.reelpreset`; resolves sibling `.reelwt` |
| **Save** | Write current patch as `.reelpreset` |
| **WT** menu | Open/Save `.reelwt`, factory banks, Vital/WAV/Serum import |
| **Piano** | Show/hide on-screen keyboard |
| **MIDI** combo | Select hardware MIDI input device |
| **Status** | Audio/MIDI state, save confirmations, errors |

---

## Keyboard shortcuts

| Input | Notes |
|-------|-------|
| `Z S X D C V G B H N J M` | One octave (when app focused) |
| Click piano keys | Same as QWERTY when piano visible |
| MIDI controller | Full keyboard range; MPE dual-zone enabled in engine |

No global DAW-style transport — ReelSynth is a sound-design instrument, not a sequencer.

---

## Oscillator column (left rail)

![Osc, filter, ADSR](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/03-osc-filter-adsr.png)

- **Tabs** — switch OSC 1 / 2 / 3
- **Level, pan, detune, unison** — per oscillator
- **FM** — modulator routing (algorithm, ratio, index)
- **Warp** — phase warp modes on wavetable playback

WT position for the active osc syncs with the center WT strip.

---

## Center column

### Filter

- Filter 1 + Filter 2 (serial)
- Cutoff, resonance, type, key tracking, drive

### Envelopes

- **Amp ADSR** — overall loudness shape
- **Filter envelope** — filter movement over time

### LFO

- LFO 1 + LFO 2 — rate, depth, shape
- Targets via mod matrix

### Mod matrix

![Mod matrix and FX](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/05-mod-fx.png)

Up to 16 slots: source → target → amount. Sources include LFOs, envelopes, macros, velocity, mod wheel.

### FX rack

Serial chain: delay, reverb, chorus, etc. Per-slot mix and bypass.

---

## Wavetable editor

![WT 2D and 3D views](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/04-wt-editor-2d-3d.png)

| Element | Function |
|---------|----------|
| **Position strip** | Scrub through 256 frames |
| **Morph A / B / amount** | Crossfade between frame ranges |
| **2D view** | Current frame waveform |
| **3D view** | Bank surface (frame index × sample) |
| **Toolbar** | View options |

Morph settings are per-oscillator; active tab syncs with WT controls.

---

## On-screen piano

![Piano keyboard](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/06-piano-keyboard.png)

- **3 octaves** starting at C3 (MIDI note 48)
- Toggle via header **Piano** button
- Height: 88 px; white keys 16 px wide

---

## Scopes

Footer scope strip shows four taps: oscillator → filter → FX → output. Useful for debugging clipping and filter behavior while designing.

---

## Plugin editor (preview only)

`reelsynth-plugin-editor` shares this UI but **does not process audio or MIDI** yet. Message: *"Plugin editor spike — UI only (no audio I/O)"*. Real host bindings: S7 roadmap — see [plugin/README.md](../plugin/README.md).

---

## Screenshots

Images load from [GitHub Releases](https://github.com/reeldemo/reelsynth/releases) (not committed to the repo). Re-capture when UI sprints change layout — see [CONTRIBUTING.md](../CONTRIBUTING.md).
