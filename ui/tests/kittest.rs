//! egui_kittest interaction smoke tests (UI-K01 … UI-K06) + UI audit matrix.

mod common;

use std::collections::HashSet;

use common::audit_harness::{
    assert_full_ui_audit, run_shell_audit, FullUiAuditOptions, ShellAuditScenario,
};
use egui::Rect;
use egui_kittest::Harness;
use reelsynth::Patch;
use reelsynth_ui::{
    audit_center, audit_header_clusters, audit_osc_sidebar_stacks, audit_panel_utilization,
    audit_shell, audit_theme_tokens, audit_id_rect, compute_center_regions, count_base_audit_variants,
    default_effect_slots, draw_shell, embed_piano_in_center, osc_type_index, record_region,
    record_used, AuditId, REGISTRY_VARIANT_COUNT, ShellLayout, ShellLayoutOptions, ShellMidiDevices,
    ShellMode, WtView3dMode,
    center_morph_used_rect_id, center_scope_used_rect_id,
    center_strip_used_rect_id, center_used_rect_id, center_views_used_rect_id, footer_used_rect_id,
    fx_strip_used_rect_id, header_used_rect_id, mod_strip_used_rect_id,
    osc_fx_allocated_rect_id, osc_fx_used_rect_id, osc_mod_allocated_rect_id, osc_mod_used_rect_id,
    osc_used_rect_id, piano_used_rect_id, rail_filter_allocated_rect_id, rail_filter_used_rect_id,
    rail_used_rect_id,
    ShellConfig, UiState, APP_HEIGHT_FULL, APP_MIN_WIDTH, APP_WIDTH, SPACE_SM, utilization,
};
use reelsynth_ui::widgets::{adsr_graph, button_ghost, button_toggle, labeled_select, panel_audit, reel_combo, select_value_text, tab_bar, Knob, KnobSize, PianoKeyboard};
use reelsynth_ui_theme;

