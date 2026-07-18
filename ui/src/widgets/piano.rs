use std::collections::HashSet;

use egui::{Color32, Pos2, Rect, Response, ScrollArea, Sense, Ui, Vec2};
use reelsynth::{note_in_scale, PerformanceLayout, Scale};
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::layout::{
    PIANO_BLACK_HEIGHT_RATIO, PIANO_BLACK_WIDTH_RATIO, PIANO_END_NOTE, PIANO_LEGACY_START,
    PIANO_OCTAVES, PIANO_START_NOTE, PIANO_WHITE_KEY_WIDTH,
};

pub struct PianoResponse {
    pub note_on: Option<u8>,
    pub note_off: Option<u8>,
}

/// Whether the shared on-screen piano should scale-fold (dim + block out-of-scale keys).
///
/// Design and Compose both use this — fold follows **Scale** layout only, never shell mode.
pub fn piano_scale_fold_enabled(layout: PerformanceLayout) -> bool {
    matches!(layout, PerformanceLayout::Scale)
}

pub struct PianoKeyboard<'a> {
    pub keys_down: &'a HashSet<u8>,
    pub start_note: u8,
    pub end_note: u8,
    pub white_key_width: f32,
    pub key_height: f32,
    /// Dim/hide out-of-scale keys when true.
    pub scale_fold: bool,
    pub scale_root: u8,
    pub scale: Scale,
    pub horizontal_scroll: bool,
}

impl<'a> PianoKeyboard<'a> {
    pub fn new(keys_down: &'a HashSet<u8>) -> Self {
        Self {
            keys_down,
            start_note: PIANO_START_NOTE,
            end_note: PIANO_END_NOTE,
            white_key_width: 0.0,
            key_height: 0.0,
            scale_fold: false,
            scale_root: 0,
            scale: Scale::Chromatic,
            horizontal_scroll: true,
        }
    }

    /// Compact 3-octave window from C3 (legacy Design preview).
    pub fn compact(keys_down: &'a HashSet<u8>) -> Self {
        Self {
            keys_down,
            start_note: PIANO_LEGACY_START,
            end_note: PIANO_LEGACY_START + PIANO_OCTAVES as u8 * 12 - 1,
            white_key_width: 0.0,
            key_height: 0.0,
            scale_fold: false,
            scale_root: 0,
            scale: Scale::Chromatic,
            horizontal_scroll: false,
        }
    }

    pub fn with_layout(mut self, white_key_width: f32, key_height: f32) -> Self {
        self.white_key_width = white_key_width;
        self.key_height = key_height;
        self
    }

    pub fn with_scale_fold(mut self, root: u8, scale: Scale, enabled: bool) -> Self {
        self.scale_root = root;
        self.scale = scale;
        self.scale_fold = enabled && !scale.is_chromatic();
        self
    }

