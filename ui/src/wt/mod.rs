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
    frame_to_y, nearest_slot, snap_x_to_slot, slot_x, y_to_frame, QuantHandleEditor,
    QuantHandleResponse,
};
pub use slots::{
    apply_slot_selection, effective_quant_count, frame_to_slot_coord, position_from_osc_ui,
    resolved_slots_for_ui, sync_slot_from_position, wave_quant_from_index, wave_quant_index,
    WAVE_QUANT_LABELS,
};
pub use strip::{WtStrip, WtStripResponse};
pub use toolbar::{WtEditTool, WtToolbar, WtToolbarResponse};
pub use view_2d::{WtView2d, WtView2dResponse};
pub use view_3d::{WtView3d, WtView3dResponse};
pub use view_3d_stack::{composite_stack_sample, WtView3dStack, WtView3dStackResponse};
pub use waveform::{frame_index, waveform_points};
