# Getting started with ReelSynth

ReelSynth is a free (MIT) wavetable synthesizer. The standalone app lets you play, tweak, and save sounds. Export tools send those sounds to other synths and DAWs.

## Requirements

- **Rust** ≥ 1.85 (for `cargo run`)
- **macOS / Linux / Windows** — standalone uses `cpal` for audio and `midir` for MIDI
- Optional: **Python 3** + `maturin` for offline rendering (see [SDK.md](SDK.md))

## Install and run

```bash
git clone https://github.com/reeldemo/reelsynth.git
cd reelsynth
cargo run -p reelsynth-app --bin reelsynth-app
```

You should hear audio when you play notes. If audio fails, the UI still runs (status shows the error). Pick the output device in the header **Audio** combo (speakers, headphones, or a newly plugged DI / interface). With **Auto-select new audio output** enabled in Settings (default), freshly connected outputs are selected automatically.

![ReelSynth main window](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/01-full-window.png)

*Screenshot: [GitHub Release v0.1.0](https://github.com/reeldemo/reelsynth/releases/tag/v0.1.0) — numbered regions match [UI.md](UI.md).*

## Play your first note

Three ways to trigger notes:

| Method | How |
|--------|-----|
| **QWERTY keyboard** | `Z S X D C V G B H N J M` — layout depends on **Layout** (piano / scale / chords) |
| **On-screen piano** | Toggle **Piano** in the header; click keys (3 octaves from C3) |
| **MIDI controller** | Select your device in the **MIDI** dropdown; enable scale lock to snap incoming notes |

![Header: save, MIDI, piano](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/02-header-midi-save.png)

### Connect a MIDI controller

1. Plug in the controller before or after launch.
2. Open the **MIDI** combo box in the header.
3. Pick your device (not "MIDI" / disconnected).
4. Status line should show `MIDI: <device name>`.
5. Play — notes route to the synth engine.

Supported MIDI: Note On/Off, pitch bend, channel pressure, poly aftertouch, control change (CC1 = mod wheel).

### Select an audio output

1. Open the **Audio** combo in the header (next to MIDI).
2. Pick speakers, headphones, or an interface / DI box.
3. Status shows `Audio: <device name>`.
4. Plug in a new device with **Auto-select new audio output** on (Settings) — ReelSynth switches to the newly appeared device and updates the status line.

The last selected device name is remembered in app settings. If that device is missing at launch, ReelSynth falls back to the system default (or UI-only if no outputs exist).

## Shape a basic sound

While holding a note:

1. **Wavetable position** — move the WT strip or rail knob; hear timbre change.
2. **Filter cutoff** — center column; lower = darker.
3. **ADSR envelope** — short attack + decay = pluck; long release = pad.
4. **Unison / detune** — oscillator column for width.

![Oscillator, filter, ADSR](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/03-osc-filter-adsr.png)

See [UI.md](UI.md) for every region.

## Save your work

ReelSynth splits **sound design** into two files:

| File | Contains |
|------|----------|
| **`.reelpreset`** | Patch: oscillators, filter, envelopes, LFO, mod matrix, FX |
| **`.reelwt`** | Wavetable bank: the raw morphable waves |

**Save patch:** header **Save** → choose `my_sound.reelpreset`.

**Save wavetable:** header **WT** menu → **Save .reelwt…**

Keep both in the same folder. The preset references the table by `wavetable_id` or `wavetable_path` (see [FORMAT.md](FORMAT.md)).

**Open patch:** header **Open** → picks `.reelpreset`; app resolves sibling `.reelwt` automatically.

## Import wavetables from other synths

**WT** menu → **Import**:

- Vital (`.vitaltable`)
- WAV folder (single-cycle waves, sorted by filename)
- Serum (`.fxp` — wavetable subset only)

Imported tables save as `.reelwt`. Factory banks (Saw Morph, Formant, …) are under **WT → Factory banks**.

![Wavetable editor](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/04-wt-editor-2d-3d.png)

## What to do next

| Goal | Next doc |
|------|----------|
| Record a melody and use this sound in a DAW | [WORKFLOW.md](WORKFLOW.md) |
| Use only free DAWs and synths | [FREE_STACK.md](FREE_STACK.md) |
| Export to Vital / Ableton / Serum | [WORKFLOW.md § Export](WORKFLOW.md#export-a-daw-ready-bundle) |
| Script rendering or build a tool | [SDK.md](SDK.md) |
| Reeldemo Studio + Ableton handoff | [REELDEMO_INTEGRATION.md](REELDEMO_INTEGRATION.md) |

## Honest limits (v0.1)

| Works today | Not yet |
|-------------|---------|
| Live play (MIDI, piano, QWERTY) | VST3 / AU / CLAP plugin in DAW (S7 roadmap) |
| Save/load `.reelpreset` + `.reelwt` | In-app MIDI recording |
| CLI / Python export to `reelpack/` | Export of your live performance as MIDI |
| Offline single-note audio render | |

Always record **melody MIDI in your DAW** until in-app recording ships. See [WORKFLOW.md](WORKFLOW.md).
