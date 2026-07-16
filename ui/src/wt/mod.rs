mod banks;
mod curve_editor;
mod gpu_waveform;
mod mod_preview;
mod morph;
mod quant_handles;
mod shape_editor;
mod slots;
mod strip;
mod toolbar;
mod view_2d;
mod view_3d;
mod view_3d_stack;
mod waveform;

pub use banks::{factory_bank, factory_label, FactoryBankEntry, FACTORY_BANKS};
pub use gpu_waveform::{paint_waveform_line, set_gpu_renderer_active, use_gpu_waveforms};
pub use morph::{morph_amount_for_position, morph_position, WtMorph, WtMorphResponse};
pub use quant_handles::{
    apply_quant_slot_amplitude, frame_to_y, knob_y_on_curve, nearest_quant_handle, nearest_slot,
    quant_control_points, quantized_curve_polyline, resample_frame_from_quant_points,
    sample_at_quant_phase, sample_from_knob_y, sample_to_y, snap_x_to_slot, slot_x, y_to_frame,
    y_to_sample, QuantHandleEditor, QuantHandleResponse, WtQuantInterp,
};
pub use slots::{
    apply_slot_selection, effective_quant_count, frame_to_slot_coord, position_from_osc_ui,
    resolved_slots_for_ui, sync_slot_from_position, wave_quant_from_index, wave_quant_index,
    WAVE_QUANT_LABELS,
};
pub use strip::{StripMode, WtStrip, WtStripResponse};
pub use toolbar::{FrameShapeTemplate, WtEditTool, WtToolbar, WtToolbarResponse};
pub use view_2d::{apply_frame_shape_template, shape_template_source_type, WtView2d, WtView2dResponse};
pub use view_3d::{WtView3d, WtView3dResponse};
pub use view_3d_stack::{composite_stack_sample, WtView3dStack, WtView3dStackResponse};
pub use waveform::{frame_index, waveform_points};
