//! Performance shell layout (was S1).

mod center;
mod footer;
mod header;
mod rail;

use egui::{Rect, Ui};
use reelsynth::Patch;
use reelsynth_ui_theme::Tokens;

use crate::audit_registry::{record_region, AuditId};
use crate::compose::draw_compose_shell;
use crate::fx_rack::{draw_effect_rack, EffectRackState};
use crate::layout::{embed_piano_in_center, ShellLayout, ShellLayoutOptions};
use crate::layout_audit::{fx_strip_used_rect_id, mod_strip_used_rect_id};
use crate::mod_matrix::{draw_mod_matrix, ModMatrixState};
use crate::region::region;

pub use crate::state::{
    OscStripContext, OscStripPreviewState, ScopeStripContext, ShellActions, ShellConfig,
    ShellMidiDevices, ShellMode, UiState, WtView3dMode,
};

// Re-exports for shell submodules (`use super::*`).
pub(super) use egui::{Color32, FontId};
pub(super) use reelsynth::WavetableBank;
pub(super) use reelsynth_ui_theme::heading_font;
pub(super) use crate::layout::{
    GRID_UNIT, SPACE_SM, WT_MORPH_HEIGHT, WT_STRIP_HEIGHT, WT_VIEW_MIN_HEIGHT,
};
pub(super) use crate::osc_column::{draw_osc_column, OscColumnState};
pub(super) use crate::scope_strip::{
    draw_scope_strip, ScopeStripInput, ScopeStripState, SCOPE_STRIP_HEIGHT,
};
pub(super) use crate::widgets::{
    adsr_graph, format_depth, format_env_time, format_lfo_rate, format_sustain, PianoKeyboard,
};
pub(super) use crate::wt::{
    morph_amount_for_position, morph_position, StripMode, WtMorph, WtStrip, WtView2d,
    WtView3dStack, FACTORY_BANKS,
};

