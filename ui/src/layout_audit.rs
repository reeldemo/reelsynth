//! Rect overlap and bounds checks for shell layout regression tests.

use egui::Rect;

use crate::center_layout::CenterRegions;
use crate::layout::{
    embed_fx_in_osc_column, embed_mod_in_rail, ShellLayout, ShellLayoutOptions, SPACE_SM,
};

const EPS: f32 = 0.5;

/// Stored in `egui::Context` temp data by `shell::rail` for tests.
pub fn rail_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.rail_used_rect")
}

pub fn header_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.header_used_rect")
}

pub fn header_left_cluster_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.header_left_cluster_rect")
}

pub fn header_right_cluster_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.header_right_cluster_rect")
}

/// Minimum horizontal gap between left and right header control clusters at default width.
pub const HEADER_CLUSTER_MIN_GAP: f32 = 4.0;

pub fn osc_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.osc_used_rect")
}

pub fn center_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.center_used_rect")
}

pub fn piano_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.piano_used_rect")
}

pub fn footer_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.footer_used_rect")
}

pub fn center_scope_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.center.scope_used_rect")
}

pub fn center_strip_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.center.strip_used_rect")
}

pub fn center_morph_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.center.morph_used_rect")
}

pub fn center_views_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.center.views_used_rect")
}

#[allow(dead_code)] // reserved for center mod/fx strip audits
pub fn center_mod_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.center.mod_used_rect")
}

#[allow(dead_code)]
pub fn center_fx_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.center.fx_used_rect")
}

pub fn center_piano_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.center.piano_used_rect")
}

pub fn mod_strip_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.mod_strip_used_rect")
}

pub fn fx_strip_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.fx_strip_used_rect")
}

pub fn osc_mod_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.osc_mod_used_rect")
}

pub fn osc_mod_allocated_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.osc_mod_allocated_rect")
}

pub fn rail_mod_used_rect_id() -> egui::Id {
    osc_mod_used_rect_id()
}

pub fn osc_fx_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.osc_fx_used_rect")
}

pub fn osc_fx_allocated_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.osc_fx_allocated_rect")
}

pub fn rail_filter_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.rail_filter_used_rect")
}

pub fn rail_filter_allocated_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.rail_filter_allocated_rect")
}

pub fn rail_mod_allocated_rect_id() -> egui::Id {
    osc_mod_allocated_rect_id()
}

/// Positive area of a rect (0 when empty or negative).
pub fn rect_area(rect: Rect) -> f32 {
    if !rect.is_positive() {
        0.0
    } else {
        rect.width() * rect.height()
    }
}

/// Fraction of `allocated` covered by `used` (intersection area / allocated area).
pub fn utilization(allocated: Rect, used: Rect) -> f32 {
    let alloc = rect_area(allocated);
    if alloc <= EPS {
        return 1.0;
    }
    rect_area(allocated.intersect(used)) / alloc
}

/// Fail when panel content uses less than `min_ratio` of its allocated bounds.
pub fn assert_min_utilization(label: &str, allocated: Rect, used: Rect, min_ratio: f32) {
    assert!(
        allocated.is_positive(),
        "{label}: allocated rect must be positive ({allocated:?})"
    );
    assert!(
        used.is_positive(),
        "{label}: used rect must be positive ({used:?})"
    );
    assert!(
        used.width() > EPS && used.height() > EPS,
        "{label}: used rect has no area ({used:?})"
    );
    let util = utilization(allocated, used);
    assert!(
        util >= min_ratio - 0.01,
        "{label}: utilization {:.1}% below minimum {:.0}% (allocated={allocated:?} used={used:?})",
        util * 100.0,
        min_ratio * 100.0,
    );
}

