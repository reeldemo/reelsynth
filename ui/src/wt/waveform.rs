//! Shared waveform sampling for WT strip and views.

use egui::{Color32, Mesh, Pos2, Rect, Shape};

pub fn frame_index(position: f32, num_frames: usize) -> usize {
    if num_frames == 0 {
        return 0;
    }
    let max = num_frames.saturating_sub(1);
    (position.round() as usize).min(max)
}

pub fn waveform_points(frame: &[f32], inner: Rect, sample_count: usize, amp: f32) -> Vec<Pos2> {
    if frame.is_empty() || sample_count < 2 {
        return Vec::new();
    }
    let mid_y = inner.center().y;
    let step = (frame.len() / sample_count).max(1);
    let denom = sample_count.saturating_sub(1).max(1) as f32;
    frame
        .iter()
        .step_by(step)
        .take(sample_count)
        .enumerate()
        .map(|(i, sample)| {
            let t = i as f32 / denom;
            let x = egui::lerp(inner.min.x..=inner.max.x, t);
            let y = mid_y - sample * inner.height() * amp;
            Pos2::new(x, y)
        })
        .collect()
}

/// Area fill between a waveform polyline and the zero baseline.
///
/// egui's [`Shape::convex_polygon`] only tessellates convex shapes (fan from the
/// first vertex). Oscillating waveforms are non-convex, so that API produces
/// crossed triangles / a hull-like blob. This builds a per-segment strip mesh
/// so the fill follows the curve down to `baseline_y` without self-crossing.
pub fn waveform_fill_shape(points: &[Pos2], baseline_y: f32, fill: Color32) -> Option<Shape> {
    if points.len() < 2 {
        return None;
    }
    let segs = points.len() - 1;
    let mut mesh = Mesh::default();
    mesh.reserve_vertices(segs * 4);
    mesh.reserve_triangles(segs * 2);
    for window in points.windows(2) {
        let a = window[0];
        let b = window[1];
        // Degenerate / vertical segment — still fine for a thin strip.
        let a0 = Pos2::new(a.x, baseline_y);
        let b0 = Pos2::new(b.x, baseline_y);
        let i = mesh.vertices.len() as u32;
        mesh.colored_vertex(a, fill);
        mesh.colored_vertex(b, fill);
        mesh.colored_vertex(b0, fill);
        mesh.colored_vertex(a0, fill);
        mesh.add_triangle(i, i + 1, i + 2);
        mesh.add_triangle(i, i + 2, i + 3);
    }
    Some(Shape::mesh(mesh))
}

pub fn peak_point(points: &[Pos2]) -> Option<Pos2> {
    points
        .iter()
        .min_by(|a, b| a.y.partial_cmp(&b.y).unwrap())
        .copied()
}

/// Minimum pixel distance from `pos` to the waveform polyline.
pub fn nearest_waveform_distance(points: &[Pos2], pos: Pos2) -> f32 {
    if points.len() < 2 {
        return f32::INFINITY;
    }
    points
        .windows(2)
        .map(|seg| distance_point_to_segment(pos, seg[0], seg[1]))
        .fold(f32::INFINITY, f32::min)
}

/// True when `pos` is within `tolerance` px of the drawn waveform path.
pub fn hit_test_waveform(points: &[Pos2], pos: Pos2, tolerance: f32) -> bool {
    nearest_waveform_distance(points, pos) <= tolerance
}

/// Layer index whose polyline is nearest to `pos` within `max_dist` px.
///
/// Same nearest-wins rule as click-to-select on Design WT layer curves.
pub fn hovered_layer_from_pointer<'a>(
    layer_points: impl IntoIterator<Item = (usize, &'a [Pos2])>,
    pos: Pos2,
    max_dist: f32,
) -> Option<usize> {
    let mut best_idx = None;
    let mut best_dist = max_dist;
    for (idx, pts) in layer_points {
        let dist = nearest_waveform_distance(pts, pos);
        if dist < best_dist {
            best_dist = dist;
            best_idx = Some(idx);
        }
    }
    best_idx
}

