//! Piano roll editor — grid, note draw/move/resize, velocity lane, selection, undo.

use egui::{pos2, Color32, Pos2, Rect, Sense, Ui, Vec2};
use reelsynth::MidiNote;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::layout::GRID_UNIT;
use crate::region::region;
use crate::widgets::button_toggle;

use super::command_history::ComposeCommand;
use super::ComposeUi;

const PITCH_TOP: u8 = 108;
const PITCH_BOTTOM: u8 = 21;
const ROW_H: f32 = 14.0;
const VELOCITY_LANE_H: f32 = 48.0;
const KEY_LABEL_W: f32 = 28.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PianoRollTool {
    Select,
    Pencil,
    Eraser,
}

pub struct PianoRollActions {
    pub sequence_changed: bool,
    pub focus_changed: bool,
}

impl Default for PianoRollActions {
    fn default() -> Self {
        Self {
            sequence_changed: false,
            focus_changed: false,
        }
    }
}

pub fn draw_piano_roll(ui: &mut Ui, rect: Rect, compose: &mut ComposeUi) -> PianoRollActions {
    let tokens = Tokens::default();
    let mut actions = PianoRollActions::default();

    region(ui, rect, |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(GRID_UNIT * 0.5, GRID_UNIT * 0.5))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = GRID_UNIT * 0.5;
                    ui.label(
                        egui::RichText::new("Piano roll")
                            .size(10.0)
                            .color(tokens.text_muted),
                    );
                    for (tool, label) in [
                        (PianoRollTool::Select, "Select"),
                        (PianoRollTool::Pencil, "Pencil"),
                        (PianoRollTool::Eraser, "Eraser"),
                    ] {
                        if button_toggle(ui, label, compose.piano_roll_tool == tool).clicked() {
                            compose.piano_roll_tool = tool;
                        }
                    }
                    if button_toggle(ui, "Undo", compose.history.can_undo()).clicked() {
                        if let Some(cmd) = compose.history.take_undo() {
                            apply_undo(compose, cmd);
                            actions.sequence_changed = true;
                        }
                    }
                    if button_toggle(ui, "Redo", compose.history.can_redo()).clicked() {
                        if let Some(cmd) = compose.history.take_redo() {
                            apply_redo(compose, cmd);
                            actions.sequence_changed = true;
                        }
                    }
                });

                let clip_info = selected_clip_mut(compose);
                if clip_info.is_none() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new(
                                "Select or double-click a clip in the arrangement",
                            )
                            .size(11.0)
                            .color(tokens.text_secondary),
                        );
                    });
                    return;
                }

                let inner_h = ui.available_height() - VELOCITY_LANE_H - GRID_UNIT;
                let grid_rect = ui.available_rect_before_wrap();
                let grid_rect = Rect::from_min_max(
                    grid_rect.min,
                    pos2(grid_rect.max.x, grid_rect.min.y + inner_h.max(80.0)),
                );

                let (note_actions, focused) =
                    paint_note_grid(ui, grid_rect, compose, &tokens);
                if note_actions {
                    actions.sequence_changed = true;
                }
                if focused != compose.piano_roll_focused {
                    compose.piano_roll_focused = focused;
                    actions.focus_changed = true;
                }

                let vel_rect = Rect::from_min_max(
                    pos2(grid_rect.min.x, grid_rect.max.y + GRID_UNIT * 0.5),
                    pos2(grid_rect.max.x, rect.max.y - GRID_UNIT),
                );
                if paint_velocity_lane(ui, vel_rect, compose, &tokens) {
                    actions.sequence_changed = true;
                }
            });
    });

    if ui.input(|i| i.key_pressed(egui::Key::Delete)) && compose.piano_roll_focused {
        if delete_selected_notes(compose) {
            actions.sequence_changed = true;
        }
    }

    actions
}

fn selected_clip_mut(compose: &mut ComposeUi) -> Option<(usize, usize)> {
    let ti = compose.selected_track;
    let ci = compose.selected_clip?;
    if ti < compose.project.tracks.len() && ci < compose.project.tracks[ti].clips.len() {
        Some((ti, ci))
    } else {
        None
    }
}

