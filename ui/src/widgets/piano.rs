use std::collections::HashSet;

use egui::{Color32, Pos2, Rect, Response, Sense, Ui, Vec2};
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::layout::{
    PIANO_BLACK_HEIGHT_RATIO, PIANO_BLACK_WIDTH_RATIO, PIANO_OCTAVES, PIANO_START_NOTE,
};

pub struct PianoResponse {
    pub note_on: Option<u8>,
    pub note_off: Option<u8>,
}

pub struct PianoKeyboard<'a> {
    pub keys_down: &'a HashSet<u8>,
    pub start_note: u8,
    pub octaves: usize,
    pub white_key_width: f32,
    pub key_height: f32,
}

impl<'a> PianoKeyboard<'a> {
    pub fn new(keys_down: &'a HashSet<u8>) -> Self {
        Self {
            keys_down,
            start_note: PIANO_START_NOTE,
            octaves: PIANO_OCTAVES,
            white_key_width: 0.0,
            key_height: 0.0,
        }
    }

    pub fn with_layout(mut self, white_key_width: f32, key_height: f32) -> Self {
        self.white_key_width = white_key_width;
        self.key_height = key_height;
        self
    }

    pub fn show(self, ui: &mut Ui) -> (Response, PianoResponse) {
        let white_count = self.octaves * 7;
        let key_w = if self.white_key_width > 0.0 {
            self.white_key_width
        } else {
            16.0
        };
        let key_h = if self.key_height > 0.0 {
            self.key_height
        } else {
            72.0
        };
        let size = Vec2::new(white_count as f32 * key_w, key_h);

        let (rect, response) = ui.allocate_exact_size(size, Sense::click_and_drag());
        self.paint(ui, rect, response)
    }

    pub fn show_in_rect(self, ui: &mut Ui, area: Rect) -> (Response, PianoResponse) {
        let white_count = self.octaves * 7;
        let pad_x = 8.0;
        let pad_y = 4.0;
        let avail_w = (area.width() - pad_x * 2.0).max(1.0);
        let avail_h = (area.height() - pad_y * 2.0).max(1.0);
        let _key_w = avail_w / white_count as f32;
        let key_h = avail_h;

        let size = Vec2::new(avail_w, key_h);
        let origin = Pos2::new(
            area.min.x + (area.width() - avail_w) * 0.5,
            area.min.y + pad_y,
        );
        let rect = Rect::from_min_size(origin, size);
        let response = ui.allocate_rect(area, Sense::hover());
        self.paint(ui, rect, response)
    }

    fn paint(self, ui: &mut Ui, rect: Rect, response: Response) -> (Response, PianoResponse) {
        let mut out = PianoResponse {
            note_on: None,
            note_off: None,
        };

        let id = ui.id().with("mouse_note");
        let mut mouse_note: Option<u8> = ui.data_mut(|d| d.get_temp(id)).unwrap_or(None);

        if !ui.is_rect_visible(rect) {
            return (response, out);
        }

        let tokens = Tokens::default();
        let painter = ui.painter_at(rect);
        let white_count = self.octaves * 7;
        let key_w = rect.width() / white_count as f32;
        let key_h = rect.height();

        let white_notes = white_key_notes(self.start_note, self.octaves);
        let black_notes = black_key_notes(self.start_note, self.octaves);

        if ui.input(|i| i.pointer.primary_down()) {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                if rect.contains(pos) {
                    if let Some(note) = hit_test(pos, rect, key_w, key_h, &white_notes, &black_notes)
                    {
                        if mouse_note != Some(note) {
                            if let Some(old) = mouse_note {
                                out.note_off = Some(old);
                            }
                            mouse_note = Some(note);
                            out.note_on = Some(note);
                        }
                    }
                }
            }
        }

        if ui.input(|i| i.pointer.primary_released()) {
            if let Some(note) = mouse_note.take() {
                out.note_off = Some(note);
            }
        }

        ui.data_mut(|d| d.insert_temp(id, mouse_note));

