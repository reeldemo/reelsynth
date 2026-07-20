use egui::{Color32, CursorIcon, Pos2, Rect, Sense, Shape, Ui, Vec2};
use reelsynth::WavetableBank;
use reelsynth_ui_theme::{ACCENT_UI, Tokens};

use crate::audit_registry::{record_region, AuditId};
use crate::layout::RADIUS_SM;

use crate::state::WtView3dMode;
use crate::region::region;

use super::waveform::{frame_index, waveform_points};

const NUM_SLICES: usize = 16;
const RIB_COUNT: usize = 12;
const HOVER_DISTANCE_PX: f32 = 12.0;
/// How quickly hovered slice breath eases in/out (1/s).
const HOVER_BREATH_SPEED: f32 = 14.0;
/// Horizontal spread when hovered slice breathes toward viewer.
const HOVER_WIDTH_EXPAND: f32 = 0.14;
/// Depth reduction on hover (0 = full depth, 1 = fully forward).
const HOVER_DEPTH_LIFT: f32 = 0.62;
/// Vertical lift on hover (fraction of depth offset removed).
const HOVER_VERTICAL_LIFT: f32 = 0.55;

fn mesh_bounds(inner: Rect) -> (f32, f32) {
    (inner.min.x + inner.width() * 0.08, inner.width() * 0.84)
}

fn smooth_hover_breath(ui: &Ui, target: f32) -> f32 {
    let id = ui.id().with("mesh_hover_breath");
    let dt = ui.ctx().input(|i| i.unstable_dt);
    let alpha = (HOVER_BREATH_SPEED * dt).clamp(0.0, 1.0);
    ui.ctx().data_mut(|d| {
        let current = d.get_temp_mut_or(id, 0.0_f32);
        *current += (target - *current) * alpha;
        *current
    })
}

pub struct WtView3dResponse {
    pub position_changed: bool,
    pub morph_changed: bool,
}

impl WtView3dResponse {
    pub fn changed(&self) -> bool {
        self.position_changed || self.morph_changed
    }
}

pub struct WtView3d<'a> {
    pub position: &'a mut f32,
    pub bank: Option<&'a WavetableBank>,
    pub morph_amount: Option<&'a mut f32>,
    pub view_mode: Option<&'a mut WtView3dMode>,
    pub time: f32,
}

