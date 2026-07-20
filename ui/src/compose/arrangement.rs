//! Multi-track arrangement timeline with clip blocks and playhead.

use egui::{Pos2, Rect, Sense, Ui, Vec2};
use reelsynth::{Clip, SequenceProject};
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::audit_registry::{record_region, AuditId};
use crate::layout::GRID_UNIT;
use crate::region::region;

use super::ComposeUi;

const BEATS_VISIBLE: f32 = 32.0;
/// Slim clip-strip row height (Layout A).
const TRACK_ROW_H: f32 = 20.0;
const RULER_H: f32 = 14.0;

pub struct ArrangementActions {
    pub clip_selected: bool,
    pub clip_created: bool,
    pub playhead_scrubbed: bool,
    pub sequence_changed: bool,
}

impl Default for ArrangementActions {
    fn default() -> Self {
        Self {
            clip_selected: false,
            clip_created: false,
            playhead_scrubbed: false,
            sequence_changed: false,
        }
    }
}

pub fn draw_arrangement(
    ui: &mut Ui,
    rect: Rect,
    compose: &mut ComposeUi,
) -> ArrangementActions {
    let tokens = Tokens::default();
    let mut actions = ArrangementActions::default();

    region(ui, rect, |ui| {
        egui::Frame::none()
            .inner_margin(egui::Margin::symmetric(GRID_UNIT, GRID_UNIT * 0.5))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    let arrow = if compose.arrangement_collapsed {
                        "▸"
                    } else {
                        "▾"
                    };
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new(format!("Clips {arrow}"))
                                    .size(10.0)
                                    .color(tokens.text_muted),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        compose.arrangement_collapsed = !compose.arrangement_collapsed;
                    }
                    if compose.arrangement_collapsed {
                        ui.label(
                            egui::RichText::new("hidden — expand for timeline / multi-clip")
                                .size(9.0)
                                .color(tokens.text_muted),
                        );
                    }
                });

                if compose.arrangement_collapsed {
                    return;
                }

                ui.add_space(GRID_UNIT * 0.25);
                let available = ui.available_rect_before_wrap();
                let strip_h = available.height().max(TRACK_ROW_H + RULER_H);
                let strip_rect =
                    Rect::from_min_size(available.min, Vec2::new(available.width(), strip_h));
                draw_clip_strip(ui, strip_rect, compose, &tokens, &mut actions);
            });
    });

    actions
}

fn draw_clip_strip(
    ui: &mut Ui,
    rect: Rect,
    compose: &mut ComposeUi,
    tokens: &Tokens,
    actions: &mut ArrangementActions,
) {
    let (response, _painter) = ui.allocate_painter(rect.size(), Sense::click_and_drag());
    let rect = response.rect;
    if !ui.is_rect_visible(rect) {
        return;
    }

    let painter = ui.painter_at(rect);
    painter.rect_filled(rect, 0.0, tokens.bg_muted);

    let track_count = compose.project.tracks.len();
    let content_top = rect.min.y + RULER_H;
    let beat_w = (rect.width() - GRID_UNIT) / BEATS_VISIBLE;

    paint_ruler(&painter, rect, beat_w, &compose.project, tokens);
    paint_grid(
        &painter,
        Rect::from_min_max(Pos2::new(rect.min.x, content_top), rect.max),
        beat_w,
        track_count,
        tokens,
    );

    for (ti, track) in compose.project.tracks.iter().enumerate() {
        let row_y = content_top + ti as f32 * TRACK_ROW_H;
        let row_rect = Rect::from_min_max(
            Pos2::new(rect.min.x, row_y),
            Pos2::new(rect.max.x, row_y + TRACK_ROW_H),
        );
        for (ci, clip) in track.clips.iter().enumerate() {
            let clip_rect = clip_block_rect(row_rect, clip, beat_w);
            paint_clip(
                &painter,
                row_rect,
                clip,
                beat_w,
                ti == compose.selected_track && compose.selected_clip == Some(ci),
                tokens,
            );
            record_region(
                ui.ctx(),
                AuditId::ComposeArrangementClip(ci),
                clip_rect,
                clip_rect,
            );
        }
    }

    let playhead_x = rect.min.x + compose.transport.playhead_beats * beat_w;
    painter.line_segment(
        [
            Pos2::new(playhead_x, rect.min.y),
            Pos2::new(playhead_x, rect.max.y),
        ],
        egui::Stroke::new(2.0_f32, ACCENT_UI),
    );

    if response.clicked() {
        if let Some(pos) = response.interact_pointer_pos() {
            if pos.y < content_top {
                let beats = ((pos.x - rect.min.x) / beat_w).clamp(0.0, BEATS_VISIBLE);
                compose.transport.playhead_beats = beats;
                actions.playhead_scrubbed = true;
            } else if response.double_clicked() {
                let track_idx = ((pos.y - content_top) / TRACK_ROW_H).floor() as usize;
                if track_idx < track_count {
                    let beat = compose.snap_beats((pos.x - rect.min.x) / beat_w);
                    let clip = Clip::new(beat, 4.0);
                    compose.project.tracks[track_idx].clips.push(clip);
                    compose.selected_track = track_idx;
                    compose.selected_clip =
                        Some(compose.project.tracks[track_idx].clips.len() - 1);
                    compose.selected_notes.clear();
                    actions.clip_created = true;
                    actions.sequence_changed = true;
                }
            } else {
                let track_idx = ((pos.y - content_top) / TRACK_ROW_H).floor() as usize;
                if track_idx < track_count {
                    let beat = (pos.x - rect.min.x) / beat_w;
                    if let Some(ci) = hit_clip(&compose.project.tracks[track_idx].clips, beat) {
                        compose.selected_track = track_idx;
                        compose.selected_clip = Some(ci);
                        compose.selected_notes.clear();
                        actions.clip_selected = true;
                    }
                }
            }
        }
    }

    if response.dragged() {
        if let Some(pos) = response.interact_pointer_pos() {
            if pos.y < content_top + track_count as f32 * TRACK_ROW_H {
                let beats = ((pos.x - rect.min.x) / beat_w).clamp(0.0, BEATS_VISIBLE);
                compose.transport.playhead_beats = beats;
                actions.playhead_scrubbed = true;
            }
        }
    }
}

