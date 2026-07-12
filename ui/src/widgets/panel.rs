use egui::{Frame, Margin, Ui};
use reelsynth_ui_theme::Tokens;

/// Branded section frame matching `.rs-panel`.
pub fn panel<R>(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let tokens = Tokens::default();
    Frame {
        fill: tokens.bg_muted,
        stroke: egui::Stroke::new(1.0_f32, tokens.border),
        rounding: egui::Rounding::same(8.0),
        inner_margin: Margin::same(8.0),
        ..Default::default()
    }
    .show(ui, |ui| {
        ui.label(
            egui::RichText::new(title.to_uppercase())
                .size(11.0)
                .strong()
                .color(tokens.text_muted),
        );
        ui.add_space(8.0);
        add_contents(ui)
    })
    .inner
}

/// Disabled panel wrapper matching `.rs-group--disabled`.
pub fn panel_disabled<R>(ui: &mut Ui, title: &str, add_contents: impl FnOnce(&mut Ui) -> R) -> R {
    let title = format!("{title} (locked)");
    ui.add_enabled_ui(false, |ui| panel(ui, &title, add_contents))
        .inner
}
