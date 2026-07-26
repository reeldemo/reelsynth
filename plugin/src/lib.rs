//! ReelSynth plugin — slim VST3/CLAP + external full editor over IPC.

pub mod clap_entry;
pub mod editor;
pub mod ipc;
pub mod nih_plugin;
pub mod plugin_state;

pub use clap_entry::{CLAP_PLUGIN_ID, CLAP_PLUGIN_NAME, CLAP_PLUGIN_VERSION};
pub use editor::{PluginEditorApp, PluginEditorConfig};
pub use ipc::{manifest_path, IpcClient, IpcServer, MANIFEST_SCHEMA};
pub use plugin_state::{PluginStateV1, PLUGIN_STATE_SCHEMA};
