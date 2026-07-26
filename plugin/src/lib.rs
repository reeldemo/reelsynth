//! ReelSynth plugin — egui editor spike + nih-plug VST3/CLAP instrument.

pub mod clap_entry;
pub mod editor;
pub mod nih_plugin;
pub mod plugin_state;

pub use clap_entry::{CLAP_PLUGIN_ID, CLAP_PLUGIN_NAME, CLAP_PLUGIN_VERSION};
pub use editor::{PluginEditorApp, PluginEditorConfig};
pub use plugin_state::{PluginStateV1, PLUGIN_STATE_SCHEMA};
