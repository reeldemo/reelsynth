//! Piano roll editor — grid, note draw/move/resize, velocity lane, selection, undo.

use std::collections::HashSet;

use egui::{pos2, Color32, Pos2, Rect, Sense, Ui, Vec2};
use reelsynth::{ArpEngine, AutomationLane, AutomationPoint, MidiNote};
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::audit_registry::{record_region, record_used, AuditId};
use crate::layout::GRID_UNIT;
use crate::mod_matrix::{automation_target_to_engine, AUTOMATION_TARGET_LABELS};
use crate::region::region;
use crate::widgets::{button_toggle, reel_combo, select_value_text, styled_menu_body, menu_selectable};

use super::command_history::ComposeCommand;
use super::ComposeUi;

const PITCH_TOP: u8 = 108;
const PITCH_BOTTOM: u8 = 21;
const ROW_H: f32 = 14.0;
const VELOCITY_LANE_H: f32 = 40.0;
const AUTOMATION_LANE_H: f32 = 44.0;
/// Interactive piano key column width (Layout A).
const KEY_COLUMN_W: f32 = 52.0;
const RESIZE_HANDLE_W: f32 = 6.0;

/// QWERTY play-row glyphs for MIDI 48–59 (C3–B3).
fn qwerty_glyph(pitch: u8) -> Option<&'static str> {
    match pitch {
        48 => Some("Z"),
        49 => Some("S"),
        50 => Some("X"),
        51 => Some("D"),
        52 => Some("C"),
        53 => Some("V"),
        54 => Some("G"),
        55 => Some("B"),
        56 => Some("H"),
        57 => Some("N"),
        58 => Some("J"),
        59 => Some("M"),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HitRegion {
    Empty,
    Body(usize),
    LeftEdge(usize),
    RightEdge(usize),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum NoteDrag {
    Move {
        originals: Vec<(usize, f32, u8)>,
    },
    ResizeLeft {
        note_idx: usize,
        orig_start: f32,
        orig_dur: f32,
    },
    ResizeRight {
        note_idx: usize,
        orig_start: f32,
        orig_dur: f32,
    },
    Pencil {
        pitch: u8,
        start_beats: f32,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PianoRollTool {
    Select,
    Pencil,
    Eraser,
}

pub struct PianoRollActions {
    pub sequence_changed: bool,
    pub focus_changed: bool,
    pub audition_note: Option<(u8, f32)>,
    pub audition_note_off: Option<u8>,
    pub open_arp_dialog: bool,
}

impl Default for PianoRollActions {
    fn default() -> Self {
        Self {
            sequence_changed: false,
            focus_changed: false,
            audition_note: None,
            audition_note_off: None,
            open_arp_dialog: false,
        }
    }
}

pub fn draw_piano_roll(
    ui: &mut Ui,
    rect: Rect,
    compose: &mut ComposeUi,
    keys_down: &HashSet<u8>,
) -> PianoRollActions {
    let tokens = Tokens::default();
    let mut actions = PianoRollActions::default();

    compose.ensure_editable_clip();

    // Flush deferred click-audition note-off from the previous frame.
    if let Some(note) = compose.pending_audition_off.take() {
        actions.audition_note_off = Some(note);
    }

    region(ui, rect, |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(GRID_UNIT * 0.5, GRID_UNIT * 0.5))
            .show(ui, |ui| {
                let toolbar_before = ui.min_rect();
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
                    if ui.button("Generate Arp").clicked() {
                        actions.open_arp_dialog = true;
                    }
                    ui.label(
                        egui::RichText::new(tool_hint(compose.piano_roll_tool))
                            .size(10.0)
                            .color(tokens.text_secondary),
                    );
                });
                record_region(
                    ui.ctx(),
                    AuditId::ComposeRollToolbar,
                    toolbar_before,
                    ui.min_rect(),
                );

                let clip_info = selected_clip_mut(compose);
                if clip_info.is_none() {
                    ui.centered_and_justified(|ui| {
                        ui.label(
                            egui::RichText::new(
                                "No clip on this track — drawing will create one automatically",
                            )
                            .size(11.0)
                            .color(tokens.text_secondary),
                        );
                    });
                    return;
                }

                let inner_h = ui.available_height() - VELOCITY_LANE_H - AUTOMATION_LANE_H - GRID_UNIT * 2.0;
                let grid_rect = ui.available_rect_before_wrap();
                let grid_rect = Rect::from_min_max(
                    grid_rect.min,
                    pos2(grid_rect.max.x, grid_rect.min.y + inner_h.max(80.0)),
                );

                let (note_actions, focused, audition, audition_off) =
                    paint_note_grid(ui, grid_rect, compose, &tokens, keys_down);
                if note_actions {
                    actions.sequence_changed = true;
                }
                if let Some((note, vel)) = audition {
                    actions.audition_note = Some((note, vel));
                }
                if let Some(note) = audition_off {
                    actions.audition_note_off = Some(note);
                }
                if focused != compose.piano_roll_focused {
                    compose.piano_roll_focused = focused;
                    actions.focus_changed = true;
                }

                let vel_rect = Rect::from_min_max(
                    pos2(grid_rect.min.x, grid_rect.max.y + GRID_UNIT * 0.5),
                    pos2(
                        grid_rect.max.x,
                        grid_rect.max.y + GRID_UNIT * 0.5 + VELOCITY_LANE_H,
                    ),
                );
                if paint_velocity_lane(ui, vel_rect, compose, &tokens) {
                    actions.sequence_changed = true;
                }
                record_region(
                    ui.ctx(),
                    AuditId::ComposeRollVelocity,
                    vel_rect,
                    vel_rect,
                );

                let auto_rect = Rect::from_min_max(
                    pos2(vel_rect.min.x, vel_rect.max.y + GRID_UNIT * 0.5),
                    pos2(vel_rect.max.x, rect.max.y - GRID_UNIT),
                );
                if paint_automation_lane(ui, auto_rect, compose, &tokens) {
                    actions.sequence_changed = true;
                }
                record_region(
                    ui.ctx(),
                    AuditId::ComposeRollAutomation,
                    auto_rect,
                    auto_rect,
                );
                record_region(
                    ui.ctx(),
                    AuditId::ComposeRollGrid,
                    grid_rect,
                    grid_rect,
                );
                let keys_rect = Rect::from_min_max(
                    grid_rect.min,
                    Pos2::new(grid_rect.min.x + KEY_COLUMN_W, grid_rect.max.y),
                );
                record_region(
                    ui.ctx(),
                    AuditId::ComposeRollKeys,
                    keys_rect,
                    keys_rect,
                );
                record_used(ui.ctx(), AuditId::ComposeRollKeys, keys_rect);
            });
    });

    if ui.input(|i| i.key_pressed(egui::Key::Delete)) && compose.piano_roll_focused {
        if delete_selected_notes(compose) {
            actions.sequence_changed = true;
        }
    }

    actions
}

