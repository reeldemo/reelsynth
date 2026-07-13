//! Standard ReelSynth buttons — `.rs-btn`, `.rs-btn--ghost`, `.rs-toggle`, tool strip.

use egui::{Align2, Color32, FontId, Response, Sense, Ui, Vec2};
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::layout::{
    BUTTON_FONT_SIZE, BUTTON_FONT_SIZE_TOOL, BUTTON_PAD_X, BUTTON_PAD_X_COMPACT,
    BUTTON_PAD_Y, BUTTON_PAD_Y_COMPACT, BUTTON_RADIUS,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonVariant {
    Primary,
    Ghost,
    Toggle,
    Tool,
    Icon,
}

#[derive(Clone, Copy, Debug)]
pub struct ButtonState {
    pub active: bool,
    pub enabled: bool,
}

impl Default for ButtonState {
    fn default() -> Self {
        Self {
            active: false,
            enabled: true,
        }
    }
}

/// Primary action button (accent fill).
pub fn button_primary(ui: &mut Ui, label: &str) -> Response {
    paint_button(ui, label, ButtonVariant::Primary, ButtonState::default())
}

/// Ghost / secondary button (transparent, bordered).
pub fn button_ghost(ui: &mut Ui, label: &str) -> Response {
    paint_button(ui, label, ButtonVariant::Ghost, ButtonState::default())
}

/// Toggle button — `on` controls active appearance.
pub fn button_toggle(ui: &mut Ui, label: &str, on: bool) -> Response {
    paint_button(
        ui,
        label,
        ButtonVariant::Toggle,
        ButtonState {
            active: on,
            enabled: true,
        },
    )
}

/// Compact toolbar tool button.
pub fn button_tool(ui: &mut Ui, label: &str, active: bool, enabled: bool) -> Response {
    paint_button(
        ui,
        label,
        ButtonVariant::Tool,
        ButtonState { active, enabled },
    )
}

/// Small icon-only control (FX reorder, close, etc.).
pub fn button_icon(ui: &mut Ui, icon: &str) -> Response {
    paint_button(ui, icon, ButtonVariant::Icon, ButtonState::default())
}

/// Cycle picker — ghost button sized for enum stepping.
pub fn button_cycle(ui: &mut Ui, label: &str) -> Response {
    button_ghost(ui, label)
}

/// Label + cycle button row (Type, Warp, Shape, etc.).
pub fn labeled_cycle(ui: &mut Ui, field_label: &str, value_label: &str) -> Response {
    let tokens = Tokens::default();
    let mut clicked = None;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(field_label)
                .size(10.0)
                .color(tokens.text_muted),
        );
        clicked = Some(button_cycle(ui, value_label));
    });
    clicked.unwrap()
}

fn paint_button(ui: &mut Ui, label: &str, variant: ButtonVariant, state: ButtonState) -> Response {
    let tokens = Tokens::default();
    let (pad_x, pad_y, font_size) = match variant {
        ButtonVariant::Tool => (BUTTON_PAD_X_COMPACT, BUTTON_PAD_Y_COMPACT, BUTTON_FONT_SIZE_TOOL),
        ButtonVariant::Icon => (6.0, 2.0, BUTTON_FONT_SIZE_TOOL),
        _ => (BUTTON_PAD_X, BUTTON_PAD_Y, BUTTON_FONT_SIZE),
    };

    let text_color = button_text_color(variant, state, &tokens);
    let galley = ui
        .painter()
        .layout_no_wrap(label.to_owned(), FontId::proportional(font_size), text_color);
    let size = Vec2::new(galley.size().x + pad_x * 2.0, galley.size().y + pad_y * 2.0);
    let sense = if state.enabled {
        Sense::click()
    } else {
        Sense::hover()
    };
    let (rect, response) = ui.allocate_exact_size(size, sense);

    if ui.is_rect_visible(rect) {
        let (fill, stroke, text) = button_colors(variant, state, &tokens, &response);
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, BUTTON_RADIUS, fill);
        painter.rect_stroke(rect, BUTTON_RADIUS, egui::Stroke::new(1.0_f32, stroke));
        painter.text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(font_size),
            text,
        );
    }

    if !state.enabled {
        return response;
    }
    response
}

fn button_text_color(variant: ButtonVariant, state: ButtonState, tokens: &Tokens) -> Color32 {
    if !state.enabled {
        return tokens.text_muted.gamma_multiply(0.5);
    }
    match variant {
        ButtonVariant::Primary => tokens.accent_on,
        ButtonVariant::Toggle if state.active => tokens.accent_on,
        ButtonVariant::Tool if state.active => tokens.accent_on,
        _ => tokens.text,
    }
}

fn button_colors(
    variant: ButtonVariant,
    state: ButtonState,
    tokens: &Tokens,
    response: &Response,
) -> (Color32, Color32, Color32) {
    let hovered = response.hovered() && state.enabled;
    let pressed = response.is_pointer_button_down_on() && state.enabled;

    let text = button_text_color(variant, state, tokens);

    match variant {
        ButtonVariant::Primary => {
            let fill = if pressed {
                tokens.accent
            } else if hovered {
                tokens.accent.gamma_multiply(1.15)
            } else {
                tokens.accent
            };
            (fill, ACCENT_UI, text)
        }
        ButtonVariant::Ghost | ButtonVariant::Icon => ghost_button_colors(tokens, hovered, pressed, text),
        ButtonVariant::Tool if state.active => {
            let fill = if pressed {
                tokens.accent
            } else if hovered {
                tokens.accent.gamma_multiply(1.1)
            } else {
                tokens.accent
            };
            (fill, ACCENT_UI, tokens.accent_on)
        }
        ButtonVariant::Tool => ghost_button_colors(tokens, hovered, pressed, text),
        ButtonVariant::Toggle => {
            if state.active {
                let fill = if pressed {
                    tokens.accent
                } else if hovered {
                    tokens.accent.gamma_multiply(1.1)
                } else {
                    tokens.accent
                };
                (fill, ACCENT_UI, tokens.accent_on)
            } else {
                let fill = if pressed {
                    tokens.accent
                } else if hovered {
                    tokens.accent_muted
                } else {
                    tokens.bg_muted
                };
                let stroke = if hovered || pressed {
                    ACCENT_UI
                } else {
                    tokens.border
                };
                let text = if pressed {
                    tokens.accent_on
                } else {
                    tokens.text_muted
                };
                (fill, stroke, text)
            }
        }
    }
}

fn ghost_button_colors(
    tokens: &Tokens,
    hovered: bool,
    pressed: bool,
    text: Color32,
) -> (Color32, Color32, Color32) {
    let fill = if pressed {
        tokens.accent
    } else if hovered {
        tokens.accent_muted
    } else {
        Color32::TRANSPARENT
    };
    let stroke = if hovered || pressed {
        ACCENT_UI
    } else {
        tokens.border
    };
    let text = if pressed { tokens.accent_on } else { text };
    (fill, stroke, text)
}
