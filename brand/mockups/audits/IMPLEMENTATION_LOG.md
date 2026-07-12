# ReelSynth UI Implementation Log

Progress tracker for the UI platform plan (`reelsynth_ui_platform` + `reelsynth_ui_redesign`).

## 2026-07-12 — Loop iteration 2

### S1 — Standalone shell ✅

| Item | Status |
|------|--------|
| Preset Open/Save (`.reelpreset` via `rfd`) | ✅ |
| MIDI input device select + note routing (`midir`) | ✅ |
| Functional header (wordmark, Open/Save, MIDI, piano toggle, status) | ✅ |
| Wired WT + filter params | ✅ (prior) |
| Piano keyboard + QWERTY | ✅ (prior) |
| `cargo test --no-default-features -j 1` | ✅ |

**Commit:** `c252ca6` — preset I/O, MIDI routing, functional header

### S2 — WT editor (in progress)

| Item | Status |
|------|--------|
| WT position strip (center + rail knob synced) | ✅ (S1) |
| 2D waveform view (`ui/src/wt/view_2d.rs`) | ✅ |
| 3D mesh surface (`ui/src/wt/view_3d.rs`) | ✅ placeholder mesh from bank |
| Reveal panels via `S1ShellConfig::show_wt_editor` | ✅ app enables |
| Bank hot-swap on preset load | ✅ |
| Frame draw/edit | ⬜ S2+ |
| Morph A→B | ⬜ S2+ |
| Import Vital/WAV/Serum | ⬜ S2+ |
| egui-in-plugin-host spike | ⬜ end of S2 |

**Commit:** `c252ca6` — S2 WT 2D/3D views + `show_wt_editor` gate

### Next loop

1. S2 parity audit vs `index.html` WT center region
2. WT import + `.reelwt` save from UI
3. Frame draw/edit stub or minimal pencil tool
4. Morph controls (position-only stub ok if engine supports)
5. S2 end spike: minimal CLAP + egui embed

### Sprint summary

| Sprint | Status |
|--------|--------|
| S-brand | ✅ |
| S0 | ✅ |
| S1 | ✅ |
| S2 | 🔄 in progress |
| S3 | ⬜ |
| S4 | ⬜ |
| S5 | ⬜ |
| S6 | ⬜ |
| S7 | roadmap |