fn paint_ruler(
    painter: &egui::Painter,
    rect: Rect,
    beat_w: f32,
    project: &SequenceProject,
    tokens: &Tokens,
) {
    let ruler = Rect::from_min_max(rect.min, Pos2::new(rect.max.x, rect.min.y + RULER_H));
    painter.rect_filled(ruler, 0.0, tokens.surface2);
    let bar_beats = project.time_sig_num as f32;
    for bar in 0..=(BEATS_VISIBLE / bar_beats).ceil() as i32 {
        let x = rect.min.x + bar as f32 * bar_beats * beat_w;
        if x > rect.max.x {
            break;
        }
        painter.line_segment(
            [Pos2::new(x, ruler.min.y), Pos2::new(x, ruler.max.y)],
            egui::Stroke::new(1.0_f32, tokens.border_strong),
        );
        painter.text(
            Pos2::new(x + 2.0, ruler.min.y + 2.0),
            egui::Align2::LEFT_TOP,
            format!("{}", bar + 1),
            egui::FontId::monospace(9.0),
            tokens.text_secondary,
        );
    }
}

fn paint_grid(
    painter: &egui::Painter,
    rect: Rect,
    beat_w: f32,
    track_count: usize,
    tokens: &Tokens,
) {
    for ti in 0..track_count {
        let y = rect.min.y + ti as f32 * TRACK_ROW_H;
        if ti % 2 == 0 {
            painter.rect_filled(
                Rect::from_min_max(
                    Pos2::new(rect.min.x, y),
                    Pos2::new(rect.max.x, y + TRACK_ROW_H),
                ),
                0.0,
                tokens.bg.gamma_multiply(0.5),
            );
        }
        painter.line_segment(
            [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
            egui::Stroke::new(1.0_f32, tokens.border),
        );
    }
    for beat in 0..=BEATS_VISIBLE as i32 {
        let x = rect.min.x + beat as f32 * beat_w;
        let strong = beat % 4 == 0;
        painter.line_segment(
            [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
            egui::Stroke::new(
                if strong { 1.0_f32 } else { 0.5_f32 },
                if strong {
                    tokens.border_strong
                } else {
                    tokens.border.gamma_multiply(0.6)
                },
            ),
        );
    }
}

fn clip_block_rect(row: Rect, clip: &Clip, beat_w: f32) -> Rect {
    let x0 = row.min.x + clip.start_beats * beat_w;
    let w = clip.length_beats * beat_w;
    let _ = (x0, w);
    Rect::from_min_max(
        Pos2::new(x0 + 1.0, row.min.y + 2.0),
        Pos2::new(x0 + w - 1.0, row.max.y - 2.0),
    )
}

fn paint_clip(
    painter: &egui::Painter,
    row: Rect,
    clip: &Clip,
    beat_w: f32,
    selected: bool,
    tokens: &Tokens,
) {
    let clip_rect = clip_block_rect(row, clip, beat_w);
    if clip_rect.width() < 2.0 {
        return;
    }
    let fill = if selected {
        ACCENT_UI.gamma_multiply(0.55)
    } else {
        tokens.accent.gamma_multiply(0.45)
    };
    painter.rect_filled(clip_rect, 4.0, fill);
    painter.rect_stroke(
        clip_rect,
        4.0,
        egui::Stroke::new(
            if selected { 1.5_f32 } else { 1.0_f32 },
            if selected {
                ACCENT_UI
            } else {
                tokens.border_strong
            },
        ),
    );
    if clip_rect.width() > 24.0 {
        painter.text(
            clip_rect.left_top() + Vec2::new(4.0, 4.0),
            egui::Align2::LEFT_TOP,
            format!("{} n", clip.notes.len()),
            egui::FontId::proportional(9.0),
            tokens.text,
        );
    }
}

fn hit_clip(clips: &[Clip], beat: f32) -> Option<usize> {
    clips
        .iter()
        .position(|c| beat >= c.start_beats && beat < c.start_beats + c.length_beats)
}
