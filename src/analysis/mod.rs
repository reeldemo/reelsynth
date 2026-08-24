//! Offline analysis helpers (harmonic decomposition, pitch detection).

mod harmonics;
pub mod pitch;

pub use harmonics::{decompose_frame, resynthesize_frame, resynthesis_error};
