//! Quant-snapped waveform knob handles for slot-based editing.

use egui::{Color32, CursorIcon, Pos2, Rect, Response, Sense, Ui};
use reelsynth::patch::WaveSlot;
use reelsynth::WavetableBank;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use super::slots::effective_quant_count;

const HANDLE_RADIUS: f32 = 6.0;
const WAVE_AMP: f32 = 0.42;

pub struct QuantHandleEditor<'a> {
    pub plot_rect: Rect,
    pub wave_quant: u8,
    pub wave_slots: &'a mut [WaveSlot],
    pub bank: &'a WavetableBank,
    pub frame_idx: usize,
}

pub struct QuantHandleResponse {
    pub changed: bool,
    pub hovered_slot: Option<usize>,
    pub dragged_slot: Option<usize>,
    pub status_label: Option<String>,
}

impl QuantHandleEditor<'_> {
    pub fn show(self, ui: &mut Ui) -> QuantHandleResponse {
        let tokens = Tokens::default();
        let accent_ui = ACCENT_UI;
        let slot_count = effective_quant_count(self.wave_quant).max(1);
        let show_all_knobs = self.wave_quant <= 64;

        let drag_slot_id = ui.id().with("quant_drag_slot");
        let locked_slot: Option<usize> = ui.ctx().data(|d| d.get_temp(drag_slot_id));

        let sense = Sense::click_and_drag();
        let response = ui.allocate_rect(self.plot_rect, sense);

        let mut changed = false;
        let mut hovered_slot = None;
        let mut dragged_slot = locked_slot;
        let mut status_label = None;

        let pointer = response
            .interact_pointer_pos()
            .filter(|p| self.plot_rect.contains(*p));

        if self.wave_quant == 0 {
            return QuantHandleResponse {
                changed: false,
                hovered_slot: None,
                dragged_slot: None,
                status_label: None,
            };
        }

        if response.drag_started() {
            if let Some(pos) = pointer {
                let slot = snap_x_to_slot(pos.x, self.plot_rect, slot_count);
                ui.ctx().data_mut(|d| d.insert_temp(drag_slot_id, slot));
                dragged_slot = Some(slot);
            }
        }

        if !response.dragged() && response.drag_stopped() {
            ui.ctx().data_mut(|d| d.remove::<usize>(drag_slot_id));
            dragged_slot = None;
        }

        if let Some(pos) = pointer {
            if locked_slot.is_none() && !response.dragged() {
                hovered_slot = Some(nearest_slot(pos.x, self.plot_rect, slot_count));
            }
        }

        if let Some(slot) = dragged_slot.or(if response.dragged() { None } else { hovered_slot }) {
            if response.dragged() {
                if let Some(pos) = pointer {
                    let frame = y_to_frame(pos.y, self.plot_rect);
                    if let Some(ws) = self.wave_slots.get_mut(slot) {
                        if (ws.frame - frame).abs() > f32::EPSILON {
                            ws.frame = frame;
                            changed = true;
                            status_label = Some(format!("Slot {} → frame {:.0}", slot + 1, frame));
                        }
                    }
                }
            }
        }

        let painter = ui.painter_at(self.plot_rect);
        for i in 0..slot_count {
            let show = show_all_knobs
                || hovered_slot == Some(i)
                || dragged_slot == Some(i)
                || i == 0;
            if !show {
                continue;
            }
            let x = slot_x(i, slot_count, self.plot_rect);
            let frame = self
                .wave_slots
                .get(i)
                .map(|s| s.frame)
                .unwrap_or(128.0);
            let y = frame_to_y(frame, self.plot_rect);
            let center = Pos2::new(x, y);
            let active = dragged_slot == Some(i) || hovered_slot == Some(i);
            let radius = if active { HANDLE_RADIUS * 1.25 } else { HANDLE_RADIUS };
            let fill = if active {
                accent_ui.gamma_multiply(0.35)
            } else {
                tokens.surface2
            };
            painter.circle_filled(center, radius, fill);
            painter.circle_stroke(
                center,
                radius,
                egui::Stroke::new(if active { 2.0 } else { 1.0 }, accent_ui),
            );

            if active && self.wave_quant > 0 {
                let band_w = self.plot_rect.width() / slot_count as f32;
                let band = Rect::from_center_size(
                    Pos2::new(x, self.plot_rect.center().y),
                    egui::vec2(band_w, self.plot_rect.height()),
                );
                painter.rect_filled(band, 0.0, tokens.accent.gamma_multiply(0.12));
            }
        }

        if response.hovered() || response.dragged() {
            let cursor = if response.dragged() {
                CursorIcon::Grabbing
            } else if hovered_slot.is_some() || dragged_slot.is_some() {
                CursorIcon::Grab
            } else {
                CursorIcon::Grab
            };
            ui.ctx().set_cursor_icon(cursor);
        }

        QuantHandleResponse {
            changed,
            hovered_slot,
            dragged_slot,
            status_label,
        }
    }
}

pub fn snap_x_to_slot(x: f32, plot: Rect, slot_count: usize) -> usize {
    nearest_slot(x, plot, slot_count)
}

pub fn nearest_slot(x: f32, plot: Rect, slot_count: usize) -> usize {
    if slot_count <= 1 {
        return 0;
    }
    let t = ((x - plot.min.x) / plot.width()).clamp(0.0, 1.0);
    (t * (slot_count - 1) as f32).round() as usize
}

pub fn slot_x(slot: usize, slot_count: usize, plot: Rect) -> f32 {
    if slot_count <= 1 {
        return plot.center().x;
    }
    let t = slot as f32 / (slot_count - 1) as f32;
    egui::lerp(plot.min.x..=plot.max.x, t)
}

pub fn frame_to_y(frame: f32, plot: Rect) -> f32 {
    let norm = (frame / 255.0).clamp(-1.0, 1.0);
    plot.center().y - norm * plot.height() * WAVE_AMP
}

pub fn y_to_frame(y: f32, plot: Rect) -> f32 {
    let norm = (plot.center().y - y) / (plot.height() * WAVE_AMP);
    (norm * 255.0).clamp(-255.0, 255.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;

    #[test]
    fn snap_x_to_slot_boundaries() {
        let plot = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(160.0, 80.0));
        assert_eq!(snap_x_to_slot(plot.min.x, plot, 16), 0);
        assert_eq!(snap_x_to_slot(plot.max.x, plot, 16), 15);
        assert_eq!(snap_x_to_slot(plot.center().x, plot, 16), 8);
    }

    #[test]
    fn frame_y_roundtrip() {
        let plot = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 100.0));
        let frame = 128.0;
        let y = frame_to_y(frame, plot);
        let back = y_to_frame(y, plot);
        assert!((back - frame).abs() < 2.0, "roundtrip {frame} -> {back}");
    }
}
