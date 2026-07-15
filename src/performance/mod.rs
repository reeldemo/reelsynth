//! Performance input: scales, chords, and preset settings.

mod arp;
mod chord;
mod scale;
mod settings;

pub use arp::{
    build_pool, ArpDirection, ArpEngine, ArpEvent, ArpInputMode, ArpRate, ArpSettings, ArpStep,
};
pub use chord::{
    diatonic_quality, resolve_chord, resolve_diatonic_chord, ChordQuality, ChordSet, ChordVoicing,
};
pub use scale::{
    note_in_scale, scale_degree_to_midi, snap_note, PerformanceLayout, Scale, ScaleBehavior,
};
pub use settings::PerformanceSettings;
