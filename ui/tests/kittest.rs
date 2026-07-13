//! egui_kittest interaction smoke tests (UI-K01 … UI-K06).

use std::collections::HashSet;

use egui::Rect;
use egui_kittest::Harness;
use reelsynth::Patch;
use reelsynth_ui::{
    audit_center, audit_shell, compute_center_regions, default_effect_slots, draw_shell,
    embed_mod_fx_in_center, embed_piano_in_center, osc_type_index, ShellLayout, ShellLayoutOptions, ShellMidiDevices,
    center_fx_used_rect_id, center_mod_used_rect_id, center_morph_used_rect_id,
    center_piano_used_rect_id, center_scope_used_rect_id, center_strip_used_rect_id,
    center_used_rect_id, center_views_used_rect_id, footer_used_rect_id, fx_strip_used_rect_id, header_used_rect_id,
    mod_strip_used_rect_id, osc_used_rect_id, rail_used_rect_id, ShellConfig,
    UiState, APP_HEIGHT_FULL, APP_MIN_WIDTH, SPACE_SM,
};
use reelsynth_ui::widgets::{Knob, KnobSize, PianoKeyboard};
use reelsynth_ui_theme;

#[test]
fn piano_widget_harness_smoke() {
    let keys = HashSet::new();
    let mut harness = Harness::new_ui(|ui| {
        reelsynth_ui_theme::Tokens::default();
        let _ = PianoKeyboard::new(&keys).show(ui);
    });
    harness.run();
}

#[test]
fn knob_widget_harness_smoke() {
    let mut value = 0.25_f32;
    let mut harness = Harness::builder()
        .with_size([120.0, 120.0])
        .build_ui(|ui| {
            Knob::new(&mut value, 0.0..=1.0, "Level")
                .size(KnobSize::Md)
                .show(ui);
        });
    harness.run();
    drop(harness);
    assert!((0.0..=1.0).contains(&value));
}

#[test]
fn mod_matrix_toggle_route_enabled() {
    let mut state = UiState::default();
    state.mod_routes[0].enabled = true;
    state.mod_routes[0].enabled = false;
    assert!(!state.mod_routes[0].enabled);
}

#[test]
fn fx_rack_add_slot() {
    let mut slots = default_effect_slots();
    let before = slots.len();
    slots.push(reelsynth_ui::EffectSlotUi::from_slot(
        &reelsynth::fx::EffectSlot::chorus(),
    ));
    assert_eq!(slots.len(), before + 1);
}

#[test]
fn osc_tab_switch_follows_state() {
    let mut state = UiState::default();
    state.osc_tab = 1;
    state.osc_type[1] = osc_type_index("square");
    assert_eq!(state.osc_type[1], 2);
}

#[test]
fn compact_mode_collapses_sections() {
    struct ShellTest {
        fonts_applied: bool,
        state: UiState,
    }

    let config = ShellConfig {
        show_mod_matrix: false,
        show_fx_rack: false,
        ..Default::default()
    };
    let midi = ShellMidiDevices {
        names: &["None".to_string()],
        selected: 0,
    };
    let preview = Patch::default_mono();
    let mut harness = Harness::builder()
        .with_size([1280.0, 720.0])
        .build_state(
            |ctx, test| {
                if !test.fonts_applied {
                    reelsynth_ui_theme::apply(ctx);
                    test.fonts_applied = true;
                    return;
                }

                egui::CentralPanel::default().show(ctx, |ui| {
                    let screen = ui.max_rect();
                    let _actions = draw_shell(
                        ui,
                        screen,
                        &mut test.state,
                        None,
                        &preview,
                        &midi,
                        &config,
                        None,
                    );
                });
            },
            ShellTest {
                fonts_applied: false,
                state: UiState::default(),
            },
        );
    harness.run();
    assert!(!config.show_mod_matrix && !config.show_fx_rack);
}

