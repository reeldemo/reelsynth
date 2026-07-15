# ReelSynth UI reference

The standalone app (`reelsynth-app`) uses a fixed **1280×880** layout (see `ui/src/layout.rs`). Regions match [brand/mockups/COMPONENT_SPEC.md](../brand/mockups/COMPONENT_SPEC.md).

![Full window with numbered regions](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/01-full-window.png)

| # | Region | Purpose |
|---|--------|---------|
| 1 | Header | Open/Save preset, **Design / Compose** toggle, WT menu, MIDI device, Piano toggle, status |
| 2 | Oscillator column (left rail) | Per-osc level, pan, detune, unison, FM — **Design mode only** |
| 3 | Center column | Filter, ADSR, LFO, mod matrix, FX rack — **Design mode only** |
| 4 | WT editor | Position strip, 2D waveform, 3D surface, morph A/B — **Design mode only** |
| 5 | Scope strip | Live osc → filter → FX → out (scrollable when 3+ oscs) |
| 6 | Piano (optional) | On-screen keyboard, **88 keys A0–C8** with horizontal scroll |

---

## Compose mode

Toggle **Compose** in the header to replace the center column with a mini-DAW layout:

| Region | ~Height | Purpose |
|--------|---------|---------|
| Transport bar | 40 px | Play / stop / record, loop, metronome, BPM, snap grid |
| Track list | left rail | Mute / solo / arm / select (180 px) |
| Arrangement | 35% | Multi-track timeline, clip blocks, playhead scrub |
| Piano roll | 45% | Selected clip editor — draw, move, resize notes; velocity lane |
| Scene grid | 12% | 8 scenes × track columns — session launch |
| Keyboard strip | 8% | 88-key piano (bottom) — record when armed, else monitor |

**Interactions**

- Click arrangement clip → loads into piano roll
- Double-click empty bar → create clip
- Pencil tool → draw notes; Select → move; Eraser → delete
- `Delete` removes selected notes; Undo / Redo in piano roll toolbar
- Arm track + record → live input writes notes at playhead (UI-side; engine sync pending)

---

## Header

![Header detail](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/02-header-midi-save.png)

| Control | Action |
|---------|--------|
| **Open** | Load `.reelpreset`; resolves sibling `.reelwt` |
| **Save** | Write current patch as `.reelpreset` |
| **Design** / **Compose** | Switch shell mode — sound design vs mini-DAW |
| **WT** menu | Open/Save `.reelwt`, factory banks, Vital/WAV/Serum import |
| **Piano** | Show/hide on-screen keyboard |
| **Key / Scale / Layout** | Performance input: root key, scale mode, piano vs scale-fold vs chord row |
| **Arp** (footer) | Toggle arpeggiator; input mode, style, rate, octaves, gate, latch |
| **MIDI** combo | Select hardware MIDI input device |
| **Settings** window | Graphics backend (Auto/WGPU/Glow), GPU waveforms, auto MIDI, keyboard layout |
| **Status** | Audio/MIDI state, save confirmations, errors |

---

## Keyboard shortcuts

| Input | Notes |
|-------|-------|
| `Z S X D C V G B H N J M` | One octave — **QWERTY** play row (auto-detected) |
| AZERTY / QWERTZ | Same semitone positions on locale play row (Settings → Keyboard layout) |
| Click piano keys | Same as computer keys when piano visible |
| MIDI controller | Full keyboard range; **auto-connect** when a keyboard-like port appears |

In **Compose** mode, QWERTY and piano input route to the armed clip when recording; otherwise they monitor through the synth. Piano roll focus + pencil tool auditions quietly.

---

## Arpeggiator

Footer **Arp** toggle enables live arpeggiation in Design and Compose monitor paths. Settings persist in presets via `PerformanceSettings.arp`.

| Control | Options |
|---------|---------|
| Input | Single note (octave spread), held chord, scale degrees |
| Style | Up, Down, Up-Down, Down-Up, Random, As Played, Converge |
| Rate | 1/4 … 1/32, triplets — synced to project BPM |
| Octaves | 1–4 (single-note / scale modes) |
| Gate | Note length as fraction of step |
| Latch | Keep arping after key release |

**Compose:** Piano roll toolbar **Generate Arp** bakes a pattern into the selected clip (uses current arp settings). Recording with arp on writes the heard arpeggiated notes, not raw held input.

---

## Oscillator column (left rail)

![Osc, filter, ADSR](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/03-osc-filter-adsr.png)

- **Tabs** — switch OSC 1 / 2 / 3
- **Level, pan, detune, unison** — per oscillator
- **FM** — modulator routing (algorithm, ratio, index)
- **Warp** — phase warp modes on wavetable playback

WT position for the active osc syncs with the center WT strip.

### Wave stack (engine / Advanced)

`wave_layers` remain in the engine and Osc-column **Stack** panel for preset compatibility:

