# Getting started

ReelSynth is a free (MIT) wavetable synth. Play and tweak in the standalone app, then export to other synths or a DAW.

## Install (recommended)

1. Download from [reeldemo.github.io/reelsynth](https://reeldemo.github.io/reelsynth/#download):
   - **macOS:** `.pkg` (Apple Silicon or Intel)
   - **Windows:** setup `.exe`
   - **Linux:** `.tar.gz` archive (no Ableton installer)
2. Run the installer. It places the standalone app, the VST3, and the Ableton external editor/config.
3. **Ableton:** quit Live if open → Preferences → Plug-ins → Rescan VST3 → load **ReelSynth** on a MIDI track (editor should open automatically).

**Unsigned builds (v1):** macOS may need right-click the app → **Open** the first time (Gatekeeper). Windows SmartScreen: **More info** → **Run anyway**.

Archives (zip/tar.gz) remain on [GitHub Releases](https://github.com/reeldemo/reelsynth/releases/latest) as a portable fallback. Contributor/dev Ableton scripts: [ABLETON.md](ABLETON.md).

## Requirements (from source)

- Rust ≥ 1.85 (for `cargo run`)
- macOS / Linux / Windows — audio via `cpal`, MIDI via `midir`
- Optional: Python 3 + `maturin` for offline render ([SDK.md](SDK.md))

## Run from source

```bash
git clone https://github.com/reeldemo/reelsynth.git
cd reelsynth
cargo run -p reelsynth-app --bin reelsynth-app
```

Play a note — you should hear audio. If the device fails, the UI still opens and the status line shows why. Pick output in the header **Audio** combo. With **Auto-select new audio output** on (Settings, default), a newly plugged interface gets selected for you.

![ReelSynth main window](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/01-full-window.png)

*From [Release v0.1.0](https://github.com/reeldemo/reelsynth/releases/tag/v0.1.0) — numbered regions match [UI.md](UI.md).*

## Play a note

| Method | How |
|--------|-----|
| QWERTY | `Z S X D C V G B H N J M` — depends on **Layout** (piano / scale / chords) |
| On-screen piano | Header **Piano**; click keys (3 octaves from C3, or 88-key strip when expanded) |
| MIDI | Header **MIDI** dropdown; scale lock snaps incoming notes if you want it |

![Header: save, MIDI, piano](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/02-header-midi-save.png)

### MIDI

1. Plug in the controller (before or after launch).
2. Open **MIDI** in the header.
3. Pick your device (not the disconnected placeholder).
4. Status should read `MIDI: <name>`.
5. Play.

Supported: Note On/Off, pitch bend, channel pressure, poly aftertouch, CC (CC1 = mod wheel).

### Audio out

1. Open **Audio** next to MIDI.
2. Pick speakers, headphones, or an interface.
3. Status: `Audio: <name>`.
4. With auto-select on, a new device that appears gets chosen automatically.

Last device name is remembered. Missing at launch → system default, or UI-only if nothing is there.

## Shape the sound

While holding a note:

1. **Wavetable position** — WT strip or rail knob.
2. **Filter cutoff** — center column; lower is darker.
3. **ADSR** — short attack/decay = pluck; long release = pad.
4. **Unison / detune** — oscillator column for width.

![Oscillator, filter, ADSR](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/03-osc-filter-adsr.png)

Full map: [UI.md](UI.md).

## Save

Two files:

| File | Holds |
|------|-------|
| `.reelpreset` | Patch — oscs, filter, envelopes, LFO, mod matrix, FX |
| `.reelwt` | Wavetable bank |

**Save patch:** header **Save** → `my_sound.reelpreset`.  
**Save wavetable:** **WT → Save .reelwt…**

Keep them together. The preset points at the table by `wavetable_id` or `wavetable_path` ([FORMAT.md](FORMAT.md)).

**Open:** header **Open** — loads `.reelpreset` and finds a sibling `.reelwt` when it can.

## Import tables

**WT → Import:**

- Vital (`.vitaltable`)
- WAV folder (single-cycle waves, sorted by name)
- Serum (`.fxp` — wavetable subset only)

Imports become `.reelwt`. Factory banks: **WT → Factory banks**.

![Wavetable editor](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/04-wt-editor-2d-3d.png)

## Clean wrap crackle (ReelAI)

Single-cycle tables loop forever. If first and last samples disagree, you get a wrap click on sustained notes (common after imports). ReelSynth fixes that with a **bake before play** — rewrite frames once, then play as usual (no realtime neural lag).

| Control | Where | Effect |
|---------|--------|--------|
| **Result·ReelAI** (etc.) | Header | Heals the whole bank |
| **Layer·…** | Selected toolbar | Heals this layer’s frame only — mix DualCosine / ReelAI / Off per layer |
| **Noise2Noise** | Either dropdown | Embedded U-Net-lite baseline for A/B |
| **DualCosine** | Either dropdown | Classical periodize — fast, predictable |
| **Fit ends** | Selected toolbar | One-shot DualCosine on the selected layer frame |
| Soft / Adapt / Off | Either dropdown | Light fade, adaptive fade, or raw ends |

After an import: load the bank → hold a note → if you hear a tick each cycle, set header **ReelAI** (or **Fit ends**). A/B with DualCosine / Noise2Noise; Off / Adapt restores from snapshot where applicable.

Seam mode is session-only (not written into `.reelpreset`). Big morph/stack edits can reopen discontinuities — run Fit ends or re-pick ReelAI if clicks come back. More: [UI.md § ReelAI](UI.md#reelai-seam-heal).

## Next

| Goal | Doc |
|------|-----|
| Compose / DAW handoff | [WORKFLOW.md](WORKFLOW.md) |
| Free DAWs only | [FREE_STACK.md](FREE_STACK.md) |
| Ableton | [ABLETON.md](ABLETON.md) |
| Script / embed | [SDK.md](SDK.md) |
| Reeldemo Studio | [REELDEMO_INTEGRATION.md](REELDEMO_INTEGRATION.md) |

## Limits worth knowing

| Works | Not yet / watch out |
|-------|---------------------|
| Live play (MIDI, piano, QWERTY), Compose clips | `daw/midi/melody.mid` in export is a **demo note**, not your performance |
| Save/load preset + table | Lossy export to Vital / Serum / Ableton — see [INTEROP.md](INTEROP.md) |
| CLI / Python `reelpack` | Full SMF of Compose songs still landing |
| VST3 in Ableton (Win/macOS) + external UI | Editor stuffed into Live’s small pane (won’t do that) |

Until Compose SMF export ships, record arrangement MIDI in the DAW when you need a full session there. See [WORKFLOW.md](WORKFLOW.md).
