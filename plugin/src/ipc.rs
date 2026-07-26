//! Localhost JSON-lines IPC between the VST3 audio plugin and the external editor.
//!
//! Design rule: the audio thread must never block on IPC. Heavy decode/encode happens
//! off the audio thread; notes use a lock-free queue; state uses `try_lock` + `Arc` swaps.

use crossbeam::queue::SegQueue;
use parking_lot::Mutex;
use reelsynth::{Patch, WavetableBank};
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU16, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::plugin_state::PluginStateV1;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcRequest {
    Ping,
    GetState,
    SetState { state: PluginStateV1 },
    NoteOn { note: u8, velocity: f32 },
    NoteOff { note: u8 },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum IpcResponse {
    Ok { state: Option<PluginStateV1> },
    Err { message: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InstanceManifest {
    pub schema: String,
    pub port: u16,
    pub pid: u32,
    pub name: String,
}

pub const MANIFEST_SCHEMA: &str = "reelsynth-plugin-ipc-v1";

pub fn manifest_path() -> PathBuf {
    if let Ok(p) = std::env::var("REELSYNTH_PLUGIN_IPC_MANIFEST") {
        return PathBuf::from(p);
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(base) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(base)
                .join("ReelSynth")
                .join("plugin_ipc.json");
        }
    }
    #[cfg(not(target_os = "windows"))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".local")
                .join("share")
                .join("ReelSynth")
                .join("plugin_ipc.json");
        }
    }
    PathBuf::from("reelsynth_plugin_ipc.json")
}

#[derive(Clone, Copy, Debug)]
pub enum PendingNote {
    On { note: u8, velocity: f32 },
    Off { note: u8 },
}

/// Shared patch/bank for the editor; audio thread pulls via `try_lock` + cheap `Arc` clones.
pub struct IpcEngineState {
    pub patch: Arc<Patch>,
    pub bank: Arc<WavetableBank>,
    pub dirty: bool,
}

impl IpcEngineState {
    pub fn new(patch: Patch, bank: WavetableBank) -> Self {
        Self {
            patch: Arc::new(patch),
            bank: Arc::new(bank),
            dirty: false,
        }
    }
}

/// Bridge owned by the plugin instance.
pub struct IpcBridge {
    pub state: Arc<Mutex<IpcEngineState>>,
    pub notes: Arc<SegQueue<PendingNote>>,
    pub port: Arc<AtomicU16>,
    running: Arc<AtomicBool>,
}

impl IpcBridge {
    pub fn start(patch: Patch, bank: WavetableBank) -> std::io::Result<Self> {
        let state = Arc::new(Mutex::new(IpcEngineState::new(patch, bank)));
        let notes = Arc::new(SegQueue::new());
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let port_atom = Arc::new(AtomicU16::new(port));
        let running = Arc::new(AtomicBool::new(true));

        write_manifest(port)?;

        let running_bg = running.clone();
        let state_bg = state.clone();
        let notes_bg = notes.clone();
        thread::Builder::new()
            .name("reelsynth-ipc".into())
            .spawn(move || {
                while running_bg.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
                            let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                            handle_client(stream, state_bg.clone(), notes_bg.clone());
                        }
                        Err(_) => thread::sleep(Duration::from_millis(50)),
                    }
                }
            })?;

        // Delay auto-open so Live can finish activating audio before another process starts.
        maybe_auto_spawn_external_editor_delayed();

        Ok(Self {
            state,
            notes,
            port: port_atom,
            running,
        })
    }

    /// Non-blocking: take a pending editor snapshot if one is ready.
    pub fn try_take_dirty(&self) -> Option<(Arc<Patch>, Arc<WavetableBank>)> {
        let mut g = self.state.try_lock()?;
        if !g.dirty {
            return None;
        }
        g.dirty = false;
        Some((Arc::clone(&g.patch), Arc::clone(&g.bank)))
    }

    pub fn push_note_from_host_view(&self, note: PendingNote) {
        self.notes.push(note);
    }
}

impl Drop for IpcBridge {
    fn drop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        let _ = std::fs::remove_file(manifest_path());
    }
}

fn write_manifest(port: u16) -> std::io::Result<()> {
    let path = manifest_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let doc = InstanceManifest {
        schema: MANIFEST_SCHEMA.into(),
        port,
        pid: std::process::id(),
        name: "ReelSynth".into(),
    };
    let text = serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".into());
    std::fs::write(path, text)
}

