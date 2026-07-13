//! Minimal plugin editor host — egui embed spike for S6.
//!
//! Run: `cargo run -p reelsynth-plugin --bin reelsynth-plugin-editor`

use reelsynth_plugin::{PluginEditorApp, PluginEditorConfig};

fn main() -> eframe::Result<()> {
    PluginEditorApp::run_native(PluginEditorConfig::default())
}
