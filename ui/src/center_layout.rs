//! Center-column region allocation (scope, WT, mod matrix, FX).

use egui::Rect;

use crate::layout::{GRID_UNIT, WT_MORPH_HEIGHT, WT_STRIP_HEIGHT, WT_VIEW_MIN_HEIGHT};
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
}

impl CenterRegions {
    pub fn named(&self) -> [(&str, Rect); 6] {
        [
            ("scope", self.scope),
            ("wt_strip", self.wt_strip),
            ("morph", self.morph),
            ("mod_matrix", self.mod_matrix),
            ("fx_rack", self.fx_rack),
            ("wt_views", self.wt_views),
        ]
    }
}

pub fn compute_center_regions(
    inner: Rect,
    config: &ShellConfig,
    scale: f32,
    embedded: bool,
) -> CenterRegions {
    let scope_h = SCOPE_STRIP_HEIGHT * scale;
    let strip_h = WT_STRIP_HEIGHT * scale;
    let morph_line_h = WT_MORPH_HEIGHT * scale;
    let gap = GRID_UNIT * scale;

    let scope = rect_row(inner, inner.min.y, scope_h);
    let mut y = scope.max.y + gap;

    if config.show_osc_column {
        let wt_strip = rect_row(inner, y, strip_h);
        y = wt_strip.max.y + gap;

        let morph = if config.show_wt_editor {
            let r = rect_row(inner, y, morph_line_h);
            y = r.max.y + gap;
            r
        } else {
            Rect::NOTHING
        };

        if embedded {
            let remaining = (inner.max.y - y).max(0.0);
            let (preview_h, mod_h, fx_h) = embedded_heights(
                remaining,
                gap,
                scale,
                config.show_wt_editor,
                config.show_mod_matrix,
                config.show_fx_rack,
            );

            let wt_views = if preview_h > EPS {
                let r = rect_row(inner, y, preview_h);
                y = r.max.y + gap;
                r
            } else {
                Rect::NOTHING
            };

            let mod_matrix = if mod_h > EPS {
                let r = rect_row(inner, y, mod_h);
                y = r.max.y + gap;
                r
            } else {
                Rect::NOTHING
            };

            let fx_rack = if fx_h > EPS {
                rect_row(inner, y, fx_h.min((inner.max.y - y).max(0.0)))
            } else {
                Rect::NOTHING
            };

            CenterRegions {
                scope,
                wt_strip,
                morph,
                mod_matrix,
                fx_rack,
                wt_views,
            }
        } else {
            let wt_views = if config.show_wt_editor
                && y < inner.max.y - WT_VIEW_MIN_HEIGHT * scale * 0.5
            {
                Rect::from_min_max(egui::pos2(inner.min.x, y), inner.max)
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
            }
        }
    } else {
        let views_h = if config.show_wt_editor {
            (WT_VIEW_MIN_HEIGHT * scale + gap)
                .min((inner.height() - scope_h - gap - strip_h - gap)
                    .max(WT_VIEW_MIN_HEIGHT * scale * 0.5))
        } else {
            0.0
        };
        let morph_block_h = if config.show_wt_editor {
            morph_line_h + gap
        } else {
            0.0
        };

        let wt_views = if config.show_wt_editor && views_h > EPS {
            Rect::from_min_max(
                egui::pos2(inner.min.x, inner.max.y - views_h),
                inner.max,
            )
        } else {
            Rect::NOTHING
        };

        let morph = if config.show_wt_editor && morph_block_h > EPS {
            Rect::from_min_max(
                egui::pos2(inner.min.x, wt_views.min.y - morph_block_h),
                egui::pos2(inner.max.x, wt_views.min.y - gap),
            )
        } else {
            Rect::NOTHING
        };

        let strip_top = if config.show_wt_editor {
            morph.min.y - gap - strip_h
        } else {
            inner.max.y - strip_h
        };
        let wt_strip = Rect::from_min_max(
            egui::pos2(inner.min.x, strip_top.max(scope.max.y + gap)),
            egui::pos2(inner.max.x, strip_top + strip_h),
        );

        CenterRegions {
            scope,
            wt_strip,
            morph,
            mod_matrix: Rect::NOTHING,
            fx_rack: Rect::NOTHING,
            wt_views,
        }
    }
}

