//! Center-column region allocation (scope, WT, piano — visualization focus).

use egui::Rect;

use crate::layout::{
    CENTER_GAP, PIANO_HEIGHT, WT_MORPH_HEIGHT, WT_STRIP_HEIGHT, WT_VIEW_MIN_HEIGHT,
};
use crate::scope_strip::SCOPE_STRIP_HEIGHT;
use crate::state::ShellConfig;

#[derive(Debug, Clone, Copy)]
pub struct CenterRegions {
    pub scope: Rect,
    pub wt_strip: Rect,
    pub morph: Rect,
    pub mod_matrix: Rect,
    pub fx_rack: Rect,
    pub wt_views: Rect,
    pub piano: Rect,
}

impl CenterRegions {
    pub fn named(&self) -> [(&str, Rect); 7] {
        [
            ("scope", self.scope),
            ("wt_strip", self.wt_strip),
            ("morph", self.morph),
            ("mod_matrix", self.mod_matrix),
            ("fx_rack", self.fx_rack),
            ("wt_views", self.wt_views),
            ("piano", self.piano),
        ]
    }
}

pub fn compute_center_regions(
    inner: Rect,
    config: &ShellConfig,
    scale: f32,
    piano_in_center: bool,
    layer_first_design: bool,
) -> CenterRegions {
    let gap = CENTER_GAP * scale;
    let scope_h = SCOPE_STRIP_HEIGHT * scale;
    let strip_h = WT_STRIP_HEIGHT * scale;
    let morph_line_h = if layer_first_design {
        0.0
    } else {
        WT_MORPH_HEIGHT * scale
    };

    let scope = rect_row(inner, inner.min.y, scope_h);
    let mut y = scope.max.y + gap;

    if config.show_osc_column {
        let wt_strip = rect_row(inner, y, strip_h);
        y = wt_strip.max.y + gap;

        let morph = if config.show_wt_editor && morph_line_h > EPS {
            let r = rect_row(inner, y, morph_line_h);
            y = r.max.y + gap;
            r
        } else {
            Rect::NOTHING
        };

        let remaining = (inner.max.y - y).max(0.0);
        let piano_h = if piano_in_center {
            PIANO_HEIGHT * scale
        } else {
            0.0
        };
        let gap_before_piano = if piano_in_center && remaining > piano_h + gap {
            gap
        } else {
            0.0
        };
        let views_budget = (remaining - piano_h - gap_before_piano).max(0.0);
        let views_min = if config.show_wt_editor {
            WT_VIEW_MIN_HEIGHT * scale * 0.5
        } else {
            0.0
        };
        let views_h = if config.show_wt_editor && views_budget > views_min {
            views_budget
        } else if config.show_wt_editor && remaining > piano_h + gap + views_min {
            remaining - piano_h - gap
        } else {
            0.0
        };

        let wt_views = if views_h > EPS {
            let r = rect_row(inner, y, views_h);
            y = r.max.y + gap_before_piano;
            r
        } else {
            Rect::NOTHING
        };

        let piano = if piano_in_center && piano_h > EPS {
            let remaining_h = (inner.max.y - y).max(0.0);
            if remaining_h > EPS {
                rect_row(inner, y, remaining_h.max(piano_h))
            } else {
                Rect::NOTHING
            }
        } else {
            Rect::NOTHING
        };

        CenterRegions {
            scope,
            wt_strip,
            morph,
            mod_matrix: Rect::NOTHING,
            fx_rack: Rect::NOTHING,
            wt_views,
            piano,
        }
    } else {
        // Compact (no osc column): pack scope -> views -> morph -> strip so WT views
        // expand into leftover height instead of leaving a dead black band.
        let mut y = scope.max.y + gap;
        let morph_h = if config.show_wt_editor {
            morph_line_h
        } else {
            0.0
        };
        let strip_needed = if config.show_wt_editor { strip_h } else { 0.0 };
        let chrome_below = if config.show_wt_editor {
            strip_needed
                + morph_h
                + gap
                + if morph_h > EPS { gap } else { 0.0 }
        } else {
            0.0
        };
        let remaining = (inner.max.y - y).max(0.0);
        let views_min = if config.show_wt_editor {
            WT_VIEW_MIN_HEIGHT * scale * 0.5
        } else {
            0.0
        };
        let views_budget = (remaining - chrome_below).max(0.0);
        let views_h = if config.show_wt_editor && views_budget > views_min {
            views_budget
        } else if config.show_wt_editor {
            views_min.min(remaining)
        } else {
            0.0
        };

        let wt_views = if views_h > EPS {
            let r = rect_row(inner, y, views_h);
            y = r.max.y + gap;
            r
        } else {
            Rect::NOTHING
        };

        let morph = if config.show_wt_editor && morph_h > EPS {
            let r = rect_row(inner, y, morph_h);
            y = r.max.y + gap;
            r
        } else {
            Rect::NOTHING
        };

        let wt_strip = if config.show_wt_editor && strip_needed > EPS {
            rect_row(inner, y, strip_needed)
        } else {
            Rect::NOTHING
        };

        CenterRegions {
            scope,
            wt_strip,
            morph,
            mod_matrix: Rect::NOTHING,
            fx_rack: Rect::NOTHING,
            wt_views,
            piano: Rect::NOTHING,
        }
    }
}

