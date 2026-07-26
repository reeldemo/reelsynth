# Tasks: Ableton Live Integration

Ordered implementation units. Technical plan: [spec.md](spec.md). Requirements: [requirements.md](requirements.md).

| ID | Task | Depends | DoD |
|----|------|---------|-----|
| T0 | Spec locked (`spec.md` status SPEC LOCKED) | — | Done — no further human gate on spec |
| T1 | Bump Ableton export to `reelsynth-ableton-wt-v2` with `parameters`, `live_param_aliases`, `frames` in `src/export/ableton.rs`; update reelpack child path | T0 | **Done** AC-1.1–1.3 |
| T2 | Implement multi-cycle WAV writer; reelpack writes `synth/ableton/table_multicycle.wav`; map `frames.multi_cycle_wav` points at it | T1 | **Done** AC-1.4, AC-1.6 |
| T3 | Ensure dropped params still recorded for mod overflow / sub / noise / FX | T1 | **Done** AC-1.5 |
| T4 | Resolve inbox path helper (`REELSYNTH_ABLETON_INBOX` + Win/mac defaults) + README.txt generator | T2 | **Done** |
| T5 | Thin AbletonOSC client: probe :11000, create MIDI track, insert Wavetable, apply aliased params | T1 | **Done** (offline probe + best-effort push) |
| T6 | Wire **Send to Ableton** in app/UI: export inbox, call OSC if up, status/toast honesty | T4, T5 | **Done** AC-2.1–2.6 |
| T7 | Docs (reelsynth): INTEROP, WORKFLOW, REELDEMO_INTEGRATION, CHANGELOG for bridge | T6 | **Done** AC-5.1, AC-5.3 (Send) |
| T8 | **Human gate / other repo:** clone `reeldemo-ableton`; alias apply prefers wavetable_map; stage multicycle in handover inbox | T2 | AC-3.1, AC-3.2 (OSC path) |
| T9 | Implement Extension `importHandoverBundle` for `midi_device` (or document OSC-only if Extension blocked) | T8 | AC-3.3, AC-3.4 |
| T10 | Plugin crate: adopt nih-plug, set license GPL-3.0-or-later, `nih_export_vst3!` + `nih_export_clap!`, remove null clap stub | T0 | **Done** (stub kept for IDs; nih_plugin exports real entries) |
| T11 | Plugin processor: wrap `SynthEngine`, host MIDI → notes, stereo process, SR re-init | T10 | **Done** compile-time; Live smoke pending |
| T12 | Plugin params subset + state blob (preset + bank) save/restore | T11 | **Done** (params + `reelsynth-plugin-state-v1`) |
| T13 | Embed `draw_shell` via `nih_plug_egui`; plugin host mode hides devices; Compose disabled/noted | T11 | **Blocked** egui 0.30 vs adapter 0.34 — Ableton-first after Live QA |
| T14 | Bundle `.vst3`; document install; Live 12 manual QA checklist | T12, T13 | Docs in `docs/ABLETON.md`; bundle/Live smoke **open** |
| T15 | Docs: VST3 primary path; bridge = fallback; capability matrix update; CHANGELOG | T14 | Partial — matrix = Preview |
| T8–T9, T16 | Studio Extension | — | **Deferred** until Ableton path QA’d (user priority) |
| T17 | SDD analyze | — | **analyze.md** written; reopen when T12–T16 land |

## Deferred

| Item | Reason |
|------|--------|
| M4L inbox helper | Out of scope unless Extension/OSC gaps remain |
| Full Compose → SMF export | Separate feature |
| AU packaging | After VST3 Windows green; macOS follow-up |
| Full automatable mod matrix in host | Expand after v1 param subset |

## Suggested parallel tracks

```text
Track A (bridge):  T1 → T2 → T3 → T4 → T5 → T6 → T7
Track B (plugin):  T10 → T11 → T12 / T13 → T14 → T15
Track C (Studio):  T8 → T9 → T16   (after T2; T16 after T14)
Merge gate:        T17
```

Track A and Track B may run in parallel after T0/T1.
