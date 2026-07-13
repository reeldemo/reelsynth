use egui::{Frame, Margin, Ui};
use reelsynth_ui_theme::{heading_font, Tokens};

use crate::layout::RADIUS_SM;

/// Branded section frame matching `.rs-panel`.
pub fn panel<R>(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let tokens = Tokens::default();
    Frame {
        fill: tokens.bg_muted,
        stroke: egui::Stroke::new(1.0_f32, tokens.border),
        rounding: egui::Rounding::same(RADIUS_SM),
        inner_margin: Margin::same(6.0),
        ..Default::default()
    }
    .show(ui, |ui| {
        let display = if let Some(base) = title.strip_suffix(" (locked)") {
            format!("{} (locked)", base.to_uppercase())
        } else {
            title.to_uppercase()
        };
        ui.label(
            egui::RichText::new(display)
                .font(heading_font(10.0))
                .color(tokens.text_muted),
        );
        ui.add_space(6.0);
        add_contents(ui)
    })
    .inner
}

/// Disabled panel wrapper matching `.rs-group--disabled`.
pub fn panel_disabled<R>(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let locked_title = format!("{} (locked)", title.to_uppercase());
    ui.add_enabled_ui(false, |ui| panel(ui, &locked_title, add_contents))
        .inner
}