#[test]
fn piano_widget_harness_smoke() {
    let keys = HashSet::new();
    let mut harness = Harness::new_ui(|ui| {
        reelsynth_ui_theme::Tokens::default();
        let _ = PianoKeyboard::compact(&keys).show(ui);
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
    state.oscillators[1].osc_type = osc_type_index("square");
    assert_eq!(state.oscillators[1].osc_type, 2);
}

#[test]
fn add_remove_oscillator_state() {
    let mut state = UiState::default();
    let initial = state.oscillators.len();
    state.add_oscillator();
    assert_eq!(state.oscillators.len(), initial + 1);
    state.remove_oscillator(0);
    assert_eq!(state.oscillators.len(), initial);
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
    let regions = compute_center_regions(inner, &config, scale, embed_piano_in_center(options), true);
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
        embed_piano_in_center(options),
        true,
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
    assert!(
        layout.rail.max.y <= layout.piano_wrap.min.y + 0.5,
        "right rail must end above full-width piano"
    );
    assert!(
        layout.osc.max.y <= layout.piano_wrap.min.y + 0.5,
        "left osc column must end above full-width piano"
    );

    assert!(layout.piano_wrap.is_positive());
    assert!((layout.piano_wrap.width() - layout.footer.width()).abs() < 0.5);
    assert!(!center_regions.piano.is_positive());
    let piano_used = get_used(&harness.ctx, piano_used_rect_id(), "piano wrap");
    assert!(
        fits_max_slack(layout.piano_wrap, piano_used, 12.0),
        "piano used rect out of bounds: used={piano_used:?} piano={:?}",
        layout.piano_wrap
    );
    // Sidebar paint must not land in the keyboard band.
    assert!(
        rail_used.max.y <= layout.piano_wrap.min.y + 12.0,
        "rail content spills into piano: rail_used={rail_used:?} piano={:?}",
        layout.piano_wrap
    );

    let footer_used = get_used(&harness.ctx, footer_used_rect_id(), "footer");
    assert!(fits_max_slack(layout.footer, footer_used, 12.0));

    let center_used = get_used(&harness.ctx, center_used_rect_id(), "center");
    assert!(fits_max_slack(layout.center, center_used, 12.0));

    // Center subregions
    let scope_used = get_used(&harness.ctx, center_scope_used_rect_id(), "center scope");
    assert!(fits_max_slack(center_regions.scope, scope_used, 12.0));

    let strip_used = get_used(&harness.ctx, center_strip_used_rect_id(), "center strip");
    assert!(fits_max_slack(center_regions.wt_strip, strip_used, 12.0));

    let morph_used = harness.ctx.data(|d| d.get_temp::<egui::Rect>(center_morph_used_rect_id()));
    assert!(
        morph_used.is_none() || !center_regions.morph.is_positive(),
        "Design layer-first layout should not allocate morph bar"
    );

    let views_used = get_used(&harness.ctx, center_views_used_rect_id(), "center views");
    assert!(fits_max_slack(center_regions.wt_views, views_used, 12.0));

    let osc_fx_used = get_used(&harness.ctx, osc_fx_used_rect_id(), "osc fx");
    assert!(fits_max_slack(layout.osc, osc_fx_used, 12.0));

    let osc_mod_used = get_used(&harness.ctx, osc_mod_used_rect_id(), "osc mod");
    assert!(
        osc_mod_used.min.x >= layout.osc.min.x - 12.0
            && osc_mod_used.max.x <= layout.osc.max.x + 12.0,
        "osc mod matrix extends outside left column: used={osc_mod_used:?} osc={:?}",
        layout.osc
    );

    audit_osc_sidebar_stacks(&harness.ctx);

    // Bottom strips only exist when not in sidebars; should be absent here.
    assert!(harness.ctx.data(|d| d.get_temp::<egui::Rect>(mod_strip_used_rect_id())).is_none());
    assert!(harness.ctx.data(|d| d.get_temp::<egui::Rect>(fx_strip_used_rect_id())).is_none());
}

const PANEL_UTIL_MIN: f32 = 0.50;

struct ShellHarnessTest {
    fonts_applied: bool,
    state: UiState,
}

#[test]
fn panel_whitespace_utilization_at_1280x880() {
    let options = ShellLayoutOptions {
        piano_visible: true,
        show_osc_column: true,
        show_mod_matrix: true,
        mod_matrix_open: true,
        show_fx_rack: true,
        fx_rack_open: true,
    };
    let screen = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 880.0));
    let layout = ShellLayout::compute_with_options(screen, options);
    audit_shell(&layout, screen, options);

    let config = ShellConfig {
        show_wt_editor: true,
        show_osc_column: true,
        show_mod_matrix: true,
        show_fx_rack: true,
    };
    let midi = ShellMidiDevices {
        names: &["None".to_string()],
        selected: 0,
    };
    let preview = Patch::default_mono();

    let mut harness = Harness::builder()
        .with_size([1280.0, 880.0])
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
                        None,
                    );
                });
            },
            ShellHarnessTest {
                fonts_applied: false,
                state: UiState::default(),
            },
        );
    harness.run();
    audit_panel_utilization(&harness.ctx, PANEL_UTIL_MIN);
    audit_osc_sidebar_stacks(&harness.ctx);

    let osc_fx_alloc = get_used(&harness.ctx, osc_fx_allocated_rect_id(), "osc fx allocated");
    let osc_fx_used = get_used(&harness.ctx, osc_fx_used_rect_id(), "osc fx used");
    assert!(
        utilization(osc_fx_alloc, osc_fx_used) >= PANEL_UTIL_MIN,
        "osc fx sidebar under-utilized"
    );

    let filter_alloc = get_used(&harness.ctx, rail_filter_allocated_rect_id(), "filter allocated");
    let filter_used = get_used(&harness.ctx, rail_filter_used_rect_id(), "filter used");
    assert!(
        utilization(filter_alloc, filter_used) >= PANEL_UTIL_MIN,
        "filter panel under-utilized"
    );

    let mod_alloc = get_used(&harness.ctx, osc_mod_allocated_rect_id(), "mod allocated");
    let mod_used = get_used(&harness.ctx, osc_mod_used_rect_id(), "mod used");
    assert!(
        utilization(mod_alloc, mod_used) >= PANEL_UTIL_MIN,
        "osc mod matrix under-utilized"
    );
}

#[test]
fn header_control_clusters_no_overlap_at_default_width() {
    run_header_cluster_audit(ShellHarnessTest {
        fonts_applied: false,
        state: UiState::default(),
    });
}

