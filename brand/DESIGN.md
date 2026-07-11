# Design System: ReelSynth

**Project ID:** `95409489-3d96-4083-b35e-08bf5c824bfa`  
**Palette:** Base 1 (`palette:0`) — deep teal `#183d50` on pitch black

## 1. Visual theme

Dark studio instrument UI. ReelSynth reads as **hardware-in-software**: knobs, wavetable surfaces, modulation grids — not a chat or SaaS dashboard.

## 2. Color roles (dark default)

| Token | Hex | Use |
|-------|-----|-----|
| `--bg` | `#0a0a0a` | Main window, WT editor backdrop |
| `--bg-muted` | `#18181b` | Panels, osc tabs, mod matrix rows |
| `--text` | `#fafafa` | Labels, values |
| `--text-muted` | `#a1a1aa` | Hints, secondary labels |
| `--accent` | `#183d50` | Primary buttons, active tab, WT playhead |
| `--accent-on` | `#fafafa` | Text on accent |
| `--accent-muted` | `#061e2a` | Hover, pressed, knob arc |
| `--border` | `#27272a` | Panel dividers, input outlines |

Light theme tokens in [design/tokens.css](./design/tokens.css).

## 3. Typography

| Role | Font |
|------|------|
| Headings / section titles | IBM Plex Sans |
| Body / UI | Inter |
| Mono / MIDI / param IDs | JetBrains Mono |

## 4. Component guidance (egui)

- **Primary button:** `--accent` fill, `--accent-on` label
- **Knobs:** arc `--accent`, track `--border`, value `--text`
- **Panels:** `--bg-muted` fill, 1px `--border`
- **WT strip:** `--accent` playhead; frame thumbnails on `--surface2`
- **Hover rows:** `color-mix(in oklch, var(--accent) 14%, var(--bg-muted))`

## 5. Spacing and radius

- Spacing: xs 8, sm 12, md 20, lg 32, xl 48 px
- Radius: sm 10, md 16, lg 24 px

## 6. Motion

120ms / 200ms, `cubic-bezier(0.4, 0, 0.2, 1)` — snappy, not bouncy.

## 7. Logo

Pending user selection in [Majico Studio canvas](http://localhost:3000/canvas?project=95409489-3d96-4083-b35e-08bf5c824bfa&cursor=1). Placeholder until `logo/reelsynth-mark.svg` synced.
