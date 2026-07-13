use egui::{Rect, Ui};
use reelsynth::Patch;

use super::*;
use super::header::{sync_morph_from_active_tab, sync_osc_position_from_wt};
use crate::layout::UiScale;
use crate::region::region;

pub(super) fn draw_center(
    ui: &mut Ui,
    rect: Rect,
    state: &mut UiState,
    mut bank: Option<&mut WavetableBank>,
    preview_patch: &Patch,
    config: &ShellConfig,
    scope: Option<ScopeStripContext<'_>>,
    actions: &mut ShellActions,
    scale: UiScale,
) {
    let s = scale.ui();
    let inner = rect.shrink(SPACE_SM * s);
    let (scope_rect, strip_rect, morph_rect, views_rect) =
        center_regions(inner, config, s);

    let bank_name = state.wt_bank_name.clone();

    if scope_rect.is_positive() {
        region(ui, scope_rect, |ui| {
            if let Some(ctx) = scope {
                draw_scope_strip(
                    ui,
                    scope_rect,
                    ScopeStripInput {
                        patch: preview_patch,
                        banks: ctx.banks,
                        bank_for_osc: ctx.bank_for_osc,
                        live: ctx.live,
                        is_playing: ctx.is_playing,
                        now_secs: ctx.now_secs,
                        state: ctx.state,
                    },
                );
            } else if let Some(b) = bank.as_deref() {
                let bank_for_osc: &dyn Fn(usize) -> usize = &|_| 0;
                let mut strip_state = ScopeStripState::default();
                draw_scope_strip(
                    ui,
                    scope_rect,
                    ScopeStripInput {
                        patch: preview_patch,
                        banks: std::slice::from_ref(b),
                        bank_for_osc: &bank_for_osc,
                        live: None,
                        is_playing: false,
                        now_secs: ui.input(|i| i.time),
                        state: &mut strip_state,
                    },
                );
            }
        });
    }

    if config.show_wt_editor && morph_rect.is_positive() {
        region(ui, morph_rect, |ui| {
            let morph = WtMorph {
                frame_a: &mut state.wt_morph_a,
                frame_b: &mut state.wt_morph_b,
                amount: &mut state.wt_morph_amount,
                position: &mut state.wt_position,
            };
            if morph.show(ui).changed {
                sync_osc_position_from_wt(state);
                sync_morph_from_active_tab(state);
                actions.params_changed = true;
            }
        });
    }

    if config.show_wt_editor && views_rect.is_positive() {
        let view_min = WT_VIEW_MIN_HEIGHT * s;
        let views_h = views_rect.height().max(view_min * 0.5);
        region(ui, views_rect, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = GRID_UNIT;
                let half_w = (ui.available_width() - GRID_UNIT) * 0.5;
                ui.allocate_ui_with_layout(
                    egui::vec2(half_w, views_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        let view = WtView2d {
                            position: state.wt_position,
                            bank: bank.as_deref_mut(),
                            bank_name: Some(bank_name.as_str()),
                            tool: &mut state.wt_edit_tool,
                        };
                        if view.show(ui).frame_edited {
                            actions.frame_edited = true;
                        }
                    },
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(half_w, views_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        WtView3d {
                            position: state.wt_position,
                            bank: bank.as_deref(),
                        }
                        .show(ui);
                    },
                );
            });
        });
    }

    if strip_rect.is_positive() {
        region(ui, strip_rect, |ui| {
            let strip = WtStrip {
                position: &mut state.wt_position,
                bank: bank.as_deref(),
                bank_name: Some(bank_name.as_str()),
                visible_frames: 16,
            };
            if strip.show(ui).changed {
                sync_osc_position_from_wt(state);
                state.wt_morph_amount =
                    morph_amount_for_position(state.wt_morph_a, state.wt_morph_b, state.wt_position);
                sync_morph_from_active_tab(state);
                actions.params_changed = true;
            }
        });
    }
}

fn center_regions(inner: Rect, config: &ShellConfig, scale: f32) -> (Rect, Rect, Rect, Rect) {
    let scope_h = SCOPE_STRIP_HEIGHT * scale;
    let strip_h = WT_STRIP_HEIGHT * scale;
    let morph_line_h = WT_MORPH_HEIGHT * scale;
    let gap = GRID_UNIT * scale;
    let view_min = WT_VIEW_MIN_HEIGHT * scale;

    let scope_rect = Rect::from_min_max(
        inner.min,
        egui::pos2(inner.max.x, (inner.min.y + scope_h).min(inner.max.y)),
    );
    let mut y = scope_rect.max.y + gap;

    if config.show_osc_column {
        let strip_rect = rect_row(inner, y, strip_h);
        y = strip_rect.max.y + gap;

        let morph_rect = if config.show_wt_editor {
            let r = rect_row(inner, y, morph_line_h);
            y = r.max.y + gap;
            r
        } else {
            Rect::NOTHING
        };

        let views_rect = if config.show_wt_editor && y < inner.max.y - view_min * 0.5 {
            Rect::from_min_max(egui::pos2(inner.min.x, y), inner.max)
        } else {
            Rect::NOTHING
        };

        (scope_rect, strip_rect, morph_rect, views_rect)
    } else {
        let views_h = if config.show_wt_editor {
            (view_min + gap)
                .min((inner.height() - scope_h - gap - strip_h - gap).max(view_min * 0.5))
        } else {
            0.0
        };
        let morph_block_h = if config.show_wt_editor {
            morph_line_h + gap
        } else {
            0.0
        };

        let views_rect = if config.show_wt_editor && views_h > 0.0 {
            Rect::from_min_max(
                egui::pos2(inner.min.x, inner.max.y - views_h),
                inner.max,
            )
        } else {
            Rect::NOTHING
        };

        let morph_rect = if config.show_wt_editor && morph_block_h > 0.0 {
            Rect::from_min_max(
                egui::pos2(inner.min.x, views_rect.min.y - morph_block_h),
                egui::pos2(inner.max.x, views_rect.min.y - gap),
            )
        } else {
            Rect::NOTHING
        };

        let strip_top = if config.show_wt_editor {
            morph_rect.min.y - gap - strip_h
        } else {
            inner.max.y - strip_h
        };
        let strip_rect = Rect::from_min_max(
            egui::pos2(inner.min.x, strip_top.max(scope_rect.max.y + gap)),
            egui::pos2(inner.max.x, strip_top + strip_h),
        );

        (scope_rect, strip_rect, morph_rect, views_rect)
    }
}

fn rect_row(inner: Rect, y: f32, height: f32) -> Rect {
    Rect::from_min_max(
        egui::pos2(inner.min.x, y),
        egui::pos2(inner.max.x, (y + height).min(inner.max.y)),
    )
}
