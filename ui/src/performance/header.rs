//! Header controls for key, scale, and layout mode.

use egui::Ui;
use reelsynth_ui_theme::Tokens;

use crate::audit_registry::{record_region, AuditId};

use crate::widgets::{
    menu_divider, menu_section_label, menu_selectable, reel_combo, select_value_text,
    styled_menu_body,
};
use crate::UiState;

use super::{CHORD_DEGREE_LABELS, LAYOUT_NAMES, ROOT_NAMES, SCALE_NAMES};

pub struct PerformanceHeaderActions {
    pub params_changed: bool,
    pub chord_degree_on: Option<usize>,
    pub chord_degree_off: Option<usize>,
}

impl Default for PerformanceHeaderActions {
    fn default() -> Self {
        Self {
            params_changed: false,
            chord_degree_on: None,
            chord_degree_off: None,
        }
    }
}

fn performance_summary(state: &UiState) -> String {
    let perf = &state.performance;
    let root = ROOT_NAMES[perf.root.min(ROOT_NAMES.len().saturating_sub(1))];
    let scale = SCALE_NAMES[perf.scale.min(SCALE_NAMES.len().saturating_sub(1))];
    let layout = LAYOUT_NAMES[perf.layout.min(LAYOUT_NAMES.len().saturating_sub(1))];
    let mut summary = format!("{root} · {scale} · {layout}");
    if perf.layout == 2 {
        if let Some(deg) = state.active_chord_degree {
            let label = CHORD_DEGREE_LABELS[deg.min(CHORD_DEGREE_LABELS.len().saturating_sub(1))];
            summary.push_str(" · ");
            summary.push_str(label);
        }
    }
    summary
}

/// Compact Performance dropdown: key, scale, layout, and chord degree when applicable.
pub fn draw_performance_header(
    ui: &mut Ui,
    state: &mut UiState,
) -> PerformanceHeaderActions {
    let tokens = Tokens::default();
    let mut actions = PerformanceHeaderActions::default();
    let summary = performance_summary(state);

    let perf_start = ui.cursor().min;
    ui.label(
        egui::RichText::new("Perf")
            .size(10.0)
            .color(tokens.text_muted),
    );
    let combo = reel_combo(ui, "perf_settings", select_value_text(&summary), 110.0, |ui| {
        styled_menu_body(ui, |ui| {
            let perf = &mut state.performance;

            menu_section_label(ui, "Key");
            for (idx, name) in ROOT_NAMES.iter().enumerate() {
                if menu_selectable(ui, perf.root == idx, name).clicked() {
                    perf.root = idx;
                    actions.params_changed = true;
                }
            }

            menu_divider(ui);
            menu_section_label(ui, "Scale");
            for (idx, name) in SCALE_NAMES.iter().enumerate() {
                if menu_selectable(ui, perf.scale == idx, name).clicked() {
                    perf.scale = idx;
                    actions.params_changed = true;
                }
            }

            menu_divider(ui);
            menu_section_label(ui, "Layout");
            for (idx, name) in LAYOUT_NAMES.iter().enumerate() {
                if menu_selectable(ui, perf.layout == idx, name).clicked() {
                    perf.layout = idx;
                    actions.params_changed = true;
                }
            }

            if perf.layout == 2 {
                menu_divider(ui);
                menu_section_label(ui, "Chord degree");
                for (deg, label) in CHORD_DEGREE_LABELS.iter().enumerate() {
                    let active = state.active_chord_degree == Some(deg);
                    if menu_selectable(ui, active, label).clicked() {
                        if active {
                            actions.chord_degree_off = Some(deg);
                            state.active_chord_degree = None;
                        } else {
                            if let Some(prev) = state.active_chord_degree {
                                actions.chord_degree_off = Some(prev);
                            }
                            actions.chord_degree_on = Some(deg);
                            state.active_chord_degree = Some(deg);
                        }
                    }
                }
            }
        });
    });
    let perf_rect = egui::Rect::from_min_max(
        egui::pos2(perf_start.x, ui.min_rect().min.y),
        combo.response.rect.max,
    );
    if perf_rect.is_positive() {
        record_region(ui.ctx(), AuditId::HeaderPerformance, perf_rect, perf_rect);
    }

    actions
}
