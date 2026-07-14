//! ReelSynth Q&A integration test suite (see `docs/qa/MATRIX.md`).

#[path = "qa/helpers.rs"]
mod helpers;
#[path = "qa/foundation.rs"]
mod foundation;
#[path = "qa/oscillator.rs"]
mod oscillator;
#[path = "qa/fm.rs"]
mod fm;
#[path = "qa/effects.rs"]
mod effects;
#[path = "qa/scopes.rs"]
mod scopes;
#[path = "qa/modulation.rs"]
mod modulation;
#[path = "qa/integration.rs"]
mod integration;
#[path = "qa/pitch_grid.rs"]
mod pitch_grid;
#[path = "qa/invariants.rs"]
mod invariants;
#[path = "qa/matrix_factory_lead.rs"]
mod matrix_factory_lead;
#[path = "qa/sweep_smoke.rs"]
mod sweep_smoke;
#[path = "qa/sequence.rs"]
mod sequence;