/// Commit `selected_layer_idx` from a curve click on Result / Layers.
///
/// Quant knob proximity wins: when the pointer is on a knob, return `None`
/// so the click does not steal selection from knob editing.
pub fn selection_from_curve_click(
    hovered_curve: Option<usize>,
    over_quant_knob: bool,
) -> Option<usize> {
    if over_quant_knob {
        None
    } else {
        hovered_curve
    }
}

/// Layers that may show / edit Quant knobs: any enabled audible curve when Quant > 0.
///
/// VA layers (saw/sine/…) are promoted to wavetable on first Quant paint so knobs
/// can reshape a real frame; see [`crate::wt::promote_va_layer_for_quant`].
pub fn layer_quant_editable(layer: &crate::oscillator_ui::WaveLayerUi) -> bool {
    layer.enabled && layer.level > 0.0
}

/// Whether Design panes should expose Quant knobs for `selected` at `wave_quant`.
pub fn quant_knobs_for_selection(
    selected: Option<usize>,
    layers: &[crate::oscillator_ui::WaveLayerUi],
    wave_quant: u8,
) -> Option<usize> {
    if wave_quant == 0 {
        return None;
    }
    selected.filter(|&i| layers.get(i).is_some_and(layer_quant_editable))
}

/// Selected (right) pane shows Quant knobs whenever the selection is editable.
///
/// Independent of edit tool — Shape/Curve may own alternate drags, but knobs
/// must still be visible for WT/residual layers when Quant > 0.
pub fn selected_pane_shows_quant_knobs(
    selected: Option<usize>,
    layers: &[crate::oscillator_ui::WaveLayerUi],
    wave_quant: u8,
) -> bool {
    quant_knobs_for_selection(selected, layers, wave_quant).is_some()
}

/// Selected (right) pane: pointer is near the displayed layer curve and not on a knob.
pub fn selected_curve_hovered(
    points: &[Pos2],
    pos: Pos2,
    over_quant_knob: bool,
    max_dist: f32,
) -> bool {
    !over_quant_knob && hit_test_waveform(points, pos, max_dist)
}

/// Layers multi-curve pointer: prefer selecting a *different* hovered curve over
/// grabbing knobs on the currently selected Quant layer (knobs often overlap
/// siblings spatially and otherwise trap selection on the last WT / L3).
pub fn layers_pointer_prefers_curve_select(
    hovered_curve: Option<usize>,
    quant_layer: Option<usize>,
    over_quant_knob: bool,
) -> bool {
    match (hovered_curve, quant_layer) {
        (Some(curve), Some(q)) if curve != q => true,
        (Some(_), _) if !over_quant_knob => true,
        _ => false,
    }
}

