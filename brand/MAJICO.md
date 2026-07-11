# Majico sync — ReelSynth

| Field | Value |
|-------|--------|
| **Project name** | ReelSynth |
| **Project ID** | `95409489-3d96-4083-b35e-08bf5c824bfa` |
| **Slug** | `reelsynth` |
| **MCP base** | `http://127.0.0.1:3000/mcp` |
| **Created** | 2026-07-11 |
| **Relationship** | Separate from Reeldemo Ableton (`252e664f-…`) |

## Sync log

| Date | Tool | Result |
|------|------|--------|
| 2026-07-11 | `create_project` | OK — new project |
| 2026-07-11 | `run_niche_research` | OK — job `fe8743ef-…` (async) |
| 2026-07-11 | `generate_brand_md` | OK — scaffold in `BRAND.md` |
| 2026-07-11 | `list_palette_options` | 7 total, 3 shown |
| 2026-07-11 | `select_palette` | **palette:0** (Base 1, `#183d50`) confirmed |
| 2026-07-11 | `get_design_tokens` | Synced → `design/tokens.css` |
| 2026-07-11 | `list_logo_candidates` | 8 templates — **awaiting Studio pick** |

## Tool calls (scoped)

Pass on every branding tool call:

```json
{ "projectId": "95409489-3d96-4083-b35e-08bf5c824bfa" }
```

OAuth MCP — no API key in repo. Project API key exists for automation only (`projectApiKeyAutomationOnly: true`).

## Palette picker

http://localhost:3000/mcp/preview/palette-picker?project=95409489-3d96-4083-b35e-08bf5c824bfa&cursor=1

## Logo canvas

http://localhost:3000/canvas?project=95409489-3d96-4083-b35e-08bf5c824bfa&cursor=1

## Next steps

1. User confirms palette → `select_palette` with `userConfirmed: true`
2. User confirms logo → `select_logo` with `userConfirmed: true`
3. `get_design_tokens`, `get_design_md`, `get_logo_svg` → refresh `design/tokens.css`, `DESIGN.md`, `logo/reelsynth-mark.svg`
4. `import_repo` owner `cap-jmk-launchpad` repo `reelsynth` when GitHub connected