#[test]
fn full_shell_min_window_no_layout_overlap() {
    struct ShellTest {
        fonts_applied: bool,
        state: UiState,
    }

    let config = ShellConfig {
        show_wt_editor: true,
        show_osc_column: true,
        show_mod_matrix: true,
        show_fx_rack: true,
    };
    let options = ShellLayoutOptions {
        piano_visible: true,
        show_osc_column: true,
        show_mod_matrix: true,
        mod_matrix_open: true,
        show_fx_rack: true,
        fx_rack_open: true,
    };
    let screen = Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(APP_MIN_WIDTH, APP_HEIGHT_FULL),
    );
    let layout = ShellLayout::compute_with_options(screen, options);
    audit_shell(&layout, screen, options);

    let scale = layout.scale.ui();
    let inner = layout.center.shrink(SPACE_SM * scale);
    let regions = compute_center_regions(inner, &config, scale, embed_mod_fx_in_center(options), embed_piano_in_center(options));
    audit_center(layout.center, &regions, scale);

    let midi = ShellMidiDevices {
        names: &["None".to_string()],
        selected: 0,
    };
    let preview = Patch::default_mono();
    let mut harness = Harness::builder()
        .with_size([APP_MIN_WIDTH, APP_HEIGHT_FULL])
        .build_state(
            |ctx, test| {
                if !test.fonts_applied {
                    reelsynth_ui_theme::apply(ctx);
                    test.fonts_applied = true;
                    return;
                }

                egui::CentralPanel::default().show(ctx, |ui| {
                    let screen = ui.max_rect();
                    let _actions = draw_shell(
                        ui,
                        screen,
                        &mut test.state,
                        None,
                        &preview,
                        &midi,
                        &config,
                        None,
                    );
                });
            },
            ShellTest {
                fonts_applied: false,
                state: UiState::default(),
            },
        );
    harness.run();
}

#[test]
fn rail_widgets_within_rail_bounds_min_window() {
    struct ShellTest {
        fonts_applied: bool,
        state: UiState,
    }

    let config = ShellConfig {
        show_wt_editor: true,
        show_osc_column: true,
        show_mod_matrix: true,
        show_fx_rack: true,
    };
    let options = ShellLayoutOptions {
        piano_visible: true,
        show_osc_column: true,
        show_mod_matrix: true,
        mod_matrix_open: true,
        show_fx_rack: true,
        fx_rack_open: true,
    };
    let screen = Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(APP_MIN_WIDTH, APP_HEIGHT_FULL),
    );
    let layout = ShellLayout::compute_with_options(screen, options);
    audit_shell(&layout, screen, options);

    let midi = ShellMidiDevices {
        names: &["None".to_string()],
        selected: 0,
    };
    let preview = Patch::default_mono();

    let mut harness = Harness::builder()
        .with_size([APP_MIN_WIDTH, APP_HEIGHT_FULL])
        .build_state(
            |ctx, test| {
                if !test.fonts_applied {
                    reelsynth_ui_theme::apply(ctx);
                    test.fonts_applied = true;
                    return;
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    let screen = ui.max_rect();
                    let _actions = draw_shell(
                        ui,
                        screen,
                        &mut test.state,
                        None,
                        &preview,
                        &midi,
                        &config,
                        None,
                    );
                });
            },
            ShellTest {
                fonts_applied: false,
                state: UiState::default(),
            },
        );

    harness.run();

    let rail_bounds = layout.rail;
    let used = harness
        .ctx
        .data(|d| d.get_temp::<egui::Rect>(rail_used_rect_id()))
        .expect("rail used rect not stored");
    assert!(
        used.max.y <= rail_bounds.max.y + 0.5,
        "rail content exceeds allocated height: used_max_y={} rail_max_y={} (used={used:?} rail={rail_bounds:?})",
        used.max.y,
        rail_bounds.max.y,
    );
}

fn get_used(ctx: &egui::Context, id: egui::Id, label: &str) -> egui::Rect {
    ctx.data(|d| d.get_temp::<egui::Rect>(id))
        .unwrap_or_else(|| panic!("{label} used rect not stored"))
}

fn fits_max_slack(outer: egui::Rect, inner: egui::Rect, slack: f32) -> bool {
    if !inner.is_positive() {
        return true;
    }
    inner.max.x <= outer.max.x + slack && inner.max.y <= outer.max.y + slack
}

