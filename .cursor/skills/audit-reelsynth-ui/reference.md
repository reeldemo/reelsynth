# ReelSynth UI Audit — Reference

## Related skills (gap analysis)

| Skill | Location | Relevance | Gap |
|-------|----------|-----------|-----|
| `canvas` | `~/.cursor/skills-cursor/canvas/` | Rich side-by-side regression layouts | Generic; no ReelSynth regions, tokens, or sprint rules |
| `pr-review-canvas` | plugins/cache/…/pr-review-canvas/ | PR diff review canvas | Code diffs, not pixel/mockup parity |
| `review` / `review-bugbot` | skills-cursor | Code quality & security | No visual/UI dimension |
| `studio-design-review` | lic-sim-export-wt project | Screenshot iteration + UX scoring | Li Studio web stack, not egui/mockup workflow |
| `studio-ui-ux-rubric` | lic-sim-export-wt project | UX-01…14 competitive rubric | Wrong product; no COMPONENT_SPEC mapping |
| `figma-use` / `figma-implement-motion` | Figma plugins | Design tool ops | ReelSynth uses HTML mockups, not Figma |
| `docs-canvas` | plugins/cache/…/docs-canvas/ | Documentation layout | Not UI audit |

**This skill fills:** ReelSynth-specific screenshot → mockup parity workflow for Rust/egui, with sprint visibility rules, token/checklist mapping to `brand/mockups/`, Gate 1/2 gates, and fix routing to `ui/` + `ui-theme/`.

---

## Severity rubric

| Severity | Definition | Examples |
|----------|------------|----------|
| **Critical** | Blocks gate sign-off; wrong sprint scope; broken affordance | Unshipped osc column visible in S1; live knobs not wired; theme not applied (light/wrong bg); piano unusable |
| **Major** | Visible parity break >4px or wrong token/component | Header 56px not 48px; knob 40px not 48px; ADSR interactive in S1; wrong accent colour; missing WT strip |
| **Minor** | Within ~4px but noticeable; secondary typography | Label 12px not 13px; gutter 6px not 8px; muted text too bright |
| **Polish** | Cosmetic; motion; micro-interaction | Hover mix slightly off; chevron rotation timing; badge spacing |

**Parity tolerance:** ≤4px on layout regions vs mockup at 1× (1280×720 S1 / 1280×820 S6 canonical). Widget internal geometry (arc stroke, gradient) judged qualitatively against `components.html`.

---

## Agent screenshot audit (summary)

| Step | Action |
|------|--------|
| **1. Capture** | Launch `reelsynth-ui` → `screencapture` → `brand/mockups/audits/` |
| **2. Compare** | Read PNG + diff vs `s1-performance.html` + `COMPONENT_SPEC.md` |
| **3. Loop** | `/loop audit UI` — max 5×: screenshot → audit → fix Critical/Major in `ui/` → rebuild |
| **4. Exit** | S1 ~4px parity; piano ~18px keys; alignments match mockup |

