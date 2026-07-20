//! Compose mode shell — arrangement, piano roll, scenes, transport.

mod arrangement;
mod command_history;
mod piano_roll;
mod scene_grid;
mod track_list;
mod transport_bar;

pub use command_history::CommandHistory;
pub use piano_roll::PianoRollTool;
pub use transport_bar::TransportBarActions;

use egui::{Rect, Ui};
use reelsynth_ui_theme::Tokens;

use crate::audit_registry::{record_region, AuditId};
use crate::layout::{GRID_UNIT, UiScale};
use crate::region::region;
use crate::state::{ShellActions, UiState};

use arrangement::draw_arrangement;
use piano_roll::draw_piano_roll;
use scene_grid::draw_scene_grid;
use track_list::draw_track_list;
use transport_bar::draw_transport_bar;

pub use reelsynth::{
    Clip, ClipRef, MidiNote, QuantizeDivision, QuantizeGrid, Scene, SequenceProject, Track,
};

pub const TRACK_LIST_WIDTH: f32 = 180.0;
pub const TRANSPORT_BAR_HEIGHT: f32 = 40.0;

/// Transport UI state (engine sync pending backend).
#[derive(Clone, Debug, PartialEq)]
pub struct TransportUi {
    pub playing: bool,
    pub recording: bool,
    pub playhead_beats: f32,
    pub loop_enabled: bool,
    pub metronome: bool,
}

impl Default for TransportUi {
    fn default() -> Self {
        Self {
            playing: false,
            recording: false,
            playhead_beats: 0.0,
            loop_enabled: true,
            metronome: false,
        }
    }
}

/// Compose-mode UI state held in [`UiState`].
#[derive(Clone, Debug, PartialEq)]
pub struct ComposeUi {
    pub project: SequenceProject,
    pub transport: TransportUi,
    pub snap_division: QuantizeDivision,
    pub snap_enabled: bool,
    pub selected_track: usize,
    pub selected_clip: Option<usize>,
    pub selected_notes: std::collections::HashSet<usize>,
    pub piano_roll_tool: PianoRollTool,
    pub piano_roll_focused: bool,
    pub launched_scene: Option<usize>,
    pub active_scene_slots: Vec<Option<ClipRef>>,
    pub automation_target: usize,
    pub history: CommandHistory,
    pub(crate) drag_state: Option<piano_roll::DragState>,
    /// Live notes from engine recorder overlay (not yet committed to clip).
    pub live_record_overlay: Vec<MidiNote>,
    pub arp_dialog_open: bool,
    pub arp_generate_bars: f32,
    pub arp_replace_notes: bool,
    /// Scenes panel collapsed by default (Layout A clip-editor shell).
    pub scenes_collapsed: bool,
    /// Clip strip (arrangement) collapsed by default — piano roll is the primary surface.
    pub arrangement_collapsed: bool,
    /// Rows scrolled down from MIDI pitch 108 (C8).
    pub pitch_scroll: f32,
    /// Beats scrolled from clip start.
    pub beat_scroll: f32,
    /// Horizontal zoom: 1.0 = fit clip; >1 zooms in.
    pub beat_zoom: f32,
    /// Pitch held by pointer on the key column (for note-off on release).
    pub key_pointer_held: Option<u8>,
    /// Deferred note-off for click audition (fires next frame).
    pub pending_audition_off: Option<u8>,
}

impl Default for ComposeUi {
    fn default() -> Self {
        let mut project = SequenceProject::default();
        // Default clip so the roll is never empty on first Compose entry.
        if let Some(track) = project.tracks.first_mut() {
            if track.clips.is_empty() {
                track.clips.push(Clip::new(0.0, 8.0));
            }
        }
        Self {
            project,
            transport: TransportUi::default(),
            snap_division: QuantizeDivision::Sixteenth,
            snap_enabled: true,
            selected_track: 0,
            selected_clip: Some(0),
            selected_notes: std::collections::HashSet::new(),
            piano_roll_tool: PianoRollTool::Pencil,
            piano_roll_focused: false,
            launched_scene: None,
            active_scene_slots: Vec::new(),
            automation_target: 0,
            history: CommandHistory::new(64),
            drag_state: None,
            live_record_overlay: Vec::new(),
            arp_dialog_open: false,
            arp_generate_bars: 2.0,
            arp_replace_notes: true,
            scenes_collapsed: true,
            arrangement_collapsed: true,
            // Start near middle-C window (C8=108 → scroll ~48 rows ≈ C4).
            pitch_scroll: 48.0,
            beat_scroll: 0.0,
            beat_zoom: 1.0,
            key_pointer_held: None,
            pending_audition_off: None,
        }
    }
}

impl ComposeUi {
    pub fn armed_track(&self) -> Option<usize> {
        self.project.armed_track_index()
    }

    pub fn snap_beats(&self, beats: f32) -> f32 {
        if !self.snap_enabled {
            return beats.max(0.0);
        }
        let step = self.snap_division.beats_per_step();
        (beats / step).round() * step
    }

    /// Ensure the active track has a clip and `selected_clip` points at it so the
    /// piano roll is immediately editable (no arrangement click required).
    pub fn ensure_editable_clip(&mut self) {
        if self.project.tracks.is_empty() {
            self.project.tracks.push(Track::new("Track 1"));
        }
        if self.selected_track >= self.project.tracks.len() {
            self.selected_track = 0;
        }
        let ti = self.selected_track;
        if self.project.tracks[ti].clips.is_empty() {
            self.project.tracks[ti].clips.push(Clip::new(0.0, 8.0));
        }
        let clip_count = self.project.tracks[ti].clips.len();
        match self.selected_clip {
            Some(ci) if ci < clip_count => {}
            _ => {
                self.selected_clip = Some(0);
                self.selected_notes.clear();
            }
        }
    }
}

