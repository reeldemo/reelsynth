---
name: reelsynth-workflow
description: >-
  Guide users through ReelSynth sound design, export, and DAW handoff. Use when
  the user asks how to use the synth, compose melodies, connect MIDI, export
  reelpack, move sounds to Vital/Ableton, or integrate with Reeldemo Studio.
---

# ReelSynth workflow skill

Help users design sounds and get them into a DAW. Don’t overpromise SMF export or perfect foreign-format round-trips.

## Doc map

| Question | Doc |
|----------|-----|
| Install, first note, save | [docs/GETTING_STARTED.md](../../docs/GETTING_STARTED.md) |
| UI, MIDI/Audio, piano | [docs/UI.md](../../docs/UI.md) |
| Melody + sound in DAW | [docs/WORKFLOW.md](../../docs/WORKFLOW.md) |
| Free DAWs / Vital | [docs/FREE_STACK.md](../../docs/FREE_STACK.md) |
| Ableton VST3 / Send | [docs/ABLETON.md](../../docs/ABLETON.md) |
| Python, CLI, Rust | [docs/SDK.md](../../docs/SDK.md) |
| Reeldemo Studio | [docs/REELDEMO_INTEGRATION.md](../../docs/REELDEMO_INTEGRATION.md) |
| Export loss | [docs/INTEROP.md](../../docs/INTEROP.md) |

Index: [docs/README.md](../../docs/README.md)

## Hard rules

1. **`daw/midi/melody.mid` is a demo note** — not their performance.
2. **Compose** can record clips in-app; full SMF export of that song is still landing — for a finished DAW session, record or copy MIDI into the DAW when needed.
3. **VST3** works in Ableton on Win/macOS with an external Design UI ([ABLETON.md](../../docs/ABLETON.md)). Elsewhere, export → Vital / SFZ is the usual path.
4. **Exports are lossy** — truth is `.reelpreset` + `.reelwt`.
5. **Reeldemo Studio is commercial** — optional; standalone is MIT.

## Path A — manual

### 1. Launch

```bash
cargo run -p reelsynth-app --bin reelsynth-app
```

### 2. Input

- MIDI → header **MIDI**
- Audio → header **Audio** (auto-select new outputs when Settings allow)
- Or QWERTY `Z–M` / **Piano**

### 3. Design

WT position, filter, ADSR, LFO, mod matrix, FX while playing.

### 4. Save

- **Save** → `.reelpreset`
- **WT → Save .reelwt** if the table changed
- Keep both together

### 5. Export

```bash
cargo run --bin reelsynth-export -- reelpack my_patch.reelpreset -o out/ \
  --targets vital,wav,serum,ableton,sfz,midi,audio
```

Free path: `synth/vital/table.vitaltable`.

### 6. DAW

- MIDI on a track (Compose and/or DAW record)
- Vital / Wavetable / Live VST3 with the exported (or installed) sound
- Arrange and mix

## Path B — Studio

Compose → grade → Ableton handoff. [REELDEMO_INTEGRATION.md](../../docs/REELDEMO_INTEGRATION.md). Studio is not required for ReelSynth.

## Free stack

Point at [FREE_STACK.md](../../docs/FREE_STACK.md). Vital is the usual free synth target for `reelpack`.

## Troubleshooting

| Symptom | Check |
|---------|-------|
| No audio | Status line; cpal device; UI-only fallback |
| MIDI silent | MIDI dropdown; cable/driver |
| Export missing WT | Sibling `.reelwt` next to preset |
| Sound differs in Vital | Expected — `export_report.json`, tweak by ear |
| Wants Ableton plugin | [ABLETON.md](../../docs/ABLETON.md) installers; or Send + one drag |

## UI issues

Use `@audit-reelsynth-ui` — not this skill.

## Screenshots

Release URLs (`releases/download/v0.1.0/...`). Capture: [CONTRIBUTING.md](../../CONTRIBUTING.md).

## Checklist (“how do I use this with my DAW?”)

- [ ] Sound vs notes are separate
- [ ] Standalone (or VST3) for the sound
- [ ] Compose and/or DAW for MIDI
- [ ] `reelpack` / Vital / Ableton paths as needed
- [ ] Link the right doc
- [ ] Don’t claim lossless foreign export or finished SMF export
