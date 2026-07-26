# Analyze: Ableton Live Integration

Date: 2026-07-26

## AC status

| AC | Status | Evidence |
|----|--------|----------|
| AC-1.1–1.6 | Pass | `src/export/ableton.rs`, qa tests |
| AC-2.1–2.6 | Pass | header Ableton + `send_ableton` / `ableton_osc` |
| AC-3.* | Deferred | Requires `reeldemo-ableton` clone (T8–T9, T16) |
| AC-4.1 | Partial | nih-plug VST3/CLAP exports compile; Live install/QA + `.vst3` bundle packaging pending |
| AC-4.2 | Partial | `nih_plugin.rs` MIDI + `process_stereo`; needs Live smoke |
| AC-4.3 | Open | egui embed blocked on 0.30 vs nih-plug egui 0.34 ecosystem |
| AC-4.4 | Partial | 5 FloatParams exposed |
| AC-4.5 | Open | state blob not yet persisted |
| AC-4.6 | Pass | `plugin/Cargo.toml` license GPL-3.0-or-later |
| AC-4.7 | Pass | CLAP + VST3 exported; docs say Live needs VST3 |
| AC-4.8 | Partial | no Compose in plugin process path (no editor yet) |
| AC-5.1–5.3 | Pass (bridge) | INTEROP/WORKFLOW/CHANGELOG/REELDEMO_INTEGRATION |
| AC-5.4 | Hold | capability matrix stays No until Live QA |

## Remaining tasks

- T8–T9, T16 — Studio repo
- T12–T15 — state, egui embed, bundle, docs after Live smoke
- T17 — re-run after above

## Honest product claim

**Today:** Send-to-Ableton bridge is usable. VST3/CLAP **builds** as an instrument shell; treat as developer preview until Live QA and editor land.