fn handle_client(
    stream: TcpStream,
    shared: Arc<Mutex<IpcEngineState>>,
    notes: Arc<SegQueue<PendingNote>>,
) {
    let Ok(clone) = stream.try_clone() else {
        return;
    };
    let mut reader = BufReader::new(clone);
    let mut writer = stream;
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Err(_) => break,
            Ok(_) => {}
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<IpcRequest>(trimmed) {
            Ok(IpcRequest::Ping) => IpcResponse::Ok { state: None },
            Ok(IpcRequest::GetState) => {
                // Clone Arcs under lock; encode outside so audio can proceed.
                let (patch, bank) = {
                    let g = shared.lock();
                    (Arc::clone(&g.patch), Arc::clone(&g.bank))
                };
                let st = PluginStateV1::from_patch_bank(patch.as_ref(), bank.as_ref());
                IpcResponse::Ok { state: Some(st) }
            }
            Ok(IpcRequest::SetState { state }) => match state.into_patch_bank() {
                Ok((patch, bank)) => {
                    {
                        let mut g = shared.lock();
                        g.patch = Arc::new(patch);
                        g.bank = Arc::new(bank);
                        g.dirty = true;
                    }
                    IpcResponse::Ok { state: None }
                }
                Err(message) => IpcResponse::Err { message },
            },
            Ok(IpcRequest::NoteOn { note, velocity }) => {
                notes.push(PendingNote::On {
                    note: note.min(127),
                    velocity: velocity.clamp(0.0, 1.0),
                });
                IpcResponse::Ok { state: None }
            }
            Ok(IpcRequest::NoteOff { note }) => {
                notes.push(PendingNote::Off {
                    note: note.min(127),
                });
                IpcResponse::Ok { state: None }
            }
            Err(e) => IpcResponse::Err {
                message: e.to_string(),
            },
        };
        if let Ok(text) = serde_json::to_string(&response) {
            let _ = writeln!(writer, "{text}");
            let _ = writer.flush();
        }
    }
}

fn maybe_auto_spawn_external_editor_delayed() {
    use crate::ableton_config::load_config;

    let cfg = load_config();
    let env_force = std::env::var("REELSYNTH_AUTO_EDITOR").ok().as_deref() == Some("1");
    if !cfg.auto_editor && !env_force {
        return;
    }
    thread::spawn(|| {
        thread::sleep(Duration::from_millis(1000));
        let _ = launch_external_editor();
    });
}

/// Spawn the full Design UI (`reelsynth-plugin-editor`). Used by auto-open and the host button.
pub fn launch_external_editor() -> Result<(), String> {
    use crate::ableton_config::{editor_candidates, load_config};

    let cfg = load_config();
    for path in editor_candidates(&cfg) {
        if path.is_file() {
            std::process::Command::new(&path)
                .spawn()
                .map_err(|e| format!("Failed to start {}: {e}", path.display()))?;
            return Ok(());
        }
    }

    if let Ok(root) = std::env::var("REELSYNTH_ROOT") {
        std::process::Command::new("cargo")
            .args([
                "run",
                "-p",
                "reelsynth-plugin",
                "--release",
                "--bin",
                "reelsynth-plugin-editor",
            ])
            .current_dir(&root)
            .spawn()
            .map_err(|e| format!("cargo run editor failed: {e}"))?;
        return Ok(());
    }

    Err(
        "Editor not found. Run scripts/install-ableton.ps1 (or .sh), or set REELSYNTH_EDITOR / editor_path in config.json."
            .into(),
    )
}

/// Client used by the external editor process.
pub struct IpcClient {
    stream: TcpStream,
}

impl IpcClient {
    pub fn connect_from_manifest() -> Result<Self, String> {
        let path = manifest_path();
        let text = std::fs::read_to_string(&path).map_err(|e| {
            format!(
                "No Live plugin instance found at {} ({e}). Load ReelSynth in Ableton first.",
                path.display()
            )
        })?;
        let man: InstanceManifest =
            serde_json::from_str(&text).map_err(|e| format!("bad manifest: {e}"))?;
        if man.schema != MANIFEST_SCHEMA {
            return Err(format!("unsupported manifest schema {}", man.schema));
        }
        Self::connect(man.port)
    }

    pub fn connect(port: u16) -> Result<Self, String> {
        let stream = TcpStream::connect(("127.0.0.1", port))
            .map_err(|e| format!("connect 127.0.0.1:{port}: {e}"))?;
        let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
        Ok(Self { stream })
    }

    pub fn request(&mut self, req: &IpcRequest) -> Result<IpcResponse, String> {
        let mut stream = &self.stream;
        let line = serde_json::to_string(req).map_err(|e| e.to_string())?;
        writeln!(stream, "{line}").map_err(|e| e.to_string())?;
        stream.flush().map_err(|e| e.to_string())?;
        let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
        let mut resp = String::new();
        reader
            .read_line(&mut resp)
            .map_err(|e| e.to_string())?;
        serde_json::from_str(resp.trim()).map_err(|e| e.to_string())
    }

    pub fn get_state(&mut self) -> Result<PluginStateV1, String> {
        match self.request(&IpcRequest::GetState)? {
            IpcResponse::Ok {
                state: Some(state),
            } => Ok(state),
            IpcResponse::Ok { state: None } => Err("empty state".into()),
            IpcResponse::Err { message } => Err(message),
        }
    }

    pub fn set_state(&mut self, state: PluginStateV1) -> Result<(), String> {
        match self.request(&IpcRequest::SetState { state })? {
            IpcResponse::Ok { .. } => Ok(()),
            IpcResponse::Err { message } => Err(message),
        }
    }

    pub fn note_on(&mut self, note: u8, velocity: f32) -> Result<(), String> {
        match self.request(&IpcRequest::NoteOn { note, velocity })? {
            IpcResponse::Ok { .. } => Ok(()),
            IpcResponse::Err { message } => Err(message),
        }
    }

    pub fn note_off(&mut self, note: u8) -> Result<(), String> {
        match self.request(&IpcRequest::NoteOff { note })? {
            IpcResponse::Ok { .. } => Ok(()),
            IpcResponse::Err { message } => Err(message),
        }
    }
}
