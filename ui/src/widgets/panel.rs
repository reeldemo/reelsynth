use egui::{Color32, FontId, Frame, Margin, Ui};
use reelsynth_ui_theme::{heading_font, ACCENT_UI, Tokens};

use crate::layout::{RADIUS_SM, SPACE_SM};

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

/// Native sidebar section — same chrome as [`panel`], with optional meta line (Effects, Mod Matrix).
pub fn sidebar_panel<R>(
    ui: &mut Ui,
    title: &str,
    meta: &str,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let tokens = Tokens::default();
    panel(ui, title, |ui| {
        if !meta.is_empty() {
            ui.horizontal(|ui| {
                ui.with_layout(
                    egui::Layout::right_to_left(egui::Align::Center),
                    |ui| {
                        let meta_color = if meta.contains('⚠') {
                            Color32::from_rgb(0xe8, 0xa8, 0x40)
                        } else {
                            tokens.text_muted
                        };
                        ui.label(
                            egui::RichText::new(meta)
                                .font(egui::FontId::monospace(10.0))
                                .color(meta_color),
                        );
                    },
                );
            });
            ui.add_space(4.0);
        }
        add_contents(ui)
    })
}

/// Collapsible sidebar section — bottom-strip layout when FX/mod are not embedded.
pub fn collapsible_panel(
    ui: &mut Ui,
    title: &str,
    meta: &str,
    open: &mut bool,
    add_contents: impl FnOnce(&mut Ui),
) {
    let tokens = Tokens::default();
    let mut is_open = *open;
    Frame {
        fill: tokens.bg_muted,
        stroke: egui::Stroke::new(1.0_f32, tokens.border),
        rounding: egui::Rounding::same(RADIUS_SM),
        inner_margin: Margin::ZERO,
        ..Default::default()
    }
    .show(ui, |ui| {
        ui.set_width(ui.available_width());
        let header_h = 26.0;
        let (header_rect, header_resp) = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), header_h),
            egui::Sense::click(),
        );
        if ui.is_rect_visible(header_rect) {
            let painter = ui.painter_at(header_rect);
            painter.rect_filled(header_rect, egui::Rounding {
                nw: RADIUS_SM,
                ne: RADIUS_SM,
                sw: if is_open { 0.0 } else { RADIUS_SM },
                se: if is_open { 0.0 } else { RADIUS_SM },
            }, tokens.surface2);
            painter.line_segment(
                [header_rect.left_bottom(), header_rect.right_bottom()],
                egui::Stroke::new(1.0_f32, tokens.border),
            );
            let chevron = if is_open { "▼" } else { "▶" };
            painter.text(
                egui::pos2(header_rect.min.x + SPACE_SM, header_rect.center().y),
                egui::Align2::LEFT_CENTER,
                chevron,
                FontId::proportional(10.0),
                tokens.text_secondary,
            );
            painter.text(
                egui::pos2(header_rect.min.x + SPACE_SM + 16.0, header_rect.center().y),
                egui::Align2::LEFT_CENTER,
                title.to_uppercase(),
                heading_font(11.0),
                tokens.text,
            );
            let meta_color = if meta.contains('⚠') {
                Color32::from_rgb(0xe8, 0xa8, 0x40)
            } else {
                tokens.text_secondary
            };
            painter.text(
                egui::pos2(header_rect.max.x - SPACE_SM, header_rect.center().y),
                egui::Align2::RIGHT_CENTER,
                meta,
                FontId::monospace(10.0),
                meta_color,
            );
        }
        if header_resp.clicked() {
            is_open = !is_open;
        }

        if is_open {
            egui::Frame::none()
                .inner_margin(Margin::symmetric(SPACE_SM, 6.0))
                .show(ui, |ui| add_contents(ui));
        }
    });
    *open = is_open;
}

/// Highlight stroke for active / hovered cards in sidebar grids.
pub fn card_stroke(active: bool, hovered: bool, tokens: &Tokens) -> Color32 {
    if active {
        ACCENT_UI
    } else if hovered {
        tokens.border_strong
    } else {
        tokens.border
    }
}