/// FX and mod matrix in the left column must not overlap and share the same width.
pub fn audit_osc_sidebar_stacks(ctx: &egui::Context) {
    ctx.data(|d| {
        if let (Some(fx), Some(mod_r)) = (
            d.get_temp::<Rect>(osc_fx_allocated_rect_id()),
            d.get_temp::<Rect>(osc_mod_allocated_rect_id()),
        ) {
            let overlap = overlap_area(fx, mod_r);
            assert!(
                overlap < 1.0,
                "osc fx overlaps mod matrix by {overlap:.1}px² (fx={fx:?} mod={mod_r:?})"
            );
            assert!(
                mod_r.min.y >= fx.max.y - EPS,
                "mod matrix must stack below FX (fx.max.y={} mod.min.y={})",
                fx.max.y,
                mod_r.min.y,
            );
            assert!(
                (mod_r.min.x - fx.min.x).abs() < 1.5,
                "FX and mod should align on left edge (fx.x={} mod.x={})",
                fx.min.x,
                mod_r.min.x,
            );
            assert!(
                (mod_r.width() - fx.width()).abs() < 2.0,
                "FX and mod should share column width (fx.w={} mod.w={})",
                fx.width(),
                mod_r.width(),
            );
        }
    });
}

/// Whitespace heuristics for embedded sidebar panels at default window size.
pub fn audit_panel_utilization(ctx: &egui::Context, min_ratio: f32) {
    ctx.data(|d| {
        if let (Some(allocated), Some(used)) = (
            d.get_temp::<Rect>(osc_fx_allocated_rect_id()),
            d.get_temp::<Rect>(osc_fx_used_rect_id()),
        ) {
            assert_min_utilization("osc fx sidebar", allocated, used, min_ratio);
        }
        if let (Some(allocated), Some(used)) = (
            d.get_temp::<Rect>(osc_mod_allocated_rect_id()),
            d.get_temp::<Rect>(osc_mod_used_rect_id()),
        ) {
            assert_min_utilization("osc mod matrix", allocated, used, min_ratio);
        }
        if let (Some(allocated), Some(used)) = (
            d.get_temp::<Rect>(rail_filter_allocated_rect_id()),
            d.get_temp::<Rect>(rail_filter_used_rect_id()),
        ) {
            assert_min_utilization("rail filter panel", allocated, used, min_ratio);
        }
    });
}

/// Positive overlap area between two rects (0 if adjacent or disjoint).
pub fn overlap_area(a: Rect, b: Rect) -> f32 {
    if !a.is_positive() || !b.is_positive() {
        return 0.0;
    }
    let left = a.min.x.max(b.min.x);
    let right = a.max.x.min(b.max.x);
    let top = a.min.y.max(b.min.y);
    let bottom = a.max.y.min(b.max.y);
    if left + EPS >= right || top + EPS >= bottom {
        return 0.0;
    }
    (right - left) * (bottom - top)
}

pub fn within_bounds(outer: Rect, inner: Rect) -> bool {
    if !inner.is_positive() {
        return true;
    }
    inner.min.x >= outer.min.x - EPS
        && inner.min.y >= outer.min.y - EPS
        && inner.max.x <= outer.max.x + EPS
        && inner.max.y <= outer.max.y + EPS
}

fn assert_no_overlap(label: &str, rects: &[(&str, Rect)]) {
    let active: Vec<_> = rects.iter().filter(|(_, r)| r.is_positive()).collect();
    for i in 0..active.len() {
        for j in (i + 1)..active.len() {
            let area = overlap_area(active[i].1, active[j].1);
            assert!(
                area <= EPS,
                "{label}: `{}` overlaps `{}` by {area:.1}px² ({:?} vs {:?})",
                active[i].0,
                active[j].0,
                active[i].1,
                active[j].1
            );
        }
    }
}

fn assert_within(label: &str, outer: Rect, inner: Rect, name: &str) {
    assert!(
        within_bounds(outer, inner),
        "{label}: `{name}` extends outside bounds ({inner:?} not in {outer:?})"
    );
}

