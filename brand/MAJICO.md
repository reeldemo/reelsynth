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
| 2026-07-11 | `run_niche_research` | OK — job `fe8743ef-…` |
| 2026-07-11 | `generate_brand_md` | OK — scaffold in `BRAND.md` |
| 2026-07-11 | `select_palette` | **palette:0** Base 1 (`#183d50`) |
| 2026-07-11 | `get_design_tokens` | Synced → `design/tokens.css` |
| 2026-07-11 | `get_design_md` | Synced → `DESIGN.md` |
| 2026-07-11 | `ui-theme` crate | egui theme + smoke example |
| 2026-07-11 | Logo | **Pending** — pick in Studio canvas |

## Tool calls (scoped)

```json
{ "projectId": "95409489-3d96-4083-b35e-08bf5c824bfa" }
```

OAuth MCP — no API key in repo.

## Links

- Palette: http://localhost:3000/mcp/preview/palette-picker?project=95409489-3d96-4083-b35e-08bf5c824bfa&cursor=1
- Logo canvas: http://localhost:3000/canvas?project=95409489-3d96-4083-b35e-08bf5c824bfa&cursor=1

## Next steps

1. Pick logo in Studio → `get_logo_svg` → update `logo/reelsynth-mark.svg`
2. `import_repo` for `reeldemo/reelsynth` when GitHub connected
3. `rustup update stable` then `cargo test -p reelsynth-ui-theme`
