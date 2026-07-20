mod banks;
mod curve_editor;
mod gpu_waveform;
mod mod_preview;
mod morph;
mod residual;
mod view_result;
mod view_selected;
mod quant_handles;
mod shape_editor;
mod slots;
mod strip;
mod toolbar;
mod view_2d;
mod view_3d;
mod view_3d_stack;
mod view_zoom;
mod waveform;

pub use banks::{factory_bank, factory_label, FactoryBankEntry, FACTORY_BANKS};
pub use gpu_waveform::{paint_waveform_line, set_gpu_renderer_active, use_gpu_waveforms};
pub use residual::{
    ensure_residual_layer, find_residual_layer_idx, layer_curve_label,
    residual_samples_from_desired,
};
pub use view_result::{WtViewResult, WtViewResultResponse};
pub use view_selected::{WtSelectedLayerView, WtSelectedLayerResponse};
pub use morph::{morph_amount_for_position, morph_position, WtMorph, WtMorphResponse};
pub use quant_handles::{
    apply_quant_slot_amplitude, frame_to_y, knob_y_on_curve, nearest_quant_handle, nearest_slot,
    paint_quant_knob, periodize_quant_frame, periodize_quant_frame_with_mode, quant_control_points,
    quant_curve_stroke, quant_hover_status_label, quant_knob_visual, quantized_curve_polyline,
    resample_frame_from_quant_points, resample_frame_from_quant_points_uniform,
    sample_at_quant_phase, sample_from_knob_y, sample_to_y, set_crackle_amount,
    current_crackle_amount, set_quant_seam_mode, snap_x_to_slot,
    slot_x, y_to_frame, y_to_sample, QuantHandleEditor, QuantHandleResponse, QuantKnobVisual,
    QuantSeamMode,
};
pub use crate::quant_interp::WtQuantInterp;
pub use slots::{
    apply_slot_selection, effective_quant_count, frame_to_slot_coord, position_from_osc_ui,
    resolved_slots_for_ui, sync_slot_from_position, wave_quant_from_index, wave_quant_index,
    WAVE_QUANT_LABELS,
};
pub use strip::{StripMode, WtStrip, WtStripResponse};
pub use toolbar::{FrameShapeTemplate, WtEditTool, WtToolbar, WtToolbarResponse};
pub use view_2d::{
    allocate_unused_wt_frame, apply_frame_shape_template, promote_va_layer_for_quant,
    shape_template_source_type, va_source_to_shape_template, WtView2d, WtView2dResponse,
};
pub use view_3d::{WtView3d, WtView3dResponse};
pub use view_3d_stack::{composite_stack_sample, WtView3dStack, WtView3dStackResponse};
pub use view_zoom::{consume_plot_scroll, WtCurveViewTransform};
pub use waveform::{
    frame_index, layer_quant_editable, layers_pointer_prefers_curve_select,
    quant_knobs_for_selection, selected_pane_shows_quant_knobs, waveform_points,
};