fn tool_hint(tool: PianoRollTool) -> &'static str {
    match tool {
        PianoRollTool::Pencil => "Pencil: click or drag to draw notes",
        PianoRollTool::Select => "Select: drag notes · edges resize · Delete removes",
        PianoRollTool::Eraser => "Eraser: click notes to delete",
    }
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
    keys_down: &HashSet<u8>,
) -> (bool, bool, Option<(u8, f32)>, Option<u8>) {
    let (ti, ci) = match selected_clip_mut(compose) {
        Some(v) => v,
        None => return (false, false, None, None),
    };

    let clip_start = compose.project.tracks[ti].clips[ci].start_beats;
    let clip_len = compose.project.tracks[ti].clips[ci].length_beats;
    let _pitch_top = visible_pitch_top(compose);
    let visible_beats = (clip_len / compose.beat_zoom.max(0.25)).max(1.0);
    let max_scroll = (clip_len - visible_beats).max(0.0);
    compose.beat_scroll = compose.beat_scroll.clamp(0.0, max_scroll);
    let _beat_scroll = compose.beat_scroll;
    let beat_w = (rect.width() - KEY_COLUMN_W) / visible_beats;
    let grid_left = rect.min.x + KEY_COLUMN_W;
    let grid_rect = Rect::from_min_max(
        Pos2::new(grid_left, rect.min.y),
        rect.max,
    );

    let (response, _painter) =
        ui.allocate_painter(rect.size(), Sense::click_and_drag());
    let rect = response.rect;
    let painter = ui.painter_at(rect);

    // Scroll / zoom (pitch vertical; beat horizontal or Ctrl+wheel zoom).
    if response.hovered() {
        let (scroll, zoom_mods) = ui.input(|i| {
            (
                i.smooth_scroll_delta,
                i.modifiers.ctrl || i.modifiers.command,
            )
        });
        if zoom_mods && scroll.y.abs() > 0.0 {
            let factor = if scroll.y > 0.0 { 1.1 } else { 1.0 / 1.1 };
            compose.beat_zoom = (compose.beat_zoom * factor).clamp(0.5, 8.0);
        } else {
            if scroll.y.abs() > 0.0 {
                compose.pitch_scroll =
                    (compose.pitch_scroll - scroll.y / ROW_H).clamp(0.0, (PITCH_TOP - PITCH_BOTTOM) as f32);
            }
            if scroll.x.abs() > 0.0 {
                compose.beat_scroll =
                    (compose.beat_scroll - scroll.x / beat_w).clamp(0.0, max_scroll);
            }
        }
    }

    let pitch_top = visible_pitch_top(compose);
    let beat_scroll = compose.beat_scroll;

    painter.rect_filled(rect, 0.0, tokens.bg);

    // Visible pitch rows + grid shading.
    let rows_visible = ((rect.height() / ROW_H).ceil() as u8).saturating_add(1);
    for row in 0..rows_visible {
        let p = pitch_top.saturating_sub(row);
        if p < PITCH_BOTTOM {
            break;
        }
        let y = rect.min.y + row as f32 * ROW_H;
        if is_black_key(p) {
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
        }
    }

    // Interactive key column (after grid so keys sit on top for hit-testing paint).
    let mut audition_note = None;
    let mut audition_note_off = None;
    {
        let keys_rect = Rect::from_min_max(
            rect.min,
            Pos2::new(grid_left, rect.max.y),
        );
        let key_resp = ui.interact(keys_rect, ui.id().with("roll_keys"), Sense::click_and_drag());
        paint_key_column(
            &painter,
            keys_rect,
            pitch_top,
            keys_down,
            compose.key_pointer_held,
            tokens,
        );

        if key_resp.drag_started() || (key_resp.is_pointer_button_down_on() && key_resp.clicked()) {
            if let Some(pos) = key_resp.interact_pointer_pos() {
                let pitch = pitch_at_y(pos.y, rect.min.y, pitch_top);
                if compose.key_pointer_held != Some(pitch) {
                    if let Some(prev) = compose.key_pointer_held.take() {
                        audition_note_off = Some(prev);
                    }
                    compose.key_pointer_held = Some(pitch);
                    audition_note = Some((pitch, 0.9));
                }
            }
        }
        if key_resp.dragged() {
            if let Some(pos) = key_resp.interact_pointer_pos() {
                let pitch = pitch_at_y(pos.y, rect.min.y, pitch_top);
                if compose.key_pointer_held != Some(pitch) {
                    if let Some(prev) = compose.key_pointer_held.take() {
                        audition_note_off = Some(prev);
                    }
                    compose.key_pointer_held = Some(pitch);
                    audition_note = Some((pitch, 0.9));
                }
            }
        }
        if key_resp.drag_stopped() || (!key_resp.is_pointer_button_down_on() && compose.key_pointer_held.is_some() && !key_resp.hovered()) {
            if let Some(prev) = compose.key_pointer_held.take() {
                audition_note_off = Some(prev);
            }
        }
        // Release when pointer up anywhere after holding a key.
        if compose.key_pointer_held.is_some()
            && ui.input(|i| i.pointer.any_released())
        {
            if let Some(prev) = compose.key_pointer_held.take() {
                audition_note_off = Some(prev);
            }
        }
    }

    let step = compose.snap_division.beats_per_step();
    let mut beat = beat_scroll;
    let beat_end = beat_scroll + visible_beats;
    while beat <= beat_end + 0.001 {
        let x = grid_left + (beat - beat_scroll) * beat_w;
        painter.line_segment(
            [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
            egui::Stroke::new(
                if (beat - beat.floor()).abs() < 0.001 {
                    1.0
                } else {
                    0.5
                },
                tokens.border,
            ),
        );
        beat += step;
    }

    // Clip-relative playhead.
    let playhead_local = compose.transport.playhead_beats - clip_start;
    let playhead_x = grid_left + (playhead_local - beat_scroll) * beat_w;
    if playhead_x >= grid_left && playhead_x <= rect.max.x {
        painter.line_segment(
            [
                Pos2::new(playhead_x, rect.min.y),
                Pos2::new(playhead_x, rect.max.y),
            ],
            egui::Stroke::new(1.5, ACCENT_UI),
        );
    }

    let notes: Vec<MidiNote> = compose.project.tracks[ti].clips[ci].notes.clone();
    for (ni, note) in notes.iter().enumerate() {
        let selected = compose.selected_notes.contains(&ni);
        paint_note(
            &painter,
            grid_rect,
            note,
            beat_w,
            beat_scroll,
            pitch_top,
            selected,
            tokens,
            false,
        );
    }

    // Live recording overlay (engine recorder snapshot).
    if compose.transport.recording {
        for note in &compose.live_record_overlay {
            paint_note(
                &painter,
                grid_rect,
                note,
                beat_w,
                beat_scroll,
                pitch_top,
                false,
                tokens,
                true,
            );
        }
    }

    // Pencil drag preview.
    if let Some(drag) = &compose.drag_state {
        if let NoteDrag::Pencil { pitch, start_beats } = &drag.drag {
            if let Some(pos) = response.interact_pointer_pos() {
                let end_beat = beat_at_pos(pos, grid_left, beat_w, beat_scroll).clamp(0.0, clip_len);
                let snapped_end = compose.snap_beats(end_beat);
                let snapped_start = compose.snap_beats(*start_beats);
                let duration = (snapped_end - snapped_start).max(step);
                if duration > 0.0 {
                    let preview = MidiNote {
                        pitch: *pitch,
                        start_beats: snapped_start.min(snapped_end),
                        duration_beats: duration,
                        velocity: 0.78,
                    };
                    paint_note(
                        &painter,
                        grid_rect,
                        &preview,
                        beat_w,
                        beat_scroll,
                        pitch_top,
                        false,
                        tokens,
                        true,
                    );
                }
            }
        }
    }

    let mut changed = false;
    let focused = response.hovered() || response.has_focus();
    let shift = ui.input(|i| i.modifiers.shift);
    let recording = compose.transport.recording;

    if response.drag_started() {
        if let Some(pos) = response.interact_pointer_pos() {
            if grid_rect.contains(pos) {
                let beat = beat_at_pos(pos, grid_left, beat_w, beat_scroll).clamp(0.0, clip_len);
                let pitch = pitch_at_y(pos.y, rect.min.y, pitch_top);
                let hit = hit_test_at_pos(
                    &compose.project.tracks[ti].clips[ci].notes,
                    pos,
                    grid_rect,
                    beat_w,
                    beat_scroll,
                    pitch_top,
                );

                match compose.piano_roll_tool {
                    PianoRollTool::Pencil => {
                        let snapped = compose.snap_beats(beat);
                        compose.drag_state = Some(DragState {
                            drag: NoteDrag::Pencil {
                                pitch,
                                start_beats: snapped,
                            },
                            start_pos: pos,
                            last_pos: Some(pos),
                        });
                        audition_note = Some((pitch, 0.85));
                    }
                    PianoRollTool::Select => match hit {
                        HitRegion::LeftEdge(ni) => {
                            let note = &compose.project.tracks[ti].clips[ci].notes[ni];
                            compose.selected_notes.clear();
                            compose.selected_notes.insert(ni);
                            compose.drag_state = Some(DragState {
                                drag: NoteDrag::ResizeLeft {
                                    note_idx: ni,
                                    orig_start: note.start_beats,
                                    orig_dur: note.duration_beats,
                                },
                                start_pos: pos,
                                last_pos: Some(pos),
                            });
                        }
                        HitRegion::RightEdge(ni) => {
                            let note = &compose.project.tracks[ti].clips[ci].notes[ni];
                            compose.selected_notes.clear();
                            compose.selected_notes.insert(ni);
                            compose.drag_state = Some(DragState {
                                drag: NoteDrag::ResizeRight {
                                    note_idx: ni,
                                    orig_start: note.start_beats,
                                    orig_dur: note.duration_beats,
                                },
                                start_pos: pos,
                                last_pos: Some(pos),
                            });
                        }
                        HitRegion::Body(ni) => {
                            if shift {
                                let dup_indices =
                                    duplicate_selected_notes(compose, ti, ci, &[ni]);
                                if !dup_indices.is_empty() {
                                    compose.selected_notes = dup_indices.iter().copied().collect();
                                    let originals: Vec<(usize, f32, u8)> = dup_indices
                                        .iter()
                                        .filter_map(|&idx| {
                                            compose.project.tracks[ti].clips[ci]
                                                .notes
                                                .get(idx)
                                                .map(|n| (idx, n.start_beats, n.pitch))
                                        })
                                        .collect();
                                    compose.drag_state = Some(DragState {
                                        drag: NoteDrag::Move { originals },
                                        start_pos: pos,
                                        last_pos: Some(pos),
                                    });
                                    changed = true;
                                }
                            } else {
                                if !compose.selected_notes.contains(&ni) {
                                    compose.selected_notes.clear();
                                    compose.selected_notes.insert(ni);
                                }
                                let originals: Vec<(usize, f32, u8)> = compose
                                    .selected_notes
                                    .iter()
                                    .filter_map(|&idx| {
                                        compose.project.tracks[ti].clips[ci]
                                            .notes
                                            .get(idx)
                                            .map(|n| (idx, n.start_beats, n.pitch))
                                    })
                                    .collect();
                                compose.drag_state = Some(DragState {
                                    drag: NoteDrag::Move { originals },
                                    start_pos: pos,
                                    last_pos: Some(pos),
                                });
                            }
                        }
                        HitRegion::Empty => {
                            compose.selected_notes.clear();
                        }
                    }
                    PianoRollTool::Eraser => {}
                }
            }
        }
    }

    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if grid_rect.contains(pos) {
                let beat = beat_at_pos(pos, grid_left, beat_w, beat_scroll).clamp(0.0, clip_len);
                let pitch = pitch_at_y(pos.y, rect.min.y, pitch_top);
                let hit = hit_test_at_pos(
                    &compose.project.tracks[ti].clips[ci].notes,
                    pos,
                    grid_rect,
                    beat_w,
                    beat_scroll,
                    pitch_top,
                );

                match compose.piano_roll_tool {
                    PianoRollTool::Eraser => {
                        if let HitRegion::Body(ni)
                        | HitRegion::LeftEdge(ni)
                        | HitRegion::RightEdge(ni) = hit
                        {
                            let removed =
                                compose.project.tracks[ti].clips[ci].notes.remove(ni);
                            compose.history.push(ComposeCommand::DeleteNotes {
                                track: ti,
                                clip: ci,
                                notes: vec![(ni, removed)],
                            });
                            compose.selected_notes.clear();
                            changed = true;
                        }
                    }
                    PianoRollTool::Pencil
                        if hit == HitRegion::Empty
                            && compose.drag_state.is_none()
                            && !response.drag_started()
                            && !response.drag_stopped() =>
                    {
                        let snapped = compose.snap_beats(beat);
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
                        audition_note = Some((pitch, 0.85));
                        compose.pending_audition_off = Some(pitch);
                        changed = true;
                    }
                    PianoRollTool::Select if hit == HitRegion::Empty => {
                        compose.selected_notes.clear();
                    }
                    PianoRollTool::Select if !recording => {
                        if let HitRegion::Body(ni) = hit {
                            let note = &compose.project.tracks[ti].clips[ci].notes[ni];
                            audition_note = Some((note.pitch, note.velocity));
                            compose.pending_audition_off = Some(note.pitch);
                        }
                    }
                    _ => {}
                }

                let _ = (beat, pitch);
            }
        }
    }

    if response.drag_stopped() {
        if matches!(
            compose.drag_state.as_ref().map(|d| &d.drag),
            Some(NoteDrag::Pencil { .. })
        ) {
            if let Some(NoteDrag::Pencil { pitch, .. }) =
                compose.drag_state.as_ref().map(|d| d.drag.clone())
            {
                audition_note_off = Some(pitch);
            }
        }
    }

    if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            if let Some(drag) = compose.drag_state.as_mut() {
                drag.last_pos = Some(pos);
            }
        }
        if let Some(drag) = compose.drag_state.as_ref() {
            if let Some(pos) = response.interact_pointer_pos() {
                let beat = beat_at_pos(pos, grid_left, beat_w, beat_scroll).clamp(0.0, clip_len);
                match &drag.drag {
                    NoteDrag::ResizeLeft { note_idx, orig_start, orig_dur } => {
                        let snapped = compose.snap_beats(beat);
                        let end = orig_start + orig_dur;
                        let new_start = snapped.min(end - step);
                        let new_dur = (end - new_start).max(step);
                        if let Some(note) = compose
                            .project
                            .tracks[ti]
                            .clips[ci]
                            .notes
                            .get_mut(*note_idx)
                        {
                            note.start_beats = new_start.max(0.0);
                            note.duration_beats = new_dur;
                        }
                        changed = true;
                    }
                    NoteDrag::ResizeRight { note_idx, orig_start, orig_dur: _ } => {
                        let snapped = compose.snap_beats(beat);
                        let new_dur = (snapped - orig_start).max(step);
                        if let Some(note) = compose
                            .project
                            .tracks[ti]
                            .clips[ci]
                            .notes
                            .get_mut(*note_idx)
                        {
                            note.duration_beats = new_dur;
                        }
                        changed = true;
                    }
                    NoteDrag::Move { originals } => {
                        let snap_enabled = compose.snap_enabled;
                        let snap = compose.snap_division;
                        let snap_beats = |beats: f32| {
                            if !snap_enabled {
                                return beats.max(0.0);
                            }
                            let step = snap.beats_per_step();
                            (beats / step).round() * step
                        };
                        let delta_beats = beat_at_pos(pos, grid_left, beat_w, beat_scroll)
                            - beat_at_pos(drag.start_pos, grid_left, beat_w, beat_scroll);
                        let delta_pitch_i16 = pitch_at_y(pos.y, rect.min.y, pitch_top) as i16
                            - pitch_at_y(drag.start_pos.y, rect.min.y, pitch_top) as i16;
                        for &(idx, orig_start, orig_pitch) in originals {
                            let new_start =
                                snap_beats((orig_start + delta_beats).max(0.0));
                            let new_pitch = (orig_pitch as i16 + delta_pitch_i16)
                                .clamp(PITCH_BOTTOM as i16, PITCH_TOP as i16)
                                as u8;
                            if let Some(note) = compose
                                .project
                                .tracks[ti]
                                .clips[ci]
                                .notes
                                .get_mut(idx)
                            {
                                note.start_beats = new_start;
                                note.pitch = new_pitch;
                            }
                        }
                        changed = true;
                    }
                    NoteDrag::Pencil { .. } => {}
                }
            }
        }
    }

    if response.drag_stopped() {
        if let Some(drag) = compose.drag_state.take() {
            // interact_pointer_pos() is often None on release — use last/start pos.
            let pos = response
                .interact_pointer_pos()
                .or(drag.last_pos)
                .or_else(|| ui.input(|i| i.pointer.latest_pos()))
                .unwrap_or(drag.start_pos);
            {
                match drag.drag {
                    NoteDrag::Pencil { pitch, start_beats } => {
                        let end_beat =
                            beat_at_pos(pos, grid_left, beat_w, beat_scroll).clamp(0.0, clip_len);
                        let snapped_start = compose.snap_beats(start_beats.min(end_beat));
                        let snapped_end = compose.snap_beats(start_beats.max(end_beat));
                        let duration = (snapped_end - snapped_start).max(step);
                        let note = MidiNote {
                            pitch,
                            start_beats: snapped_start,
                            duration_beats: duration,
                            velocity: 0.78,
                        };
                        compose.history.push(ComposeCommand::AddNote {
                            track: ti,
                            clip: ci,
                            note: note.clone(),
                        });
                        compose.project.tracks[ti].clips[ci].notes.push(note);
                        audition_note_off = Some(pitch);
                        changed = true;
                    }
                    NoteDrag::ResizeLeft { note_idx, orig_start, orig_dur }
                    | NoteDrag::ResizeRight { note_idx, orig_start, orig_dur } => {
                        if let Some(note) =
                            compose.project.tracks[ti].clips[ci].notes.get(note_idx)
                        {
                            if (note.start_beats - orig_start).abs() > 0.001
                                || (note.duration_beats - orig_dur).abs() > 0.001
                            {
                                compose.history.push(ComposeCommand::ResizeNotes {
                                    track: ti,
                                    clip: ci,
                                    entries: vec![(
                                        note_idx,
                                        orig_start,
                                        orig_dur,
                                        note.start_beats,
                                        note.duration_beats,
                                    )],
                                });
                            }
                        }
                    }
                    NoteDrag::Move { originals } => {
                        let delta_beats = beat_at_pos(pos, grid_left, beat_w, beat_scroll)
                            - beat_at_pos(drag.start_pos, grid_left, beat_w, beat_scroll);
                        let delta_pitch = (pitch_at_y(pos.y, rect.min.y, pitch_top) as i16
                            - pitch_at_y(drag.start_pos.y, rect.min.y, pitch_top) as i16)
                            .clamp(i8::MIN as i16, i8::MAX as i16) as i8;
                        if delta_beats.abs() > 0.001 || delta_pitch != 0 {
                            let entries: Vec<(usize, f32, f32, u8)> = originals
                                .iter()
                                .map(|&(idx, start, pitch)| (idx, start, start, pitch))
                                .collect();
                            compose.history.push(ComposeCommand::MoveNotes {
                                track: ti,
                                clip: ci,
                                entries,
                                delta_beats,
                                delta_pitch,
                            });
                        }
                    }
                }
            }
        }
    }

    (changed, focused, audition_note, audition_note_off)
}

