//! ReelSynth plugin — egui editor spike + nih-plug VST3/CLAP instrument.

pub mod clap_entry;
pub mod editor;
pub mod nih_plugin;

pub use clap_entry::{clap_entry_pending, CLAP_PLUGIN_ID, CLAP_PLUGIN_NAME, CLAP_PLUGIN_VERSION};
pub use editor::{PluginEditorApp, PluginEditorConfig};