        for (i, &note) in white_notes.iter().enumerate() {
            let x = rect.min.x + i as f32 * key_w;
            let key_rect = Rect::from_min_size(Pos2::new(x, rect.min.y), Vec2::new(key_w, key_h));
            let active = self.keys_down.contains(&note);
            let top = if active {
                ACCENT_UI
            } else {
                Color32::from_rgb(244, 244, 245)
            };
            let bottom = if active {
                tokens.accent
            } else {
                Color32::from_rgb(212, 212, 216)
            };
            painter.rect_filled(key_rect, 4.0, bottom);
            painter.rect_filled(
                key_rect.shrink2(Vec2::new(0.0, key_rect.height() * 0.12)),
                4.0,
                top,
            );
            painter.rect_stroke(
                key_rect,
                4.0,
                egui::Stroke::new(1.0_f32, Color32::from_rgb(82, 82, 91)),
            );
        }

        let black_h = key_h * PIANO_BLACK_HEIGHT_RATIO;
        let black_w = key_w * PIANO_BLACK_WIDTH_RATIO;

        for &(note, slot) in &black_notes {
            let slot_min_x = rect.min.x + slot as f32 * key_w;
            let slot_max_x = slot_min_x + key_w;
            let key_right = slot_max_x + key_w * 0.29;
            let key_left = key_right - black_w;
            let cx = (key_left + key_right) * 0.5;
            let key_rect = Rect::from_center_size(
                Pos2::new(cx, rect.min.y + black_h * 0.5),
                Vec2::new(black_w, black_h),
            );
            let active = self.keys_down.contains(&note);
            let fill = if active {
                ACCENT_UI
            } else {
                Color32::from_rgb(63, 63, 70)
            };
            painter.rect_filled(key_rect, 3.0, fill);
            painter.rect_stroke(
                key_rect,
                3.0,
                egui::Stroke::new(1.0_f32, Color32::from_rgb(9, 9, 11)),
            );
        }

        (response, out)
    }
}

fn white_key_notes(start: u8, octaves: usize) -> Vec<u8> {
    let mut notes = Vec::with_capacity(octaves * 7);
    for o in 0..octaves {
        let base = start + o as u8 * 12;
        for semi in [0u8, 2, 4, 5, 7, 9, 11] {
            notes.push(base + semi);
        }
    }
    notes
}

fn black_key_notes(start: u8, octaves: usize) -> Vec<(u8, usize)> {
    let mut out = Vec::new();
    let mut white_slot = 0usize;
    for o in 0..octaves {
        let base = start + o as u8 * 12;
        for semi in 0u8..12 {
            let is_white = matches!(semi, 0 | 2 | 4 | 5 | 7 | 9 | 11);
            if is_white {
                white_slot += 1;
            } else if matches!(semi, 1 | 3 | 6 | 8 | 10) {
                out.push((base + semi, white_slot - 1));
            }
        }
    }
    out
}

fn hit_test(
    pos: Pos2,
    rect: Rect,
    key_w: f32,
    key_h: f32,
    white_notes: &[u8],
    black_notes: &[(u8, usize)],
) -> Option<u8> {
    let black_h = key_h * PIANO_BLACK_HEIGHT_RATIO;
    let black_w = key_w * PIANO_BLACK_WIDTH_RATIO;

    if pos.y < rect.min.y + black_h {
        for &(note, slot) in black_notes {
            let slot_min_x = rect.min.x + slot as f32 * key_w;
            let slot_max_x = slot_min_x + key_w;
            let key_right = slot_max_x + key_w * 0.29;
            let key_left = key_right - black_w;
            let cx = (key_left + key_right) * 0.5;
            let key_rect = Rect::from_center_size(
                Pos2::new(cx, rect.min.y + black_h * 0.5),
                Vec2::new(black_w, black_h),
            );
            if key_rect.contains(pos) {
                return Some(note);
            }
        }
    }

    let col = ((pos.x - rect.min.x) / key_w).floor() as usize;
    white_notes.get(col).copied()
}