/// Audit shell rects: no overlaps, all within screen, vertical stack order preserved.
pub fn audit_shell(layout: &ShellLayout, screen: Rect, options: ShellLayoutOptions) {
    let vertical = [
        ("header", layout.header),
        ("mod_matrix", layout.mod_matrix),
        ("fx_rack", layout.fx_rack),
        ("piano_wrap", layout.piano_wrap),
        ("footer", layout.footer),
    ];

    for (name, rect) in vertical {
        assert_within("shell", screen, rect, name);
    }
    assert_within("shell", screen, layout.main, "main");

    assert_no_overlap("shell vertical bands", &vertical);

    if layout.osc.is_positive() {
        assert_within("shell", layout.main, layout.osc, "osc");
        assert_within("shell", layout.main, layout.center, "center");
        assert_within("shell", layout.main, layout.rail, "rail");
        assert_no_overlap(
            "shell main row",
            &[
                ("osc", layout.osc),
                ("center", layout.center),
                ("rail", layout.rail),
            ],
        );
        let used = layout.osc.width() + layout.center.width() + layout.rail.width();
        assert!(
            (used - layout.main.width()).abs() < 1.0,
            "main row width mismatch: {used} vs {}",
            layout.main.width()
        );
    } else if layout.rail.is_positive() {
        assert_within("shell", layout.main, layout.center, "center");
        assert_within("shell", layout.main, layout.rail, "rail");
        assert_no_overlap(
            "shell main row",
            &[("center", layout.center), ("rail", layout.rail)],
        );
    }

    assert!(layout.header.max.y <= layout.main.min.y + EPS);
    assert!(layout.main.max.y <= layout.piano_wrap.min.y + EPS || !layout.piano_wrap.is_positive());
    if layout.piano_wrap.is_positive() {
        assert!(layout.piano_wrap.max.y <= layout.footer.min.y + EPS);
    } else if !embed_mod_in_rail(options) && !embed_fx_in_osc_column(options) {
        if layout.mod_matrix.is_positive() {
            assert!(layout.main.max.y <= layout.mod_matrix.min.y + EPS);
        }
        if layout.fx_rack.is_positive() {
            let above = if layout.mod_matrix.is_positive() {
                layout.mod_matrix.max.y
            } else {
                layout.main.max.y
            };
            assert!(above <= layout.fx_rack.min.y + EPS);
        }
    }
    assert!(layout.footer.max.y <= screen.max.y + EPS);
    assert!(layout.header.min.y >= screen.min.y - EPS);
}

/// Left and right header clusters must not overlap horizontally and should leave breathing room.
pub fn audit_header_clusters(ctx: &egui::Context, header: Rect) {
    const VERT_SLACK: f32 = 12.0;

    ctx.data(|d| {
        let (Some(left), Some(right)) = (
            d.get_temp::<Rect>(header_left_cluster_rect_id()),
            d.get_temp::<Rect>(header_right_cluster_rect_id()),
        ) else {
            panic!("header cluster rects not stored (left/right)");
        };

        for (name, cluster) in [("left cluster", left), ("right cluster", right)] {
            assert!(
                cluster.min.x >= header.min.x - EPS && cluster.max.x <= header.max.x + EPS,
                "header: `{name}` extends outside horizontal bounds ({cluster:?} not in {header:?})"
            );
            assert!(
                cluster.max.y <= header.max.y + VERT_SLACK,
                "header: `{name}` extends too far below header ({cluster:?} header={header:?})"
            );
        }

        let overlap = overlap_area(
            Rect::from_min_max(
                egui::pos2(left.min.x, header.min.y),
                egui::pos2(left.max.x, header.max.y),
            ),
            Rect::from_min_max(
                egui::pos2(right.min.x, header.min.y),
                egui::pos2(right.max.x, header.max.y),
            ),
        );
        assert!(
            overlap <= EPS,
            "header clusters overlap horizontally by {overlap:.1}px (left={left:?} right={right:?})"
        );

        let gap = right.min.x - left.max.x;
        assert!(
            gap >= HEADER_CLUSTER_MIN_GAP - EPS,
            "header clusters too close: gap={gap:.1}px (min={HEADER_CLUSTER_MIN_GAP}, left={left:?} right={right:?})"
        );
    });
}

