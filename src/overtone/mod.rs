//! Master-bus overtone / crackle suppression chain (before musical FxChain).

mod chain;
mod harshness;
mod processors;
mod types;

pub use chain::OvertoneFilterChain;
pub use harshness::{curve_harshness, fixture_saw_wrap, fixture_sine, hf_harshness, wrap_harshness};
pub use types::{OvertoneFilterSlot, OvertoneFilterType};

#[cfg(test)]
mod tests;