    pub fn show(self, ui: &mut Ui) -> (Response, PianoResponse) {
        let white_count = white_key_count(self.start_note, self.end_note);
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
        let pad_x = 4.0;
        let pad_y = 2.0;
        let avail_w = (area.width() - pad_x * 2.0).max(1.0);
        let avail_h = (area.height() - pad_y * 2.0).max(1.0);
        let white_count = white_key_count(self.start_note, self.end_note);
        let key_w = if self.white_key_width > 0.0 {
            self.white_key_width
        } else if self.horizontal_scroll {
            PIANO_WHITE_KEY_WIDTH
        } else {
            (avail_w / white_count as f32).clamp(10.0, PIANO_WHITE_KEY_WIDTH * 1.35)
        };
        let keyboard_w = key_w * white_count as f32;
        let key_h = avail_h;

        let mut combined = PianoResponse {
            note_on: None,
            note_off: None,
        };
        let area_response = ui.allocate_rect(area, Sense::hover());

        if self.horizontal_scroll && keyboard_w > avail_w {
            crate::region::region(ui, area, |ui| {
                ScrollArea::horizontal()
                    .id_salt("piano_88_scroll")
                    .show(ui, |ui| {
                        ui.set_min_height(key_h);
                        let size = Vec2::new(keyboard_w, key_h);
                        let (rect, response) =
                            ui.allocate_exact_size(size, Sense::click_and_drag());
                        let (_, piano) = self.paint(ui, rect, response);
                        if piano.note_on.is_some() {
                            combined.note_on = piano.note_on;
                        }
                        if piano.note_off.is_some() {
                            combined.note_off = piano.note_off;
                        }
                    });
            });
        } else {
            let size = Vec2::new(keyboard_w.min(avail_w), key_h);
            let origin = Pos2::new(
                area.min.x + pad_x + (avail_w - size.x) * 0.5,
                area.min.y + pad_y,
            );
            let rect = Rect::from_min_size(origin, size);
            let (_, piano) = self.paint(ui, rect, area_response.clone());
            combined = piano;
        }

        (area_response, combined)
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
        let white_notes = white_key_notes(self.start_note, self.end_note);
        let white_count = white_notes.len().max(1);
        let key_w = rect.width() / white_count as f32;
        let key_h = rect.height();
        let black_notes = black_key_notes(self.start_note, self.end_note);

        if ui.input(|i| i.pointer.primary_down()) {
            if let Some(pos) = ui.input(|i| i.pointer.interact_pos()) {
                if rect.contains(pos) {
                    if let Some(note) =
                        hit_test(pos, rect, key_w, key_h, &white_notes, &black_notes)
                    {
                        if !self.key_playable(note) {
                            return (response, out);
                        }
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
            let key_rect =
                Rect::from_min_size(Pos2::new(x, rect.min.y), Vec2::new(key_w, key_h));
            let active = self.keys_down.contains(&note);
            let in_scale = self.key_playable(note);
            let (top, bottom, stroke) = key_colors(active, in_scale, &tokens);
            painter.rect_filled(key_rect, 4.0, bottom);
            painter.rect_filled(
                key_rect.shrink2(Vec2::new(0.0, key_rect.height() * 0.12)),
                4.0,
                top,
            );
            painter.rect_stroke(key_rect, 4.0, egui::Stroke::new(1.0_f32, stroke));

            if note % 12 == 0 {
                let octave = (note as i32 / 12) - 1;
                painter.text(
                    key_rect.left_bottom() + Vec2::new(2.0, -2.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("C{octave}"),
                    egui::FontId::monospace(7.0),
                    tokens.text_secondary.gamma_multiply(if in_scale { 1.0 } else { 0.35 }),
                );
            }
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
            let in_scale = self.key_playable(note);
            let fill = if active {
                ACCENT_UI
            } else if in_scale {
                Color32::from_rgb(63, 63, 70)
            } else {
                Color32::from_rgb(63, 63, 70).gamma_multiply(0.35)
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

    fn key_playable(&self, note: u8) -> bool {
        if !self.scale_fold {
            return true;
        }
        note_in_scale(note, self.scale_root, self.scale)
    }
}

fn key_colors(active: bool, in_scale: bool, tokens: &Tokens) -> (Color32, Color32, Color32) {
    if active {
        return (
            ACCENT_UI,
            tokens.accent,
            Color32::from_rgb(82, 82, 91),
        );
    }
    if in_scale {
        (
            Color32::from_rgb(244, 244, 245),
            Color32::from_rgb(212, 212, 216),
            Color32::from_rgb(82, 82, 91),
        )
    } else {
        (
            Color32::from_rgb(244, 244, 245).gamma_multiply(0.35),
            Color32::from_rgb(212, 212, 216).gamma_multiply(0.35),
            Color32::from_rgb(82, 82, 91).gamma_multiply(0.5),
        )
    }
}

fn is_white_semitone(semi: u8) -> bool {
    matches!(semi, 0 | 2 | 4 | 5 | 7 | 9 | 11)
}

fn white_key_count(start: u8, end: u8) -> usize {
    white_key_notes(start, end).len()
}

fn white_key_notes(start: u8, end: u8) -> Vec<u8> {
    let mut notes = Vec::new();
    for note in start..=end {
        if is_white_semitone(note % 12) {
            notes.push(note);
        }
    }
    notes
}

fn black_key_notes(start: u8, end: u8) -> Vec<(u8, usize)> {
    let white_notes = white_key_notes(start, end);
    let mut out = Vec::new();
    for note in start..=end {
        if !is_white_semitone(note % 12) {
            let slot = white_notes
                .iter()
                .position(|&w| w >= note)
                .unwrap_or(white_notes.len())
                .saturating_sub(1);
            out.push((note, slot));
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// MIDI black-key pitches in one octave (C# … A#).
    const BLACK_KEYS: [u8; 5] = [49, 51, 54, 56, 58]; // C#3 D#3 F#3 G#3 A#3
    const WHITE_C: u8 = 48; // C3
    const WHITE_D: u8 = 50; // D3

    #[test]
    fn scale_fold_follows_layout_not_shell_mode() {
        assert!(
            !piano_scale_fold_enabled(PerformanceLayout::Piano),
            "Piano layout (Design + Compose default) must keep chromatic black keys"
        );
        assert!(piano_scale_fold_enabled(PerformanceLayout::Scale));
        assert!(!piano_scale_fold_enabled(PerformanceLayout::Chords));
    }

    #[test]
    fn piano_layout_playable_matches_design_and_compose() {
        let keys = HashSet::new();
        // Shared path: both modes call with_scale_fold(..., piano_scale_fold_enabled(Piano)).
        let fold = piano_scale_fold_enabled(PerformanceLayout::Piano);
        let piano = PianoKeyboard::new(&keys).with_scale_fold(0, Scale::Major, fold);

        for &n in &BLACK_KEYS {
            assert!(
                piano.key_playable(n),
                "black key {n} must be playable on shared Piano layout (Compose ≡ Design)"
            );
        }
        assert!(piano.key_playable(WHITE_C));
        assert!(piano.key_playable(WHITE_D));
    }

    #[test]
    fn scale_layout_blocks_out_of_scale_black_keys() {
        let keys = HashSet::new();
        let fold = piano_scale_fold_enabled(PerformanceLayout::Scale);
        let piano = PianoKeyboard::new(&keys).with_scale_fold(0, Scale::Major, fold);

        for &n in &BLACK_KEYS {
            assert!(
                !piano.key_playable(n),
                "C major + Scale fold should block black key {n}"
            );
        }
        assert!(piano.key_playable(WHITE_C));
        assert!(piano.key_playable(60)); // C4
    }

    #[test]
    fn hit_test_returns_black_keys_same_as_white() {
        let start = 48u8;
        let end = 59u8;
        let white = white_key_notes(start, end);
        let black = black_key_notes(start, end);
        let rect = Rect::from_min_size(Pos2::new(0.0, 0.0), Vec2::new(280.0, 100.0));
        let key_w = rect.width() / white.len() as f32;
        let key_h = rect.height();
        let black_h = key_h * PIANO_BLACK_HEIGHT_RATIO;
        let black_w = key_w * PIANO_BLACK_WIDTH_RATIO;

        // White key hit (first column = C).
        let white_hit = hit_test(
            Pos2::new(key_w * 0.5, rect.min.y + key_h * 0.8),
            rect,
            key_w,
            key_h,
            &white,
            &black,
        );
        assert_eq!(white_hit, Some(WHITE_C));

        // Each black key center must resolve to that black pitch (same path as white).
        for &(note, slot) in &black {
            let slot_min_x = rect.min.x + slot as f32 * key_w;
            let slot_max_x = slot_min_x + key_w;
            let key_right = slot_max_x + key_w * 0.29;
            let key_left = key_right - black_w;
            let cx = (key_left + key_right) * 0.5;
            let cy = rect.min.y + black_h * 0.5;
            let hit = hit_test(Pos2::new(cx, cy), rect, key_w, key_h, &white, &black);
            assert_eq!(hit, Some(note), "black key hit should return MIDI {note}");
        }
    }
}
