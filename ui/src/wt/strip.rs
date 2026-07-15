use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Ui, Vec2};
use reelsynth::patch::{Oscillator, WaveSlot};
use reelsynth::{resolve_wt_position, WavetableBank};
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::layout::{RADIUS_SM, WT_STRIP_HEIGHT};

use super::waveform::waveform_points;
use super::slots::effective_quant_count;
use super::toolbar::WtEditTool;

pub struct WtStripResponse {
    pub response: Response,
    pub changed: bool,
}

pub struct WtStrip<'a> {
    pub position: &'a mut f32,
    pub wave_quant: u8,
    pub wave_slot: &'a mut u8,
    pub wave_slot_fine: &'a mut f32,
    pub wave_slots: &'a [WaveSlot],
    pub bank: Option<&'a WavetableBank>,
    pub bank_name: Option<&'a str>,
    pub visible_frames: usize,
    pub edit_tool: WtEditTool,
}

impl<'a> WtStrip<'a> {
    pub fn show(self, ui: &mut Ui) -> WtStripResponse {
        let tokens = Tokens::default();
        let accent_ui = ACCENT_UI;
        let num_frames = self
            .bank
            .map(|b| b.num_frames)
            .unwrap_or(256);
        let slot_mode = self.wave_quant > 0;
        let cell_count = if slot_mode {
            effective_quant_count(self.wave_quant)
        } else {
            self.visible_frames.min(num_frames).max(8)
        };

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), WT_STRIP_HEIGHT), Sense::click_and_drag());

        let mut changed = false;
        if response.clicked() || response.dragged() {
            if let Some(pos) = response.interact_pointer_pos() {
                let t = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                if slot_mode {
                    let slot = (t * (cell_count.saturating_sub(1)) as f32).round() as u8;
                    if *self.wave_slot != slot || (*self.wave_slot_fine).abs() > f32::EPSILON {
                        select_slot(
                            self.wave_quant,
                            self.wave_slot,
                            self.wave_slot_fine,
                            self.wave_slots,
                            self.position,
                            slot,
                            num_frames,
                        );
                        changed = true;
                    }
                } else if let Some(new_pos) = continuous_position(t, num_frames) {
                    if (*self.position - new_pos).abs() > 0.01 {
                        *self.position = new_pos;
                        changed = true;
                    }
                }
            }
        }

        if ui.is_rect_visible(rect) {
            paint_strip(
                ui,
                rect,
                &tokens,
                accent_ui,
                num_frames,
                cell_count,
                slot_mode,
                self,
            );
        }

        WtStripResponse { response, changed }
    }
}

fn continuous_position(t: f32, num_frames: usize) -> Option<f32> {
    if num_frames == 0 {
        None
    } else {
        Some(t * (num_frames.saturating_sub(1)) as f32)
    }
}

fn select_slot(
    wave_quant: u8,
    wave_slot: &mut u8,
    wave_slot_fine: &mut f32,
    wave_slots: &[WaveSlot],
    position: &mut f32,
    slot: u8,
    num_frames: usize,
) {
    let max_slot = effective_quant_count(wave_quant).saturating_sub(1) as u8;
    *wave_slot = slot.min(max_slot);
    *wave_slot_fine = 0.0;
    let osc = Oscillator {
        position: *position,
        wave_quant,
        wave_slot: *wave_slot,
        wave_slot_fine: *wave_slot_fine,
        wave_slots: wave_slots.to_vec(),
        ..Oscillator::default_va()
    };
    *position = resolve_wt_position(&osc, 0.0, 0.0, num_frames);
}