fn paint_note_grid(
    ui: &mut Ui,
    rect: Rect,
    compose: &mut ComposeUi,
    tokens: &Tokens,
) -> (bool, bool) {
    let (ti, ci) = match selected_clip_mut(compose) {
        Some(v) => v,
        None => return (false, false),
    };

    let clip_len = compose.project.tracks[ti].clips[ci].length_beats;
    let beat_w = (rect.width() - KEY_LABEL_W) / clip_len.max(1.0);
    let grid_left = rect.min.x + KEY_LABEL_W;
    let grid_rect = Rect::from_min_max(
        Pos2::new(grid_left, rect.min.y),
        rect.max,
    );

    let (response, painter) =
        ui.allocate_painter(rect.size(), Sense::click_and_drag());
    let rect = response.rect;
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 0.0, tokens.bg);

    for p in PITCH_BOTTOM..=PITCH_TOP {
        let row = (PITCH_TOP - p) as f32;
        let y = rect.min.y + row * ROW_H;
        if y + ROW_H > rect.max.y {
            break;
        }
        let is_black = is_black_key(p);
        if is_black {
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(grid_left, y),
                    Pos2::new(rect.max.x, y + ROW_H),
                ),
                0.0,
                tokens.bg_muted.gamma_multiply(0.85),
            );
        }
        if p % 12 == 0 {
            painter.line_segment(
                [
                    Pos2::new(grid_left, y),
                    Pos2::new(rect.max.x, y),
                ],
                egui::Stroke::new(1.0, tokens.border_strong),
            );
            painter.text(
                Pos2::new(rect.min.x + 2.0, y + 1.0),
                egui::Align2::LEFT_TOP,
                pitch_label(p),
                egui::FontId::monospace(8.0),
                tokens.text_secondary,
            );
        }
    }

    let step = compose.snap_division.beats_per_step();
    let mut beat = 0.0;
    while beat <= clip_len {
        let x = grid_left + beat * beat_w;
        painter.line_segment(
            [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
            egui::Stroke::new(
                if beat % 1.0 < 0.001 { 1.0 } else { 0.5 },
                tokens.border,
            ),
        );
        beat += step;
    }

    let playhead_x = grid_left + compose.transport.playhead_beats * beat_w;
    painter.line_segment(
        [
            Pos2::new(playhead_x, rect.min.y),
            Pos2::new(playhead_x, rect.max.y),
        ],
        egui::Stroke::new(1.5, ACCENT_UI),
    );

    let notes: Vec<MidiNote> = compose.project.tracks[ti].clips[ci].notes.clone();
    for (ni, note) in notes.iter().enumerate() {
        let selected = compose.selected_notes.contains(&ni);
        paint_note(
            &painter,
            grid_rect,
            note,
            beat_w,
            selected,
            tokens,
        );
    }

    let mut changed = false;
    let focused = response.hovered() || response.has_focus();

    if response.clicked() || response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            if grid_rect.contains(pos) {
                let beat = ((pos.x - grid_left) / beat_w).clamp(0.0, clip_len);
                let pitch = PITCH_TOP
                    - ((pos.y - rect.min.y) / ROW_H).floor() as u8;
                let pitch = pitch.clamp(PITCH_BOTTOM, PITCH_TOP);
                let snapped = compose.snap_beats(beat);

                match compose.piano_roll_tool {
                    PianoRollTool::Pencil if response.clicked() => {
                        let note = MidiNote {
                            pitch,
                            start_beats: snapped,
                            duration_beats: step,
                            velocity: 0.78,
                        };
                        compose.history.push(ComposeCommand::AddNote {
                            track: ti,
                            clip: ci,
                            note: note.clone(),
                        });
                        compose.project.tracks[ti].clips[ci].notes.push(note);
                        changed = true;
                    }
                    PianoRollTool::Eraser if response.clicked() => {
                        if let Some(ni) = hit_note(&compose.project.tracks[ti].clips[ci].notes, beat, pitch) {
                            let removed = compose.project.tracks[ti].clips[ci].notes.remove(ni);
                            compose.history.push(ComposeCommand::DeleteNotes {
                                track: ti,
                                clip: ci,
                                notes: vec![(ni, removed)],
                            });
                            compose.selected_notes.clear();
                            changed = true;
                        }
                    }
                    PianoRollTool::Select => {
                        if let Some(ni) = hit_note(&compose.project.tracks[ti].clips[ci].notes, beat, pitch) {
                            if ui.input(|i| i.modifiers.shift) {
                                if compose.selected_notes.contains(&ni) {
                                    compose.selected_notes.remove(&ni);
                                } else {
                                    compose.selected_notes.insert(ni);
                                }
                            } else {
                                compose.selected_notes.clear();
                                compose.selected_notes.insert(ni);
                            }
                            compose.drag_state = Some(DragState {
                                start_pos: pos,
                                original: compose
                                    .selected_notes
                                    .iter()
                                    .filter_map(|&idx| {
                                        compose.project.tracks[ti].clips[ci]
                                            .notes
                                            .get(idx)
                                            .map(|n| (idx, n.start_beats, n.pitch))
                                    })
                                    .collect(),
                            });
                        } else if response.clicked() {
                            compose.selected_notes.clear();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if response.drag_stopped() {
        if let Some(drag) = compose.drag_state.take() {
            if compose.piano_roll_tool == PianoRollTool::Select && !compose.selected_notes.is_empty() {
                if let Some(pos) = response.interact_pointer_pos() {
                    let delta_beats = (pos.x - drag.start_pos.x) / beat_w;
                    let delta_pitch =
                        -((pos.y - drag.start_pos.y) / ROW_H).round() as i8;
                    if delta_beats.abs() > 0.001 || delta_pitch != 0 {
                        let entries: Vec<(usize, f32, f32, u8)> = drag
                            .original
                            .iter()
                            .map(|&(idx, start, pitch)| (idx, start, start, pitch))
                            .collect();
                        apply_move_notes(compose, ti, ci, &entries, delta_beats, delta_pitch);
                        compose.history.push(ComposeCommand::MoveNotes {
                            track: ti,
                            clip: ci,
                            entries,
                            delta_beats,
                            delta_pitch,
                        });
                        changed = true;
                    }
                }
            }
        }
    }

    (changed, focused)
}

fn paint_velocity_lane(
    ui: &mut Ui,
    rect: Rect,
    compose: &mut ComposeUi,
    tokens: &Tokens,
) -> bool {
    let (ti, ci) = match selected_clip_mut(compose) {
        Some(v) => v,
        None => return false,
    };

    let (response, painter) = ui.allocate_painter(rect.size(), Sense::hover());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, tokens.surface2);
    painter.text(
        rect.left_top() + Vec2::new(4.0, 2.0),
        egui::Align2::LEFT_TOP,
        "Velocity",
        egui::FontId::proportional(9.0),
        tokens.text_muted,
    );

    let clip_len = compose.project.tracks[ti].clips[ci].length_beats;
    let beat_w = rect.width() / clip_len.max(1.0);
    let notes = &compose.project.tracks[ti].clips[ci].notes;

    for (ni, note) in notes.iter().enumerate() {
        let x = rect.min.x + note.start_beats * beat_w;
        let w = note.duration_beats * beat_w;
        let h = note.velocity * (rect.height() - 14.0);
        let bar = Rect::from_min_max(
            Pos2::new(x, rect.max.y - h),
            Pos2::new(x + w.max(2.0), rect.max.y),
        );
        let color = if compose.selected_notes.contains(&ni) {
            ACCENT_UI
        } else {
            tokens.accent.gamma_multiply(0.75)
        };
        painter.rect_filled(bar, 2.0, color);
    }

    false
}

fn paint_note(
    painter: &egui::Painter,
    grid: Rect,
    note: &MidiNote,
    beat_w: f32,
    selected: bool,
    tokens: &Tokens,
) {
    let row = (PITCH_TOP - note.pitch) as f32;
    let x = grid.min.x + note.start_beats * beat_w;
    let w = note.duration_beats * beat_w;
    let note_rect = Rect::from_min_max(
        Pos2::new(x + 1.0, grid.min.y + row * ROW_H + 1.0),
        Pos2::new(x + w - 1.0, grid.min.y + (row + 1.0) * ROW_H - 1.0),
    );
    let fill = if selected {
        ACCENT_UI
    } else {
        Color32::from_rgb(0x3d, 0x8a, 0xa8)
    };
    painter.rect_filled(note_rect, 3.0, fill);
    painter.rect_stroke(
        note_rect,
        3.0,
        egui::Stroke::new(
            if selected { 1.5 } else { 1.0 },
            if selected {
                tokens.text
            } else {
                tokens.border_strong
            },
        ),
    );
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DragState {
    start_pos: Pos2,
    original: Vec<(usize, f32, u8)>,
}

fn hit_note(notes: &[MidiNote], beat: f32, pitch: u8) -> Option<usize> {
    notes.iter().enumerate().find_map(|(i, n)| {
        if n.pitch == pitch && beat >= n.start_beats && beat < n.start_beats + n.duration_beats {
            Some(i)
        } else {
            None
        }
    })
}

fn is_black_key(pitch: u8) -> bool {
    matches!(pitch % 12, 1 | 3 | 6 | 8 | 10)
}

fn pitch_label(pitch: u8) -> String {
    const NAMES: [&str; 12] = [
        "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B",
    ];
    let octave = (pitch as i32 / 12) - 1;
    format!("{}{}", NAMES[(pitch % 12) as usize], octave)
}

fn delete_selected_notes(compose: &mut ComposeUi) -> bool {
    let (ti, ci) = match selected_clip_mut(compose) {
        Some(v) => v,
        None => return false,
    };
    if compose.selected_notes.is_empty() {
        return false;
    }
    let mut indices: Vec<usize> = compose.selected_notes.iter().copied().collect();
    indices.sort_unstable_by(|a, b| b.cmp(a));
    let mut removed = Vec::new();
    for idx in indices {
        if idx < compose.project.tracks[ti].clips[ci].notes.len() {
            let note = compose.project.tracks[ti].clips[ci].notes.remove(idx);
            removed.push((idx, note));
        }
    }
    compose.selected_notes.clear();
    if !removed.is_empty() {
        compose.history.push(ComposeCommand::DeleteNotes {
            track: ti,
            clip: ci,
            notes: removed,
        });
        true
    } else {
        false
    }
}

fn apply_move_notes(
    compose: &mut ComposeUi,
    ti: usize,
    ci: usize,
    entries: &[(usize, f32, f32, u8)],
    delta_beats: f32,
    delta_pitch: i8,
) {
    let snap = compose.snap_division;
    for &(idx, _start, _, _pitch) in entries {
        if let Some(note) = compose.project.tracks[ti].clips[ci].notes.get_mut(idx) {
            note.start_beats = (note.start_beats + delta_beats).max(0.0);
            let step = snap.beats_per_step();
            note.start_beats = (note.start_beats / step).round() * step;
            note.pitch = (note.pitch as i16 + delta_pitch as i16)
                .clamp(PITCH_BOTTOM as i16, PITCH_TOP as i16)
                as u8;
        }
    }
}

fn apply_undo(compose: &mut ComposeUi, cmd: ComposeCommand) {
    match cmd {
        ComposeCommand::AddNote { track, clip, note } => {
            if let Some(ni) = compose.project.tracks[track].clips[clip]
                .notes
                .iter()
                .position(|n| n == &note)
            {
                compose.project.tracks[track].clips[clip].notes.remove(ni);
            }
        }
        ComposeCommand::DeleteNotes { track, clip, notes } => {
            let clip_notes = &mut compose.project.tracks[track].clips[clip].notes;
            for (idx, note) in notes {
                if idx <= clip_notes.len() {
                    clip_notes.insert(idx, note);
                }
            }
        }
        ComposeCommand::MoveNotes {
            track,
            clip,
            entries,
            delta_beats,
            delta_pitch,
        } => {
            apply_move_notes(compose, track, clip, &entries, -delta_beats, -delta_pitch);
        }
        ComposeCommand::ResizeNotes { track, clip, entries } => {
            for (idx, old_dur) in entries {
                if let Some(note) = compose.project.tracks[track].clips[clip].notes.get_mut(idx) {
                    note.duration_beats = old_dur;
                }
            }
        }
    }
}

fn apply_redo(compose: &mut ComposeUi, cmd: ComposeCommand) {
    match cmd {
        ComposeCommand::AddNote { track, clip, note } => {
            compose.project.tracks[track].clips[clip].notes.push(note);
        }
        ComposeCommand::DeleteNotes { track, clip, notes } => {
            let mut indices: Vec<usize> = notes.iter().map(|(i, _)| *i).collect();
            indices.sort_unstable_by(|a, b| b.cmp(a));
            for idx in indices {
                if idx < compose.project.tracks[track].clips[clip].notes.len() {
                    compose.project.tracks[track].clips[clip].notes.remove(idx);
                }
            }
        }
        ComposeCommand::MoveNotes {
            track,
            clip,
            entries,
            delta_beats,
            delta_pitch,
        } => {
            apply_move_notes(compose, track, clip, &entries, delta_beats, delta_pitch);
        }
        ComposeCommand::ResizeNotes { track, clip, entries } => {
            for (idx, new_dur) in entries {
                if let Some(note) = compose.project.tracks[track].clips[clip].notes.get_mut(idx) {
                    note.duration_beats = new_dur;
                }
            }
        }
    }
}
