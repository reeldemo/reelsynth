//! Compose-mode sequence engine: arrangement, transport, recording.

mod automation;
mod clock;
mod quantize;
mod recorder;
mod runtime;
mod scheduler;
pub mod schema;
mod transport;

pub use automation::{compute_automation_mods, evaluate_lane};
pub use clock::{BeatRange, SampleClock};
pub use quantize::{quantize_note, quantize_notes, snap_beat};
pub use recorder::{MidiRecorder, RecordTarget};
pub use runtime::SequencerRuntime;
pub use scheduler::{clips_at_playhead, NoteScheduler, SchedEvent, SEQ_CHANNEL};
pub use schema::*;
pub use transport::TransportState;

#[cfg(test)]
mod tests;