fn paint_strip(
    ui: &mut Ui,
    rect: Rect,
    tokens: &Tokens,
    accent_ui: Color32,
    num_frames: usize,
    cell_count: usize,
    slot_mode: bool,
    strip: WtStrip<'_>,
) {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, RADIUS_SM, tokens.surface2);
    painter.rect_stroke(
        rect,
        RADIUS_SM,
        egui::Stroke::new(1.0_f32, tokens.border),
    );

    let pad = 4.0;
    let inner = rect.shrink(pad);
    let cell_w = inner.width() / cell_count as f32;

    let (active_idx, norm_pos) = if slot_mode {
        let idx = *strip.wave_slot as usize;
        let fine = strip.wave_slot_fine.clamp(0.0, 1.0);
        let norm = if cell_count > 1 {
            (idx as f32 + fine) / (cell_count - 1) as f32
        } else {
            0.0
        };
        (idx, norm)
    } else {
        let norm = if num_frames > 1 {
            *strip.position / (num_frames - 1) as f32
        } else {
            0.0
        };
        let idx = (norm * (cell_count - 1) as f32).round() as usize;
        (idx, norm)
    };

    for i in 0..cell_count {
        let x = inner.min.x + i as f32 * cell_w;
        let cell = Rect::from_min_size(
            Pos2::new(x + 0.5, inner.min.y),
            Vec2::new(cell_w - 1.0, inner.height()),
        );
        let is_active = i == active_idx;
        painter.rect_filled(cell, 4.0, tokens.bg);
        if is_active {
            painter.rect_stroke(cell, 4.0, egui::Stroke::new(1.0_f32, accent_ui));
        } else {
            painter.rect_stroke(cell, 4.0, egui::Stroke::new(1.0_f32, tokens.border));
        }

        if let Some(bank) = strip.bank {
            let fi = if slot_mode {
                strip
                    .wave_slots
                    .get(i)
                    .map(|s| s.frame.round() as usize)
                    .unwrap_or_else(|| {
                        (i * num_frames / cell_count.max(1)).min(num_frames.saturating_sub(1))
                    })
            } else {
                (i * num_frames / cell_count).min(num_frames.saturating_sub(1))
            };
            paint_waveform_thumbnail(&painter, cell, bank, fi, is_active, accent_ui, tokens.accent);
        } else {
            paint_placeholder_wave(&painter, cell, is_active, accent_ui, tokens.accent);
        }

        if slot_mode {
            let label = strip
                .wave_slots
                .get(i)
                .map(|s| s.label.as_str())
                .filter(|l| !l.is_empty())
                .unwrap_or("");
            if !label.is_empty() {
                painter.text(
                    Pos2::new(cell.center().x, cell.max.y - 3.0),
                    egui::Align2::CENTER_BOTTOM,
                    label,
                    egui::FontId::proportional(10.0),
                    if is_active {
                        accent_ui
                    } else {
                        tokens.text_muted
                    },
                );
            }
            if strip.edit_tool == WtEditTool::Curve {
                let slot = strip.wave_slots.get(i);
                let frame_norm = slot
                    .map(|s| s.frame / 255.0)
                    .unwrap_or(i as f32 / cell_count.max(1) as f32);
                let bar_h = 3.0;
                let bar_y = cell.max.y - 6.0;
                let bar_w = cell.width() * 0.8;
                let fill_w = bar_w * frame_norm.clamp(0.0, 1.0);
                let bar_rect = Rect::from_min_size(
                    Pos2::new(cell.center().x - bar_w * 0.5, bar_y),
                    egui::vec2(bar_w, bar_h),
                );
                painter.rect_filled(bar_rect, 1.0, tokens.border);
                painter.rect_filled(
                    Rect::from_min_size(bar_rect.min, egui::vec2(fill_w, bar_h)),
                    1.0,
                    accent_ui.gamma_multiply(0.7),
                );
            }
        }
    }

    let playhead_x = inner.min.x + norm_pos * inner.width();
    painter.line_segment(
        [
            Pos2::new(playhead_x, inner.min.y),
            Pos2::new(playhead_x, inner.max.y),
        ],
        egui::Stroke::new(2.0_f32, accent_ui),
    );

    let label = if slot_mode {
        let slot_label = strip
            .wave_slots
            .get(active_idx)
            .map(|s| s.label.as_str())
            .filter(|l| !l.is_empty())
            .unwrap_or("");
        let name = strip.bank_name.unwrap_or("Wavetable");
        if slot_label.is_empty() {
            format!(
                "{name} · slot {}/{} · frame {:.0}",
                active_idx + 1,
                cell_count,
                strip.position
            )
        } else {
            format!(
                "{name} · {slot_label} (slot {}) · frame {:.0}",
                active_idx + 1,
                strip.position
            )
        }
    } else if let Some(name) = strip.bank_name {
        let frame_i = strip.position.round() as u32;
        format!("{name} · {num_frames} frames · pos {frame_i}")
    } else {
        format!(
            "Position · frame {:.0} / {}",
            strip.position,
            num_frames.saturating_sub(1)
        )
    };
    painter.text(
        Pos2::new(rect.min.x + 8.0, rect.min.y + 4.0),
        egui::Align2::LEFT_TOP,
        label,
        egui::FontId::proportional(10.0),
        tokens.text_muted,
    );
}

fn paint_waveform_thumbnail(
    painter: &egui::Painter,
    rect: Rect,
    bank: &WavetableBank,
    frame_idx: usize,
    active: bool,
    accent_ui: Color32,
    accent: Color32,
) {
    let frame = bank.frame(frame_idx);
    let points = waveform_points(frame, rect, 32, 0.35);
    if points.len() >= 2 {
        let color = if active { accent } else { accent_ui };
        painter.add(Shape::line(
            points,
            egui::Stroke::new(if active { 2.0_f32 } else { 1.5_f32 }, color),
        ));
    }
}

fn paint_placeholder_wave(
    painter: &egui::Painter,
    rect: Rect,
    active: bool,
    accent_ui: Color32,
    accent: Color32,
) {
    let mid_y = rect.center().y;
    let w = rect.width();
    let points: Vec<Pos2> = (0..=8)
        .map(|i| {
            let t = i as f32 / 8.0;
            let x = rect.min.x + t * w;
            let y = mid_y + (t * std::f32::consts::TAU * 2.0).sin() * rect.height() * 0.25;
            Pos2::new(x, y)
        })
        .collect();
    let color = if active { accent } else { accent_ui };
    painter.add(Shape::line(points, egui::Stroke::new(1.5_f32, color)));
}
