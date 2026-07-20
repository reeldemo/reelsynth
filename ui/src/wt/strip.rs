use egui::{Color32, Pos2, Rect, Response, Sense, Shape, Ui, Vec2};
use reelsynth::patch::{Oscillator, WaveSlot};
use reelsynth::{resolve_wt_position, WavetableBank};
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::audit_registry::{record_used, AuditId};
use crate::layout::{RADIUS_SM, WT_STRIP_HEIGHT};
use crate::oscillator_ui::WaveLayerUi;

use super::waveform::waveform_points;
use super::slots::effective_quant_count;
use super::toolbar::WtEditTool;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StripMode {
    /// Full-width layer chips (Design home).
    #[default]
    Layers,
    /// Frame/slot scrub strip (advanced / Compose paths).
    Frames,
}

pub struct WtStripResponse {
    pub response: Response,
    pub changed: bool,
    pub params_changed: bool,
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
    pub wave_layers: &'a mut Vec<WaveLayerUi>,
    pub selected_layer_idx: &'a mut Option<usize>,
    /// Design = `Layers` (full-width chips, no frame scrub). Advanced = `Frames`.
    pub strip_mode: StripMode,
    /// Legacy flag: when true and `strip_mode == Frames`, paint layer chips beside frames.
    pub show_layer_chips: bool,
}

impl<'a> WtStrip<'a> {
    pub fn show(self, ui: &mut Ui) -> WtStripResponse {
        let tokens = Tokens::default();
        let accent_ui = ACCENT_UI;
        let num_frames = self
            .bank
            .map(|b| b.num_frames)
            .unwrap_or(256);
        let layers_mode = self.strip_mode == StripMode::Layers;
        let has_layers = !self.wave_layers.is_empty()
            && (layers_mode || (self.show_layer_chips && self.strip_mode == StripMode::Frames));
        let layer_frac = if layers_mode && has_layers {
            1.0
        } else if has_layers {
            0.38
        } else {
            0.0
        };
        let paint_frames = self.strip_mode == StripMode::Frames;

        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), WT_STRIP_HEIGHT), Sense::click_and_drag());

        let mut changed = false;
        let mut params_changed = false;

        if ui.is_rect_visible(rect) {
            let pad = 4.0;
            let inner = rect.shrink(pad);
            let layer_w = inner.width() * layer_frac;
            let gap = if has_layers { 4.0 } else { 0.0 };
            let frame_rect = Rect::from_min_max(
                Pos2::new(inner.min.x + layer_w + gap, inner.min.y),
                inner.max,
            );
            let layer_rect = if has_layers {
                Some(Rect::from_min_max(inner.min, Pos2::new(inner.min.x + layer_w, inner.max.y)))
            } else {
                None
            };

            if let Some(lr) = layer_rect {
                params_changed |= paint_layer_chips(
                    ui,
                    lr,
                    &tokens,
                    accent_ui,
                    self.wave_layers,
                    self.selected_layer_idx,
                    self.bank,
                    layers_mode,
                );
            }

            if paint_frames {
                changed |= paint_frame_strip(
                    ui,
                    frame_rect,
                    &tokens,
                    accent_ui,
                    num_frames,
                    WtStrip {
                        position: self.position,
                        wave_quant: self.wave_quant,
                        wave_slot: self.wave_slot,
                        wave_slot_fine: self.wave_slot_fine,
                        wave_slots: self.wave_slots,
                        bank: self.bank,
                        bank_name: self.bank_name,
                        visible_frames: self.visible_frames,
                        edit_tool: self.edit_tool,
                        wave_layers: self.wave_layers,
                        selected_layer_idx: self.selected_layer_idx,
                        strip_mode: self.strip_mode,
                        show_layer_chips: self.show_layer_chips,
                    },
                    &response,
                );
            }
        }

        WtStripResponse {
            response,
            changed,
            params_changed,
        }
    }
}

