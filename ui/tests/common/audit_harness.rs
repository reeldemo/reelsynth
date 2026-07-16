//! Shared egui_kittest harness helpers for UI audit tests.

use egui::Rect;
use egui_kittest::Harness;
use reelsynth::Patch;
use reelsynth_ui::{
    audit_all_elements, audit_center, audit_compose_panels, audit_header_clusters,
    audit_osc_sidebar_stacks, audit_panel_utilization, audit_rail_panels, audit_shell,
    compute_center_regions, draw_shell, embed_piano_in_center, record_region, AuditId,
    ShellAppSettings, ShellConfig, ShellLayout, ShellLayoutOptions, ShellMidiDevices, ShellMode,
    UiState, APP_HEIGHT_FULL, APP_MIN_WIDTH, SPACE_SM,
};

pub struct ShellAuditScenario {
    pub state: UiState,
    pub config: ShellConfig,
    pub size: [f32; 2],
    pub preview: Patch,
    pub midi_names: Vec<String>,
    pub midi_selected: usize,
    pub app_settings: Option<ShellAppSettings>,
}

impl Default for ShellAuditScenario {
    fn default() -> Self {
        Self {
            state: UiState::default(),
            config: ShellConfig {
                show_wt_editor: true,
                show_osc_column: true,
                show_mod_matrix: true,
                show_fx_rack: true,
            },
            size: [APP_MIN_WIDTH, APP_HEIGHT_FULL],
            preview: Patch::factory_lead(),
            midi_names: vec!["None".to_string(), "Virtual MIDI".to_string()],
            midi_selected: 0,
            app_settings: Some(ShellAppSettings::default()),
        }
    }
}

impl ShellAuditScenario {
    pub fn compose_mode(mut self) -> Self {
        self.state.shell_mode = ShellMode::Compose;
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.size = [w, h];
        self
    }

    pub fn no_osc_column(mut self) -> Self {
        self.config.show_osc_column = false;
        self
    }

    pub fn layout_options(&self) -> ShellLayoutOptions {
        let compose = self.state.shell_mode == ShellMode::Compose;
        ShellLayoutOptions {
            piano_visible: self.state.piano_visible,
            show_osc_column: self.config.show_osc_column && !compose,
            show_mod_matrix: self.config.show_mod_matrix && !compose,
            mod_matrix_open: self.state.mod_matrix_open,
            show_fx_rack: self.config.show_fx_rack && !compose,
            fx_rack_open: self.state.fx_rack_open,
        }
    }
}

struct ShellHarnessState {
    fonts_applied: bool,
    scenario: ShellAuditScenario,
}

pub struct ShellAuditRun {
    pub ctx: egui::Context,
    pub layout: ShellLayout,
    pub shell_mode: ShellMode,
    pub config: ShellConfig,
    pub size: [f32; 2],
    pub layout_options: ShellLayoutOptions,
}

pub fn run_shell_audit(scenario: ShellAuditScenario) -> ShellAuditRun {
    let layout_options = scenario.layout_options();
    let screen = Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(scenario.size[0], scenario.size[1]),
    );
    let layout = ShellLayout::compute_with_options(screen, layout_options);

    let preview = scenario.preview.clone();
    let config = scenario.config;
    let shell_mode = scenario.state.shell_mode;
    let size = scenario.size;

    let mut harness = Harness::builder()
        .with_size(scenario.size)
        .build_state(
            move |ctx, test| {
                if !test.fonts_applied {
                    reelsynth_ui_theme::apply(ctx);
                    test.fonts_applied = true;
                    return;
                }
                let scenario = &mut test.scenario;
                let midi = ShellMidiDevices {
                    names: &scenario.midi_names,
                    selected: scenario.midi_selected,
                };
                egui::CentralPanel::default().show(ctx, |ui| {
                    let screen = ui.max_rect();
                    let _actions = draw_shell(
                        ui,
                        screen,
                        &mut scenario.state,
                        None,
                        &preview,
                        &midi,
                        &config,
                        None,
                        None,
                        scenario.app_settings.as_mut(),
                    );
                });
            },
            ShellHarnessState {
                fonts_applied: false,
                scenario,
            },
        );
    harness.run();
    let ctx = harness.ctx.clone();
    ShellAuditRun {
        ctx,
        layout,
        shell_mode,
        config,
        size,
        layout_options,
    }
}

pub struct FullUiAuditOptions {
    pub panel_util_min: f32,
    pub audit_registry: bool,
    pub audit_center_regions: bool,
}

impl Default for FullUiAuditOptions {
    fn default() -> Self {
        Self {
            panel_util_min: 0.50,
            audit_registry: true,
            audit_center_regions: true,
        }
    }
}

pub fn assert_full_ui_audit(run: &ShellAuditRun, options: &FullUiAuditOptions) {
    let shell_options = run.layout_options;
    let screen = Rect::from_min_size(
        egui::pos2(0.0, 0.0),
        egui::vec2(run.size[0], run.size[1]),
    );

    audit_shell(&run.layout, screen, shell_options);
    audit_header_clusters(&run.ctx, run.layout.header);

    if run.shell_mode == ShellMode::Compose {
        audit_compose_panels(&run.ctx, run.layout.main);
    } else if run.layout.rail.is_positive() {
        audit_rail_panels(&run.ctx, run.layout.rail);
        audit_osc_sidebar_stacks(&run.ctx);
        audit_panel_utilization(&run.ctx, options.panel_util_min);
    }

    if options.audit_center_regions && run.layout.center.is_positive() {
        let scale = run.layout.scale.ui();
        let inner = run.layout.center.shrink(SPACE_SM * scale);
        let regions = compute_center_regions(
            inner,
            &run.config,
            scale,
            embed_piano_in_center(shell_options),
            run.shell_mode == reelsynth_ui::ShellMode::Design,
        );
        audit_center(run.layout.center, &regions, scale);
    }

    if options.audit_registry {
        audit_all_elements(&run.ctx, &run.layout, run.shell_mode);
    }
}