impl WtView3d<'_> {
    pub fn show(self, ui: &mut Ui) -> WtView3dResponse {
        let tokens = Tokens::default();
        let accent_ui = ACCENT_UI;
        let view_h = ui.available_height().max(48.0);
        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(ui.available_width(), view_h),
            Sense::click_and_drag(),
        );

        let mut position_changed = false;
        let mut morph_changed = false;

        if !ui.is_rect_visible(rect) {
            return WtView3dResponse {
                position_changed,
                morph_changed,
            };
        }

        let inner = rect.shrink2(egui::vec2(6.0, 14.0));
        let num_frames = self
            .bank
            .map(|b| b.num_frames)
            .unwrap_or(256)
            .max(1);
        let max_pos = (num_frames - 1) as f32;

        let hover_pos = if response.hovered() {
            response.hover_pos()
        } else {
            None
        };

        if response.dragged() {
            let delta = response.drag_delta();
            if delta.x.abs() > 0.0 {
                let px_per_frame = inner.width() / max_pos.max(1.0);
                let next = (*self.position + delta.x / px_per_frame).clamp(0.0, max_pos);
                if (next - *self.position).abs() > 0.01 {
                    *self.position = next;
                    position_changed = true;
                }
            }
            if delta.y.abs() > 0.0 {
                if let Some(morph) = self.morph_amount {
                    let delta_amount = -delta.y / inner.height();
                    let next = (*morph + delta_amount).clamp(0.0, 1.0);
                    if (next - *morph).abs() > f32::EPSILON {
                        *morph = next;
                        morph_changed = true;
                    }
                }
            }
        } else if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                if inner.contains(pos) {
                    let layout = MeshLayout::new(inner, None, 0.0);
                    let next = position_from_mesh_x(&layout, pos.x, num_frames);
                    if (next - *self.position).abs() > 0.01 {
                        *self.position = next;
                        position_changed = true;
                    }
                }
            }
        }

        if response.hovered() {
            ui.ctx().set_cursor_icon(CursorIcon::ResizeHorizontal);
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, RADIUS_SM, tokens.bg);
        painter.rect_stroke(
            rect,
            RADIUS_SM,
            egui::Stroke::new(1.0_f32, tokens.border),
        );

        let center_frame = self
            .bank
            .map(|b| frame_index(*self.position, b.num_frames))
            .unwrap_or(0);

        let label = if let Some(bank) = self.bank {
            format!(
                "Frame stack · this osc · {}/{} · frame {center_frame}",
                bank.num_frames, num_frames
            )
        } else {
            format!("Frame stack · this osc · frame {center_frame}")
        };
        painter.text(
            Pos2::new(rect.min.x + 8.0, rect.min.y + 6.0),
            egui::Align2::LEFT_TOP,
            label,
            egui::FontId::proportional(10.0),
            tokens.text_secondary,
        );

        if let Some(mode) = self.view_mode {
            region(
                ui,
                Rect::from_min_max(
                    egui::pos2(rect.max.x - 120.0, rect.min.y + 4.0),
                    egui::pos2(rect.max.x - 4.0, rect.min.y + 22.0),
                ),
                |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(mode, WtView3dMode::Stack, "Stack");
                        ui.selectable_value(mode, WtView3dMode::Morph, "Morph");
                    });
                    let toggle_rect = ui.min_rect();
                    record_region(
                        ui.ctx(),
                        AuditId::CenterWt3dModeToggle,
                        toggle_rect,
                        toggle_rect,
                    );
                },
            );
        }

        paint_grid(&painter, inner, tokens.border);

        let layout_base = MeshLayout::new(inner, None, 0.0);
        let mesh_base = self
            .bank
            .map(|bank| build_mesh_slices(&layout_base, bank, *self.position));

        let hover = hover_pos.and_then(|pos| {
            mesh_base
                .as_ref()
                .map(|m| nearest_mesh_target(&layout_base, pos, m))
        });

        let on_slice = hover
            .filter(|h| {
                hover_pos.is_some()
                    && distance_to_polyline(
                        hover_pos.unwrap(),
                        &mesh_base.as_ref().unwrap().slices[h.slice].points,
                    ) <= HOVER_DISTANCE_PX
            })
            .map(|h| h.slice);

        let breath_target = if on_slice.is_some() { 1.0 } else { 0.0 };
        let breath = smooth_hover_breath(ui, breath_target);
        let layout = MeshLayout::new(inner, on_slice, breath);

        let mesh = self
            .bank
            .map(|bank| build_mesh_slices(&layout, bank, *self.position));

        if let (Some(bank), Some(mesh)) = (self.bank, mesh.as_ref()) {
            paint_mesh_from_bank(
                &painter,
                &layout,
                bank,
                mesh,
                hover,
                accent_ui,
                tokens.accent,
            );
        } else {
            paint_placeholder_mesh(&painter, inner, accent_ui);
        }

        if let Some(hover) = hover {
            if hover_pos.is_some() {
                let tip = if hover.slice == layout.half {
                    format!("Frame {center_frame} · drag ↔ position · ↕ morph")
                } else {
                    format!("Frame {} · drag ↔ scrub", hover.frame_index)
                };
                egui::show_tooltip_at_pointer(ui.ctx(), ui.layer_id(), ui.id().with("hover"), |ui| {
                    ui.label(tip);
                });
            }
        }

        record_region(ui.ctx(), AuditId::CenterWt3dMorph, rect, rect);

        WtView3dResponse {
            position_changed,
            morph_changed,
        }
    }
}

struct MeshLayout {
    inner: Rect,
    mesh_left: f32,
    mesh_width: f32,
    depth_pitch: f32,
    half: usize,
    base_amp: f32,
    depth_scale: f32,
    hover_slice: Option<usize>,
    hover_breath: f32,
}