fn paint_layer_chips(
    ui: &mut Ui,
    rect: Rect,
    tokens: &Tokens,
    accent_ui: Color32,
    layers: &mut Vec<WaveLayerUi>,
    selected: &mut Option<usize>,
    bank: Option<&WavetableBank>,
    layers_mode: bool,
) -> bool {
    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, RADIUS_SM, tokens.surface2);
    painter.rect_stroke(rect, RADIUS_SM, egui::Stroke::new(1.0, tokens.border));

    let mut changed = false;
    let add_remove_w = if layers_mode { 44.0 } else { 0.0 };
    let inner = rect.shrink2(egui::vec2(4.0, 2.0));
    let chips_rect = Rect::from_min_max(
        inner.min,
        Pos2::new(inner.max.x - add_remove_w, inner.max.y),
    );
    let chip_count = layers.len().max(1);
    let chip_w = (chips_rect.width() - 4.0 * (chip_count as f32 - 1.0)) / chip_count as f32;

    for (i, layer) in layers.iter_mut().enumerate() {
        let x = chips_rect.min.x + i as f32 * (chip_w + 4.0);
        let cell = Rect::from_min_size(Pos2::new(x, chips_rect.min.y), Vec2::new(chip_w, chips_rect.height()));
        let is_sel = *selected == Some(i);
        if is_sel {
            painter.rect_stroke(cell, 4.0, egui::Stroke::new(1.5, accent_ui));
        } else {
            painter.rect_stroke(cell, 4.0, egui::Stroke::new(1.0, tokens.border));
        }
        painter.rect_filled(cell, 4.0, tokens.bg);

        if let Some(bank) = bank {
            if layer.is_wavetable() {
                let fi = layer.wt_position.round() as usize;
                let thumb = cell.shrink2(egui::vec2(4.0, 14.0));
                paint_waveform_thumbnail(
                    &painter,
                    thumb,
                    bank,
                    fi.min(bank.num_frames.saturating_sub(1)),
                    is_sel,
                    accent_ui,
                    tokens.accent,
                );
            } else {
                paint_va_chip_thumbnail(&painter, cell.shrink2(egui::vec2(4.0, 14.0)), &layer.source_type, is_sel, accent_ui, tokens.accent);
            }
        }

        let type_label = if layer.residual {
            "Residual".to_string()
        } else {
            layer.source_type.chars().take(3).collect::<String>()
        };
        painter.text(
            Pos2::new(cell.min.x + 4.0, cell.min.y + 2.0),
            egui::Align2::LEFT_TOP,
            format!("L{} · {type_label}", i + 1),
            egui::FontId::proportional(9.0),
            if is_sel { accent_ui } else { tokens.text_muted },
        );

        record_used(ui.ctx(), AuditId::CenterWtStripLayerChip(i), cell);

        let sign_rect = Rect::from_min_size(
            Pos2::new(cell.max.x - 28.0, cell.max.y - 16.0),
            Vec2::new(26.0, 14.0),
        );

        let chip_resp = ui.interact(cell, ui.id().with(("layer_chip", i)), Sense::click());
        if chip_resp.clicked() {
            *selected = Some(i);
            changed = true;
        }

        if chip_resp.dragged() {
            if let Some(pos) = chip_resp.interact_pointer_pos() {
                let level_t = 1.0 - ((pos.y - cell.min.y) / cell.height()).clamp(0.0, 1.0);
                let next = level_t.clamp(0.0, 1.0);
                if (layer.level - next).abs() > 0.01 {
                    layer.level = next;
                    changed = true;
                }
            }
        }

        let plus = ui.interact(
            Rect::from_min_size(sign_rect.min, Vec2::new(12.0, 14.0)),
            ui.id().with(("layer_plus", i)),
            Sense::click(),
        );
        let minus = ui.interact(
            Rect::from_min_size(sign_rect.min + Vec2::new(14.0, 0.0), Vec2::new(12.0, 14.0)),
            ui.id().with(("layer_minus", i)),
            Sense::click(),
        );
        painter.text(
            plus.rect.center(),
            egui::Align2::CENTER_CENTER,
            "+",
            egui::FontId::proportional(10.0),
            if !layer.invert { accent_ui } else { tokens.text_muted },
        );
        painter.text(
            minus.rect.center(),
            egui::Align2::CENTER_CENTER,
            "−",
            egui::FontId::proportional(10.0),
            if layer.invert { accent_ui } else { tokens.text_muted },
        );
        if plus.clicked() {
            layer.invert = false;
            changed = true;
        }
        if minus.clicked() {
            layer.invert = true;
            changed = true;
        }
    }

    if layers_mode {
        let ctrl = Rect::from_min_max(
            Pos2::new(inner.max.x - add_remove_w + 4.0, inner.min.y),
            inner.max,
        );
        let add_btn = ui.interact(
            Rect::from_min_size(ctrl.min, Vec2::new(18.0, ctrl.height())),
            ui.id().with("layer_add"),
            Sense::click(),
        );
        let rem_btn = ui.interact(
            Rect::from_min_size(ctrl.min + Vec2::new(22.0, 0.0), Vec2::new(18.0, ctrl.height())),
            ui.id().with("layer_remove"),
            Sense::click(),
        );
        painter.text(
            add_btn.rect.center(),
            egui::Align2::CENTER_CENTER,
            "+",
            egui::FontId::proportional(14.0),
            accent_ui,
        );
        painter.text(
            rem_btn.rect.center(),
            egui::Align2::CENTER_CENTER,
            "−",
            egui::FontId::proportional(14.0),
            if layers.len() > 1 {
                accent_ui
            } else {
                tokens.text_muted
            },
        );
        if add_btn.clicked() {
            layers.push(WaveLayerUi::default());
            *selected = Some(layers.len() - 1);
            changed = true;
        }
        if rem_btn.clicked() && layers.len() > 1 {
            let idx = selected.unwrap_or(layers.len() - 1).min(layers.len() - 1);
            layers.remove(idx);
            *selected = Some(idx.min(layers.len().saturating_sub(1)));
            changed = true;
        }
    }

    changed
}