fn distance_point_to_segment(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.x * ab.x + ab.y * ab.y;
    if len_sq <= f32::EPSILON {
        return p.distance(a);
    }
    let t = ((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let closest = Pos2::new(a.x + ab.x * t, a.y + ab.y * t);
    p.distance(closest)
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::{Color32, Rect, Shape};

    #[test]
    fn frame_index_clamps() {
        assert_eq!(frame_index(300.0, 64), 63);
        assert_eq!(frame_index(0.0, 0), 0);
    }

    #[test]
    fn waveform_fill_shape_none_for_short_polyline() {
        assert!(waveform_fill_shape(&[], 50.0, Color32::WHITE).is_none());
        assert!(waveform_fill_shape(&[Pos2::new(0.0, 10.0)], 50.0, Color32::WHITE).is_none());
    }

    #[test]
    fn waveform_points_bounds() {
        let frame: Vec<f32> = (0..256).map(|i| (i as f32 * 0.1).sin()).collect();
        let rect = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 80.0));
        let pts = waveform_points(&frame, rect, 32, 0.45);
        assert_eq!(pts.len(), 32);
        for p in &pts {
            assert!(p.x >= rect.min.x && p.x <= rect.max.x);
            assert!(p.y >= rect.min.y && p.y <= rect.max.y);
        }
    }

    #[test]
    fn waveform_fill_shape_strips_to_baseline_without_hull() {
        // Non-convex sine-like polyline that would look wrong with convex_polygon fan.
        let pts = vec![
            Pos2::new(0.0, 40.0),
            Pos2::new(25.0, 10.0),
            Pos2::new(50.0, 40.0),
            Pos2::new(75.0, 70.0),
            Pos2::new(100.0, 40.0),
        ];
        let baseline = 40.0;
        let shape = waveform_fill_shape(&pts, baseline, Color32::from_rgb(1, 2, 3)).unwrap();
        let Shape::Mesh(mesh) = shape else {
            panic!("expected mesh fill");
        };
        assert_eq!(mesh.indices.len() / 3, (pts.len() - 1) * 2);
        assert_eq!(mesh.vertices.len(), (pts.len() - 1) * 4);
        // Every other pair of verts is on the baseline (indices 2,3 of each quad).
        for quad in 0..(pts.len() - 1) {
            let base = quad * 4;
            assert!((mesh.vertices[base + 2].pos.y - baseline).abs() < 1e-4);
            assert!((mesh.vertices[base + 3].pos.y - baseline).abs() < 1e-4);
            assert_eq!(mesh.vertices[base].pos, pts[quad]);
            assert_eq!(mesh.vertices[base + 1].pos, pts[quad + 1]);
        }
        // No vertex jumps to plot corners at x=0/100 with wrong Y — fill stays under segments.
        for v in &mesh.vertices {
            assert!(v.pos.x >= 0.0 - 1e-3 && v.pos.x <= 100.0 + 1e-3);
        }
    }

    #[test]
    fn peak_point_finds_minimum_y() {
        let pts = vec![
            Pos2::new(0.0, 10.0),
            Pos2::new(1.0, 5.0),
            Pos2::new(2.0, 12.0),
        ];
        assert_eq!(peak_point(&pts).unwrap().y, 5.0);
    }

    #[test]
    fn hit_test_waveform_near_line() {
        let pts = vec![
            Pos2::new(0.0, 50.0),
            Pos2::new(100.0, 50.0),
        ];
        assert!(hit_test_waveform(&pts, Pos2::new(50.0, 50.0), 8.0));
        assert!(hit_test_waveform(&pts, Pos2::new(50.0, 55.0), 8.0));
        assert!(!hit_test_waveform(&pts, Pos2::new(50.0, 70.0), 8.0));
    }

    #[test]
    fn hovered_layer_from_pointer_picks_nearest_within_tolerance() {
        let a = vec![Pos2::new(0.0, 40.0), Pos2::new(100.0, 40.0)];
        let b = vec![Pos2::new(0.0, 60.0), Pos2::new(100.0, 60.0)];
        let layers = [(0usize, a.as_slice()), (1usize, b.as_slice())];

        assert_eq!(
            hovered_layer_from_pointer(layers, Pos2::new(50.0, 42.0), 14.0),
            Some(0)
        );
        assert_eq!(
            hovered_layer_from_pointer(layers, Pos2::new(50.0, 58.0), 14.0),
            Some(1)
        );
        assert_eq!(
            hovered_layer_from_pointer(layers, Pos2::new(50.0, 100.0), 14.0),
            None
        );
    }

    #[test]
    fn hovered_layer_from_pointer_nearest_wins_on_tie_break_distance() {
        let far = vec![Pos2::new(0.0, 20.0), Pos2::new(100.0, 20.0)];
        let near = vec![Pos2::new(0.0, 50.0), Pos2::new(100.0, 50.0)];
        let layers = [(3usize, far.as_slice()), (7usize, near.as_slice())];
        assert_eq!(
            hovered_layer_from_pointer(layers, Pos2::new(50.0, 52.0), 14.0),
            Some(7)
        );
    }

    #[test]
    fn selection_from_curve_click_commits_hover_unless_on_knob() {
        assert_eq!(selection_from_curve_click(Some(2), false), Some(2));
        assert_eq!(selection_from_curve_click(Some(2), true), None);
        assert_eq!(selection_from_curve_click(None, false), None);
        assert_eq!(selection_from_curve_click(None, true), None);
    }

    fn wt_layer(level: f32) -> crate::oscillator_ui::WaveLayerUi {
        crate::oscillator_ui::WaveLayerUi {
            source_type: "wavetable".into(),
            level,
            enabled: true,
            ..crate::oscillator_ui::WaveLayerUi::default()
        }
    }

    #[test]
    fn quant_knobs_for_any_audible_layer_including_va() {
        use crate::oscillator_ui::WaveLayerUi;

        let saw = WaveLayerUi {
            source_type: "saw".into(),
            level: 1.0,
            enabled: true,
            ..WaveLayerUi::default()
        };
        let wt = wt_layer(0.5);
        let residual = WaveLayerUi {
            source_type: "wavetable".into(),
            level: 1.0,
            enabled: true,
            residual: true,
            ..WaveLayerUi::default()
        };
        let muted_wt = wt_layer(0.0);
        assert!(layer_quant_editable(&saw));
        assert!(layer_quant_editable(&wt));
        assert!(layer_quant_editable(&residual));
        assert!(!layer_quant_editable(&muted_wt));

        let layers = vec![saw, wt, residual];
        assert_eq!(quant_knobs_for_selection(Some(0), &layers, 16), Some(0));
        assert_eq!(quant_knobs_for_selection(Some(1), &layers, 16), Some(1));
        assert_eq!(quant_knobs_for_selection(Some(2), &layers, 16), Some(2));
        assert_eq!(quant_knobs_for_selection(Some(1), &layers, 0), None);
        assert!(selected_pane_shows_quant_knobs(Some(0), &layers, 16));
        assert!(selected_pane_shows_quant_knobs(Some(1), &layers, 16));
        assert!(selected_pane_shows_quant_knobs(Some(2), &layers, 16));
    }

    /// Three WT layers: knobs must track selection (L1 / L2 / L3), not stick on last.
    #[test]
    fn quant_knobs_follow_selection_across_three_wt_layers() {
        let layers = vec![wt_layer(1.0), wt_layer(0.8), wt_layer(0.6)];
        assert_eq!(quant_knobs_for_selection(Some(0), &layers, 16), Some(0));
        assert_eq!(quant_knobs_for_selection(Some(1), &layers, 16), Some(1));
        assert_eq!(quant_knobs_for_selection(Some(2), &layers, 16), Some(2));
        assert!(selected_pane_shows_quant_knobs(Some(0), &layers, 16));
        assert!(selected_pane_shows_quant_knobs(Some(1), &layers, 16));
        assert!(selected_pane_shows_quant_knobs(Some(2), &layers, 16));
        assert_eq!(quant_knobs_for_selection(None, &layers, 16), None);
    }

    #[test]
    fn selected_curve_hovered_respects_quant_knob_priority() {
        let pts = vec![Pos2::new(0.0, 50.0), Pos2::new(100.0, 50.0)];
        let pos = Pos2::new(50.0, 52.0);
        assert!(selected_curve_hovered(&pts, pos, false, 14.0));
        assert!(!selected_curve_hovered(&pts, pos, true, 14.0));
        assert!(!selected_curve_hovered(&pts, Pos2::new(50.0, 80.0), false, 14.0));
    }

    #[test]
    fn layers_pointer_prefers_other_curve_over_knob_trap() {
        // Hovering L1 while knobs are on L3 → select L1 (do not stay trapped on L3 knobs).
        assert!(layers_pointer_prefers_curve_select(Some(0), Some(2), true));
        assert!(layers_pointer_prefers_curve_select(Some(1), Some(2), true));
        // Hovering the Quant layer itself near a knob → keep knob grab.
        assert!(!layers_pointer_prefers_curve_select(Some(2), Some(2), true));
        // No knob under pointer → curve select is fine.
        assert!(layers_pointer_prefers_curve_select(Some(0), Some(2), false));
        // Only knobs, no curve hover → do not force curve select.
        assert!(!layers_pointer_prefers_curve_select(None, Some(2), true));
    }
}
