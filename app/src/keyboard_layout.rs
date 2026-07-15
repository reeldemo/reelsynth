//! OS keyboard layout detection and play-row note mapping.

use egui::Key;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ComputerLayout {
    #[default]
    Qwerty,
    Azerty,
    Qwertz,
}

impl ComputerLayout {
    pub fn label(self) -> &'static str {
        match self {
            Self::Qwerty => "QWERTY",
            Self::Azerty => "AZERTY",
            Self::Qwertz => "QWERTZ",
        }
    }
}

/// Detect keyboard layout (Windows first; others default QWERTY).
pub fn detect_layout() -> ComputerLayout {
    #[cfg(windows)]
    {
        detect_layout_windows().unwrap_or(ComputerLayout::Qwerty)
    }
    #[cfg(not(windows))]
    {
        ComputerLayout::Qwerty
    }
}

#[cfg(windows)]
fn detect_layout_windows() -> Option<ComputerLayout> {
    use std::ffi::c_uint;
    #[link(name = "user32")]
    extern "system" {
        fn GetKeyboardLayout(id_thread: c_uint) -> usize;
    }
    let layout_id = unsafe { GetKeyboardLayout(0) } as u32 & 0xFFFF;
    match layout_id {
        0x040C => Some(ComputerLayout::Azerty), // French
        0x080C | 0x0407 => Some(ComputerLayout::Qwertz), // BE/DE
        _ => Some(ComputerLayout::Qwerty),
    }
}

fn qwerty_play_row() -> HashMap<Key, u8> {
    HashMap::from([
        (Key::Z, 48),
        (Key::S, 49),
        (Key::X, 50),
        (Key::D, 51),
        (Key::C, 52),
        (Key::V, 53),
        (Key::G, 54),
        (Key::B, 55),
        (Key::H, 56),
        (Key::N, 57),
        (Key::J, 58),
        (Key::M, 59),
    ])
}

fn azerty_play_row() -> HashMap<Key, u8> {
    // Same semitone positions; AZERTY physical keys for bottom row.
    HashMap::from([
        (Key::W, 48),
        (Key::S, 49),
        (Key::X, 50),
        (Key::D, 51),
        (Key::C, 52),
        (Key::V, 53),
        (Key::G, 54),
        (Key::B, 55),
        (Key::H, 56),
        (Key::N, 57),
        (Key::J, 58),
        (Key::Comma, 59),
    ])
}

fn qwertz_play_row() -> HashMap<Key, u8> {
    HashMap::from([
        (Key::Y, 48),
        (Key::S, 49),
        (Key::X, 50),
        (Key::D, 51),
        (Key::C, 52),
        (Key::V, 53),
        (Key::G, 54),
        (Key::B, 55),
        (Key::H, 56),
        (Key::N, 57),
        (Key::J, 58),
        (Key::M, 59),
    ])
}

impl ComputerLayout {
    pub fn play_row(self) -> HashMap<Key, u8> {
        match self {
            Self::Qwerty => qwerty_play_row(),
            Self::Azerty => azerty_play_row(),
            Self::Qwertz => qwertz_play_row(),
        }
    }
}

pub fn keyboard_note(key: Key, layout: ComputerLayout) -> Option<u8> {
    layout.play_row().get(&key).copied()
}

pub fn qwer_index(key: Key, layout: ComputerLayout) -> Option<usize> {
    let notes: Vec<u8> = layout.play_row().values().copied().collect();
    layout
        .play_row()
        .get(&key)
        .and_then(|n| notes.iter().position(|v| v == n))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn azerty_w_matches_qwerty_z_note() {
        let q = keyboard_note(Key::Z, ComputerLayout::Qwerty).unwrap();
        let a = keyboard_note(Key::W, ComputerLayout::Azerty).unwrap();
        assert_eq!(q, a, "AZERTY W should match QWERTY Z semitone");
    }
}
