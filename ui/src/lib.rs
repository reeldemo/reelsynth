mod region;
mod fx_rack;
mod layout;
mod mod_matrix;
mod osc_column;
mod scope_strip;
mod shell;
mod state;
mod state_sync;
pub mod widgets;
pub mod wt;

pub use region::region;
pub use fx_rack::{
    default_effect_slots, draw_effect_rack, effect_slots_from_bypass, effect_slots_from_patch,
    effect_slots_to_bypass, effect_slots_to_patch, EffectRackState, EffectSlotUi,
};
pub use layout::*;
pub use mod_matrix::{
    default_mod_slots, draw_mod_matrix, mod_slots_from_patch, mod_slots_to_patch,
    ModMatrixState, ModPolarity, ModSlotUi,
};
pub use osc_column::{
    draw_osc_column, fm_algorithm_index, fm_source_from_algorithm, fm_source_from_index,
    fm_source_index, osc_type_from_index, osc_type_index, warp_mode_from_index,
    warp_mode_index, OscColumnResult, OscColumnState,
};
pub use scope_strip::{draw_scope_strip, ScopeStripInput, ScopeStripState, SCOPE_STRIP_HEIGHT};
pub use shell::draw_shell;
pub use state::{ShellActions, ShellMidiDevices, ShellConfig, UiState, ScopeStripContext};
pub use wt::{factory_bank, factory_label, FactoryBankEntry, FACTORY_BANKS};

pub use state_sync::{filter_mode_from_type, filter_type_from_mode, lfo_shape_from_index, lfo_shape_index, patch_from_state, sync_state_from_patch};
