---
name: audit-reelsynth-ui
description: >-
  Audits ReelSynth egui UI against HTML mockups using agent-driven screenshot
  capture, per-pixel layout checks, and optional /loop auto-fix until S1 parity.
  Compares layout regions, tokens, widgets, and sprint visibility rules;
  outputs severity-ranked findings with egui fix pointers. Use when the user
  attaches a screenshot, asks to audit UI, compare app vs mockup, run Gate
  1/2 review, /loop audit UI, or check visual parity for reelsynth-ui or
  reelsynth-app.
disable-model-invocation: true
---

# ReelSynth UI Screenshot Audit

Design-first workflow: **mockup approval → proto → app**. Layout changes require mockup update first.

## Invoke

- `@audit-reelsynth-ui` or ask to "audit ReelSynth UI"
- Shorthand: `/audit-ui` (same skill)
- **Loop mode:** `/loop` + audit UI — agent captures, audits, fixes, rebuilds until exit criteria or max 5 iterations (see [Loop mode](#loop-mode))

## Inputs

| Input | Action |
|-------|--------|
| Screenshot attached | Primary audit target — read the image |
| No screenshot | Agent captures running app automatically (see [Agent-driven screenshot audit](#agent-driven-screenshot-audit)) |
| "Compare to mockup" | Side-by-side audit against HTML reference |
| `/loop` + audit | Loop mode: screenshot → audit → fix Critical/Major → rebuild → repeat |

## Screenshot audit workflow

Concise agent loop — full script in [reference.md § Capture script](reference.md#capture-script).

### 1. Capture screenshots

After launching `reelsynth-ui`, save window PNGs to **`brand/mockups/audits/`** using macOS `screencapture` (timestamped `YYYY-MM-DD_HH-MM-ss-app.png`). Build/launch first; do not commit audit PNGs.

### 2. Read + compare

Use the **Read tool** on the captured PNG. Compare regions, spacing, and widgets against:

- `brand/mockups/s1-performance.html` — S1 layout / region map
- `brand/mockups/COMPONENT_SPEC.md` — HTML → egui sizes and tokens

Score findings Critical / Major / Minor / Polish ([reference.md](reference.md)).

### 3. Loop mode (`/loop audit UI`)

**Max 5 iterations** per session:

1. Screenshot → audit
2. Fix **Critical** and **Major** in `ui/` (+ `ui-theme/`)
3. Rebuild → re-screenshot → repeat

Trigger: `/loop` + audit UI (e.g. `/loop until UI matches mockup`). After 5 passes without exit, report blockers and arm sleeper ([Loop mode](#loop-mode)).

### 4. Exit criteria

- **S1 parity ~4px** vs `s1-performance.html` at 1280×720
- **Piano ~18px** white keys, 80px tall, readable
- **Alignments** match mockup landmarks (header 48px, rail 240px, WT strip 72px, etc.)

---

## Quick start

1. **Identify sprint context** from visible panels (S1 vs full S6 vs narrow).
2. **Load reference** — see [Reference files](#reference-files).
3. **Run region pass** — header → center hero → WT strip → right rail → footer/piano.
4. **Score findings** — Critical / Major / Minor / Polish (definitions in [reference.md](reference.md)).
5. **Emit report** using the template below.
6. **If fixing** — mockup first for layout; then egui paths in [Fix routing](#fix-routing).

## Sprint context detection

| Visible in screenshot | Reference mockup | Sprint |
|----------------------|------------------|--------|
| Preset hero + WT strip + right rail only; no osc/mod/FX/2D/3D | `s1-performance.html` | **S1** |
| Full three-column + mod matrix + FX + piano | `index.html` | **S6** |
| Collapsed mod/FX, narrower columns, piano off | `narrow.html` | Responsive |
| Widget gallery (knob/piano/tabs rows) | `components.html` | Gate 1 components |

**Sprint visibility rule:** unshipped panels must be **hidden**, not dimmed placeholders. Extra panels in S1 = Major; missing shipped panels = Critical.

## Reference files

Always read before auditing:

| File | Purpose |
|------|---------|
| `brand/mockups/s1-performance.html` | S1 layout (current app target) |
| `brand/mockups/index.html` | Full S6 layout |
| `brand/mockups/narrow.html` | Responsive collapse |
| `brand/mockups/components.html` | Widget gallery |
| `brand/mockups/COMPONENT_SPEC.md` | HTML → egui sizes |
| `brand/mockups/DECISIONS.md` | Locked layout decisions |
| `brand/design/tokens.css` | Colour + spacing tokens |
| `brand/mockups/mockups.css` | `--accent-ui` and mockup-only tokens |

Plan context: `.cursor/plans/reelsynth_ui_redesign_cc8a6033.plan.md` (Gate 1/2, ≤4px parity target).

## Audit workflow

### A. Screenshot-only (user attaches image)

1. Read screenshot; note viewport size if inferable.
2. Map visible regions to mockup regions (see [reference.md](reference.md) region map).
3. Walk checklist: spacing, colours, typography, knobs, piano, disabled states, sprint panels.
4. Produce structured report (template below).
5. Optionally describe side-by-side deltas vs mockup; use browser MCP to open mockup HTML if available.

### B. Live comparison (no screenshot)

```bash
# S1 app (audio + preset I/O)
cargo run -p reelsynth-app --bin reelsynth-ui

# Gate 2 proto (widget feel, no audio required)
cargo run -p reelsynth-ui --bin reelsynth-ui-proto
```

Capture screenshot (user or browser MCP), then follow workflow A. Open mockup at `file://…/brand/mockups/s1-performance.html` for side-by-side.

### C. Agent-driven screenshot audit

When no screenshot is attached, the agent **must** capture the running app itself (macOS).

#### 1. Build and launch

```bash
cd /Users/julian/Documents/coding-projects/reelsynth
cargo build -p reelsynth-app --bin reelsynth-ui
./target/debug/reelsynth-ui &
sleep 3   # wait for window to appear and first frame to paint
```

Kill any prior instance first: `pkill -f 'target/debug/reelsynth-ui' || true`

#### 2. Capture app window (macOS)

**Preferred — window by process name + title:**

```bash
AUDIT_DIR=brand/mockups/audits
mkdir -p "$AUDIT_DIR"
STAMP=$(date +%Y-%m-%d_%H-%M-%S)
OUT="$AUDIT_DIR/${STAMP}-app.png"

# Resolve window ID (process name matches binary; title is "ReelSynth")
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
  screencapture -l"$WIN_ID" "$OUT"
else
  # Fallback: full screen then manual crop, or frontmost window
  screencapture -o -l$(osascript -e 'tell app "System Events" to id of front window') "$OUT"
fi
echo "Saved: $OUT"
```

**Reliability notes:**

- Window title is `"ReelSynth"` (`app/src/main.rs` → `with_title("ReelSynth")`).
- Process name is `reelsynth-ui` (debug build path).
- `-l<windowID>` captures only that window (no shadow with `-o`).
- If osascript fails (Accessibility permissions), ask user to grant Terminal/Cursor **Accessibility** in System Settings → Privacy, or attach a manual screenshot.

#### 3. Capture reference mockup (optional each run)

Either read the static HTML/CSS directly (preferred for layout numbers) or capture for pixel diff:

```bash
# Browser MCP: navigate to file:///…/brand/mockups/s1-performance.html at 1280×720, screenshot
# Or export once: open mockup in browser, screencapture mockup reference to brand/mockups/audits/s1-reference.png
```

#### 4. Analysis workflow

1. **Read images** — use the Read tool on `$OUT` (app) and optionally reference PNG or `s1-performance.html` + `mockups.css`.
2. **Walk regions** — header → center hero → WT strip → right rail → piano wrap → footer (checklist in [reference.md](reference.md)).
3. **Per-pixel / layout diff** when alignment is suspected — compare key landmarks against mockup:

   | Landmark | Expected |
   |----------|----------|
   | Header height | 48px |
   | Right rail width | 240px |
   | WT strip height | 72px |
   | Piano wrap total | 96px (16px pad + 80px keyboard) |
   | Piano white key | 18px wide |
   | Footer height | 36px |

4. **Score findings** — Critical / Major / Minor / Polish.
5. **Auto-fix** — in loop mode, apply Critical/Major fixes in egui without asking (see [Fix routing](#fix-routing)).
6. **Re-screenshot** after rebuild to verify.

#### 5. Cleanup

```bash
pkill -f 'target/debug/reelsynth-ui' || true
```

### D. Loop mode

**Trigger:** user says `/loop` + audit UI (e.g. `/loop until UI matches mockup`).

**Per iteration (max 5):**

1. `cargo build -p reelsynth-app --bin reelsynth-ui`
2. Launch app → wait 3s → screencapture to `brand/mockups/audits/YYYY-MM-DD_HH-MM-ss-app.png`
3. Read screenshot + reference; audit all regions
4. Fix **Critical** and **Major** issues in egui (`ui/src/`, `ui-theme/`)
5. If exit criteria met → **DONE** (report, do not arm sleeper)
6. Else rebuild and repeat

**Exit criteria (S1 parity):**

- Layout matches `s1-performance.html` at 1280×720
- Piano readable size (18px white keys, 80px tall)
- Region alignments within ~4px of mockup landmarks
- No missing panel borders
- ADSR/LFO visibly greyed (`panel_disabled`), not interactive
- Only S1 panels visible (no osc/mod/FX columns)

**After 5 iterations without pass:** report remaining blockers and arm dynamic sleeper:

```bash
sleep 300
echo 'AGENT_LOOP_WAKE_reelsynth-ui-audit {"prompt":"Continue ReelSynth UI audit loop: screenshot app, audit vs s1-performance mockup, fix Critical/Major alignment issues, rebuild. Use @audit-reelsynth-ui skill. Report when exit criteria met."}'
```

**Loop completion report** (when DONE):

```markdown
# ReelSynth UI Audit Loop — DONE

| Iteration | Fixes applied |
|-----------|---------------|
| 1 | … |

**Final screenshot:** `brand/mockups/audits/YYYY-MM-DD_HH-MM-ss-app.png`
**Run app:** `cargo run -p reelsynth-app --bin reelsynth-ui`
**Verdict:** S1 parity ✅ (≤4px)
```

### E. Gate reviews

| Gate | What to audit | Pass criteria |
|------|---------------|---------------|
| **Gate 1** | Static HTML mockups | Matches DECISIONS + COMPONENT_SPEC; user sign-off |
| **Gate 2** | Proto binary | Knob drag, piano keys, disabled groups feel correct |
| **S1 parity** | App vs `s1-performance.html` | Layout ≤4px tolerance; only S1 panels visible |

## Checklist (summary)

Full checklist: [reference.md](reference.md).

- **Layout:** 8px grid; header 48px; footer 36px; osc 280px / rail 240px (S6); S1 center+rail only
- **Colours:** `#0a0a0a` canvas, `#18181b` panels, `#183d50` accent, `#2a6b8a` interactive highlights
- **Typography:** IBM Plex headings, Inter body, JetBrains Mono values
- **Knobs:** 48/56/64px; 270° arc; wired glow + "Live" badge on live params
- **Piano:** 80px tall; 18px white keys; 14 keys (2 octaves); 96px wrap; toggle in footer
- **Disabled:** ADSR/LFO greyed in S1 (`rs-group--disabled` / `panel_disabled`)
- **WT strip:** 72px; playhead; frame 108/255 default data

## Report template

```markdown
# ReelSynth UI Audit — [S1 | S6 | Components | Proto]

**Target:** [screenshot description / cargo run …]
**Reference:** [mockup file]
**Viewport:** [WxH if known]

## Summary
[1–2 sentences: pass/fail vs gate; top issues]

## Findings

| # | Severity | Region | Issue | Expected | Fix hint |
|---|----------|--------|-------|----------|----------|
| 1 | Critical | … | … | … | … |

## Region scorecard

| Region | Status | Notes |
|--------|--------|-------|
| Header | ✅ / ⚠️ / ❌ | … |
| Center hero | … | … |
| WT strip | … | … |
| Right rail | … | … |
| Footer / piano | … | … |

## Gate verdict
- [ ] Gate 1 mockup parity
- [ ] Gate 2 proto feel
- [ ] S1 app parity (≤4px)

## Recommended next steps
1. …
```

## Fix routing

**Layout or spacing change** → update mockup HTML/CSS first, get approval, then egui.

| Area | Rust paths |
|------|------------|
| Grid constants | `ui/src/layout.rs` |
| S1 shell | `ui/src/s1.rs` |
| Knobs | `ui/src/widgets/knob.rs` |
| Piano | `ui/src/widgets/piano.rs` |
| Panels / disabled chrome | `ui/src/widgets/panel.rs` |
| WT strip | `ui/src/wt/strip.rs` |
| Tabs | `ui/src/widgets/tabs.rs` |
| Theme + fonts | `ui-theme/src/lib.rs` |
| App entry + theme apply | `app/src/main.rs`, `ui/src/bin/proto.rs` |

### egui pitfalls (always check on visual bugs)

See [reference.md](reference.md) § egui pitfalls. Common: `reelsynth_ui_theme::apply(ctx)` not called in `eframe` creation callback; heading font family not bound; `ui.add_enabled(false, …)` missing on disabled groups.

## Optional canvas

For multi-screenshot regression reviews, a Cursor Canvas side-by-side layout is acceptable. Do **not** use canvas for a single quick audit — the markdown report is the deliverable.

## Do not

- Ship layout fixes without mockup update first
- Treat dimmed unshipped panels as acceptable (must hide)
- Use `/flow/*` Majico routes (retired)
- Commit audit screenshot PNGs to the repo (local-only under `brand/mockups/audits/`)

## Additional resources

- Full checklists, severity rubric, region map, egui pitfalls: [reference.md](reference.md)
- Existing skills survey (what this skill fills): [reference.md](reference.md) § Related skills