/// Fail when `used` extends outside `allocated` (width and height).
pub fn assert_content_within(allocated: Rect, used: Rect, label: &str) {
    assert!(
        allocated.is_positive(),
        "{label}: allocated rect must be positive ({allocated:?})"
    );
    assert!(
        within_bounds(allocated, used),
        "{label}: content extends outside allocated bounds (used={used:?} allocated={allocated:?})"
    );
}

/// Left and right sidebars must share the same width when both visible.
pub fn assert_sidebar_width_parity(layout: &ShellLayout) {
    if !layout.osc.is_positive() || !layout.rail.is_positive() {
        return;
    }
    let diff = (layout.osc.width() - layout.rail.width()).abs();
    assert!(
        diff < 1.5,
        "sidebar width parity: osc={:.1}px rail={:.1}px (expected equal)",
        layout.osc.width(),
        layout.rail.width()
    );
}

/// High-level auditable shell regions (legacy + registry bridge).
#[derive(Debug, Clone, Copy)]
pub enum AuditElement {
    Header,
    Osc,
    Center,
    Rail,
    Footer,
    ModStrip,
    FxStrip,
    PianoWrap,
}

pub fn audit_element(ctx: &egui::Context, element: AuditElement, outer: Rect) {
    let (used_id, label) = match element {
        AuditElement::Header => (header_used_rect_id(), "header"),
        AuditElement::Osc => (osc_used_rect_id(), "osc"),
        AuditElement::Center => (center_used_rect_id(), "center"),
        AuditElement::Rail => (rail_used_rect_id(), "rail"),
        AuditElement::Footer => (footer_used_rect_id(), "footer"),
        AuditElement::ModStrip => (mod_strip_used_rect_id(), "mod_strip"),
        AuditElement::FxStrip => (fx_strip_used_rect_id(), "fx_strip"),
        AuditElement::PianoWrap => (piano_used_rect_id(), "piano_wrap"),
    };
    ctx.data(|d| {
        if let Some(used) = d.get_temp::<Rect>(used_id) {
            assert_content_within(outer, used, label);
        }
    });
}

/// Filter + envelope + LFO panels must fit inside the rail column.
pub fn audit_rail_panels(ctx: &egui::Context, rail_bounds: Rect) {
    const FRAME_SLACK: f32 = 8.0;
    ctx.data(|d| {
        if let Some(used) = d.get_temp::<Rect>(rail_used_rect_id()) {
            assert!(
                used.min.x >= rail_bounds.min.x - FRAME_SLACK
                    && used.min.y >= rail_bounds.min.y - FRAME_SLACK
                    && used.max.x <= rail_bounds.max.x + FRAME_SLACK
                    && used.max.y <= rail_bounds.max.y + FRAME_SLACK,
                "rail column: content extends outside allocated bounds (used={used:?} allocated={rail_bounds:?})"
            );
        }
        if let (Some(_allocated), Some(used)) = (
            d.get_temp::<Rect>(rail_filter_allocated_rect_id()),
            d.get_temp::<Rect>(rail_filter_used_rect_id()),
        ) {
            assert_content_within(rail_bounds, used, "rail filter");
        }
    });
}