- Layer list: type (saw / sine / square / triangle / pulse / wavetable), level, detune, **+/− sign**, on/off
- Wavetable layers expose **WT Pos**
- **Mode**: Add, Avg (level-weighted), or **Avg Equal** (1/N per layer)
- **Autofix levels** when Add mode clips (scope **Result** shows **Stack clipping** warning)
- **+ Layer** / **Remove** per row

Design home no longer surfaces layer chips or the Stack 3D overlay — combine forms via **frames + morph**, not additive strip chips.

### WT strip (frames-only on Design)

The position strip is **frame/slot scrub only** on the Design surface — no L1/+/- layer chips, even when the patch still has `wave_layers`.

| Region | Content |
|--------|---------|
| Full-width strip | Frame/slot thumbnails + scrub; highlights the frame bound to **Edit** |

### Design WT panes (frames-first)

| Pane | Caption | Job |
|------|---------|-----|
| Left 2D | **Edit · Frame N** | Edit the strip-selected frame (knobs on wave = amplitude drag) |
| Right 3D | **Frame stack · this osc** | Morph mesh of this oscillator’s bank frames (default; Stack/Morph toggle hidden) |

### Quant hand drag (2D waveform)

When **Quant** > 0 and tool is **Select**:

- Vertical grid at each slot; **knob handles** at waveform intersections (hover when quant > 64)
- Drag snaps X to nearest slot on press; **locks slot** for entire gesture; fine Y edits **amplitude** (wave height) at that quant point
- **Interp** dropdown in the 2D toolbar (right side): **Hold** (step/rectangular bands), **Linear** (segments between knobs), **Spline** (Catmull-Rom smooth curve). Switching mode rebuilds the frame from current knob heights.
- Tooltip / status: **Drag knobs to reshape this frame**
- **Shape** menu (Saw / Square / Sine / Triangle) click-assigns a template to the selected frame
- **Curve** tool still edits slot→frame morph map; Select handles edit wave shape at quant points
- **Pencil** hidden when quant > 0 — use Select + handles instead
- The performance **piano/keyboard** fills the full-width band above the status footer; left/right sidebars stop above that band and never render into it

Factory Lead may still load engine stack layers for audio; Design UI stays frames-first. Save/reload preserves `wave_layers`, `invert`, and `stack_mode`.

### Effects (osc column sidebar)

When the osc column is visible, the **Effects** panel sits below the oscillator stack (above the mod matrix when both are open). Each slot is a full-width card in a vertical scroll chain:

| Region | Contents |
|--------|----------|
| **Header** | Muted slot index (`FX 1`, …) and **On/Off** bypass toggle — effect name is **not** repeated here |
| **Params** | Three parameters in a **2-row grid**: row 1 has two side-by-side cells (e.g. Mix + Rate); row 2 is the third param full-width. Labels sit **above** DragValues (10 px) with ≥72 px drag hit targets |
| **Footer** | ◀ ▶ ✕ reorder/remove icons (fixed strip) plus effect **type combo** (Chorus, Delay, …) using remaining width |

Slot height is sized for two readable param rows (~98 px at scale 1.0). The panel shows ~2¼ slots before scrolling; extra slots scroll inside the panel. The horizontal main FX rack (performance layout without osc column) keeps the compact single-row param layout unchanged.

---

## Center column

### Filter

- Filter 1 + Filter 2 (serial)
- Cutoff, resonance, type, key tracking, drive

### Envelopes

- **Amp ADSR** — overall loudness shape
- **Filter envelope** — filter movement over time

### LFO

- LFO 1 + LFO 2 — rate, depth, shape
- Targets via mod matrix

### Mod matrix

![Mod matrix and FX](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/05-mod-fx.png)

Up to 16 slots: source → target → amount. Sources include LFOs, envelopes, macros, velocity, mod wheel.

### FX rack

Serial chain: delay, reverb, chorus, etc. Per-slot mix and bypass.

---

## Wavetable editor

![WT 2D and 3D views](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/04-wt-editor-2d-3d.png)

### Concepts (engine)

| Concept | What it is | Sound effect |
|---------|------------|--------------|
| **Wavetable bank** | 256 frames × 2048 samples | Position picks timbre; morph moves between frames |
| **Wave quant** | Discrete slots (8–256) mapping to frame indices | Mod/LFO walks a **slot curve** — non-uniform slots = non-linear scans |
| **Wave stack** | `wave_layers[]` inside one Osc tab (saw + sine + WT…) | Additive thickness; `stack_mode: add` or `avg` |
| **Osc 1/2/3 tabs** | Separate oscillators + FM | Different from stack — FM routing between voices |

### Wavetable editor (v0.3 — frames-first)

