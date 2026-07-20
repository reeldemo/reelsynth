# ReelSynth UI reference

The standalone app (`reelsynth-app`) uses a fixed **1280×880** layout (see `ui/src/layout.rs`). Regions match [brand/mockups/COMPONENT_SPEC.md](../brand/mockups/COMPONENT_SPEC.md).

![Full window with numbered regions](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/01-full-window.png)

| # | Region | Purpose |
|---|--------|---------|
| 1 | Header | Open/Save preset, **Design / Compose** toggle, WT menu, Audio + MIDI devices, Piano toggle, status |
| 2 | Oscillator column (left rail) | Per-osc level, pan, detune, unison, FM — **Design mode only** |
| 3 | Center column | Filter, ADSR, LFO, mod matrix, FX rack — **Design mode only** |
| 4 | WT editor | Position strip, 2D waveform, 3D surface, morph A/B — **Design mode only** |
| 5 | Scope strip | Live osc → filter → FX → out (scrollable when 3+ oscs) |
| 6 | Piano (optional) | On-screen keyboard, **88 keys A0–C8** with horizontal scroll |

---

## Compose mode

Toggle **Compose** in the header for an Ableton-style **clip editor**:

| Region | ~Height | Purpose |
|--------|---------|---------|
| Transport bar | 40 px | Play / stop / record, loop, metronome, BPM, snap grid |
| Track list | left rail | Mute / solo / arm / select (180 px) |
| Clips strip | collapsed | Header **Clips ▸** — expand for multi-track timeline / multi-clip (hidden by default) |
| Piano roll | ~70%+ | Dominant clip editor — playable keys, beat grid, notes, velocity + automation |
| Scenes | collapsed | Header **Scenes ▸**; expand for 8×track session launch grid |
| Keyboard strip | footer | Optional 88-key piano — record when armed, else monitor |

A default empty clip is auto-selected on the active track so you can draw immediately (no clip-strip click required).

**Interactions**

- Hold left **piano keys** → note on/off with highlight (same pitches as QWERTY Z–M with letter glyphs on white keys)
- **Pencil** (default) → click or drag on the grid to draw notes (toolbar hint shows the active tool)
- **Select** → drag notes to move; drag note edges to resize; click to audition
- **Eraser** → click notes to delete; `Delete` also removes the selection
- Undo / Redo in the piano roll toolbar
- Expand **Clips ▸** only when you need the timeline (select / create additional clips, scrub playhead)
- Scroll wheel → pitch scroll; Shift/horizontal scroll → beat scroll; Ctrl+wheel → beat zoom
- Transport **▶** plays scheduled clip notes through the synth (seek into selected clip if playhead is outside it)
- Arm track + record → live input writes notes at playhead and monitors

---

## Header

![Header detail](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/02-header-midi-save.png)

| Control | Action |
|---------|--------|
| **Open** | Load `.reelpreset`; resolves sibling `.reelwt` |
| **Save** | Write current patch as `.reelpreset` |
| **Design** / **Compose** | Switch shell mode — sound design vs mini-DAW |
| **WT** menu | Open/Save `.reelwt`, **factory wavetables** (applies bank to the Design wave stack — promotes a wavetable layer so sound matches the editor), Vital/WAV/Serum import |
| **Piano** | Show/hide on-screen keyboard |
| **Key / Scale / Layout** | Performance input: root key, scale mode, piano vs scale-fold vs chord row |
| **Arp** (footer) | Toggle arpeggiator; input mode, style, rate, octaves, gate, latch |
| **MIDI** combo | Select hardware MIDI input device |
| **Audio** combo | Select CPAL output device (speakers, headphones, DI / interface) |
| **Settings** | Header **Settings** dropdown (not a modal): graphics backend, GPU waveforms, auto MIDI, auto audio output, keyboard layout |
| **Status** | Audio/MIDI state, save confirmations, errors |

---

## Keyboard shortcuts

| Input | Notes |
|-------|-------|
| `Z S X D C V G B H N J M` | One octave — **QWERTY** play row (auto-detected) |
| AZERTY / QWERTZ | Same semitone positions on locale play row (Settings → Keyboard layout) |
| Click piano keys | Same as computer keys when piano visible |
| MIDI controller | Full keyboard range; **auto-connect** when a keyboard-like port appears |
| Audio output | Header **Audio** combo; **auto-select** when a new output device appears (DI / interface) |

In **Compose** mode, QWERTY, MIDI, left piano-roll keys, and pencil/select audition share one note on/off path (performance Key/Scale/Layout still apply). Recording to an armed clip also monitors.

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