impl MeshLayout {
    fn new(inner: Rect, hover_slice: Option<usize>, hover_breath: f32) -> Self {
        let (mesh_left, mesh_width) = mesh_bounds(inner);
        Self {
            inner,
            mesh_left,
            mesh_width,
            depth_pitch: inner.width() * 0.028,
            half: NUM_SLICES / 2,
            base_amp: 0.30,
            depth_scale: 0.22,
            hover_slice,
            hover_breath: hover_breath.clamp(0.0, 1.0),
        }
    }

    fn slice_geometry(&self, slice: usize) -> (f32, f32, Rect) {
        let base_depth = (slice as f32 / NUM_SLICES as f32 - 0.5).abs();
        let base_z = (slice as f32 - self.half as f32) * self.depth_pitch;
        let z_offset = base_z;
        let mut y_offset = base_depth * self.inner.height() * self.depth_scale;
        let mut slice_rect = Rect::from_min_max(
            Pos2::new(self.mesh_left + z_offset, self.inner.min.y + y_offset),
            Pos2::new(
                self.mesh_left + z_offset + self.mesh_width,
                self.inner.max.y - y_offset,
            ),
        );

        if self.hover_slice == Some(slice) && self.hover_breath > 0.001 {
            let b = self.hover_breath;
            y_offset *= 1.0 - HOVER_VERTICAL_LIFT * b;
            let effective_z = z_offset * (1.0 - HOVER_DEPTH_LIFT * b);
            let expand = self.mesh_width * HOVER_WIDTH_EXPAND * b;
            let lift_y = self.inner.height() * 0.12 * b;
            slice_rect = Rect::from_min_max(
                Pos2::new(
                    self.mesh_left + effective_z - expand,
                    self.inner.min.y + y_offset - lift_y,
                ),
                Pos2::new(
                    self.mesh_left + effective_z + self.mesh_width + expand,
                    self.inner.max.y - y_offset - lift_y,
                ),
            );
            return (effective_z, y_offset, slice_rect);
        }

        (z_offset, y_offset, slice_rect)
    }
}

struct MeshSlice {
    frame_index: usize,
    points: Vec<Pos2>,
}

#[derive(Clone, Copy)]
struct MeshHover {
    slice: usize,
    rib: Option<usize>,
    frame_index: usize,
}

struct MeshData {
    center_frame: usize,
    slices: Vec<MeshSlice>,
}

fn build_mesh_slices(layout: &MeshLayout, bank: &WavetableBank, position: f32) -> MeshData {
    let center_frame = frame_index(position, bank.num_frames);
    let center_frame = center_frame.clamp(0, bank.num_frames.saturating_sub(1));

    let mut slices = Vec::with_capacity(NUM_SLICES);
    for s in 0..NUM_SLICES {
        let fi = (center_frame as i32 + s as i32 - layout.half as i32)
            .clamp(0, bank.num_frames.saturating_sub(1) as i32) as usize;
        let (_, _, slice_rect) = layout.slice_geometry(s);
        let frame = bank.frame(fi);
        let points = waveform_points(frame, slice_rect, 64, layout.base_amp);
        slices.push(MeshSlice {
            frame_index: fi,
            points,
        });
    }

    MeshData {
        center_frame,
        slices,
    }
}

fn position_from_mesh_x(layout: &MeshLayout, x: f32, num_frames: usize) -> f32 {
    let max_pos = (num_frames.saturating_sub(1)) as f32;
    let t = ((x - layout.mesh_left) / layout.mesh_width).clamp(0.0, 1.0);
    t * max_pos
}

fn distance_to_polyline(pos: Pos2, points: &[Pos2]) -> f32 {
    if points.len() < 2 {
        return f32::MAX;
    }
    points
        .windows(2)
        .map(|seg| distance_to_segment(pos, seg[0], seg[1]))
        .fold(f32::MAX, f32::min)
}