/// Draw the full Compose mode layout inside `main` rect.
pub fn draw_compose_shell(
    ui: &mut Ui,
    main: Rect,
    state: &mut UiState,
    actions: &mut ShellActions,
    scale: UiScale,
) {
    let tokens = Tokens::default();
    let s = scale.ui();
    let border = egui::Stroke::new(1.0_f32, tokens.border);

    let transport_h = TRANSPORT_BAR_HEIGHT * s;
    let transport_rect = Rect::from_min_max(
        main.min,
        egui::pos2(main.max.x, main.min.y + transport_h),
    );
    let body_top = transport_rect.max.y;
    let body = Rect::from_min_max(
        egui::pos2(main.min.x, body_top),
        main.max,
    );

    let mut transport_actions = TransportBarActions::default();
    region(ui, transport_rect, |ui| {
        transport_actions = draw_transport_bar(ui, ui.max_rect(), &mut state.compose);
    });
    record_region(
        ui.ctx(),
        AuditId::ComposeTransport,
        transport_rect,
        transport_rect,
    );
    if transport_actions.play {
        actions.transport_play = true;
    }
    if transport_actions.stop {
        actions.transport_stop = true;
    }
    if transport_actions.record {
        actions.transport_record = true;
    }
    if transport_actions.params_changed {
        actions.sequence_changed = true;
    }

    let track_w = (TRACK_LIST_WIDTH * s).min(body.width() * 0.22);
    let track_rect = Rect::from_min_max(body.min, egui::pos2(body.min.x + track_w, body.max.y));
    let content = Rect::from_min_max(
        egui::pos2(body.min.x + track_w, body.min.y),
        body.max,
    );

    ui.painter_at(body).line_segment(
        [track_rect.right_top(), track_rect.right_bottom()],
        border,
    );

    state.compose.ensure_editable_clip();

    let track_actions = draw_track_list(ui, track_rect, &mut state.compose);
    record_region(
        ui.ctx(),
        AuditId::ComposeTrackList,
        track_rect,
        track_rect,
    );
    if track_actions.track_state_changed {
        actions.sequence_changed = true;
    }

    // Dominant piano roll; clip strip + scenes collapsed by default.
    let content_h = content.height();
    let scene_h = if state.compose.scenes_collapsed {
        22.0 * s
    } else {
        (content_h * 0.12).max(64.0 * s)
    };
    let arrangement_h = if state.compose.arrangement_collapsed {
        22.0 * s
    } else {
        (content_h * 0.15).clamp(56.0 * s, 120.0 * s)
    };
    let gap = GRID_UNIT * 0.5 * s;
    let piano_h = (content_h - arrangement_h - scene_h - gap * 2.0).max(120.0 * s);

    let mut y = content.min.y;
    let arrangement_rect = Rect::from_min_max(
        egui::pos2(content.min.x, y),
        egui::pos2(content.max.x, y + arrangement_h),
    );
    y += arrangement_h + gap;
    let piano_rect = Rect::from_min_max(
        egui::pos2(content.min.x, y),
        egui::pos2(content.max.x, y + piano_h),
    );
    y += piano_h + gap;
    let scene_rect = Rect::from_min_max(
        egui::pos2(content.min.x, y),
        egui::pos2(content.max.x, (y + scene_h).min(content.max.y)),
    );

    let arr_actions = draw_arrangement(ui, arrangement_rect, &mut state.compose);
    record_region(
        ui.ctx(),
        AuditId::ComposeArrangement,
        arrangement_rect,
        arrangement_rect,
    );
    if arr_actions.sequence_changed {
        actions.sequence_changed = true;
    }
    if arr_actions.playhead_scrubbed {
        actions.transport_seek = Some(state.compose.transport.playhead_beats);
    }

    let roll_actions = draw_piano_roll(ui, piano_rect, &mut state.compose, &state.keys_down);
    record_region(
        ui.ctx(),
        AuditId::ComposePianoRoll,
        piano_rect,
        piano_rect,
    );
    if roll_actions.sequence_changed {
        actions.sequence_changed = true;
    }
    if roll_actions.open_arp_dialog {
        state.compose.arp_dialog_open = true;
    }
    if piano_roll::draw_arp_generate_dialog(ui.ctx(), &mut state.compose, &state.performance) {
        actions.sequence_changed = true;
    }
    if let Some((note, vel)) = roll_actions.audition_note {
        actions.note_on = Some(note);
        let _ = vel;
    }
    if let Some(note) = roll_actions.audition_note_off {
        actions.note_off = Some(note);
    }

    let scene_actions = draw_scene_grid(ui, scene_rect, &mut state.compose);
    record_region(
        ui.ctx(),
        AuditId::ComposeSceneGrid,
        scene_rect,
        scene_rect,
    );
    if scene_actions.scene_launched.is_some() {
        actions.scene_launch = scene_actions.scene_launched;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ensure_editable_clip_creates_and_selects() {
        let mut compose = ComposeUi::default();
        compose.selected_clip = None;
        compose.project.tracks[0].clips.clear();
        compose.ensure_editable_clip();
        assert_eq!(compose.selected_clip, Some(0));
        assert!(!compose.project.tracks[0].clips.is_empty());
    }

    #[test]
    fn ensure_editable_clip_on_other_track() {
        let mut compose = ComposeUi::default();
        compose.selected_track = 1;
        compose.selected_clip = None;
        compose.ensure_editable_clip();
        assert_eq!(compose.selected_track, 1);
        assert_eq!(compose.selected_clip, Some(0));
        assert!(!compose.project.tracks[1].clips.is_empty());
    }
}
