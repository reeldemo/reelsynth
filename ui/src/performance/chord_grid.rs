//! Chord degree grid for Design mode when layout is Chords.

use egui::{Rect, Sense, Ui, Vec2};
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::audit_registry::{record_used, AuditId};
use crate::performance::CHORD_DEGREE_LABELS;
use crate::region::region;
use crate::UiState;

pub struct ChordGridActions {
    pub chord_degree_on: Option<usize>,
    pub chord_degree_off: Option<usize>,
}

impl Default for ChordGridActions {
    fn default() -> Self {
        Self {
            chord_degree_on: None,
            chord_degree_off: None,
        }
    }
}

/// Diatonic chord pad row (I … vii°) — replaces piano keyboard in Chords layout.
pub fn draw_chord_grid(ui: &mut Ui, rect: Rect, state: &mut UiState) -> ChordGridActions {
    let tokens = Tokens::default();
    let mut actions = ChordGridActions::default();

    let pad_w = ((rect.width() - 6.0 * 4.0) / 7.0).clamp(36.0, 72.0);
    let pad_h = (rect.height() - 8.0).clamp(28.0, 56.0);

    region(ui, rect, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(4.0, 0.0);
            for (deg, label) in CHORD_DEGREE_LABELS.iter().enumerate() {
                let active = state.active_chord_degree == Some(deg);
                let (cell, resp) = ui.allocate_exact_size(Vec2::new(pad_w, pad_h), Sense::click());
                let fill = if active {
                    ACCENT_UI.gamma_multiply(0.85)
                } else if resp.hovered() {
                    tokens.surface2
                } else {
                    tokens.bg_muted
                };
                ui.painter_at(cell).rect_filled(cell, 6.0, fill);
                ui.painter_at(cell).rect_stroke(
                    cell,
                    6.0,
                    egui::Stroke::new(1.0, tokens.border_strong),
                );
                ui.painter_at(cell).text(
                    cell.center(),
                    egui::Align2::CENTER_CENTER,
                    *label,
                    egui::FontId::proportional(12.0),
                    if active {
                        tokens.text
                    } else {
                        tokens.text_secondary
                    },
                );
                if resp.clicked() {
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
                record_used(ui.ctx(), AuditId::FooterChordPad(deg), cell);
            }
        });
    });

    actions
}
