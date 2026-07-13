//! ReelSynth plugin shell (S6) — CLAP entry stub + shared egui editor.

pub mod clap_entry;
pub mod editor;

pub use clap_entry::{clap_entry_stub, CLAP_PLUGIN_ID, CLAP_PLUGIN_NAME, CLAP_PLUGIN_VERSION};
pub use editor::{PluginEditorApp, PluginEditorConfig};
