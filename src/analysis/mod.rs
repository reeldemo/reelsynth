//! Offline analysis helpers (harmonic decomposition, etc.).

mod harmonics;

pub use harmonics::{decompose_frame, resynthesize_frame, resynthesis_error};