#[test]
fn header_control_clusters_no_overlap_chords_layout() {
    let mut state = UiState::default();
    state.performance.layout = 2;
    state.active_chord_degree = Some(0);
    run_header_cluster_audit(ShellHarnessTest {
        fonts_applied: false,
        state,
    });
}

#[test]
fn header_control_clusters_no_overlap_compose_mode() {
    let mut state = UiState::default();
    state.shell_mode = reelsynth_ui::ShellMode::Compose;
    run_header_cluster_audit(ShellHarnessTest {
        fonts_applied: false,
        state,
    });
}

fn run_header_cluster_audit(test: ShellHarnessTest) {
    let config = ShellConfig {
        show_wt_editor: true,
        show_osc_column: true,
        show_mod_matrix: true,
        show_fx_rack: true,
    };
    let options = ShellLayoutOptions {
        piano_visible: true,
        show_osc_column: test.state.shell_mode != reelsynth_ui::ShellMode::Compose,
        show_mod_matrix: test.state.shell_mode != reelsynth_ui::ShellMode::Compose,
        mod_matrix_open: true,
        show_fx_rack: test.state.shell_mode != reelsynth_ui::ShellMode::Compose,
        fx_rack_open: true,
    };
    let screen = Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(APP_WIDTH, APP_HEIGHT_FULL),
    );
    let layout = ShellLayout::compute_with_options(screen, options);
    audit_shell(&layout, screen, options);

    let midi = ShellMidiDevices {
        names: &["None".to_string(), "Virtual MIDI".to_string()],
        selected: 0,
    };
    let preview = Patch::default_mono();

    let mut harness = Harness::builder()
        .with_size([APP_WIDTH, APP_HEIGHT_FULL])
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
                        None,
                    );
                });
            },
            test,
        );
    harness.run();
    audit_header_clusters(&harness.ctx, layout.header);
}

// --- UI audit matrix (Phase 3) ---

#[test]
fn theme_contrast_all_pairs_harness_gate() {
    audit_theme_tokens();
}

#[test]
fn audit_registry_coverage_gate() {
    assert_eq!(count_base_audit_variants(), REGISTRY_VARIANT_COUNT);
}

fn default_audit_options() -> FullUiAuditOptions {
    FullUiAuditOptions {
        audit_registry: false,
        ..Default::default()
    }
}

