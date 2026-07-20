# Piano roll clip-editor redesign

**Date:** 2026-07-16  
**Status:** Approved  
**Layout choice:** A — Clip editor first (Ableton MIDI-clip style)

## Problem

Compose-mode piano roll is not usable end-to-end:

- Left column is pitch *labels*, not playable keys
- Audition / QWERTY / MIDI / transport Play feel disconnected or silent
- Layout squeezes arrangement + roll + scenes so the roll never feels like a real editor

## Goals

1. Ableton-like clip editor as the dominant Compose surface
2. Clickable piano keys → synth note on/off with key highlight
3. QWERTY (layout-aware Z–M row) maps to those same pitches; optional letter glyphs on white keys
4. Pencil / note select / MIDI / transport ▶ all audible through the synth
5. Thin clip strip replaces fat arrangement; scenes collapsed by default

## Non-goals

- Full Ableton Session View polish
- Plugin host audio/MIDI I/O (S7)
- Scale-fold rows inside the roll (Key/Scale stay on live performance input only)
- Rewriting `SequenceProject` schema

## Shell layout

```
Transport
Thin clip strip (tracks × clips)
Piano roll (dominant): keys | grid | notes
Velocity (+ automation fold)
Scenes (collapsed by default)
Optional footer 88-key strip (same pitch space)
```

## Interaction model

| Input | Behavior |
|-------|----------|
| Hold left key | `note_on` / `note_off` via unified compose path; key lights |
| QWERTY / MIDI | Same pitches as left keys |
| Pencil / Select note | Audition with proper note-off |
| Transport ▶ | Scheduler plays clip notes through engine |
| Record ● + armed | Live input commits to clip; still monitors |

## Architecture

- UI: rewrite key column + Compose layout (`ui/src/compose/`)
- App: single compose note on/off path (`app/src/app.rs`)
- Engine: verify existing sequencer → voice dispatch on Play
- Docs: `docs/UI.md`, AGENTS.md honesty on compose playback

## Success criteria

- Hold a left-key C4 → hear patch, key lights, release stops
- Z–M plays consecutive mapped degrees matching key column
- Draw notes, press ▶ → hear them in time with playhead
- New user can edit a clip without reading docs