pub fn draw_shell(
    ui: &mut Ui,
    screen: Rect,
    state: &mut UiState,
    bank: Option<&mut WavetableBank>,
    preview_patch: &Patch,
    midi: &ShellMidiDevices<'_>,
    config: &ShellConfig,
    scope: Option<ScopeStripContext<'_>>,
    osc_preview: Option<OscStripContext<'_>>,
) -> ShellActions {
    let compose_mode = state.shell_mode == ShellMode::Compose;
    let layout_opts = ShellLayoutOptions {
        piano_visible: state.piano_visible,
        show_osc_column: config.show_osc_column && !compose_mode,
        show_mod_matrix: config.show_mod_matrix && !compose_mode,
        mod_matrix_open: state.mod_matrix_open,
        show_fx_rack: config.show_fx_rack && !compose_mode,
        fx_rack_open: state.fx_rack_open,
    };
    let layout = ShellLayout::compute_with_options(screen, layout_opts);
    let tokens = Tokens::default();
    let mut actions = ShellActions::default();

    // Keep ID-clash flame overlays off unless explicitly debugging.
    let ui_debug = std::env::var_os("REELSYNTH_UI_DEBUG").is_some();
    ui.ctx().options_mut(|o| {
        o.warn_on_id_clash = ui_debug;
    });

    let painter = ui.painter_at(screen);
    let border = egui::Stroke::new(1.0_f32, tokens.border);
    painter.rect_filled(layout.header, 0.0, tokens.surface2);
    painter.line_segment(
        [layout.header.left_bottom(), layout.header.right_bottom()],
        border,
    );
    painter.rect_filled(layout.main, 0.0, tokens.bg);
    if layout.osc.is_positive() {
        painter.rect_filled(layout.osc, 0.0, tokens.bg);
        painter.line_segment(
            [layout.osc.right_top(), layout.osc.right_bottom()],
            border,
        );
    }
    painter.rect_filled(layout.rail, 0.0, tokens.bg);
    if layout.rail.is_positive() {
        painter.line_segment(
            [layout.rail.left_top(), layout.rail.left_bottom()],
            border,
        );
    }
    if layout.mod_matrix.is_positive() {
        painter.rect_filled(layout.mod_matrix, 0.0, tokens.bg_muted);
        painter.line_segment(
            [layout.mod_matrix.left_top(), layout.mod_matrix.right_top()],
            border,
        );
    }
    if layout.fx_rack.is_positive() {
        painter.rect_filled(layout.fx_rack, 0.0, tokens.bg_muted);
        painter.line_segment(
            [layout.fx_rack.left_top(), layout.fx_rack.right_top()],
            border,
        );
    }
    if state.piano_visible && layout.piano_wrap.is_positive() {
        painter.rect_filled(layout.piano_wrap, 0.0, tokens.surface2);
        painter.line_segment(
            [layout.piano_wrap.left_top(), layout.piano_wrap.right_top()],
            border,
        );
    }
    painter.rect_filled(layout.footer, 0.0, tokens.surface2);
    painter.line_segment(
        [layout.footer.left_top(), layout.footer.right_top()],
        border,
    );

    draw_header(ui, layout.header, state, midi, &mut actions);

    if compose_mode {
        draw_compose_shell(ui, layout.main, state, &mut actions, layout.scale);
    } else {
        if layout.osc.is_positive() {
            draw_osc(
                ui,
                layout.osc,
                state,
                preview_patch,
                osc_preview,
                config,
                &mut actions,
                layout.scale,
            );
        }
        draw_center(
            ui,
            layout.center,
            state,
            bank,
            preview_patch,
            config,
            scope,
            &mut actions,
            layout.scale,
        );
        draw_rail(ui, layout.rail, state, config, &mut actions, layout.scale);
    }

    if !compose_mode && layout.mod_matrix.is_positive() {
        region(ui, layout.mod_matrix, |ui| {
            let result = draw_mod_matrix(
                ui,
                layout.mod_matrix,
                ModMatrixState {
                    open: &mut state.mod_matrix_open,
                    routes: &mut state.mod_routes,
                    total_routes: state.mod_route_total,
                },
                layout.scale,
            );
            if result.changed {
                actions.params_changed = true;
            }
            let used = ui.min_rect();
            ui.ctx()
                .data_mut(|d| d.insert_temp(mod_strip_used_rect_id(), used));
        });
    }

    if !compose_mode && layout.fx_rack.is_positive() {
        region(ui, layout.fx_rack, |ui| {
            let result = draw_effect_rack(
                ui,
                layout.fx_rack,
                EffectRackState {
                    open: &mut state.fx_rack_open,
                    slots: &mut state.fx_slots,
                },
                layout.scale,
            );
            if result.changed {
                actions.params_changed = true;
            }
            let used = ui.min_rect();
            ui.ctx()
                .data_mut(|d| d.insert_temp(fx_strip_used_rect_id(), used));
        });
    }

    if state.piano_visible
        && layout.piano_wrap.is_positive()
        && !embed_piano_in_center(layout_opts)
    {
        draw_piano_wrap(ui, layout.piano_wrap, state, &mut actions, layout.scale);
    }

    draw_footer(ui, layout.footer, state, &mut actions);

    let ctx = ui.ctx();
    record_region(ctx, AuditId::ShellHeader, layout.header, layout.header);
    record_region(ctx, AuditId::ShellMain, layout.main, layout.main);
    record_region(ctx, AuditId::ShellFooter, layout.footer, layout.footer);
    if layout.mod_matrix.is_positive() {
        record_region(ctx, AuditId::ShellModStrip, layout.mod_matrix, layout.mod_matrix);
    }
    if layout.fx_rack.is_positive() {
        record_region(ctx, AuditId::ShellFxStrip, layout.fx_rack, layout.fx_rack);
    }
    if state.piano_visible && layout.piano_wrap.is_positive() {
        record_region(ctx, AuditId::ShellPianoWrap, layout.piano_wrap, layout.piano_wrap);
    }

    actions
}


use header::{draw_header, draw_osc};
use center::draw_center;
use rail::draw_rail;
use footer::{draw_footer, draw_piano_wrap};
