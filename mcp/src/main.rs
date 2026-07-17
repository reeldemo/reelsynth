//! Minimal MCP (Model Context Protocol) stdio server for ReelSynth Design agents.
//!
//! Tools wrap [`reelsynth_ui::AgentSession`] so Cursor / other agents can select
//! layers, set Quant / seam modes, promote VA→WT, and edit quant slots without
//! driving the egui window.
//!
//! Run: `cargo run -p reelsynth-mcp --bin reelsynth-mcp`
//! Configure in Cursor MCP settings as a stdio server.

use std::io::{self, BufRead, Write};
use std::sync::Mutex;

use reelsynth_ui::AgentSession;
use serde_json::{json, Value};

struct AppState {
    session: AgentSession,
}

fn main() {
    let state = Mutex::new(AppState {
        session: AgentSession::new(),
    });
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Ok(req) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
        let params = req.get("params").cloned().unwrap_or(json!({}));
        let response = match method {
            "initialize" => ok(
                id,
                json!({
                    "protocolVersion": "2024-11-05",
                    "capabilities": { "tools": {} },
                    "serverInfo": { "name": "reelsynth-mcp", "version": "0.1.0" }
                }),
            ),
            "notifications/initialized" | "initialized" => continue,
            "tools/list" => ok(id, json!({ "tools": tool_defs() })),
            "tools/call" => {
                let name = params
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(json!({}));
                match call_tool(&state, name, &args) {
                    Ok(text) => ok(
                        id,
                        json!({
                            "content": [{ "type": "text", "text": text }],
                            "isError": false
                        }),
                    ),
                    Err(e) => ok(
                        id,
                        json!({
                            "content": [{ "type": "text", "text": e }],
                            "isError": true
                        }),
                    ),
                }
            }
            "ping" => ok(id, json!({})),
            _ => err(id, -32601, format!("Method not found: {method}")),
        };
        let _ = writeln!(stdout, "{response}");
        let _ = stdout.flush();
    }
}

fn tool_defs() -> Vec<Value> {
    vec![
        tool(
            "reelsynth_get_state",
            "Snapshot Design session (layers, quant, seam, selection)",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "reelsynth_select_layer",
            "Select layer by 0-based index",
            json!({
                "type": "object",
                "properties": { "index": { "type": "integer", "minimum": 0 } },
                "required": ["index"]
            }),
        ),
        tool(
            "reelsynth_set_quant",
            "Set wave_quant (0 disables Quant knobs)",
            json!({
                "type": "object",
                "properties": { "quant": { "type": "integer", "minimum": 0, "maximum": 256 } },
                "required": ["quant"]
            }),
        ),
        tool(
            "reelsynth_set_seam_mode",
            "Set wrap seam mode: off | soft | adaptive",
            json!({
                "type": "object",
                "properties": { "mode": { "type": "string" } },
                "required": ["mode"]
            }),
        ),
        tool(
            "reelsynth_promote_selected",
            "Promote selected VA layer to wavetable for Quant editing",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "reelsynth_set_quant_slot",
            "Set one Quant slot amplitude (−1..1); promotes VA if needed",
            json!({
                "type": "object",
                "properties": {
                    "slot": { "type": "integer", "minimum": 0 },
                    "sample": { "type": "number" }
                },
                "required": ["slot", "sample"]
            }),
        ),
        tool(
            "reelsynth_get_quant_points",
            "Read Quant control-point amplitudes for the selected layer",
            json!({ "type": "object", "properties": {} }),
        ),
        tool(
            "reelsynth_reset_session",
            "Reset to a fresh AgentSession",
            json!({ "type": "object", "properties": {} }),
        ),
    ]
}

fn tool(name: &str, description: &str, input_schema: Value) -> Value {
    json!({
        "name": name,
        "description": description,
        "inputSchema": input_schema
    })
}

fn call_tool(state: &Mutex<AppState>, name: &str, args: &Value) -> Result<String, String> {
    let mut g = state.lock().map_err(|e| e.to_string())?;
    match name {
        "reelsynth_get_state" => {
            let snap = g.session.snapshot();
            serde_json::to_string_pretty(&snap).map_err(|e| e.to_string())
        }
        "reelsynth_select_layer" => {
            let index = args.get("index").and_then(|v| v.as_u64()).ok_or("index required")? as usize;
            g.session.select_layer(index)?;
            Ok(format!("selected layer {index}"))
        }
        "reelsynth_set_quant" => {
            let quant = args.get("quant").and_then(|v| v.as_u64()).ok_or("quant required")? as u8;
            g.session.set_wave_quant(quant);
            Ok(format!("wave_quant={quant}"))
        }
        "reelsynth_set_seam_mode" => {
            let mode = args.get("mode").and_then(|v| v.as_str()).ok_or("mode required")?;
            g.session.set_seam_mode_label(mode)?;
            Ok(format!("seam={}", g.session.state.wt_quant_seam.label()))
        }
        "reelsynth_promote_selected" => {
            let promoted = g.session.promote_selected_for_quant()?;
            Ok(if promoted {
                "promoted VA→WT".into()
            } else {
                "already wavetable".into()
            })
        }
        "reelsynth_set_quant_slot" => {
            let slot = args.get("slot").and_then(|v| v.as_u64()).ok_or("slot required")? as usize;
            let sample = args.get("sample").and_then(|v| v.as_f64()).ok_or("sample required")? as f32;
            g.session.set_quant_slot(slot, sample)?;
            let pts = g.session.quant_points()?;
            Ok(serde_json::to_string(&pts).unwrap_or_default())
        }
        "reelsynth_get_quant_points" => {
            let pts = g.session.quant_points()?;
            serde_json::to_string(&pts).map_err(|e| e.to_string())
        }
        "reelsynth_reset_session" => {
            g.session = AgentSession::new();
            Ok("session reset".into())
        }
        other => Err(format!("unknown tool: {other}")),
    }
}

fn ok(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn err(id: Value, code: i32, message: String) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}
