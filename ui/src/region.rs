//! Allocate child UI at a fixed screen rect (egui 0.30+).

use egui::{Rect, Ui, UiBuilder};

/// Place a top-down child `Ui` in `rect`.
pub fn region<R>(ui: &mut Ui, rect: Rect, body: impl FnOnce(&mut Ui) -> R) -> R {
    ui.allocate_new_ui(UiBuilder::new().max_rect(rect), body)
        .inner
}