| Element | Function |
|---------|----------|
| **Position strip** | Scrub frames; click slots when quant > 0; **Curve** mode shows mini frame-index bar under cells (no layer chips) |
| **Wave quant** | 8 / 16 / 32 / 64 / **256** / Smooth (256 uses wire value `255`) |
| **Morph A / B / amount** | Crossfade between frame ranges (overrides slots when active) |
| **Edit (2D)** | Selected frame; **Select** knobs + drag; **Curve** slot→frame map; **Shape** control points; **Shape→** templates |
| **Frame stack (3D)** | Default Morph mesh of this osc’s bank (16-frame depth slices); Design home hides Stack overlay toggle |
| **Toolbar** | Select / **Curve** / **Shape** / **Shape** menu (Saw·Square·Sine·Tri) / **FFT** (engine layers; keeps Morph right pane) |

**Three concepts on Design:** **Edit** (selected frame) · **Frame stack** (this osc) · **Result** (all oscs in the scope strip).

Morph settings are per-oscillator; active tab syncs with WT controls.

Design spec: [docs/superpowers/specs/2026-07-15-wt-stack-editor-design.md](superpowers/specs/2026-07-15-wt-stack-editor-design.md)  
Implementation plan: [docs/superpowers/plans/2026-07-15-wt-stack-editor.md](superpowers/plans/2026-07-15-wt-stack-editor.md)

### Legacy reference (pre–frames-first)

| Element | Function |
|---------|----------|
| **Stack overlay** | Layer strokes + composite (engine/Advanced; not Design home chrome) |
| **Hybrid strip chips** | L1/+/- on strip (removed from Design surface) |

---

## On-screen piano

![Piano keyboard](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/06-piano-keyboard.png)

- **88 keys** — MIDI 21–108 (A0–C8) with horizontal scroll
- Toggle via header **Piano** button
- **Scale fold** — out-of-scale keys dimmed per performance settings (Compose mode and Scale layout)
- Height: 128 px; white keys 28 px wide (scroll when window is narrow)

---

## Scopes

Signal-chain strip at top of center column (~68 px, horizontally scrollable when 3+ oscillators):

| Cell | Content |
|------|---------|
| **Osc** | Per-osc waveform when ≥3 oscillators; combined cycle otherwise |
| **Filter** | Post-filter tap (responds to cutoff/resonance) |
| **FX** | Post-FX tap (distinct when delay/chorus active) |
| **Result** | Spectrum of all oscillators after Filter/FX (tooltip explains mix); slightly wider when ≥2 oscs; amber border + **Stack clipping** when Add-mode layers exceed ±1 |

Settings window (app): **Graphics** backend Auto/WGPU/Glow, GPU waveforms toggle; **Input** auto MIDI + keyboard layout override.

---

## UI audit tests

Automated layout and contrast checks live in `reelsynth-ui` (`ui/tests/kittest.rs` + `ui/tests/common/audit_harness.rs`).

### What they guard

| Check | Module | Examples |
|-------|--------|----------|
| Shell geometry | `layout_audit.rs` | No overlapping header/main/footer bands; center sub-regions stack cleanly |
| Sidebar parity | `layout.rs` + `audit_registry.rs` | Osc column and rail share **252 px** width |
| Panel utilization | `layout_audit.rs` | FX/mod/filter panels use ≥50% of allocated area at 1280×880 |
| WCAG contrast | `contrast_audit.rs` | `text`, `text_muted`, `accent_on`, scope trace colors on `bg` / `surface2` |
| Element registry | `audit_registry.rs` | ~95 `AuditId` entries with bounds / overflow / overlap checks |

### Running locally

```bash
cargo test -p reelsynth-ui --test kittest
cargo test -p reelsynth-ui
```

The harness test `full_ui_audit_with_registry` runs `audit_all_elements()` after a full Design-mode shell render.

### Adding rects for new panels

1. Add an `AuditId` variant in `ui/src/audit_registry.rs` (keep `REGISTRY_VARIANT_COUNT` in sync).
2. At the end of the panel's draw function, call:

   ```rust
   use crate::audit_registry::{record_region, AuditId};

   record_region(ui.ctx(), AuditId::MyPanel, allocated_rect, ui.min_rect());
   ```

3. Optional: pass `audit_id: Option<AuditId>` into `widgets/panel.rs` helpers for automatic recording.
4. Add a kittest scenario in `ui/tests/kittest.rs` (or extend `ShellAuditScenario`) that exercises the new UI path.

CI runs the kittest suite on every push/PR (`.github/workflows/ci.yml`).

---

## Plugin editor (preview only)

`reelsynth-plugin-editor` shares this UI but **does not process audio or MIDI** yet. Message: *"Plugin editor spike — UI only (no audio I/O)"*. Real host bindings: S7 roadmap — see [plugin/README.md](../plugin/README.md).

---

## Screenshots

Images load from [GitHub Releases](https://github.com/reeldemo/reelsynth/releases) (not committed to the repo). Re-capture when UI sprints change layout — see [CONTRIBUTING.md](../CONTRIBUTING.md).
