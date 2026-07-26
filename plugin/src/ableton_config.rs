//! User/install config for Ableton external editor.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const CONFIG_SCHEMA: &str = "reelsynth-ableton-config-v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AbletonInstallConfig {
    pub schema: String,
    /// Launch external editor when the VST loads in Live.
    pub auto_editor: bool,
    /// Absolute path to `reelsynth-plugin-editor` if not beside the plugin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub editor_path: Option<String>,
}

impl Default for AbletonInstallConfig {
    fn default() -> Self {
        Self {
            schema: CONFIG_SCHEMA.into(),
            auto_editor: false,
            editor_path: None,
        }
    }
}

pub fn config_path() -> PathBuf {
    if let Ok(p) = std::env::var("REELSYNTH_ABLETON_CONFIG") {
        return PathBuf::from(p);
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(base) = std::env::var("LOCALAPPDATA") {
            return PathBuf::from(base).join("ReelSynth").join("config.json");
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("ReelSynth")
                .join("config.json");
        }
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home)
                .join(".config")
                .join("ReelSynth")
                .join("config.json");
        }
    }
    PathBuf::from("reelsynth_ableton_config.json")
}

pub fn load_config() -> AbletonInstallConfig {
    let path = config_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        return AbletonInstallConfig::default();
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save_config(cfg: &AbletonInstallConfig) -> Result<(), String> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    std::fs::write(path, text).map_err(|e| e.to_string())
}

/// Candidate paths for the external editor binary.
pub fn editor_candidates(cfg: &AbletonInstallConfig) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Some(p) = cfg.editor_path.as_ref() {
        out.push(PathBuf::from(p));
    }
    if let Ok(p) = std::env::var("REELSYNTH_EDITOR") {
        out.push(PathBuf::from(p));
    }
    #[cfg(target_os = "windows")]
    {
        if let Ok(base) = std::env::var("LOCALAPPDATA") {
            out.push(
                PathBuf::from(base)
                    .join("ReelSynth")
                    .join("bin")
                    .join("reelsynth-plugin-editor.exe"),
            );
        }
        if let Ok(pf) = std::env::var("PROGRAMFILES") {
            out.push(
                PathBuf::from(pf)
                    .join("ReelSynth")
                    .join("reelsynth-plugin-editor.exe"),
            );
        }
    }
    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            out.push(
                PathBuf::from(home)
                    .join("Applications")
                    .join("ReelSynth Editor.app")
                    .join("Contents")
                    .join("MacOS")
                    .join("reelsynth-plugin-editor"),
            );
            out.push(
                PathBuf::from(home)
                    .join("Library")
                    .join("Application Support")
                    .join("ReelSynth")
                    .join("bin")
                    .join("reelsynth-plugin-editor"),
            );
        }
        out.push(PathBuf::from(
            "/Applications/ReelSynth Editor.app/Contents/MacOS/reelsynth-plugin-editor",
        ));
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            out.push(parent.join("reelsynth-plugin-editor.exe"));
            out.push(parent.join("reelsynth-plugin-editor"));
        }
    }
    out
}
