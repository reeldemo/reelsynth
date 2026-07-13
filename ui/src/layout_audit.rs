//! Rect overlap and bounds checks for shell layout regression tests.

use egui::Rect;

use crate::center_layout::CenterRegions;
use crate::layout::{embed_mod_fx_in_center, ShellLayout, ShellLayoutOptions, SPACE_SM};

const EPS: f32 = 0.5;

/// Stored in `egui::Context` temp data by `shell::rail` for tests.
pub fn rail_used_rect_id() -> egui::Id {
    egui::Id::new("reelsynth.audit.rail_used_rect")
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
    } else if !embed_mod_fx_in_center(options) {
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
    use crate::layout::{APP_HEIGHT_FULL, APP_MIN_WIDTH};
    use crate::state::ShellConfig;

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

        let embedded = embed_mod_fx_in_center(options);
        let regions = compute_center_regions(
            layout.center.shrink(SPACE_SM * layout.scale.ui()),
            &full_config(),
            layout.scale.ui(),
            embedded,
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
                                    embed_mod_fx_in_center(options),
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
}
