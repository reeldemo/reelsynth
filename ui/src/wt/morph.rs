//! Morph A→B position controls (S3 stub — position lerp only).

use egui::{FontId, Sense, Ui, Vec2};
use reelsynth_ui_theme::Tokens;

use crate::layout::WT_MORPH_HEIGHT;
use crate::region::region;

pub struct WtMorph<'a> {
    pub frame_a: &'a mut f32,
    pub frame_b: &'a mut f32,
    pub amount: &'a mut f32,
    pub position: &'a mut f32,
}

pub struct WtMorphResponse {
    pub changed: bool,
}

impl<'a> WtMorph<'a> {
    pub fn show(self, ui: &mut Ui) -> WtMorphResponse {
        let tokens = Tokens::default();
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), WT_MORPH_HEIGHT),
            Sense::hover(),
        );

        let mut changed = false;
        let mut endpoints_changed = false;
        region(ui, rect, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 8.0;
                ui.label(
                    egui::RichText::new("Morph")
                        .size(10.0)
                        .color(tokens.text_muted),
                );

                endpoints_changed |= morph_frame_field(ui, "A", self.frame_a);
                endpoints_changed |= morph_frame_field(ui, "B", self.frame_b);

                let max_frame = 255.0_f32;
                let a = self.frame_a.clamp(0.0, max_frame);
                let b = self.frame_b.clamp(0.0, max_frame);
                *self.frame_a = a;
                *self.frame_b = b;

                if endpoints_changed {
                    *self.position = morph_position(a, b, *self.amount);
                    changed = true;
                }

                let slider_label = format!("{:.0}%", *self.amount * 100.0_f32);
                let mut amount = *self.amount;
                let slider = egui::Slider::new(&mut amount, 0.0..=1.0)
                    .show_value(false)
                    .text(slider_label);
                if ui.add(slider).changed() {
                    *self.amount = amount;
                    *self.position = morph_position(a, b, amount);
                    changed = true;
                }

                ui.label(
                    egui::RichText::new(format!("pos {:.0}", *self.position))
                        .font(FontId::monospace(10.0))
                        .color(tokens.text_muted),
                );
            });
        });

        WtMorphResponse { changed }
    }
}

fn morph_frame_field(ui: &mut Ui, label: &str, value: &mut f32) -> bool {
    let tokens = Tokens::default();
    let mut frame = value.round();
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .size(10.0)
                .color(tokens.text_muted),
        );
        let drag = egui::DragValue::new(&mut frame)
            .range(0.0..=255.0)
            .speed(1.0);
        if ui.add(drag).changed() {
            *value = frame;
            changed = true;
        }
    });
    changed
}

/// Linear interpolation between frame indices A and B.
pub fn morph_position(a: f32, b: f32, t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    a + (b - a) * t
}

/// Normalized morph amount for a position between A and B.
pub fn morph_amount_for_position(a: f32, b: f32, position: f32) -> f32 {
    if (b - a).abs() < f32::EPSILON {
        0.0
    } else {
        ((position - a) / (b - a)).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morph_position_endpoints() {
        assert!((morph_position(0.0, 255.0, 0.0) - 0.0).abs() < 1e-5);
        assert!((morph_position(0.0, 255.0, 1.0) - 255.0).abs() < 1e-5);
        assert!((morph_position(100.0, 200.0, 0.5) - 150.0).abs() < 1e-5);
    }

    #[test]
    fn morph_amount_for_position_roundtrip() {
        let a = 20.0;
        let b = 180.0;
        let amount = 0.35;
        let pos = morph_position(a, b, amount);
        let back = morph_amount_for_position(a, b, pos);
        assert!((back - amount).abs() < 1e-4);
    }
}