fn paint_va_chip_thumbnail(
    painter: &egui::Painter,
    rect: Rect,
    source_type: &str,
    active: bool,
    accent_ui: Color32,
    accent: Color32,
) {
    let points: Vec<Pos2> = (0..=16)
        .map(|i| {
            let p = i as f32 / 16.0;
            let v = match source_type.to_ascii_lowercase().as_str() {
                "saw" => 2.0 * p - 1.0,
                "square" => if p < 0.5 { 1.0 } else { -1.0 },
                "sine" => (p * std::f32::consts::TAU).sin(),
                "triangle" | "tri" => 1.0 - 4.0 * (p - 0.5).abs(),
                _ => (p * std::f32::consts::TAU).sin(),
            };
            let x = egui::lerp(rect.min.x..=rect.max.x, p);
            let y = rect.center().y - v * rect.height() * 0.35;
            Pos2::new(x, y)
        })
        .collect();
    if points.len() >= 2 {
        let color = if active { accent } else { accent_ui };
        painter.add(Shape::line(points, egui::Stroke::new(if active { 2.0 } else { 1.5 }, color)));
    }
}

fn paint_frame_strip(
    ui: &mut Ui,
    rect: Rect,
    tokens: &Tokens,
    accent_ui: Color32,
    num_frames: usize,
    strip: WtStrip<'_>,
    outer_response: &Response,
) -> bool {
    let slot_mode = strip.wave_quant > 0;
    let cell_count = if slot_mode {
        effective_quant_count(strip.wave_quant)
    } else {
        strip.visible_frames.min(num_frames).max(8)
    };

    let mut changed = false;
    if outer_response.clicked() || outer_response.dragged() {
        if let Some(pos) = outer_response.interact_pointer_pos() {
            if rect.contains(pos) {
                let t = ((pos.x - rect.min.x) / rect.width()).clamp(0.0, 1.0);
                if slot_mode {
                    let slot = (t * (cell_count.saturating_sub(1)) as f32).round() as u8;
                    if *strip.wave_slot != slot || (*strip.wave_slot_fine).abs() > f32::EPSILON {
                        select_slot(
                            strip.wave_quant,
                            strip.wave_slot,
                            strip.wave_slot_fine,
                            strip.wave_slots,
                            strip.position,
                            slot,
                            num_frames,
                        );
                        changed = true;
                    }
                } else if let Some(new_pos) = continuous_position(t, num_frames) {
                    if (*strip.position - new_pos).abs() > 0.01 {
                        *strip.position = new_pos;
                        changed = true;
                    }
                }
            }
        }
    }

    paint_strip_cells(
        ui,
        rect,
        tokens,
        accent_ui,
        num_frames,
        cell_count,
        slot_mode,
        strip,
    );
    changed
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

fn paint_strip_cells(
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
        record_used(ui.ctx(), AuditId::CenterWtStripCell(i), cell);
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
