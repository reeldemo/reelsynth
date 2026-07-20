mod ambient;
mod agent_api;
mod audit_registry;
mod center_layout;
mod compose;
mod contrast_audit;
mod region;
mod filter_rack;
mod fx_rack;
mod overtone_rack;
mod layout;
mod layout_audit;
mod mod_matrix;
mod osc_column;
mod oscillator_ui;
mod quant_interp;
mod performance;
mod scope_strip;
mod shell;
mod state;
mod state_sync;
pub mod widgets;
pub mod wt;

pub use agent_api::{AgentLayerSnap, AgentSession, AgentSnapshot};
pub use region::region;
pub use filter_rack::{
    draw_filter_chain, filter_slots_from_patch, filter_slots_to_patch, FilterRackResult,
    FilterSlotUi,
};
pub use fx_rack::{
    default_effect_slots, draw_effect_rack, draw_effect_rack_sidebar, effect_slots_from_bypass,
    effect_slots_from_patch, effect_slots_to_bypass, effect_slots_to_patch, EffectRackState,
    EffectSlotUi,
};
pub use overtone_rack::{
    draw_overtone_chain_menu, overtone_slots_to_engine, OvertoneFilterSlotUi, OvertoneRackResult,
};
pub use audit_registry::{
    audit_all_elements, audit_compose_panels, audit_id_rect, audit_no_horizontal_overflow,
    audit_siblings_no_overlap, count_base_audit_variants, record_element, record_region,
    record_used, AuditChecks, AuditId, ElementAudit, REGISTRY_VARIANT_COUNT,
};
pub use contrast_audit::{
    assert_min_contrast, audit_scope_trace_contrast, audit_theme_tokens, contrast_ratio,
    relative_luminance, SCOPE_TRACE_COLORS,
};
pub use layout::*;
pub use layout_audit::{
    assert_content_within, assert_sidebar_width_parity, audit_center, audit_element,
    audit_header_clusters, audit_osc_sidebar_stacks, audit_panel_utilization, audit_rail_panels,
    audit_shell, center_morph_used_rect_id, center_piano_used_rect_id, center_scope_used_rect_id,
    center_strip_used_rect_id, center_used_rect_id, center_views_used_rect_id, footer_used_rect_id,
    header_left_cluster_rect_id, header_right_cluster_rect_id, header_used_rect_id, AuditElement,
    HEADER_CLUSTER_MIN_GAP,
    fx_strip_used_rect_id, mod_strip_used_rect_id, osc_fx_allocated_rect_id, osc_fx_used_rect_id,
    osc_used_rect_id, overlap_area,
    piano_used_rect_id, rail_filter_allocated_rect_id, rail_filter_used_rect_id,
    osc_mod_allocated_rect_id, osc_mod_used_rect_id, rail_mod_allocated_rect_id,
    rail_mod_used_rect_id, rail_used_rect_id,
    rect_area, utilization, within_bounds,
};
pub use center_layout::{compute_center_regions, CenterRegions};
pub use mod_matrix::{
    default_mod_slots, draw_mod_matrix, draw_mod_matrix_sidebar, mod_slots_from_patch,
    mod_slots_to_patch, ModMatrixState, ModPolarity, ModSlotUi,
};
pub use osc_column::{
    draw_osc_column, fm_algorithm_index, fm_source_from_algorithm, fm_source_from_index,
    fm_source_index, osc_type_from_index, osc_type_index, warp_mode_from_index,
    warp_mode_index, OscColumnInput, OscColumnResult, OscColumnState,
};
pub use oscillator_ui::{OscillatorUi, WaveLayerUi, MIN_OSCILLATORS};
pub use quant_interp::WtQuantInterp;
pub use performance::PerformanceUi;
pub use scope_strip::{
    autofix_stack_levels, draw_scope_strip, ScopeStripInput, ScopeStripState, SCOPE_RESULT_LABEL,
    SCOPE_STRIP_HEIGHT,
};
pub use compose::{
    Clip, ClipRef, ComposeUi, MidiNote, PianoRollTool, QuantizeGrid, Scene, SequenceProject, Track,
    TransportUi,
};
pub use reelsynth::QuantizeDivision;
pub use shell::draw_shell;
pub use state::{
    OscStripContext, OscStripPreviewState, ScopeStripContext, ShellActions, ShellAppSettings,
    ShellAudioDevices, ShellConfig, ShellMidiDevices, ShellMode, UiState, WtView3dMode,
};
pub use wt::{
    composite_stack_sample, factory_bank, factory_label, set_gpu_renderer_active, FactoryBankEntry,
    QuantSeamMode, FACTORY_BANKS,
};

pub use state_sync::{
    apply_loaded_bank_to_design, compose_to_patch_sequence, filter_mode_from_type,
    filter_type_from_mode, lfo_shape_from_index, lfo_shape_index, patch_from_state,
    sync_state_from_patch,
};