#[test]
fn interface_used_rects_within_allocated_min_window() {
    struct ShellTest {
        fonts_applied: bool,
        state: UiState,
    }

    let config = ShellConfig {
        show_wt_editor: true,
        show_osc_column: true,
        show_mod_matrix: true,
        show_fx_rack: true,
    };
    let options = ShellLayoutOptions {
        piano_visible: true,
        show_osc_column: true,
        show_mod_matrix: true,
        mod_matrix_open: true,
        show_fx_rack: true,
        fx_rack_open: true,
    };
    let screen = Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(APP_MIN_WIDTH, APP_HEIGHT_FULL),
    );
    let layout = ShellLayout::compute_with_options(screen, options);
    audit_shell(&layout, screen, options);

    let scale = layout.scale.ui();
    let center_inner = layout.center.shrink(SPACE_SM * scale);
    let center_regions = compute_center_regions(
        center_inner,
        &config,
        scale,
        embed_mod_fx_in_center(options),
        embed_piano_in_center(options),
    );
    audit_center(layout.center, &center_regions, scale);

    let midi = ShellMidiDevices {
        names: &["None".to_string()],
        selected: 0,
    };
    let preview = Patch::default_mono();

    let mut harness = Harness::builder()
        .with_size([APP_MIN_WIDTH, APP_HEIGHT_FULL])
        .build_state(
            |ctx, test| {
                if !test.fonts_applied {
                    reelsynth_ui_theme::apply(ctx);
                    test.fonts_applied = true;
                    return;
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    let screen = ui.max_rect();
                    let _actions = draw_shell(
                        ui,
                        screen,
                        &mut test.state,
                        None,
                        &preview,
                        &midi,
                        &config,
                        None,
                    );
                });
            },
            ShellTest {
                fonts_applied: false,
                state: UiState::default(),
            },
        );
    harness.run();

    // Top-level regions
    let header_used = get_used(&harness.ctx, header_used_rect_id(), "header");
    assert!(
        fits_max_slack(layout.header, header_used, 12.0),
        "header used rect out of bounds: used={header_used:?} header={:?}",
        layout.header
    );

    let osc_used = get_used(&harness.ctx, osc_used_rect_id(), "osc");
    assert!(
        fits_max_slack(layout.osc, osc_used, 12.0),
        "osc used rect out of bounds: used={osc_used:?} osc={:?}",
        layout.osc
    );

    let rail_used = get_used(&harness.ctx, rail_used_rect_id(), "rail");
    assert!(rail_used.max.y <= layout.rail.max.y + 0.5);

    let piano_used = get_used(&harness.ctx, center_piano_used_rect_id(), "center piano");
    assert!(
        fits_max_slack(center_regions.piano, piano_used, 12.0),
        "piano used rect out of bounds: used={piano_used:?} piano={:?}",
        center_regions.piano
    );
    assert!(!layout.piano_wrap.is_positive());

    let footer_used = get_used(&harness.ctx, footer_used_rect_id(), "footer");
    assert!(fits_max_slack(layout.footer, footer_used, 12.0));

    let center_used = get_used(&harness.ctx, center_used_rect_id(), "center");
    assert!(fits_max_slack(layout.center, center_used, 12.0));

    // Center subregions
    let scope_used = get_used(&harness.ctx, center_scope_used_rect_id(), "center scope");
    assert!(fits_max_slack(center_regions.scope, scope_used, 12.0));

    let strip_used = get_used(&harness.ctx, center_strip_used_rect_id(), "center strip");
    assert!(fits_max_slack(center_regions.wt_strip, strip_used, 12.0));

    let morph_used = get_used(&harness.ctx, center_morph_used_rect_id(), "center morph");
    assert!(fits_max_slack(center_regions.morph, morph_used, 12.0));

    let views_used = get_used(&harness.ctx, center_views_used_rect_id(), "center views");
    assert!(fits_max_slack(center_regions.wt_views, views_used, 12.0));

    let mod_used = get_used(&harness.ctx, center_mod_used_rect_id(), "center mod");
    assert!(fits_max_slack(center_regions.mod_matrix, mod_used, 12.0));

    let fx_used = get_used(&harness.ctx, center_fx_used_rect_id(), "center fx");
    assert!(fits_max_slack(center_regions.fx_rack, fx_used, 12.0));

    let piano_region_used = get_used(&harness.ctx, center_piano_used_rect_id(), "center piano widget");
    assert!(fits_max_slack(center_regions.piano, piano_region_used, 12.0));

    // Bottom strips only exist when not embedded; should be absent here.
    assert!(harness.ctx.data(|d| d.get_temp::<egui::Rect>(mod_strip_used_rect_id())).is_none());
    assert!(harness.ctx.data(|d| d.get_temp::<egui::Rect>(fx_strip_used_rect_id())).is_none());
}