### Wave stack (layer-first on Design)

Design home is built around additive **`wave_layers`** (default **3**: saw + sine + square at balanced levels). Empty presets auto-seed three layers on load without mutating the file until save.

| Region | Content |
|--------|---------|
| **Layer strip** | Full-width L1/L2/L3 chips — select layer, drag level, **+/−** invert sign, **+ / −** add/remove layers |
| **Osc column → Stack** | **Mode** (Add / Avg / Avg Equal), **Autofix levels**; per-layer detune / pulse / WT pos under **Advanced layer params** |

- Wavetable layers expose **WT Pos** (Advanced panel or composite-pane scrub)
- **Autofix levels** when Add mode clips (scope **Result** shows **Stack clipping** warning)

### WT strip (layer-first on Design)

The position strip is **layer chips only** on Design — no 256-frame scrub.

| Region | Content |
|--------|---------|
| Full-width strip | Layer thumbnails + select; add/remove layers at strip edge |

### Design WT panes (three columns, left → right)

| Column | Caption | Job |
|--------|---------|-----|
| **1 Result** | **Result · N · {stack_mode}** | Stack sum with fill; dim sibling layer strokes. Hover near a layer curve previews it (thicker stroke + hand cursor); click commits `selected_layer_idx`. Drag Y=level, X=phase/WT. **Overlay method** combo (`add` / `avg` / `avg_equal`) in the pane caption writes `osc.stack_mode` and updates audio. **Result Quant** (when Quant > 0) reshapes the total via **Residual**; the **selected** WT/residual curve also shows its Quant knobs (siblings and VA curves stay stroke-only). Knob proximity wins over curve click |
| **2 Layers** | **Layers · Osc N** | Every enabled layer labelled (`L1 · saw`, …). Hover nearest curve within ~14 px previews selection (`Hover · L2 · saw`); click commits selection; **Quant knobs only on the selected** WT/residual curve at its edit frame (VA / non-selected: stroke only). Hovering a *different* layer curve prefers selection over Quant knob grab (so L1/L2 stay selectable when L3 has knobs) |
| **3 Selected** | **Edit · Layer N · {type}** | Always the selected layer (fill + thick). Toolbar: **Select / Shape / Interp (All·… + per-segment)** when the layer is WT/residual and Quant > 0; **QuantHandleEditor** knobs for drag reshape; Pencil / Curve / Shape tools. VA selection shows a status hint instead of knobs |

**Residual layer** — first Result Quant drag appends one wavetable layer (`residual: true`, shown as **Residual** in the strip). Stack mode switches to **add** if needed. Further Result edits update the same layer; math: `residual[i] = (desired[i] − others[i]) / (sign × level)`.

**Shape** menu (Saw / Square / Sine / Triangle) sets the **active layer `source_type`** on the Selected column — it does not overwrite an arbitrary WT frame index.

### Quant hand drag

| Pane | Quant target |
|------|----------------|
| **Result** | Composite stack sum — writes **Residual** frame |
| **Layers** | Selected WT/residual curve knobs at that layer's edit frame |
| **Selected** | Active layer curve (level / invert scaled) |

When the **active layer** is **wavetable** and **Quant** > 0:

- Vertical grid at each slot; knob handles sit on the editable curve
- Grab only works near a dot (curve snap); cursor is **grab** / **grabbing**
- Hover snaps to the nearest knob: enlarged + brighter fill/stroke, outer glow ring, vertical slot guide, and status/tooltip `Slot N · amp ±x.xx`
- The curve under that knob (Result residual / Layers WT / Selected) thickens and brightens while hovering
- Drag locks that slot; Y edits **amplitude**; quantized polyline updates under the knobs using **per-segment** interp
- Edge knobs (slot 0 and last) are **always** shown and editable, including sparse Quant (>64)
- **Interp (All·…)** on Selected toolbar: curve-wide default — applies the same mode to **all segments** of that layer
- Click a Quant knob to select it; **slot→slot+1** dropdown edits that segment only (last knob shows `end · no next`)
- Modes: **Hold**, **Linear**, **Spline** (Catmull-Rom), **Poly** (cubic Lagrange), **Expo** (exponential ease), **MA** (linear + short box filter)
- VA layers: level/phase drag only (no frame quant)

**Morph A/B bar** is hidden on Design home (frame-bank morph remains in preset schema for compatibility). Save/reload preserves `wave_layers`, `invert`, `stack_mode`, `quant_interp`, and `quant_segment_interps`.

### Effects (osc column sidebar)

When the osc column is visible, the **Effects** panel sits below the oscillator stack (above the mod matrix when both are open). Each slot is a full-width card in a vertical scroll chain:

