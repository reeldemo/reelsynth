use egui::{Margin, Rounding, Stroke, Ui};
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::layout::{BUTTON_FONT_SIZE, BUTTON_RADIUS};

/// Tab bar matching `.rs-tabs` / `.rs-tab`.
pub fn tab_bar(ui: &mut Ui, tabs: &[&str], selected: &mut usize) {
    let tokens = Tokens::default();
    egui::Frame::none()
        .fill(tokens.surface2)
        .stroke(Stroke::new(1.0_f32, tokens.border))
        .rounding(Rounding::same(8.0))
        .inner_margin(Margin::symmetric(2.0, 2.0))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                for (i, label) in tabs.iter().enumerate() {
                    let active = *selected == i;
                    if tab_item(ui, label, active).clicked() {
                        *selected = i;
                    }
                }
            });
        });
}

fn tab_item(ui: &mut Ui, label: &str, active: bool) -> egui::Response {
    let tokens = Tokens::default();
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(BUTTON_FONT_SIZE),
        if active {
            tokens.accent_on
        } else {
            tokens.text_muted
        },
    );
    let pad_x = 10.0;
    let pad_y = 4.0;
    let size = egui::vec2(galley.size().x + pad_x * 2.0, galley.size().y + pad_y * 2.0);
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        let hovered = response.hovered();
        let pressed = response.is_pointer_button_down_on();
        let fill = if active {
            if pressed {
                tokens.accent
            } else if hovered {
                tokens.accent.gamma_multiply(1.1)
            } else {
                tokens.accent
            }
        } else if pressed {
            tokens.accent
        } else if hovered {
            tokens.accent_muted
        } else {
            egui::Color32::TRANSPARENT
        };
        let stroke = if active || hovered || pressed {
            ACCENT_UI
        } else {
            egui::Color32::TRANSPARENT
        };
        let text = if active || pressed {
            tokens.accent_on
        } else if hovered {
            tokens.text
        } else {
            tokens.text_muted
        };

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, BUTTON_RADIUS, fill);
        if stroke != egui::Color32::TRANSPARENT {
            painter.rect_stroke(rect, BUTTON_RADIUS, Stroke::new(1.0_f32, stroke));
        }
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(BUTTON_FONT_SIZE),
            text,
        );
    }

    response
}
