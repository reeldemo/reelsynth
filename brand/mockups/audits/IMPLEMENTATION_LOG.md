# ReelSynth UI Implementation Log

Progress tracker for the UI platform plan (`reelsynth_ui_platform` + `reelsynth_ui_redesign`).

## 2026-07-13 — Loop iteration 3 (S3–S5 complete)

### S3 — Osc column + WT tools ✅

| Item | Status |
|------|--------|
| 280px osc column (Osc1–3 tabs) | ✅ `osc.rs` |
| Per-osc level/pan/coarse, WT position + unison sliders | ✅ |
| Sub / Noise knobs wired to engine | ✅ |
| Macros panel (disabled stub, S6 routing note) | ✅ |
| Live ADSR + LFO rail (was disabled in S1) | ✅ |
| WT morph A→B bar (`wt/morph.rs`) | ✅ |
| Frame pencil edit + toolbar (`wt/toolbar.rs`, `view_2d.rs`) | ✅ |
| Import Vital / WAV folder / Serum from WT menu | ✅ `main.rs` |
| Center layout: strip → morph → 2D/3D (no hero when osc on) | ✅ |
| `cargo test --no-default-features -j 1` | ✅ |

**Commit:** `c8a80e6` — osc column, mod matrix, FX rack, WT tools

### S4 — Mod matrix ✅

| Item | Status |
|------|--------|
| Collapsible section below main (160px expanded) | ✅ `mod_matrix.rs` |
| 8 mock routes matching `index.html` | ✅ |
| Amount drag, curve click-cycle, On/Off toggle | ✅ UI stub |
| Layout stack: main → mod → fx → piano → footer | ✅ `layout.rs` |
| Viewport 1280×820 when mod/FX enabled | ✅ |

### S5 — FX rack ✅

| Item | Status |
|------|--------|
| Collapsible Effects section (120px expanded) | ✅ `fx_rack.rs` |
| 160px slot cards (Chorus, Delay, Reverb, + Slot) | ✅ |
| Active border + bypass toggle click | ✅ UI stub |
| 3 active meta in header | ✅ |

### S2 carry-over (closed this loop)

| Item | Status |
|------|--------|
| Frame draw/edit | ✅ S3 |
| Morph A→B | ✅ S3 |
| Import Vital/WAV/Serum | ✅ S3 |
| egui-in-plugin-host spike | ⬜ S6 |

### Next loop

1. S6: minimal CLAP plugin shell + egui embed spike
2. Wire mod matrix rows to `Patch::mod_matrix` / engine
3. FX slot bypass → audio FX chain (stub ok)
4. Macro knobs → mod matrix destinations
5. UI audit vs `index.html` landmarks (<1px gate)

### Sprint summary

| Sprint | Status |
|--------|--------|
| S-brand | ✅ |
| S0 | ✅ |
| S1 | ✅ |
| S2 | ✅ |
| S3 | ✅ |
| S4 | ✅ |
| S5 | ✅ |
| S6 | ⬜ |
| S7 | roadmap |

## 2026-07-12 — Loop iteration 2 (S2 complete)

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

### S2 — WT editor ✅

| Item | Status |
|------|--------|
| WT position strip (center + rail knob synced) | ✅ |
| 2D waveform view from current bank frame (`view_2d.rs`) | ✅ |
| 3D mesh surface from bank slices + rib grid (`view_3d.rs`) | ✅ |
| Reveal panels via `S1ShellConfig::show_wt_editor` | ✅ app enables |
| Bank hot-swap on preset load | ✅ |
| WT header menu: Open/Save `.reelwt` + factory banks | ✅ |
| Save `.reelwt` via `WavetableBank::write_file` | ✅ |
| Center layout: hero → strip → 2D/3D views (COMPONENT_SPEC) | ✅ |

**Commit:** `9730fd4` — WT import/save, 2D/3D bank views, layout audit
