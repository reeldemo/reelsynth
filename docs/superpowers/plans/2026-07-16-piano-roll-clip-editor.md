# Piano Roll Clip-Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rebuild Compose piano roll as an Ableton-style clip editor: playable keys, QWERTY mapping, audible audition, and transport playback of clip notes.

**Architecture:** Clip-editor-first shell (thin clip strip + dominant roll). Unify all compose pitch input through one note on/off path in the app. Keep `SequenceProject` / sequencer runtime; fix UI and wiring so Play and live keys actually voice.

**Tech Stack:** Rust, egui (`reelsynth-ui`), standalone app (`reelsynth-app`), existing `SequencerRuntime` / `NoteScheduler`.

**Spec:** [docs/superpowers/specs/2026-07-16-piano-roll-clip-editor-design.md](../specs/2026-07-16-piano-roll-clip-editor-design.md)

## Global Constraints

- Layout A only: clip strip + fat roll; scenes collapsed by default
- No SequenceProject schema rewrite
- No plugin host work (S7)
- Honest docs: do not claim features that fail manual play
- Prefer fixing `piano_roll.rs` / compose shell over new crates
- Update CHANGELOG for user-visible behavior

## File map

| File | Responsibility |
|------|----------------|
| `ui/src/compose/mod.rs` | Compose shell proportions: transport, clip strip, roll, folded scenes |
| `ui/src/compose/arrangement.rs` | Slim to clip-strip mode (or extract `clip_strip.rs`) |
| `ui/src/compose/piano_roll.rs` | Clickable keys, QWERTY glyphs, audition note-off, scroll/zoom basics |
| `ui/src/compose/scene_grid.rs` | Collapsed-by-default header |
| `ui/src/state.rs` / `ShellActions` | Extend actions for key note_off if needed |
| `app/src/app.rs` | Unify `handle_compose_note_on/off`; wire roll key events |
| `app/src/audio_commands.rs` / engine process | Verify Play dispatches scheduler NoteOn/Off to voices |
| `ui/tests/kittest.rs` | Compose layout + key interaction audits |
| `docs/UI.md`, `AGENTS.md`, `CHANGELOG.md` | Document new Compose behavior |

---

## Task 1: Prove transport Play voices clip notes

**Deliverable:** ▶ on a clip with notes produces audible notes (or a failing test that pins the bug).

- [ ] Trace `AudioCmd::TransportPlay` → `SequencerRuntime::begin_buffer` → `events_at_frame` → voice `note_on` in `src/engine/mod.rs`
- [ ] Add/extend a unit or QA test in `tests/qa/sequence.rs` (or engine test) that schedules a known note and asserts a note-on event fires at the expected frame
- [ ] Fix any gap (session vs arrangement timeline, empty scheduler, channel mute, missing SetSequence sync)
- [ ] Manual: Compose → select clip with notes → ▶ → hear sound
- [ ] Commit: `fix(compose): ensure transport play voices scheduled clip notes`

---

## Task 2: Unifyable piano key column + audition note-off

**Deliverable:** Hold left key → sound + highlight; release → silence.

- [ ] In `piano_roll.rs`, replace label-only `KEY_LABEL_W` strip with interactive black/white keys (~48–56px)
- [ ] Pointer down → `audition_note` / new `note_on` action with pitch; pointer up / leave → `note_off` action (extend `PianoRollActions` + `ShellActions`)
- [ ] Wire in `compose/mod.rs` and `app.rs` so both on and off hit `engine_note_on` / `engine_note_off` (not the asymmetric pencil-only path)
- [ ] Light held keys from `keys_down` or compose-local held set
- [ ] Kit/audit test: key region exists and click records interaction if harness allows
- [ ] Commit: `feat(compose): playable piano roll key column with note off`

---

## Task 3: Unify QWERTY / MIDI / pencil through one path

**Deliverable:** Z–M and MIDI always monitor in Compose; pencil/select audition uses same on/off.

- [ ] Collapse special cases in `handle_compose_note_on/off` so non-record always monitors via engine (performance layers still apply when Layout ≠ Piano)
- [ ] Pencil draw and Select-click note: emit paired note_on then short note_off (or hold while pointer down)
- [ ] Draw QWERTY letter glyphs on white keys for the active octave window (reuse `keyboard_note` / `ComputerLayout` from app or shared helper)
- [ ] Octave shift: reuse existing performance octave if present; otherwise document +/- keys
- [ ] Commit: `fix(compose): unify live pitch input and show QWERTY on keys`

---

## Task 4: Clip-editor-first shell layout

**Deliverable:** Compose matches wireframe A.

- [ ] Restructure `draw_compose_shell` in `mod.rs`: transport → thin clip strip (~15%) → piano roll (~70%+) → velocity/automation → scenes collapsed
- [ ] Adapt arrangement into clip-strip height (horizontal clips per track, arm/mute on left rail kept)
- [ ] Scene grid: collapsed header “Scenes ▸”; expand restores previous grid
- [ ] Auto-select or create a default clip so roll is never empty on first Compose entry
- [ ] Update kittest compose layout audits for new region sizes / audit IDs
- [ ] Commit: `feat(compose): clip-editor-first layout with collapsible scenes`

---

## Task 5: Roll usability polish (Ableton basics)

**Deliverable:** Roll feels editable without fighting the UI.

- [ ] Vertical scroll for pitch range; keep playhead and beat grid correct
- [ ] Horizontal scroll/zoom within clip length (mouse wheel + modifiers matching egui norms)
- [ ] Ensure Delete, undo/redo, snap still work after key-column hit testing
- [ ] Commit: `feat(compose): piano roll scroll and zoom`

---

## Task 6: Docs + changelog

**Deliverable:** Docs match reality.

- [ ] Update `docs/UI.md` Compose section for clip-editor layout, playable keys, QWERTY overlays, Play behavior
- [ ] Soften/fix `AGENTS.md` compose bullet if full transport playback now works
- [ ] `CHANGELOG.md` user-facing entry
- [ ] Commit: `docs(compose): document Ableton-style clip editor`

---

## Verification (before claiming done)

```bash
cargo test --workspace --no-default-features -j 1
cargo run -p reelsynth-app --bin reelsynth-app
```

Manual checklist:

1. Compose → hear C4 from left keys and from Z-row
2. Draw notes → ▶ hears them
3. Record armed + play → notes land in clip
4. Scenes collapsed; clip strip usable
5. Design mode still plays as before