Full workflow: [SKILL.md § Screenshot audit workflow](SKILL.md#screenshot-audit-workflow).

---

## Capture script (macOS)

Run from repo root after build + launch. Output: `brand/mockups/audits/YYYY-MM-DD_HH-MM-ss-app.png`.

### Build + launch

```bash
cd /Users/julian/Documents/coding-projects/reelsynth
pkill -f 'target/debug/reelsynth-ui' || true
cargo build -p reelsynth-app --bin reelsynth-ui
./target/debug/reelsynth-ui &
sleep 3   # wait for window + first frame
```

### Capture app window

```bash
AUDIT_DIR=brand/mockups/audits
mkdir -p "$AUDIT_DIR"
STAMP=$(date +%Y-%m-%d_%H-%M-%S)
OUT="$AUDIT_DIR/${STAMP}-app.png"

WIN_ID=$(osascript <<'APPLESCRIPT'
tell application "System Events"
  repeat with p in (every process whose name is "reelsynth-ui")
    try
      repeat with w in (every window of p whose name contains "ReelSynth")
        return id of w
      end repeat
    end try
  end repeat
end tell
APPLESCRIPT
)

if [ -n "$WIN_ID" ] && [ "$WIN_ID" != "missing value" ]; then
  screencapture -o -l"$WIN_ID" "$OUT"
else
  screencapture -o -l$(osascript -e 'tell app "System Events" to id of front window') "$OUT"
fi
echo "Saved: $OUT"
```

| Method | When to use |
|--------|-------------|
| `screencapture -l<winID>` | **Preferred** — crops to ReelSynth window |
| Front-window fallback | osascript process lookup failed |
| Browser MCP screenshot | Optional mockup reference at 1280×720 |

**Window identity:** title `"ReelSynth"`, process `reelsynth-ui`, viewport 1280×720 (`APP_HEIGHT_S1`).

### Compare (after capture)

1. **Read** the PNG with the Read tool.
2. **Reference:** `brand/mockups/s1-performance.html` + `brand/mockups/COMPONENT_SPEC.md` (+ `mockups.css` for tokens).
3. Walk regions; score Critical / Major / Minor / Polish.

### Loop + cleanup

```bash
# Loop: fix Critical/Major in ui/ → cargo build -p reelsynth-app --bin reelsynth-ui → re-capture (max 5×)
pkill -f 'target/debug/reelsynth-ui' || true
```

---

## Pixel-check landmarks (S1)

Use these when screenshot diff suggests misalignment. Source: `COMPONENT_SPEC.md` + `mockups.css` + `ui/src/layout.rs`.

| Landmark | CSS / mockup | egui constant | Tolerance |
|----------|--------------|---------------|-----------|
| Viewport | 1280×720 | `APP_WIDTH` × `APP_HEIGHT_S1` | exact |
| Header height | 48px (`--grid-unit` × 6) | `HEADER_HEIGHT` | ≤4px |
| Footer height | 36px | `FOOTER_HEIGHT` | ≤4px |
| Right rail width | 240px | `RAIL_WIDTH` | ≤4px |
| WT strip height | 72px | `WT_STRIP_HEIGHT` | ≤4px |
| Piano keyboard height | 80px (`--piano-h`) | `PIANO_HEIGHT` | ≤4px |
| Piano white key width | 18px (`--piano-white-w`) | `PIANO_WHITE_KEY_WIDTH` | ≤4px |
| Piano wrap total | 96px (16px pad + 80px keys) | `GRID_UNIT * 2 + PIANO_HEIGHT` | ≤4px |
| Knob sm / lg | 48px / 64px | `KNOB_SM` / `KNOB_LG` | ≤4px |
| Panel padding | 8px | `GRID_UNIT` | ≤4px |
| Knob row gap | 12px | `SPACE_SM` | ≤4px |
| ADSR graph height | 80px | rail panel | ≤4px |

**Piano readability check:** 14 white keys × 18px = 252px keyboard width, centered in wrap. Keys must not stretch to fill viewport width.

**Disabled ADSR/LFO check:** opacity ~0.38, non-interactive (`panel_disabled` / `ui.add_enabled_ui(false, …)`). Visible but greyed — not hidden, not fully live.

---

## Loop mode exit criteria

| Criterion | Pass signal |
|-----------|-------------|
| S1 layout | Matches `s1-performance.html` region map |
| Piano | 18px keys, 80px tall, readable |
| Alignment | All landmarks within ~4px |
| Borders | Panel edges visible (`--border` #27272a) |
| Disabled groups | ADSR/LFO greyed, knobs non-draggable |
| Sprint scope | No osc/mod/FX/2D/3D columns |

Max **5 iterations** per session; arm `AGENT_LOOP_WAKE_reelsynth-ui-audit` sleeper if not done.

---

## Region map

### S1 (`s1-performance.html`)

```
┌─────────────────────────────────────────────────────────┐
│ Header 48px — wordmark only (no preset bar/transport)   │
├──────────────────────────────────────┬──────────────────┤
│ Center (flex)                        │ Right rail 240px │
│  · Preset hero (name, category,      │  · WT position  │
│    static spectrum SVG)              │    knob (wired)  │
│  · WT position strip 72px            │  · Filter knobs  │
│                                      │  · ADSR disabled │
│                                      │  · LFO disabled  │
├──────────────────────────────────────┴──────────────────┤
│ Piano wrap (optional) — 96px total (16px pad + 80px keyboard), 18px white keys │
├─────────────────────────────────────────────────────────┤
│ Footer 36px — piano toggle + status/MIDI                 │
└─────────────────────────────────────────────────────────┘
```

**Must NOT appear in S1:** osc left column, mod matrix, FX rack, WT 2D/3D views, preset file bar in header.

### S6 full (`index.html`)

Three columns: osc 280px | center hero (WT strip + 2D + 3D) | rail 240px. Below: mod matrix 160px (collapsible), FX rack 120px (collapsible), piano footer.

### Narrow (`narrow.html`)

Osc 220px, rail 200px, mod/FX collapsed, piano hidden, WT views compressed but visible.

---

## Layout checklist

| Check | S1 expected | S6 expected | Source |
|-------|-------------|-------------|--------|
| Viewport default | 1280×720 (`APP_HEIGHT_S1`) | 1280×820 | `layout.rs`, COMPONENT_SPEC |
| Header height | 48px | 48px | `--grid-unit` × 6 |
| Footer height | 36px | 36px | 4.5×8 snapped |
| Osc column | hidden | 280px min | DECISIONS |
| Right rail | 240px | 240px | `RAIL_WIDTH` |
| WT strip | 72px | 72px | `WT_STRIP_HEIGHT` |
| Piano height | 80px | 80px | `PIANO_HEIGHT` / `--piano-h` |
| Piano wrap | 96px | 96px | `GRID_UNIT * 2 + PIANO_HEIGHT` |
| White key width | 18px fixed | 18px fixed | `--piano-white-w`; not flex-stretch |
| Panel padding | 8px | 8px | `--space-xs` |
| Knob row gap | 12px | 12px | `--space-sm` |

---

## Colour & token checklist

| Token | Hex | Where to verify |
|-------|-----|---------------|
| `--bg` | `#0a0a0a` | App canvas / window fill |
| `--bg-muted` / panels | `#18181b` | Panel fills |
| `--surface2` | `#141416` | Header bar |
| `--text` | `#fafafa` | Primary labels |
| `--text-muted` | `#a1a1aa` | Secondary labels |
| `--accent` | `#183d50` | Fills, playhead, active tab |
| `--accent-ui` | `#2a6b8a` | Knob arc, wired badge, hover border |
| `--accent-muted` | `#061e2a` | Hover backgrounds |
| `--border` | `#27272a` | Panel edges, knob track |

egui mapping: `ui-theme/src/lib.rs` → `Tokens` + `ACCENT_UI`.

---

## Typography checklist

| Context | Font | Size | egui helper |
|---------|------|------|-------------|
| Wordmark | IBM Plex Sans semibold | 15px | `heading_font(15.0)` |
| Panel title | IBM Plex Sans uppercase | 11px | `heading_font(11.0)` |
| Body labels | Inter | 13px | Proportional default |
| Knob values, mono | JetBrains Mono | 11px | `FontFamily::Monospace` |
| Preset name | IBM Plex Sans | 14px | `heading_font(14.0)` |

**Audit signal:** generic system font on wordmark/panel titles → font binding bug.

---

## Widget checklist

### Knobs (`.rs-knob`)

| Variant | Size | Notes |
|---------|------|-------|
| `--sm` | 48×48 | Rack default |
| `--md` | 56×56 | |
| `--lg` | 64×64 | S1 emphasis |
| `--wired` | + glow ring | "Live" badge on WT position, filter |
| `--disabled` | same dial | grey pointer, non-interactive |

Arc: 270° sweep, 3px stroke, track `--border`, fill accent/accent-ui. Pointer: 2×38% height rect rotated by value.

### Piano (`.rs-piano`)

- 14 white keys, 2 octaves, start C3 (note 48)
- Container 80px tall (`--piano-h`); wrap 96px with 16px vertical padding
- White key: 18px fixed width (`--piano-white-w`); total keyboard 252px centered
- Black key: 58% white width, 56% piano height
- Active key: `--accent-ui` gradient fill
- Toggle: footer `.rs-toggle` / `state.piano_visible`

### WT strip (`.rs-wt-strip`)

- 72px container; equal-width frame cells
- Playhead 2px at position/255
- Active frame border `--accent-ui`
- Default demo: frame 108/255, Saw Morph bank

### Disabled groups (S1)

- ADSR graph + 4 knobs: visible but `rs-group--disabled` (opacity ~0.38)
- LFO panel: same
- Must use `panel_disabled` / `ui.add_enabled_ui(false, …)` — not hidden, not fully interactive

### Tabs, sliders, mod matrix, FX

See `COMPONENT_SPEC.md` for S6; only audit when visible in screenshot.

---

## Sprint panel visibility

| Panel | S1 | S2+ | Rule |
|-------|----|----|------|
| Preset hero | ✅ | ✅ | Static spectrum S1 |
| WT strip | ✅ | ✅ | |
| WT 2D/3D | ❌ hidden | ✅ | No dimmed placeholder |
| Osc column | ❌ | ✅ | |
| Mod matrix | ❌ | ✅ collapsible | |
| FX rack | ❌ | ✅ collapsible | |
| ADSR/LFO rail | visible disabled | ✅ live | S1 honest grey-out |

---

## egui pitfalls

Check these when audit findings point to "looks wrong" but layout numbers seem fine:

1. **Theme not applied** — `reelsynth_ui_theme::apply(ctx)` must run in `eframe::App::new` / creation callback (`app/src/main.rs`, `ui/src/bin/proto.rs`). Symptom: default egui blue/light widgets.

2. **Fonts not loaded** — `apply_fonts()` in ui-theme; bundled assets under `ui-theme/assets/`. Symptom: wordmark/panel titles use system proportional font.

3. **Heading vs body** — use `heading_font()` for wordmark and panel titles, not default `FontId::proportional`.

4. **Disabled vs hidden** — S1 ADSR/LFO must be visible-disabled; unshipped S6 regions must be absent entirely.

5. **Knob interaction** — proto/app should use custom `Knob` widget (`widgets/knob.rs`), not stock `DragValue` without arc paint.

6. **Piano key sizing** — fixed `PIANO_WHITE_KEY_WIDTH` (18px), not stretched to fill width.

7. **CentralPanel vs custom layout** — S1 uses `draw_s1` with computed `S1Layout` rects; verify `screen`/`max_rect` matches full viewport.

8. **Accent contrast** — `#183d50` alone too dark on knobs; wired/live highlights need `ACCENT_UI` (`#2a6b8a`).

9. **Viewport size** — S1 uses 720px height; full S6 uses 820px. Wrong height → region squeeze.

10. **Proto demo window** — `reelsynth-ui-proto` opens extra "Widget demo" window; exclude from S1 parity unless auditing Gate 2 widgets.

---

## Browser mockup URLs

Open locally for side-by-side (adjust path):

```
file:///Users/julian/Documents/coding-projects/reelsynth/brand/mockups/s1-performance.html
file:///Users/julian/Documents/coding-projects/reelsynth/brand/mockups/index.html
file:///Users/julian/Documents/coding-projects/reelsynth/brand/mockups/components.html
```

With browser MCP: navigate, snapshot, screenshot mockup at same viewport as app screenshot.

---

## Default demo data (realistic labels)

Match mockup and app defaults when checking content parity:

| Field | Value |
|-------|-------|
| Preset | Factory Lead |
| Category | Bass · Wavetable · Saw Morph |
| WT position | 108 / 255 |
| Filter cutoff | ~1.2 kHz |
| Status | Audio OK — click keys or use QWERTY row |

---

## Gate exit criteria (from plan)

| Gate | Exit |
|------|------|
| Gate 1 | User approves `components.html`, `s1-performance.html`, `index.html` in browser |
| Gate 1b | Majico palette pass on `tokens.css` + mockups (when MCP ready) |
| Gate 2 | Proto: knob drag, piano, disabled-state feel approved |
| S1 parity | App matches `s1-performance.html`; screenshot diff ≤4px; only shipped panels visible |