const EPS: f32 = 0.01;

fn rect_row(inner: Rect, y: f32, height: f32) -> Rect {
    if height <= EPS || y >= inner.max.y - EPS {
        return Rect::NOTHING;
    }
    Rect::from_min_max(
        egui::pos2(inner.min.x, y),
        egui::pos2(inner.max.x, (y + height).min(inner.max.y)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout_audit::{audit_center, overlap_area};
    use crate::layout::{
        APP_HEIGHT_FULL, APP_MIN_WIDTH, CENTER_GAP, ShellLayout, ShellLayoutOptions, SPACE_SM,
    };

    #[test]
    fn center_regions_no_overlap_at_min_window() {
        let screen = Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(APP_MIN_WIDTH, APP_HEIGHT_FULL),
        );
        let options = ShellLayoutOptions {
            piano_visible: true,
            show_osc_column: true,
            show_mod_matrix: true,
            mod_matrix_open: true,
            show_fx_rack: true,
            fx_rack_open: true,
        };
        let layout = ShellLayout::compute_with_options(screen, options);
        let config = ShellConfig {
            show_wt_editor: true,
            show_osc_column: true,
            show_mod_matrix: true,
            show_fx_rack: true,
        };
        let scale = layout.scale.ui();
        let inner = layout.center.shrink(SPACE_SM * scale);
        let regions = compute_center_regions(inner, &config, scale, false, false);

        for (name, rect) in regions.named() {
            if rect.is_positive() {
                assert!(
                    overlap_area(rect, inner) > 0.0 || within(rect, inner),
                    "{name} outside inner"
                );
            }
        }
        audit_center(layout.center, &regions, scale);
        assert!(!regions.piano.is_positive());
        assert!(regions.wt_views.is_positive());
        assert!(!regions.fx_rack.is_positive());
        assert!(!regions.mod_matrix.is_positive());
        assert!(layout.piano_wrap.is_positive());
        assert!(
            regions.wt_views.height() > layout.piano_wrap.height() * 0.5,
            "views should keep substantial height with full-width piano"
        );
    }

    #[test]
    fn compact_center_fills_views_without_dead_band() {
        let screen = Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(APP_MIN_WIDTH, APP_HEIGHT_FULL),
        );
        let options = ShellLayoutOptions {
            piano_visible: true,
            show_osc_column: false,
            show_mod_matrix: false,
            mod_matrix_open: false,
            show_fx_rack: false,
            fx_rack_open: false,
        };
        let layout = ShellLayout::compute_with_options(screen, options);
        let config = ShellConfig {
            show_wt_editor: true,
            show_osc_column: false,
            show_mod_matrix: false,
            show_fx_rack: false,
        };
        let scale = layout.scale.ui();
        let inner = layout.center.shrink(SPACE_SM * scale);
        let regions = compute_center_regions(inner, &config, scale, false, false);
        assert!(regions.scope.is_positive());
        assert!(regions.wt_views.is_positive());
        assert!(regions.wt_strip.is_positive());
        let gap_after_scope = regions.wt_views.min.y - regions.scope.max.y;
        assert!(
            gap_after_scope < CENTER_GAP * scale * 3.0 + 1.0,
            "dead band after scope too large: {gap_after_scope}"
        );
        let covered = regions.scope.height()
            + regions.wt_views.height()
            + regions.morph.height()
            + regions.wt_strip.height();
        let util = covered / inner.height().max(1.0);
        assert!(
            util > 0.85,
            "compact center should fill most of the column (util={util:.2})"
        );
    }

    fn within(rect: Rect, outer: Rect) -> bool {
        rect.min.x >= outer.min.x - 0.5
            && rect.min.y >= outer.min.y - 0.5
            && rect.max.x <= outer.max.x + 0.5
            && rect.max.y <= outer.max.y + 0.5
    }
}
