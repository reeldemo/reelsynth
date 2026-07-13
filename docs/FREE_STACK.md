# Free and low-cost production stack

ReelSynth itself is **MIT licensed — completely free**. To compose melodies and finish tracks without paid software, combine ReelSynth with the tools below. No single path is mandatory; pick what fits your OS and workflow.

## Component overview

| Need | Free / low-cost options | Notes |
|------|-------------------------|-------|
| **Sound design** | ReelSynth standalone | MIT, no license fee |
| **Synth in DAW** | [Vital](https://vital.audio/) (free) | Load `.vitaltable` from `reelpack` export |
| **DAW (MIDI + arrange)** | See table below | Record melody; ReelSynth cannot record MIDI yet |
| **Audio editing** | [Audacity](https://www.audacityteam.org/) | Trim stems, normalize, basic FX |
| **Notation / MIDI view** | [MuseScore](https://musescore.org/) | Inspect `.mid` files |

---

## DAW options (equal comparison)

| DAW | License | OS | MIDI recording | VST/AU plugins | Learning curve | ReelSynth handoff |
|-----|---------|-----|----------------|----------------|----------------|-------------------|
| **[Reaper](https://www.reaper.fm/)** | 60-day full trial; inexpensive license | Win, macOS, Linux | Excellent | Yes | Moderate | Export `reelpack` → Vital VST |
| **[LMMS](https://lmms.io/)** | GPL, free | Win, Linux (macOS experimental) | Yes | LADSPA, some VST | Moderate | Vital via VeSTige (Linux/Win) |
| **[Ardour](https://ardour.org/)** | GPL / pay-what-you-want | Win, macOS, Linux | Excellent | LV2, VST, AU | Steeper | SFZ or Vital LV2 |
| **[Cakewalk](https://www.bandlab.com/products/cakewalk)** | Free | Windows only | Excellent | VST | Moderate | Vital + `reelpack` |
| **[GarageBand](https://www.apple.com/mac/garageband/)** | Free | macOS, iOS | Yes | Limited AU | Easy | Manual WT load; no Vital AU on iOS |
| **[Ableton Live Lite](https://www.ableton.com/en/live-lite/)** | Free with hardware | Win, macOS | Yes | Limited | Moderate | `wavetable_map.json` + wav frames |
| **[Waveform Free](https://www.tracktion.com/products/waveform-free)** | Free | Win, macOS, Linux | Yes | VST | Moderate | Vital VST |

ReelSynth does **not** ship as a plugin today. Always export sound via [WORKFLOW.md](WORKFLOW.md) and load into a host synth (usually Vital).

---

## Recommended free workflow (manual)

Works on any OS with a DAW + Vital:

1. **Design** — ReelSynth standalone + MIDI controller (audition only).
2. **Save** — `.reelpreset` + `.reelwt`.
3. **Export** — `reelsynth-export reelpack …` → get `synth/vital/table.vitaltable`.
4. **Compose** — Record MIDI melody in your DAW (any placeholder instrument).
5. **Load sound** — Vital on same track; import `.vitaltable`; match filter/ADSR by ear.
6. **Finish** — Arrange, mix, export WAV from DAW.

Detailed steps: [WORKFLOW.md](WORKFLOW.md).

---

## Synth targets from export (all free to load)

| Export target | Free host | File in `reelpack/` |
|---------------|-----------|---------------------|
| Vital | Vital | `synth/vital/table.vitaltable` |
| WAV frames | Any sampler / Wavetable | `synth/wav_frames/frame_*.wav` |
| SFZ | Sforzando, Ardour | `daw/sfz/` |
| Audio stem | Any DAW | `daw/audio/melody.wav` (single-note preview) |

Serum and Ableton Wavetable exports are useful if you already own those products — not required for a free stack.

---

## What stays paid or commercial

| Product | Role |
|---------|------|
| **Serum** | Optional import/export target |
| **Ableton Live (full)** | Optional; Lite is free with gear |
| **Reeldemo Studio** | Commercial agent + Ableton handoff — see [REELDEMO_INTEGRATION.md](REELDEMO_INTEGRATION.md) |

None of these are required to use ReelSynth or finish tracks with Vital + a free DAW.

---

## Cost summary

| Stack | Approx. cost |
|-------|----------------|
| ReelSynth + Vital + LMMS + Audacity | **$0** |
| ReelSynth + Vital + Reaper (after trial) | **~$60** one-time (Reaper license) |
| ReelSynth + Reeldemo Studio + Ableton | Commercial — see integration doc |
