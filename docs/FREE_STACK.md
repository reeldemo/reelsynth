# Free stack

ReelSynth is MIT. You can finish tracks without paid soft synths or a full Live license.

## Pieces

| Need | Option | Notes |
|------|--------|-------|
| Sound design | ReelSynth standalone | Free |
| Synth in DAW | [Vital](https://vital.audio/) | Load `.vitaltable` from `reelpack` |
| Arrange / MIDI | DAW table below | Record melody there (or in Compose) |
| Audio edit | [Audacity](https://www.audacityteam.org/) | Trim, normalize |
| Notation / MIDI peek | [MuseScore](https://musescore.org/) | Inspect `.mid` |

## DAWs

| DAW | Cost | OS | Plugins | ReelSynth handoff |
|-----|------|-----|---------|-------------------|
| [Reaper](https://www.reaper.fm/) | Trial, then cheap license | Win, Mac, Linux | VST/AU | `reelpack` → Vital |
| [LMMS](https://lmms.io/) | Free (GPL) | Win, Linux (Mac experimental) | LADSPA / some VST | Vital via VeSTige |
| [Ardour](https://ardour.org/) | GPL / pay what you want | Win, Mac, Linux | LV2, VST, AU | SFZ or Vital |
| [Cakewalk](https://www.bandlab.com/products/cakewalk) | Free | Windows | VST | Vital + `reelpack` |
| [GarageBand](https://www.apple.com/mac/garageband/) | Free | Mac, iOS | Limited AU | Manual WT; no Vital AU on iOS |
| [Ableton Live Lite](https://www.ableton.com/en/live-lite/) | Free with hardware | Win, Mac | Limited | Multicycle WAV + map; or VST3 if you build it |
| [Waveform Free](https://www.tracktion.com/products/waveform-free) | Free | Win, Mac, Linux | VST | Vital |

There’s a VST3 for Live on Win/macOS ([ABLETON.md](ABLETON.md)). Everywhere else, export → Vital (or SFZ) is the usual path — [WORKFLOW.md](WORKFLOW.md).

## Simple free loop

1. Design in ReelSynth; save `.reelpreset` + `.reelwt`.
2. `reelsynth-export reelpack …` → `synth/vital/table.vitaltable`.
3. Record MIDI in the DAW (or Compose, then re-enter / export later).
4. Vital on that track; import the table; match filter/ADSR by ear.
5. Arrange, mix, bounce from the DAW.

## Free export targets

| Target | Host | In `reelpack/` |
|--------|------|----------------|
| Vital | Vital | `synth/vital/table.vitaltable` |
| WAV frames | Sampler / Wavetable | `synth/wav_frames/frame_*.wav` |
| SFZ | Sforzando, Ardour | `daw/sfz/` |
| Audio stem | Any DAW | `daw/audio/melody.wav` (one preview note) |

Serum / full Ableton exports help if you already own them — not required here.

## Paid / commercial (optional)

| Product | Role |
|---------|------|
| Serum | Extra import/export target |
| Ableton Live (full) | Optional; Lite comes with some gear |
| Reeldemo Studio | Agent + Ableton handoff — [REELDEMO_INTEGRATION.md](REELDEMO_INTEGRATION.md) |

## Cost ballpark

| Stack | About |
|-------|--------|
| ReelSynth + Vital + LMMS + Audacity | $0 |
| ReelSynth + Vital + Reaper | ~$60 once (Reaper) |
| + Studio + Ableton | Commercial |