| Region | Contents |
|--------|----------|
| **Header** | Muted slot index (`FX 1`, …) and **On/Off** bypass toggle — effect name is **not** repeated here |
| **Params** | Three parameters in a **2-row grid**: row 1 has two side-by-side cells (e.g. Mix + Rate); row 2 is the third param full-width. Labels sit **above** DragValues (10 px) with ≥72 px drag hit targets |
| **Footer** | ◀ ▶ ✕ reorder/remove icons (fixed strip) plus effect **type combo** (Chorus, Delay, …) using remaining width |

Slot height is sized for two readable param rows (~98 px at scale 1.0). The panel shows ~2¼ slots before scrolling; extra slots scroll inside the panel. The horizontal main FX rack (performance layout without osc column) keeps the compact single-row param layout unchanged.

### Overtone (header menu)

Header **Overtone** opens a master-bus anti-crackle chain (runs **after** voices sum + master gain, **before** the musical Effects rack). Same interaction pattern as Effects:

| Control | Behavior |
|---------|----------|
| **+ Add filter** | Append Lowpass at 100% strength |
| Type combo | **Lowpass** \| **Harmonic** \| **Slew** (stackable; order matters) |
| ◀ / ▶ / ✕ | Reorder / remove (empty chain = Off / identity) |
| Strength | Per-slot 0–100%; effective amount also scales with WT frame harshness |
| On/Off | Per-slot bypass |

Separate from **Quant Seam** (WT toolbar; edits the frame wrap). Session-only for v1 (not saved in `.reelpreset`).

---

## Center column

### Filter

- **Chainable** serial SVF slots (FxChain / Overtone UX): **+ Add filter**, type combo, ◀ ▶ ✕ reorder/remove, per-slot On/Off
- Types: **Lowpass**, **Highpass**, **Bandpass**, **Notch** (up to 8 slots)
- Per-slot cutoff, resonance, drive, key tracking
- Empty chain = bypass; legacy presets without `filters` keep Filter 1 → Filter 2 dual behavior
- Distinct from header **Overtone** (master-bus anti-crackle)

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
| **Factory wavetable (WT menu)** | Loads a bank **and** promotes a wavetable layer (ducks VA siblings) | Sound matches the editor — not bank-name-only |

### Wavetable editor (v0.3 — layer-first Design)

| Element | Function |
|---------|----------|
| **Layer strip** | Select layers; level drag; add/remove; invert sign per chip |
| **Wave quant** | 8 / 16 / 32 / 64 / **256** / Smooth — active only for **wavetable** layers with quant > 0 |
| **Morph A / B / amount** | Hidden on Design home; still in preset schema |
| **Result (col 1)** | Stack sum + residual quant (`WtViewResult`) |
| **Layers (col 2)** | Labelled per-layer strokes + multi-WT quant (`WtView3dStack`) |
| **Selected (col 3)** | Active layer edit + toolbar (`WtSelectedLayerView`) |

**Three concepts on Design:** **Result** (col 1 stack sum + residual quant) · **Layers** (col 2 labelled curves + strip chips) · **Selected** (col 3 edit focus) · **Scope Result** (all oscs after Filter/FX).

Morph mesh / frame-bank scrub remain available on non-Design paths (Compose / advanced).

Design spec: [docs/superpowers/specs/2026-07-15-wt-stack-editor-design.md](superpowers/specs/2026-07-15-wt-stack-editor-design.md)  
Implementation plan: [docs/superpowers/plans/2026-07-15-wt-stack-editor.md](superpowers/plans/2026-07-15-wt-stack-editor.md)

### Legacy reference (frame-bank morph)

| Element | Function |
|---------|----------|
| **Frame strip** | 256-frame / slot scrub (`StripMode::Frames`) |
| **Morph A/B bar** | Crossfade between frame ranges |
| **Frame stack (3D mesh)** | `WtView3d` morph mesh (not wired on Design home) |

---

## On-screen piano

![Piano keyboard](https://github.com/reeldemo/reelsynth/releases/download/v0.1.0/06-piano-keyboard.png)

- **88 keys** — MIDI 21–108 (A0–C8) with horizontal scroll
- Toggle via header **Piano** button
- **Scale fold** — out-of-scale keys dimmed when performance layout is **Scale** (same rule in Design and Compose; default **Piano** layout stays chromatic so black keys play)
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

Settings (header dropdown): **Graphics** backend Auto/WGPU/Glow, GPU waveforms toggle; **Input** auto MIDI, auto-select new audio output, keyboard layout override.

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
