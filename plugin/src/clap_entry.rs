//! CLAP plugin identifiers (S7). Real entry is `nih_export_clap!` in `nih_plugin.rs`.

/// CLAP plugin identifier (reverse-DNS).
pub const CLAP_PLUGIN_ID: &str = "xyz.reelsynth";

/// Human-readable plugin name shown in DAW browsers.
pub const CLAP_PLUGIN_NAME: &str = "ReelSynth";

/// Plugin version string (semver).
pub const CLAP_PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_metadata_is_stable() {
        assert_eq!(CLAP_PLUGIN_ID, "xyz.reelsynth");
        assert_eq!(CLAP_PLUGIN_NAME, "ReelSynth");
        assert!(CLAP_PLUGIN_VERSION.starts_with("0."));
    }
}
