# Agent API & MCP

Headless Design automation for Cursor agents (no egui window).

## Rust API

```rust
use reelsynth_ui::AgentSession;

let mut s = AgentSession::new();
s.select_layer(0)?;
s.set_wave_quant(16);
s.set_seam_mode_label("adaptive")?;
s.promote_selected_for_quant()?;
s.set_quant_slot(15, 0.6)?;
let snap = s.snapshot();
```

Unit tests: `cargo test -p reelsynth-ui --lib -- agent_api`

## MCP server

Crate: `reelsynth-mcp` (stdio JSON-RPC).

```bash
cargo run -p reelsynth-mcp --bin reelsynth-mcp
```

Copy [`.cursor/mcp.json.example`](../.cursor/mcp.json.example) into your Cursor MCP config (adjust `cwd` if needed).

| Tool | Purpose |
|------|---------|
| `reelsynth_get_state` | Layer / quant / seam snapshot |
| `reelsynth_select_layer` | Select layer index |
| `reelsynth_set_quant` | Set `wave_quant` |
| `reelsynth_set_seam_mode` | `off` / `soft` / `adaptive` |
| `reelsynth_promote_selected` | VA → WT for Quant |
| `reelsynth_set_quant_slot` | Edit one Quant amplitude |
| `reelsynth_get_quant_points` | Read control points |
| `reelsynth_reset_session` | Fresh session |