#[test]
fn design_shell_geometry() {
    let run = run_shell_audit(ShellAuditScenario::default().size(1440.0, 900.0));
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_header_subelement() {
    let run = run_shell_audit(ShellAuditScenario::default());
    audit_header_clusters(&run.ctx, run.layout.header);
}

#[test]
fn design_osc_column_wt() {
    let run = run_shell_audit(ShellAuditScenario::default());
    assert!(run.layout.osc.is_positive());
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_osc_column_fm() {
    let mut scenario = ShellAuditScenario::default();
    scenario.state.oscillators[0].osc_type = osc_type_index("wavetable");
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_osc_column_pulse() {
    let mut scenario = ShellAuditScenario::default();
    scenario.state.oscillators[0].osc_type = osc_type_index("pulse");
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_fx_sidebar_slots() {
    let mut scenario = ShellAuditScenario::default();
    scenario.state.fx_slots.push(reelsynth_ui::EffectSlotUi::from_slot(
        &reelsynth::fx::EffectSlot::chorus(),
    ));
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_osc_fx_sidebar() {
    let mut scenario = ShellAuditScenario::default();
    scenario.state.fx_slots.push(reelsynth_ui::EffectSlotUi::from_slot(
        &reelsynth::fx::EffectSlot::delay(),
    ));
    scenario = scenario.size(APP_MIN_WIDTH, APP_HEIGHT_FULL);
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_mod_matrix_rows() {
    let mut scenario = ShellAuditScenario::default();
    while scenario.state.mod_routes.len() < 4 {
        scenario.state.mod_routes.push(reelsynth_ui::ModSlotUi::default());
    }
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_center_scope() {
    let run = run_shell_audit(ShellAuditScenario::default());
    let used = run.ctx.data(|d| d.get_temp::<Rect>(center_scope_used_rect_id()));
    assert!(used.is_some());
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_default_three_layers() {
    let state = UiState::default();
    assert_eq!(state.selected_layer_idx, Some(0));
    assert!(
        state.oscillators[0].wave_layers.len() >= 3,
        "default Design should seed at least 3 stack layers"
    );
}

#[test]
fn design_shape_template_maps_to_layer_type() {
    use reelsynth_ui::wt::shape_template_source_type;
    use reelsynth_ui::wt::FrameShapeTemplate;
    assert_eq!(shape_template_source_type(FrameShapeTemplate::Saw), "saw");
    assert_eq!(shape_template_source_type(FrameShapeTemplate::Tri), "triangle");
}

#[test]
fn design_wt_strip_morph() {
    let run = run_shell_audit(ShellAuditScenario::default());
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_wt_tool_curve() {
    let mut scenario = ShellAuditScenario::default();
    scenario.state.wt_edit_tool = reelsynth_ui::wt::WtEditTool::Curve;
    scenario.state.oscillators[0].wave_quant = 64;
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_wt_tool_shape() {
    let mut scenario = ShellAuditScenario::default();
    scenario.state.wt_edit_tool = reelsynth_ui::wt::WtEditTool::Shape;
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_wt_3d_stack() {
    let mut scenario = ShellAuditScenario::default();
    scenario.state.wt_view_3d_mode = WtView3dMode::Stack;
    let run = run_shell_audit(scenario);
    let stack = audit_id_rect(&run.ctx, AuditId::CenterWt3dStack);
    assert!(stack.is_some(), "Design right pane should record composite stack overlay");
    let toggle = audit_id_rect(&run.ctx, AuditId::CenterWt3dModeToggle);
    assert!(toggle.is_none(), "Stack/Morph toggle hidden on Design home");
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_stack_overlay_with_layers() {
    use reelsynth_ui::WaveLayerUi;
    let mut scenario = ShellAuditScenario::default();
    scenario.state.wt_view_3d_mode = WtView3dMode::Stack;
    scenario.state.oscillators[0].wave_layers = vec![
        WaveLayerUi {
            source_type: "saw".into(),
            level: 0.65,
            enabled: true,
            ..WaveLayerUi::default()
        },
        WaveLayerUi {
            source_type: "sine".into(),
            level: 0.35,
            enabled: true,
            invert: true,
            ..WaveLayerUi::default()
        },
    ];
    scenario.state.oscillators[0].stack_mode = "add".into();
    let run = run_shell_audit(scenario);
    let views = audit_id_rect(&run.ctx, AuditId::CenterWtViews);
    assert!(views.is_some(), "wt views region should be recorded");
    let chip_rect = audit_id_rect(&run.ctx, AuditId::CenterWtStripLayerChip(0));
    assert!(
        chip_rect.is_some(),
        "Design strip should show layer chips when layers present"
    );
    let stack = audit_id_rect(&run.ctx, AuditId::CenterWt3dStack);
    assert!(stack.is_some(), "right pane shows composite stack overlay with layers");
}

#[test]
fn design_wt_3d_morph() {
    let mut scenario = ShellAuditScenario::default();
    scenario.state.wt_view_3d_mode = WtView3dMode::Morph;
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_wt_3d_default_is_stack() {
    assert_eq!(UiState::default().wt_view_3d_mode, WtView3dMode::Stack);
    let run = run_shell_audit(ShellAuditScenario::default());
    let stack = audit_id_rect(&run.ctx, AuditId::CenterWt3dStack);
    assert!(stack.is_some(), "default Design opens composite stack overlay");
    let toggle = audit_id_rect(&run.ctx, AuditId::CenterWt3dModeToggle);
    assert!(toggle.is_none(), "Stack/Morph dual toggle not on Design home");
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_strip_layer_chips_with_layers() {
    use reelsynth_ui::WaveLayerUi;
    let mut scenario = ShellAuditScenario::default();
    scenario.state.oscillators[0].wave_layers = vec![
        WaveLayerUi {
            source_type: "saw".into(),
            level: 0.5,
            enabled: true,
            ..WaveLayerUi::default()
        },
        WaveLayerUi {
            source_type: "square".into(),
            level: 0.4,
            enabled: true,
            ..WaveLayerUi::default()
        },
    ];
    let run = run_shell_audit(scenario);
    assert!(
        audit_id_rect(&run.ctx, AuditId::CenterWtStripLayerChip(0)).is_some(),
        "L1 chip visible"
    );
    assert!(
        audit_id_rect(&run.ctx, AuditId::CenterWtStripLayerChip(1)).is_some(),
        "L2 chip visible"
    );
    let strip = audit_id_rect(&run.ctx, AuditId::CenterWtStrip);
    assert!(strip.is_some());
    assert!(
        audit_id_rect(&run.ctx, AuditId::CenterWtStripCell(0)).is_none(),
        "no frame cells on layer-first strip"
    );
}

#[test]
fn design_scope_result_label() {
    assert_eq!(reelsynth_ui::SCOPE_RESULT_LABEL, "Result");
    // Shell audit harness passes no WT bank, so paint the strip isolated to audit Result.
    let mut strip_state = reelsynth_ui::ScopeStripState::default();
    let bank = reelsynth::WavetableBank::factory_saw_morph();
    let patch = Patch::factory_lead();
    let bank_for_osc: &dyn Fn(usize) -> usize = &|_| 0;
    let mut harness = Harness::builder()
        .with_size([480.0, 72.0])
        .build_ui(|ui| {
            let rect = ui.max_rect();
            reelsynth_ui::draw_scope_strip(
                ui,
                rect,
                reelsynth_ui::ScopeStripInput {
                    patch: &patch,
                    banks: std::slice::from_ref(&bank),
                    bank_for_osc: &bank_for_osc,
                    live: None,
                    is_playing: false,
                    now_secs: 1.0,
                    state: &mut strip_state,
                },
            );
        });
    harness.run();
    let out = audit_id_rect(&harness.ctx, AuditId::CenterScopeCellOut);
    assert!(out.is_some(), "Result cell (audit id CenterScopeCellOut) recorded");
    if let Some(r) = out {
        assert!(r.width() > 8.0 && r.height() > 8.0);
    }
}

#[test]
fn design_analyze_dialog() {
    let mut scenario = ShellAuditScenario::default();
    scenario.state.analyze_dialog_open = true;
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_rail_filter_env_lfo() {
    let run = run_shell_audit(ShellAuditScenario::default());
    audit_panel_utilization(&run.ctx, PANEL_UTIL_MIN);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_chord_grid() {
    let mut scenario = ShellAuditScenario::default();
    scenario.state.performance.layout = 2;
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_full_width_piano() {
    let run = run_shell_audit(ShellAuditScenario::default());
    assert!(run.layout.piano_wrap.is_positive());
    assert!((run.layout.piano_wrap.width() - run.size[0]).abs() < 1.0);
    assert!(run.layout.rail.max.y <= run.layout.piano_wrap.min.y + 0.5);
    assert!(run.layout.osc.max.y <= run.layout.piano_wrap.min.y + 0.5);
    assert!(!embed_piano_in_center(run.layout_options));
    let piano_used = run.ctx.data(|d| d.get_temp::<Rect>(piano_used_rect_id()));
    assert!(piano_used.is_some());
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_wt_quant_handles() {
    let mut scenario = ShellAuditScenario::default();
    scenario.state.wt_edit_tool = reelsynth_ui::wt::WtEditTool::Select;
    scenario.state.oscillators[0].wave_quant = 16;
    let run = run_shell_audit(scenario);
    let plot = audit_id_rect(&run.ctx, AuditId::CenterWt2dPlot);
    assert!(plot.is_some(), "2d plot region should be recorded for quant handles");
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_sidebar_parity() {
    let run = run_shell_audit(ShellAuditScenario::default());
    let diff = (run.layout.osc.width() - run.layout.rail.width()).abs();
    assert!(diff < 1.5, "osc={} rail={}", run.layout.osc.width(), run.layout.rail.width());
}

#[test]
fn design_min_scale_072() {
    let run = run_shell_audit(ShellAuditScenario::default().size(1280.0, 880.0));
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn design_compact_no_osc() {
    let run = run_shell_audit(ShellAuditScenario::default().no_osc_column());
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn compose_full_layout() {
    let run = run_shell_audit(ShellAuditScenario::default().compose_mode());
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn compose_transport_controls() {
    let mut scenario = ShellAuditScenario::default().compose_mode();
    scenario.state.compose.transport.recording = true;
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn compose_piano_roll_pencil() {
    let mut scenario = ShellAuditScenario::default().compose_mode();
    scenario.state.compose.piano_roll_tool = reelsynth_ui::PianoRollTool::Pencil;
    scenario.state.compose.selected_clip = Some(0);
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn compose_scene_launch() {
    let mut scenario = ShellAuditScenario::default().compose_mode();
    scenario.state.compose.launched_scene = Some(0);
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn compose_arrangement_clips() {
    let mut scenario = ShellAuditScenario::default().compose_mode();
    if scenario.state.compose.project.tracks.len() < 2 {
        scenario.state.compose.project.tracks.push(reelsynth::Track::new("Track 2"));
    }
    let run = run_shell_audit(scenario);
    assert_full_ui_audit(&run, &default_audit_options());
}

#[test]
fn widget_knob_sizes() {
    let mut harness = Harness::builder()
        .with_size([120.0, 120.0])
        .build_ui(|ui| {
            reelsynth_ui_theme::apply(ui.ctx());
            let mut v = 0.5_f32;
            let sm = Knob::new(&mut v, 0.0..=1.0, "Sm")
                .size(KnobSize::Sm)
                .show(ui);
            record_used(ui.ctx(), AuditId::WidgetKnobSm, sm.response.rect);
            let md = Knob::new(&mut v, 0.0..=1.0, "Md")
                .size(KnobSize::Md)
                .show(ui);
            record_used(ui.ctx(), AuditId::WidgetKnobMd, md.response.rect);
            let lg = Knob::new(&mut v, 0.0..=1.0, "Lg")
                .size(KnobSize::Lg)
                .show(ui);
            record_used(ui.ctx(), AuditId::WidgetKnobLg, lg.response.rect);
        });
    harness.run();
}

#[test]
fn widget_audit_registry_harness() {
    struct FontState {
        applied: bool,
    }
    let mut harness = Harness::builder()
        .with_size([480.0, 320.0])
        .build_state(
            |ctx, state| {
                if !state.applied {
                    reelsynth_ui_theme::apply(ctx);
                    state.applied = true;
                    return;
                }
                egui::CentralPanel::default().show(ctx, |ui| {
                    panel_audit(ui, "Panel", Some(AuditId::WidgetPanel), |ui| {
                        let ghost = button_ghost(ui, "Ghost");
                        record_used(ui.ctx(), AuditId::WidgetButtonGhost, ghost.rect);
                        let toggle = button_toggle(ui, "Toggle", true);
                        record_used(ui.ctx(), AuditId::WidgetButtonToggle, toggle.rect);
                        let mut idx = 0usize;
                        let select_before = ui.min_rect();
                        labeled_select(ui, "Type", &["A", "B"], &mut idx);
                        record_region(
                            ui.ctx(),
                            AuditId::WidgetLabeledSelect,
                            select_before,
                            ui.min_rect(),
                        );
                        let combo_before = ui.min_rect();
                        reel_combo(ui, "test_combo", select_value_text("Opt"), 120.0, |ui| {
                            let _ = ui.label("item");
                        });
                        record_region(
                            ui.ctx(),
                            AuditId::WidgetReelCombo,
                            combo_before,
                            ui.min_rect(),
                        );
                        let mut tab = 0usize;
                        let tabs_before = ui.min_rect();
                        tab_bar(ui, &["One", "Two"], &mut tab);
                        record_region(
                            ui.ctx(),
                            AuditId::WidgetTabBar,
                            tabs_before,
                            ui.min_rect(),
                        );
                        let graph_before = ui.min_rect();
                        let mut a = 0.01_f32;
                        let mut d = 0.2;
                        let mut s = 0.7;
                        let mut r = 0.3;
                        adsr_graph(ui, &mut a, &mut d, &mut s, &mut r, 1.0, "kit_adsr");
                        record_region(
                            ui.ctx(),
                            AuditId::WidgetAdsrGraph,
                            graph_before,
                            ui.min_rect(),
                        );
                    });
                    panel_audit(ui, "Sidebar", Some(AuditId::WidgetSidebarPanel), |ui| {
                        ui.label("meta");
                    });
                });
            },
            FontState { applied: false },
        );
    harness.run();
}

#[test]
fn full_ui_audit_with_registry() {
    let run = run_shell_audit(ShellAuditScenario::default());
    let mut opts = FullUiAuditOptions::default();
    opts.audit_registry = true;
    assert_full_ui_audit(&run, &opts);
}
