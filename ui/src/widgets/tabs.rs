use egui::{Ui, WidgetText};

/// Stub tab bar matching `.rs-tabs` / `.rs-tab`.
pub fn tab_bar(ui: &mut Ui, tabs: &[&str], selected: &mut usize) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 2.0;
        for (i, label) in tabs.iter().enumerate() {
            let active = *selected == i;
            if ui.selectable_label(active, WidgetText::from(*label)).clicked() {
                *selected = i;
            }
        }
    });
}