const EPS: f32 = 0.01;

/// Split remaining height among preview / mod / FX without overflow.
fn embedded_heights(
    remaining: f32,
    gap: f32,
    scale: f32,
    show_preview: bool,
    show_mod: bool,
    show_fx: bool,
) -> (f32, f32, f32) {
    let mut segments: Vec<(f32, f32)> = Vec::new(); // (min_h, weight)
    if show_preview {
        segments.push((32.0 * scale, 0.22));
    }
    if show_mod {
        segments.push((64.0 * scale, 0.52));
    }
    if show_fx {
        segments.push((48.0 * scale, 0.26));
    }

    let n = segments.len();
    if n == 0 || remaining <= EPS {
        return (0.0, 0.0, 0.0);
    }

    let gap_total = gap * (n.saturating_sub(1)) as f32;
    let budget = (remaining - gap_total).max(0.0);
    let heights = distribute_heights(budget, &segments);

    let mut preview_h = 0.0;
    let mut mod_h = 0.0;
    let mut fx_h = 0.0;
    let mut i = 0;
    if show_preview {
        preview_h = heights[i];
        i += 1;
    }
    if show_mod {
        mod_h = heights[i];
        i += 1;
    }
    if show_fx {
        fx_h = heights[i];
    }

    (preview_h, mod_h, fx_h)
}

fn distribute_heights(budget: f32, segments: &[(f32, f32)]) -> Vec<f32> {
    if segments.is_empty() || budget <= EPS {
        return vec![0.0; segments.len()];
    }

    let min_sum: f32 = segments.iter().map(|(m, _)| m).sum();
    let mut out: Vec<f32> = segments.iter().map(|(m, _)| *m).collect();

    if min_sum > budget {
        let s = budget / min_sum;
        for h in &mut out {
            *h *= s;
        }
        return out;
    }

    let spare = budget - min_sum;
    let weight_sum: f32 = segments.iter().map(|(_, w)| w).sum();
    if weight_sum > EPS {
        for ((_, weight), h) in segments.iter().zip(out.iter_mut()) {
            *h += spare * (*weight / weight_sum);
        }
    } else {
        let each = spare / segments.len() as f32;
        for h in &mut out {
            *h += each;
        }
    }
    out
}

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
    use crate::layout::{APP_HEIGHT_FULL, APP_MIN_WIDTH, ShellLayout, ShellLayoutOptions, SPACE_SM};

    #[test]
    fn embedded_heights_never_exceed_budget() {
        for remaining in (80..=600).step_by(17) {
            let (p, m, f) = embedded_heights(
                remaining as f32,
                8.0,
                1.0,
                true,
                true,
                true,
            );
            assert!(
                p + m + f + 16.0 <= remaining as f32 + 0.5,
                "overflow at remaining={remaining}: {p}+{m}+{f}"
            );
        }
    }

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
        let regions = compute_center_regions(inner, &config, scale, true);

        for (name, rect) in regions.named() {
            if rect.is_positive() {
                assert!(
                    overlap_area(rect, inner) > 0.0 || within(rect, inner),
                    "{name} outside inner"
                );
            }
        }
        audit_center(layout.center, &regions, scale);
    }

    fn within(rect: Rect, outer: Rect) -> bool {
        rect.min.x >= outer.min.x - 0.5
            && rect.min.y >= outer.min.y - 0.5
            && rect.max.x <= outer.max.x + 0.5
            && rect.max.y <= outer.max.y + 0.5
    }
}
