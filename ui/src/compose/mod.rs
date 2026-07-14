//! Compose mode shell — arrangement, piano roll, scenes, transport.

mod arrangement;
mod command_history;
mod piano_roll;
mod scene_grid;
mod track_list;
mod transport_bar;

pub use command_history::{CommandHistory, ComposeCommand};
pub use piano_roll::PianoRollTool;
pub use transport_bar::TransportBarActions;

use egui::{Rect, Ui};
use reelsynth_ui_theme::Tokens;

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
    pub selected_track: usize,
    pub selected_clip: Option<usize>,
    pub selected_notes: std::collections::HashSet<usize>,
    pub piano_roll_tool: PianoRollTool,
    pub piano_roll_focused: bool,
    pub launched_scene: Option<usize>,
    pub active_scene_slots: Vec<Option<ClipRef>>,
    pub history: CommandHistory,
    pub(crate) drag_state: Option<piano_roll::DragState>,
}

impl Default for ComposeUi {
    fn default() -> Self {
        Self {
            project: SequenceProject::default(),
            transport: TransportUi::default(),
            snap_division: QuantizeDivision::Sixteenth,
            selected_track: 0,
            selected_clip: None,
            selected_notes: std::collections::HashSet::new(),
            piano_roll_tool: PianoRollTool::Pencil,
            piano_roll_focused: false,
            launched_scene: None,
            active_scene_slots: Vec::new(),
            history: CommandHistory::new(64),
            drag_state: None,
        }
    }
}

impl ComposeUi {
    pub fn armed_track(&self) -> Option<usize> {
        self.project.armed_track_index()
    }

    pub fn snap_beats(&self, beats: f32) -> f32 {
        let step = self.snap_division.beats_per_step();
        (beats / step).round() * step
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

    let track_actions = draw_track_list(ui, track_rect, &mut state.compose);
    if track_actions.track_state_changed {
        actions.sequence_changed = true;
    }

    let content_h = content.height();
    let arrangement_h = content_h * 0.35;
    let piano_h = content_h * 0.45;
    let scene_h = content_h * 0.12;

    let mut y = content.min.y;
    let arrangement_rect = Rect::from_min_max(
        egui::pos2(content.min.x, y),
        egui::pos2(content.max.x, y + arrangement_h),
    );
    y += arrangement_h + GRID_UNIT * 0.5 * s;
    let piano_rect = Rect::from_min_max(
        egui::pos2(content.min.x, y),
        egui::pos2(content.max.x, y + piano_h),
    );
    y += piano_h + GRID_UNIT * 0.5 * s;
    let scene_rect = Rect::from_min_max(
        egui::pos2(content.min.x, y),
        egui::pos2(content.max.x, (y + scene_h).min(content.max.y)),
    );

    let arr_actions = draw_arrangement(ui, arrangement_rect, &mut state.compose);
    if arr_actions.sequence_changed {
        actions.sequence_changed = true;
    }
    if arr_actions.playhead_scrubbed {
        actions.transport_seek = Some(state.compose.transport.playhead_beats);
    }

    let roll_actions = draw_piano_roll(ui, piano_rect, &mut state.compose);
    if roll_actions.sequence_changed {
        actions.sequence_changed = true;
    }

    let scene_actions = draw_scene_grid(ui, scene_rect, &mut state.compose);
    if scene_actions.scene_launched.is_some() {
        actions.scene_launch = scene_actions.scene_launched;
    }
}