fn paint_key_column(
    painter: &egui::Painter,
    keys_rect: Rect,
    pitch_top: u8,
    keys_down: &HashSet<u8>,
    pointer_held: Option<u8>,
    tokens: &Tokens,
) {
    painter.rect_filled(keys_rect, 0.0, tokens.surface2);
    let rows_visible = ((keys_rect.height() / ROW_H).ceil() as u8).saturating_add(1);
    for row in 0..rows_visible {
        let p = pitch_top.saturating_sub(row);
        if p < PITCH_BOTTOM {
            break;
        }
        let y = keys_rect.min.y + row as f32 * ROW_H;
        let key_rect = Rect::from_min_max(
            Pos2::new(keys_rect.min.x, y),
            Pos2::new(keys_rect.max.x - 1.0, y + ROW_H),
        );
        let held = keys_down.contains(&p) || pointer_held == Some(p);
        let black = is_black_key(p);
        let fill = if held {
            ACCENT_UI.gamma_multiply(0.85)
        } else if black {
            Color32::from_rgb(0x22, 0x24, 0x2a)
        } else {
            Color32::from_rgb(0xe8, 0xea, 0xef)
        };
        painter.rect_filled(key_rect, 1.0, fill);
        painter.rect_stroke(key_rect, 1.0, egui::Stroke::new(0.5, tokens.border));
        if p % 12 == 0 {
            painter.text(
                Pos2::new(key_rect.min.x + 3.0, key_rect.min.y + 1.0),
                egui::Align2::LEFT_TOP,
                pitch_label(p),
                egui::FontId::monospace(8.0),
                if black || held {
                    tokens.text
                } else {
                    Color32::from_rgb(0x33, 0x36, 0x3d)
                },
            );
        }
        if let Some(glyph) = qwerty_glyph(p) {
            if !black {
                painter.text(
                    Pos2::new(key_rect.max.x - 3.0, key_rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    glyph,
                    egui::FontId::monospace(9.0),
                    if held {
                        tokens.text
                    } else {
                        Color32::from_rgb(0x55, 0x5a, 0x66)
                    },
                );
            }
        }
    }
}

fn visible_pitch_top(compose: &ComposeUi) -> u8 {
    let scroll = compose
        .pitch_scroll
        .clamp(0.0, (PITCH_TOP - PITCH_BOTTOM) as f32);
    (PITCH_TOP as f32 - scroll).round() as u8
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

fn paint_automation_lane(
    ui: &mut Ui,
    rect: Rect,
    compose: &mut ComposeUi,
    tokens: &Tokens,
) -> bool {
    let (ti, ci) = match selected_clip_mut(compose) {
        Some(v) => v,
        None => return false,
    };

    let (response, painter) = ui.allocate_painter(rect.size(), Sense::click_and_drag());
    let rect = response.rect;
    painter.rect_filled(rect, 0.0, tokens.surface2);

    region(
        ui,
        Rect::from_min_max(rect.min, pos2(rect.min.x + 180.0, rect.min.y + 16.0)),
        |ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Auto")
                        .size(9.0)
                        .color(tokens.text_muted),
                );
                let idx = compose
                    .automation_target
                    .min(AUTOMATION_TARGET_LABELS.len().saturating_sub(1));
                let label = AUTOMATION_TARGET_LABELS[idx];
                reel_combo(ui, "auto_target", select_value_text(label), 72.0, |ui| {
                    styled_menu_body(ui, |ui| {
                        for (i, name) in AUTOMATION_TARGET_LABELS.iter().enumerate() {
                            if menu_selectable(ui, compose.automation_target == i, name).clicked() {
                                compose.automation_target = i;
                            }
                        }
                    });
                });
            });
        },
    );

    let target_label =
        AUTOMATION_TARGET_LABELS[compose.automation_target.min(AUTOMATION_TARGET_LABELS.len().saturating_sub(1))];
    let target_id = automation_target_to_engine(target_label);
    let clip_len = compose.project.tracks[ti].clips[ci].length_beats;
    let beat_w = rect.width() / clip_len.max(1.0);
    let graph_top = rect.min.y + 18.0;
    let graph_h = (rect.height() - 20.0).max(8.0);
    let graph = Rect::from_min_max(
        pos2(rect.min.x, graph_top),
        pos2(rect.max.x, graph_top + graph_h),
    );

    painter.line_segment(
        [graph.left_bottom(), graph.right_bottom()],
        egui::Stroke::new(0.5, tokens.border),
    );

    let snap = compose.snap_division;
    let snap_beats = |beats: f32| {
        let step = snap.beats_per_step();
        (beats / step).round() * step
    };

    let clip = &mut compose.project.tracks[ti].clips[ci];
    if clip.automation.iter().all(|l| l.target != target_id) && response.clicked() {
        clip.automation.push(AutomationLane {
            target: target_id.clone(),
            points: Vec::new(),
        });
    }
    let lane_idx = clip
        .automation
        .iter()
        .position(|l| l.target == target_id);

    let mut changed = false;
    if let Some(li) = lane_idx {
        let points_snapshot = clip.automation[li].points.clone();

        if points_snapshot.len() >= 2 {
            let mut sorted = points_snapshot.clone();
            sorted.sort_by(|a, b| {
                a.beats
                    .partial_cmp(&b.beats)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            for window in sorted.windows(2) {
                let a = &window[0];
                let b = &window[1];
                let x0 = graph.min.x + a.beats * beat_w;
                let y0 = graph.max.y - a.value * graph_h;
                let x1 = graph.min.x + b.beats * beat_w;
                let y1 = graph.max.y - b.value * graph_h;
                painter.line_segment(
                    [pos2(x0, y0), pos2(x1, y1)],
                    egui::Stroke::new(1.5, ACCENT_UI),
                );
            }
        }

        for pt in &points_snapshot {
            let cx = graph.min.x + pt.beats * beat_w;
            let cy = graph.max.y - pt.value * graph_h;
            painter.circle_filled(pos2(cx, cy), 4.0, ACCENT_UI.gamma_multiply(0.9));
        }

        if let Some(pos) = response.interact_pointer_pos() {
            if response.clicked() && graph.contains(pos) {
                if let Some(hit) = nearest_point(&points_snapshot, pos, graph, beat_w) {
                    clip.automation[li].points.remove(hit);
                    changed = true;
                } else {
                    let beats = ((pos.x - graph.min.x) / beat_w).clamp(0.0, clip_len);
                    let value = ((graph.max.y - pos.y) / graph_h).clamp(0.0, 1.0);
                    clip.automation[li].points.push(AutomationPoint {
                        beats: snap_beats(beats),
                        value,
                    });
                    changed = true;
                }
            } else if response.dragged() {
                if let Some(hit) = nearest_point(&points_snapshot, pos, graph, beat_w) {
                    let beats = ((pos.x - graph.min.x) / beat_w).clamp(0.0, clip_len);
                    let value = ((graph.max.y - pos.y) / graph_h).clamp(0.0, 1.0);
                    clip.automation[li].points[hit].beats = snap_beats(beats);
                    clip.automation[li].points[hit].value = value;
                    changed = true;
                }
            }
        }
    }

    changed
}

fn nearest_point(
    points: &[AutomationPoint],
    pos: Pos2,
    graph: Rect,
    beat_w: f32,
) -> Option<usize> {
    points
        .iter()
        .enumerate()
        .min_by(|(_, a), (_, b)| {
            let ax = graph.min.x + a.beats * beat_w;
            let ay = graph.max.y - a.value * graph.height();
            let bx = graph.min.x + b.beats * beat_w;
            let by = graph.max.y - b.value * graph.height();
            let da = (pos.x - ax).powi(2) + (pos.y - ay).powi(2);
            let db = (pos.x - bx).powi(2) + (pos.y - by).powi(2);
            da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
        })
        .and_then(|(i, pt)| {
            let cx = graph.min.x + pt.beats * beat_w;
            let cy = graph.max.y - pt.value * graph.height();
            let dist = (pos.x - cx).powi(2) + (pos.y - cy).powi(2);
            if dist < 100.0 { Some(i) } else { None }
        })
}

fn paint_note(
    painter: &egui::Painter,
    grid: Rect,
    note: &MidiNote,
    beat_w: f32,
    beat_scroll: f32,
    pitch_top: u8,
    selected: bool,
    tokens: &Tokens,
    overlay: bool,
) {
    let row = pitch_top as i16 - note.pitch as i16;
    if row < -1 || row > 200 {
        return;
    }
    let x = grid.min.x + (note.start_beats - beat_scroll) * beat_w;
    let w = note.duration_beats * beat_w;
    let note_rect = Rect::from_min_max(
        Pos2::new(x + 1.0, grid.min.y + row as f32 * ROW_H + 1.0),
        Pos2::new(x + w - 1.0, grid.min.y + (row as f32 + 1.0) * ROW_H - 1.0),
    );
    if note_rect.max.x < grid.min.x || note_rect.min.x > grid.max.x {
        return;
    }
    let fill = if overlay {
        ACCENT_UI.gamma_multiply(0.55)
    } else if selected {
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
    if selected && !overlay && w > RESIZE_HANDLE_W * 2.0 {
        let left = Rect::from_min_max(
            note_rect.left_top(),
            Pos2::new(note_rect.min.x + RESIZE_HANDLE_W, note_rect.max.y),
        );
        let right = Rect::from_min_max(
            Pos2::new(note_rect.max.x - RESIZE_HANDLE_W, note_rect.min.y),
            note_rect.right_bottom(),
        );
        painter.rect_filled(left, 1.0, tokens.text.gamma_multiply(0.85));
        painter.rect_filled(right, 1.0, tokens.text.gamma_multiply(0.85));
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct DragState {
    pub(crate) drag: NoteDrag,
    pub(crate) start_pos: Pos2,
    /// Last pointer position during the drag (release often clears interact_pointer_pos).
    pub(crate) last_pos: Option<Pos2>,
}

fn pitch_at_y(y: f32, grid_top: f32, pitch_top: u8) -> u8 {
    let pitch = pitch_top.saturating_sub(((y - grid_top) / ROW_H).floor() as u8);
    pitch.clamp(PITCH_BOTTOM, PITCH_TOP)
}

fn beat_at_pos(pos: Pos2, grid_left: f32, beat_w: f32, beat_scroll: f32) -> f32 {
    beat_scroll + (pos.x - grid_left) / beat_w
}

fn hit_test_at_pos(
    notes: &[MidiNote],
    pos: Pos2,
    grid: Rect,
    beat_w: f32,
    beat_scroll: f32,
    pitch_top: u8,
) -> HitRegion {
    let beat = beat_at_pos(pos, grid.min.x, beat_w, beat_scroll);
    let pitch = pitch_at_y(pos.y, grid.min.y, pitch_top);
    for (i, note) in notes.iter().enumerate().rev() {
        if note.pitch != pitch {
            continue;
        }
        if beat < note.start_beats || beat >= note.start_beats + note.duration_beats {
            continue;
        }
        let x = grid.min.x + (note.start_beats - beat_scroll) * beat_w;
        let w = note.duration_beats * beat_w;
        let rel_x = pos.x - x;
        if rel_x <= RESIZE_HANDLE_W {
            return HitRegion::LeftEdge(i);
        }
        if rel_x >= w - RESIZE_HANDLE_W {
            return HitRegion::RightEdge(i);
        }
        return HitRegion::Body(i);
    }
    HitRegion::Empty
}

fn duplicate_selected_notes(
    compose: &mut ComposeUi,
    ti: usize,
    ci: usize,
    indices: &[usize],
) -> Vec<usize> {
    let mut new_indices = Vec::new();
    for &ni in indices {
        if let Some(note) = compose.project.tracks[ti].clips[ci].notes.get(ni).cloned() {
            compose.project.tracks[ti].clips[ci].notes.push(note.clone());
            let idx = compose.project.tracks[ti].clips[ci].notes.len() - 1;
            compose.history.push(ComposeCommand::AddNote {
                track: ti,
                clip: ci,
                note,
            });
            new_indices.push(idx);
        }
    }
    new_indices
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
            for (idx, old_start, old_dur, _, _) in entries {
                if let Some(note) = compose.project.tracks[track].clips[clip].notes.get_mut(idx) {
                    note.start_beats = old_start;
                    note.duration_beats = old_dur;
                }
            }
        }
        ComposeCommand::AddNotes { track, clip, notes } => {
            let clip_notes = &mut compose.project.tracks[track].clips[clip].notes;
            for note in notes {
                if let Some(pos) = clip_notes.iter().position(|n| n == &note) {
                    clip_notes.remove(pos);
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
            for (idx, _, _, new_start, new_dur) in entries {
                if let Some(note) = compose.project.tracks[track].clips[clip].notes.get_mut(idx) {
                    note.start_beats = new_start;
                    note.duration_beats = new_dur;
                }
            }
        }
        ComposeCommand::AddNotes { track, clip, notes } => {
            compose.project.tracks[track].clips[clip]
                .notes
                .extend(notes);
        }
    }
}

/// Generate arpeggiated notes into the selected clip using performance arp settings.
pub fn generate_arp_into_clip(
    compose: &mut ComposeUi,
    performance: &crate::performance::PerformanceUi,
    track: usize,
    clip: usize,
) -> bool {
    let settings = performance.to_settings();
    let arp = settings.arp.clone();
    if !arp.enabled {
        return false;
    }

    let clip_len = compose.project.tracks[track].clips[clip].length_beats;
    let length_beats = (compose.arp_generate_bars * 4.0).min(clip_len);
    if length_beats <= 0.0 {
        return false;
    }

    let notes = &compose.project.tracks[track].clips[clip].notes;
    let pool: Vec<u8> = if !compose.selected_notes.is_empty() {
        compose
            .selected_notes
            .iter()
            .filter_map(|&i| notes.get(i).map(|n| n.pitch))
            .collect()
    } else {
        notes.iter().map(|n| n.pitch).collect()
    };

    let pool = if pool.is_empty() {
        vec![60]
    } else {
        reelsynth::build_pool(&pool, &arp, &settings)
    };

    let generated = ArpEngine::build_pattern_notes(&pool, &arp, length_beats, 0.85);
    if generated.is_empty() {
        return false;
    }

    if compose.arp_replace_notes {
        compose.project.tracks[track].clips[clip].notes.clear();
        compose.selected_notes.clear();
    }

    compose.history.push(ComposeCommand::AddNotes {
        track,
        clip,
        notes: generated.clone(),
    });
    compose.project.tracks[track].clips[clip]
        .notes
        .extend(generated);
    true
}

/// Modal dialog for arp generation parameters.
pub fn draw_arp_generate_dialog(
    ctx: &egui::Context,
    compose: &mut ComposeUi,
    performance: &crate::performance::PerformanceUi,
) -> bool {
    if !compose.arp_dialog_open {
        return false;
    }

    let mut committed = false;
    let close = std::cell::Cell::new(false);
    let mut open = true;
    egui::Window::new("Generate Arp")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label("Bake an arpeggio pattern into the selected clip.");
            ui.add(
                egui::Slider::new(&mut compose.arp_generate_bars, 1.0..=8.0)
                    .text("Bars")
                    .fixed_decimals(0),
            );
            ui.checkbox(&mut compose.arp_replace_notes, "Replace existing notes");
            let arp = performance.arp.to_settings();
            ui.label(format!(
                "Using: {} · {} · {}",
                super::super::performance::INPUT_MODE_NAMES
                    [performance.arp.input_mode.min(2)],
                super::super::performance::STYLE_NAMES
                    [performance.arp.direction.min(6)],
                super::super::performance::RATE_NAMES[performance.arp.rate.min(5)],
            ));
            if !arp.enabled {
                ui.colored_label(Color32::YELLOW, "Enable Arp in the footer to generate.");
            }
            ui.horizontal(|ui| {
                if ui.button("Cancel").clicked() {
                    close.set(true);
                }
                if ui.button("Generate").clicked() && arp.enabled {
                    if let (Some(ti), Some(ci)) = (Some(compose.selected_track), compose.selected_clip) {
                        if ti < compose.project.tracks.len()
                            && ci < compose.project.tracks[ti].clips.len()
                        {
                            committed = generate_arp_into_clip(compose, performance, ti, ci);
                        }
                    }
                    close.set(true);
                }
            });
        });
    compose.arp_dialog_open = open && !close.get();
    committed
}
