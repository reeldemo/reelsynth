//! CLAP plugin entry stub (S6).
//!
//! Real host bindings (clap-sys / nih-plug) land in S7. This module exposes
//! stable identifiers and a null entry pointer for packaging smoke tests.

/// CLAP plugin identifier (reverse-DNS).
pub const CLAP_PLUGIN_ID: &str = "xyz.reelsynth";

/// Human-readable plugin name shown in DAW browsers.
pub const CLAP_PLUGIN_NAME: &str = "ReelSynth";

/// Plugin version string (semver).
pub const CLAP_PLUGIN_VERSION: &str = env!("CARGO_PKG_VERSION");

/// CLAP factory entry — returns null until S7 host bindings are wired.
#[no_mangle]
pub extern "C" fn clap_entry() -> *const () {
    clap_entry_stub()
}

/// Stub entry used by tests and packaging scripts.
pub fn clap_entry_stub() -> *const () {
    std::ptr::null()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clap_metadata_is_stable() {
        assert_eq!(CLAP_PLUGIN_ID, "xyz.reelsynth");
        assert_eq!(CLAP_PLUGIN_NAME, "ReelSynth");
        assert!(CLAP_PLUGIN_VERSION.starts_with("0."));
    }

    #[test]
    fn entry_stub_is_null() {
        assert!(clap_entry_stub().is_null());
    }
}
