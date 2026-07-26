//! Localhost JSON-lines IPC between the VST3 audio plugin and the external editor.

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

/// Shared patch/bank mutated by IPC; audio thread applies when `dirty`.
pub struct IpcEngineState {
    pub patch: Patch,
    pub bank: WavetableBank,
    pub dirty: bool,
}

impl IpcEngineState {
    pub fn new(patch: Patch, bank: WavetableBank) -> Self {
        Self {
            patch,
            bank,
            dirty: false,
        }
    }

    pub fn snapshot(&self) -> PluginStateV1 {
        PluginStateV1::from_patch_bank(&self.patch, &self.bank)
    }

    pub fn apply_state(&mut self, state: PluginStateV1) -> Result<(), String> {
        let json = state.to_json()?;
        let (patch, bank) = PluginStateV1::from_json(&json)?;
        self.patch = patch;
        self.bank = bank;
        self.dirty = true;
        Ok(())
    }
}

pub struct IpcServer {
    pub port: Arc<AtomicU16>,
    pub running: Arc<AtomicBool>,
}

impl IpcServer {
    pub fn start(shared: Arc<Mutex<IpcEngineState>>) -> std::io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(false)?;
        let port = listener.local_addr()?.port();
        let port_atom = Arc::new(AtomicU16::new(port));
        let running = Arc::new(AtomicBool::new(true));

        write_manifest(port)?;

        let running_bg = running.clone();
        thread::Builder::new()
            .name("reelsynth-ipc".into())
            .spawn(move || {
                while running_bg.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let shared = shared.clone();
                            let _ = stream.set_read_timeout(Some(Duration::from_secs(30)));
                            let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                            handle_client(stream, shared);
                        }
                        Err(_) => thread::sleep(Duration::from_millis(50)),
                    }
                }
            })?;

        // Installer sets auto_editor in config.json; env REELSYNTH_AUTO_EDITOR=1 also works.
        // Host GUI "Open Editor" always launches via `launch_external_editor`.
        maybe_auto_spawn_external_editor();

        Ok(Self {
            port: port_atom,
            running,
        })
    }

    pub fn port(&self) -> u16 {
        self.port.load(Ordering::SeqCst)
    }
}

impl Drop for IpcServer {
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

fn handle_client(stream: TcpStream, shared: Arc<Mutex<IpcEngineState>>) {
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
                let st = shared.lock().snapshot();
                IpcResponse::Ok { state: Some(st) }
            }
            Ok(IpcRequest::SetState { state }) => match shared.lock().apply_state(state) {
                Ok(()) => {
                    let st = shared.lock().snapshot();
                    IpcResponse::Ok { state: Some(st) }
                }
                Err(message) => IpcResponse::Err { message },
            },
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

fn maybe_auto_spawn_external_editor() {
    use crate::ableton_config::load_config;

    let cfg = load_config();
    let env_force = std::env::var("REELSYNTH_AUTO_EDITOR").ok().as_deref() == Some("1");
    if !cfg.auto_editor && !env_force {
        return;
    }
    let _ = launch_external_editor();
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
}
