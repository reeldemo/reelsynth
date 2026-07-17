---
name: reelsynth-workflow
description: >-
  Guide users through ReelSynth sound design, export, and DAW handoff. Use when
  the user asks how to use the synth, compose melodies, connect MIDI, export
  reelpack, move sounds to Vital/Ableton, or integrate with Reeldemo Studio.
  Covers honest limits (no MIDI recording, no plugin yet) and free-tool paths.
---

# ReelSynth workflow skill

Help users design sounds in ReelSynth and hand off to a DAW — without overpromising features that do not exist yet.

## Doc map (read before answering)

| Question type | Doc |
|---------------|-----|
| Install, first note, save preset | [docs/GETTING_STARTED.md](../../docs/GETTING_STARTED.md) |
| UI regions, MIDI/Audio dropdowns, piano | [docs/UI.md](../../docs/UI.md) |
| Melody + sound in DAW | [docs/WORKFLOW.md](../../docs/WORKFLOW.md) |
| Free DAWs and Vital | [docs/FREE_STACK.md](../../docs/FREE_STACK.md) |
| Python, CLI, Rust API | [docs/SDK.md](../../docs/SDK.md) |
| Reeldemo Studio + Ableton | [docs/REELDEMO_INTEGRATION.md](../../docs/REELDEMO_INTEGRATION.md) |
| Export loss | [docs/INTEROP.md](../../docs/INTEROP.md) |

Index: [docs/README.md](../../docs/README.md)

## Hard rules (never contradict)

1. **No in-app MIDI recording** — user records melody in their DAW.
2. **`daw/midi/melody.mid` is one demo note** — not their performance.
3. **No VST/AU/CLAP in DAW yet** — S7 roadmap; use export + Vital today.
4. **Exports are lossy** — canonical state is `.reelpreset` + `.reelwt`.
5. **Reeldemo Studio is commercial** — optional; standalone is fully MIT/free.

## Standard workflow (Path A — manual)

Walk the user through these steps:

### 1. Launch

```bash
cargo run -p reelsynth-app --bin reelsynth-app
```

### 2. Connect input

- MIDI controller → header **MIDI** dropdown
- Audio output → header **Audio** dropdown (auto-selects newly connected DI / interface when enabled in Settings)
- Or QWERTY `Z–M` / on-screen **Piano**

### 3. Design sound

While playing notes: WT position, filter, ADSR, LFO, mod matrix, FX.

### 4. Save

- **Save** → `.reelpreset`
- **WT → Save .reelwt** (if table edited)
- Keep both files together

### 5. Export

```bash
cargo run --bin reelsynth-export -- reelpack my_patch.reelpreset -o out/ \
  --targets vital,wav,serum,ableton,sfz,midi,audio
```

Point user to `synth/vital/table.vitaltable` for free Vital path.

### 6. DAW

- Record MIDI melody on a DAW track (any placeholder synth)
- Load Vital (or Wavetable) with exported assets on same track
- Edit piano roll, arrange, mix

## Path B — Reeldemo Studio

If user has Reeldemo Studio: compose → grade → handover to Ableton. See [REELDEMO_INTEGRATION.md](../../docs/REELDEMO_INTEGRATION.md).

Do not imply Studio is required for ReelSynth.

## Free stack guidance

Present [FREE_STACK.md](../../docs/FREE_STACK.md) options **equally** — Reaper, LMMS, Ardour, Cakewalk, etc. Vital is the usual free synth target for `reelpack` export.

## Troubleshooting

| Symptom | Check |
|---------|-------|
| No audio | Status line; cpal device; UI-only fallback |
| MIDI silent | MIDI dropdown selection; cable/driver |
| Export missing WT | Sibling `.reelwt` next to preset |
| Sound differs in Vital | Expected — read `export_report.json`, tweak by ear |
| Wants plugin in DAW | Explain S7; offer Vital export path |

## UI troubleshooting

For visual/layout issues, use `@audit-reelsynth-ui` skill — not this workflow skill.

## Screenshots

Docs use GitHub Release URLs (`releases/download/v0.1.0/...`). Not in repo. Re-capture per [CONTRIBUTING.md](../../CONTRIBUTING.md).

## Agent checklist

When user asks "how do I use reelsynth with my DAW":

- [ ] Explain MIDI vs sound separation
- [ ] Standalone for sound design only
- [ ] DAW for MIDI recording
- [ ] Export reelpack for sound transfer
- [ ] Mention limits honestly
- [ ] Link relevant doc section