/// Audit center-column sub-regions inside the shrunk inner bounds.
pub fn audit_center(
    center: Rect,
    regions: &CenterRegions,
    scale: f32,
) {
    let inner = center.shrink(SPACE_SM * scale);
    for (name, rect) in regions.named() {
        assert_within("center", inner, rect, name);
    }
    assert_no_overlap("center sections", &regions.named());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::center_layout::compute_center_regions;
    use crate::layout::{embed_piano_in_center, APP_HEIGHT_FULL, APP_MIN_WIDTH};
    use crate::state::ShellConfig;

    #[test]
    fn rect_utilization_full_coverage() {
        let allocated = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 50.0));
        let used = Rect::from_min_size(egui::pos2(10.0, 5.0), egui::vec2(80.0, 40.0));
        assert!((utilization(allocated, used) - 0.64).abs() < 0.01);
    }

    #[test]
    fn rect_utilization_empty_used() {
        let allocated = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(100.0, 50.0));
        assert_eq!(utilization(allocated, Rect::NOTHING), 0.0);
    }

    fn full_options() -> ShellLayoutOptions {
        ShellLayoutOptions {
            piano_visible: true,
            show_osc_column: true,
            show_mod_matrix: true,
            mod_matrix_open: true,
            show_fx_rack: true,
            fx_rack_open: true,
        }
    }

    fn full_config() -> ShellConfig {
        ShellConfig {
            show_wt_editor: true,
            show_osc_column: true,
            show_mod_matrix: true,
            show_fx_rack: true,
        }
    }

    #[test]
    fn overlap_area_disjoint() {
        let a = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(10.0, 10.0));
        let b = Rect::from_min_size(egui::pos2(10.0, 0.0), egui::vec2(10.0, 10.0));
        assert_eq!(overlap_area(a, b), 0.0);
    }

    #[test]
    fn audit_full_shell_min_window() {
        let screen = Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(APP_MIN_WIDTH, APP_HEIGHT_FULL),
        );
        let options = full_options();
        let layout = ShellLayout::compute_with_options(screen, options);
        audit_shell(&layout, screen, options);

        let regions = compute_center_regions(
            layout.center.shrink(SPACE_SM * layout.scale.ui()),
            &full_config(),
            layout.scale.ui(),
            embed_piano_in_center(options),
            false,
        );
        audit_center(layout.center, &regions, layout.scale.ui());
    }

    #[test]
    fn audit_shell_matrix() {
        let sizes = [
            (1280.0, 880.0),
            (1280.0, 720.0),
            (1440.0, 900.0),
            (1920.0, 1080.0),
            (1280.0, 1000.0),
        ];

        for &(w, h) in &sizes {
            let screen = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(w, h));
            for piano in [true, false] {
                for osc in [true, false] {
                    for mod_on in [true, false] {
                        for fx_on in [true, false] {
                            let options = ShellLayoutOptions {
                                piano_visible: piano,
                                show_osc_column: osc,
                                show_mod_matrix: mod_on,
                                mod_matrix_open: true,
                                show_fx_rack: fx_on,
                                fx_rack_open: true,
                            };
                            let layout = ShellLayout::compute_with_options(screen, options);
                            audit_shell(&layout, screen, options);

                            if osc {
                                let config = ShellConfig {
                                    show_wt_editor: true,
                                    show_osc_column: true,
                                    show_mod_matrix: mod_on,
                                    show_fx_rack: fx_on,
                                };
                                let inner =
                                    layout.center.shrink(SPACE_SM * layout.scale.ui());
                                let regions = compute_center_regions(
                                    inner,
                                    &config,
                                    layout.scale.ui(),
                                    embed_piano_in_center(options),
                                    false,
                                );
                                audit_center(layout.center, &regions, layout.scale.ui());
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn collapsed_mod_fx_still_stack() {
        let screen = Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(APP_MIN_WIDTH, APP_HEIGHT_FULL),
        );
        let options = ShellLayoutOptions {
            piano_visible: true,
            show_osc_column: false,
            show_mod_matrix: true,
            mod_matrix_open: false,
            show_fx_rack: true,
            fx_rack_open: false,
        };
        let layout = ShellLayout::compute_with_options(screen, options);
        audit_shell(&layout, screen, options);
        assert!(layout.mod_matrix.height() > 0.0);
        assert!(layout.fx_rack.height() > 0.0);
    }

    #[test]
    fn header_cluster_gap_disjoint() {
        let left = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(400.0, 56.0));
        let right = Rect::from_min_size(egui::pos2(410.0, 0.0), egui::vec2(200.0, 56.0));
        assert_eq!(overlap_area(left, right), 0.0);
        assert!(right.min.x - left.max.x >= HEADER_CLUSTER_MIN_GAP);
    }
}