fn distance_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.x * ab.x + ab.y * ab.y;
    if len_sq <= f32::EPSILON {
        return (p - a).length();
    }
    let t = ((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let closest = Pos2::new(a.x + ab.x * t, a.y + ab.y * t);
    (p - closest).length()
}

fn nearest_mesh_target(layout: &MeshLayout, pos: Pos2, mesh: &MeshData) -> MeshHover {
    let mut best_slice = layout.half;
    let mut best_dist = f32::MAX;

    for (s, slice) in mesh.slices.iter().enumerate() {
        let dist = distance_to_polyline(pos, &slice.points);
        let closer_to_center = (s as i32 - layout.half as i32).unsigned_abs()
            < (best_slice as i32 - layout.half as i32).unsigned_abs();
        if dist < best_dist - 1e-3 || ((dist - best_dist).abs() <= 1e-3 && closer_to_center) {
            best_dist = dist;
            best_slice = s;
        }
    }

    if best_dist > HOVER_DISTANCE_PX {
        return MeshHover {
            slice: layout.half,
            rib: None,
            frame_index: mesh.center_frame,
        };
    }

    let mut best_rib = None;
    let mut best_rib_dist = f32::MAX;
    for rib in 0..=RIB_COUNT {
        let t = rib as f32 / RIB_COUNT as f32;
        for window in mesh.slices.windows(2) {
            if let [a, b] = window {
                if a.points.is_empty() || b.points.is_empty() {
                    continue;
                }
                let ia = ((a.points.len() - 1) as f32 * t).round() as usize;
                let ib = ((b.points.len() - 1) as f32 * t).round() as usize;
                let pa = a.points[ia.min(a.points.len() - 1)];
                let pb = b.points[ib.min(b.points.len() - 1)];
                let dist = distance_to_segment(pos, pa, pb);
                if dist < best_rib_dist {
                    best_rib_dist = dist;
                    best_rib = Some(rib);
                }
            }
        }
    }

    let rib = best_rib.filter(|_| best_rib_dist <= HOVER_DISTANCE_PX);
    MeshHover {
        slice: best_slice,
        rib,
        frame_index: mesh.slices[best_slice].frame_index,
    }
}

fn paint_grid(painter: &egui::Painter, rect: Rect, border: Color32) {
    let step = 24.0;
    let mut x = rect.min.x;
    while x <= rect.max.x {
        painter.line_segment(
            [Pos2::new(x, rect.min.y), Pos2::new(x, rect.max.y)],
            egui::Stroke::new(0.5_f32, border.gamma_multiply(0.5)),
        );
        x += step;
    }
    let mut y = rect.min.y;
    while y <= rect.max.y {
        painter.line_segment(
            [Pos2::new(rect.min.x, y), Pos2::new(rect.max.x, y)],
            egui::Stroke::new(0.5_f32, border.gamma_multiply(0.5)),
        );
        y += step;
    }
}

fn paint_mesh_from_bank(
    painter: &egui::Painter,
    layout: &MeshLayout,
    _bank: &WavetableBank,
    mesh: &MeshData,
    hover: Option<MeshHover>,
    accent_ui: Color32,
    accent: Color32,
) {
    let hover_slice = hover.as_ref().map(|h| h.slice);
    let hover_rib = hover.as_ref().and_then(|h| h.rib);
    const RIB_ALPHA: f32 = 0.22;

    for rib in 0..=RIB_COUNT {
        let t = rib as f32 / RIB_COUNT as f32;
        let rib_hovered = hover_rib == Some(rib);
        for window in mesh.slices.windows(2) {
            if let [a, b] = window {
                if a.points.is_empty() || b.points.is_empty() {
                    continue;
                }
                let ia = ((a.points.len() - 1) as f32 * t).round() as usize;
                let ib = ((b.points.len() - 1) as f32 * t).round() as usize;
                let pa = a.points[ia.min(a.points.len() - 1)];
                let pb = b.points[ib.min(b.points.len() - 1)];
                let alpha = if rib_hovered { 0.75 } else { RIB_ALPHA };
                let width = if rib_hovered { 1.1_f32 } else { 0.75_f32 };
                painter.line_segment(
                    [pa, pb],
                    egui::Stroke::new(width, accent_ui.gamma_multiply(alpha)),
                );
            }
        }
    }

    for (s, slice) in mesh.slices.iter().enumerate() {
        let points = &slice.points;
        if points.len() < 2 {
            continue;
        }
        let depth = (s as f32 / NUM_SLICES as f32 - 0.5).abs();
        let alpha = (1.0 - depth * 1.5).clamp(0.2, 1.0);
        let is_active = s == layout.half;
        let is_hovered = hover_slice == Some(s);
        let breathing = layout.hover_slice == Some(s) && layout.hover_breath > 0.01;
        let color = if is_active {
            accent
        } else if is_hovered || breathing {
            accent_ui.gamma_multiply((alpha + 0.4).min(1.0))
        } else {
            accent_ui.gamma_multiply(alpha)
        };
        let width_stroke = if is_active {
            2.0_f32
        } else if is_hovered || breathing {
            2.0_f32
        } else {
            1.0_f32
        };
        painter.add(Shape::line(
            points.clone(),
            egui::Stroke::new(width_stroke, color),
        ));
    }
}

fn paint_placeholder_mesh(painter: &egui::Painter, rect: Rect, accent_ui: Color32) {
    for i in 0..10 {
        let t = i as f32 / 9.0;
        let y_off = t * rect.height() * 0.32;
        let x_off = (t - 0.5) * rect.width() * 0.22;
        let points: Vec<Pos2> = (0..=40)
            .map(|j| {
                let u = j as f32 / 40.0;
                let x = rect.min.x + x_off + u * rect.width() * 0.78;
                let y = rect.center().y + y_off
                    + (u * std::f32::consts::TAU * 2.0 + t * 2.0).sin()
                        * rect.height()
                        * 0.18;
                Pos2::new(x, y)
            })
            .collect();
        let alpha = 0.35 + t * 0.45;
        painter.add(Shape::line(
            points,
            egui::Stroke::new(1.0_f32, accent_ui.gamma_multiply(alpha)),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Rect;

    #[test]
    fn position_from_mesh_x_endpoints() {
        let inner = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 100.0));
        let layout = MeshLayout::new(inner, None, 0.0);
        assert!((position_from_mesh_x(&layout, layout.mesh_left, 256) - 0.0).abs() < 1e-4);
        let right = layout.mesh_left + layout.mesh_width;
        assert!((position_from_mesh_x(&layout, right, 256) - 255.0).abs() < 1e-4);
        assert!((position_from_mesh_x(&layout, inner.center().x, 256) - 127.5).abs() < 1.0);
    }

    #[test]
    fn hover_breath_expands_slice_width() {
        let inner = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 100.0));
        let slice = 12;
        let base = MeshLayout::new(inner, None, 0.0);
        let breath = MeshLayout::new(inner, Some(slice), 1.0);
        let (_, _, base_rect) = base.slice_geometry(slice);
        let (_, _, hover_rect) = breath.slice_geometry(slice);
        assert!(hover_rect.width() > base_rect.width());
        assert!(hover_rect.center().y < base_rect.center().y);
    }

    #[test]
    fn distance_to_segment_on_midpoint() {
        let d = distance_to_segment(
            Pos2::new(1.0, 1.0),
            Pos2::new(0.0, 0.0),
            Pos2::new(2.0, 0.0),
        );
        assert!((d - 1.0).abs() < 1e-4);
    }

    #[test]
    fn nearest_mesh_target_picks_closest_slice() {
        let inner = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 100.0));
        let layout = MeshLayout::new(inner, None, 0.0);
        let mesh = MeshData {
            center_frame: 64,
            slices: (0..NUM_SLICES)
                .map(|s| {
                    let (_, _, slice_rect) = layout.slice_geometry(s);
                    let y = slice_rect.center().y + s as f32 * 2.0;
                    MeshSlice {
                        frame_index: s,
                        points: vec![
                            Pos2::new(slice_rect.min.x, y),
                            Pos2::new(slice_rect.max.x, y),
                        ],
                    }
                })
                .collect(),
        };
        let (_, _, front_rect) = layout.slice_geometry(layout.half);
        let target_y = front_rect.center().y + layout.half as f32 * 2.0;
        let hover = nearest_mesh_target(
            &layout,
            Pos2::new(front_rect.center().x, target_y),
            &mesh,
        );
        assert_eq!(hover.slice, layout.half);
    }
}
