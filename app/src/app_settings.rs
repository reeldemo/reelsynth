//! Persistent app-level settings (not preset schema).

use eframe::Renderer;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum GraphicsBackend {
    #[default]
    Auto,
    Wgpu,
    Glow,
}

impl GraphicsBackend {
    #[allow(dead_code)] // used by settings UI labels when exposed
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Wgpu => "GPU (WGPU)",
            Self::Glow => "OpenGL (Glow)",
        }
    }

    pub fn to_renderer(self) -> Renderer {
        match self {
            Self::Glow => Renderer::Glow,
            Self::Wgpu => Renderer::Wgpu,
            Self::Auto => Renderer::default(),
        }
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Wgpu,
            2 => Self::Glow,
            _ => Self::Auto,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Auto => 0,
            Self::Wgpu => 1,
            Self::Glow => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum KeyboardLayoutSetting {
    #[default]
    Auto,
    Qwerty,
    Azerty,
    Qwertz,
}

impl KeyboardLayoutSetting {
    #[allow(dead_code)] // used by settings UI labels when exposed
    pub fn label(self) -> &'static str {
        match self {
            Self::Auto => "Auto",
            Self::Qwerty => "QWERTY",
            Self::Azerty => "AZERTY",
            Self::Qwertz => "QWERTZ",
        }
    }

    pub fn from_index(idx: usize) -> Self {
        match idx {
            1 => Self::Qwerty,
            2 => Self::Azerty,
            3 => Self::Qwertz,
            _ => Self::Auto,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Auto => 0,
            Self::Qwerty => 1,
            Self::Azerty => 2,
            Self::Qwertz => 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    pub graphics_backend: GraphicsBackend,
    pub gpu_waveforms: bool,
    pub auto_midi_keyboard: bool,
    pub keyboard_layout: KeyboardLayoutSetting,
    pub pending_backend_restart: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            graphics_backend: GraphicsBackend::Auto,
            gpu_waveforms: true,
            auto_midi_keyboard: true,
            keyboard_layout: KeyboardLayoutSetting::Auto,
            pending_backend_restart: false,
        }
    }
}

impl AppSettings {
    fn path() -> PathBuf {
        directories::ProjectDirs::from("io", "reeldemo", "reelsynth")
            .map(|d| d.config_dir().join("settings.json"))
            .unwrap_or_else(|| PathBuf::from("reelsynth-settings.json"))
    }

    pub fn load() -> Self {
        let path = Self::path();
        fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        if let Ok(json) = serde_json::to_string_pretty(self) {
            let path = Self::path();
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let _ = fs::write(path, json);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphics_backend_roundtrip() {
        assert_eq!(
            GraphicsBackend::from_index(GraphicsBackend::Wgpu.index()),
            GraphicsBackend::Wgpu
        );
        assert_eq!(GraphicsBackend::from_index(99), GraphicsBackend::Auto);
    }
}
