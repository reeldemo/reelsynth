//! Fixed grid layout constants — synced with `brand/mockups/COMPONENT_SPEC.md`.

use egui::Rect;

pub const GRID_UNIT: f32 = 8.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 16.0;
/// Tighter vertical gap inside the center column stack.
pub const CENTER_GAP: f32 = 4.0;

pub const BUTTON_RADIUS: f32 = 6.0;
pub const BUTTON_PAD_X: f32 = 10.0;
pub const BUTTON_PAD_Y: f32 = 5.0;
pub const BUTTON_PAD_X_COMPACT: f32 = 7.0;
pub const BUTTON_PAD_Y_COMPACT: f32 = 3.0;
pub const BUTTON_FONT_SIZE: f32 = 11.0;
pub const BUTTON_FONT_SIZE_TOOL: f32 = 10.0;

/// `--radius-sm` in `brand/design/tokens.css`.
pub const RADIUS_SM: f32 = 10.0;
/// `--radius-md` in `brand/design/tokens.css`.
pub const RADIUS_MD: f32 = 16.0;

pub const APP_WIDTH: f32 = 1280.0;
pub const APP_HEIGHT_COMPACT: f32 = 720.0;
/// Default and minimum window height — full shell fits at scale 1.0 with autoscale in columns.
pub const APP_HEIGHT_FULL: f32 = 880.0;
pub const APP_MIN_WIDTH: f32 = APP_WIDTH;
pub const APP_MIN_HEIGHT: f32 = APP_HEIGHT_FULL;

/// Main-column content design height (rail with osc column at scale 1.0).
pub const MAIN_NEEDED_FULL: f32 = 620.0;
pub const MAIN_NEEDED_COMPACT: f32 = 200.0;

pub const HEADER_HEIGHT: f32 = 56.0;
pub const FOOTER_HEIGHT: f32 = 28.0;
pub const OSC_COLUMN_WIDTH: f32 = 252.0;
pub const RAIL_WIDTH: f32 = 216.0;

pub const KNOB_SM: f32 = 40.0;
pub const KNOB_MD: f32 = 44.0;
pub const KNOB_LG: f32 = 52.0;
pub const KNOB_COL_WIDTH: f32 = 56.0;

pub const WT_STRIP_HEIGHT: f32 = 48.0;
pub const WT_MORPH_HEIGHT: f32 = 20.0;
pub const WT_TOOLBAR_HEIGHT: f32 = 24.0;
pub const WT_VIEW_MIN_HEIGHT: f32 = 128.0;

pub const PIANO_HEIGHT: f32 = 128.0;
pub const PIANO_WHITE_KEY_WIDTH: f32 = 28.0;
pub const PIANO_BLACK_WIDTH_RATIO: f32 = 0.58;
pub const PIANO_BLACK_HEIGHT_RATIO: f32 = 0.56;
pub const PIANO_OCTAVES: usize = 3;
pub const PIANO_START_NOTE: u8 = 48; // C3

pub const MOD_MATRIX_HEIGHT: f32 = 120.0;
pub const MOD_ROW_HEIGHT: f32 = 22.0;
pub const FX_RACK_HEIGHT: f32 = 92.0;
pub const CENTER_MOD_HEIGHT: f32 = 108.0;
pub const CENTER_FX_HEIGHT: f32 = 88.0;
pub const SECTION_HEADER_HEIGHT: f32 = 24.0;

/// Minimum osc-column height reserved for scrollable oscillator params.
pub const OSC_COLUMN_MIN_SCROLL_HEIGHT: f32 = 112.0;

/// Minimum sidebar FX rack height for a 2-wide grid (header + padding + rows).
pub fn sidebar_fx_min_height(scale: f32, slot_count: usize) -> f32 {
    let s = scale;
    let header = SECTION_HEADER_HEIGHT * s;
    let padding = SPACE_SM * s * 2.0;
    let rows = (slot_count + 1).div_ceil(2).max(2);
    let row_h = 52.0 * s;
    let gap = GRID_UNIT * s * 0.5 * (rows.saturating_sub(1) as f32);
    header + padding + rows as f32 * row_h + gap
}

/// Split left column: osc scroll on top, FX grid + mod matrix stacked at bottom.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OscColumnHeights {
    pub osc: f32,
    pub fx: f32,
    pub mod_matrix: f32,
}

/// Minimum sidebar mod matrix height (header + a few rows).
pub fn sidebar_mod_min_height(scale: f32, route_rows: usize) -> f32 {
    let s = scale;
    let header = SECTION_HEADER_HEIGHT * s + SPACE_SM * s;
    let rows = route_rows.min(6).max(3);
    let row_h = MOD_ROW_HEIGHT * s;
    let gap = GRID_UNIT * s * 0.25 * (rows.saturating_sub(1) as f32);
    header + rows as f32 * row_h + gap + SPACE_SM * s
}

/// Split left column: scrollable osc on top; FX + mod matrix share the bottom band.
pub fn osc_column_split_heights(
    total_h: f32,
    scale: f32,
    slot_count: usize,
    show_fx: bool,
    show_mod: bool,
) -> OscColumnHeights {
    let min_osc = OSC_COLUMN_MIN_SCROLL_HEIGHT * scale;
    let mod_h = if show_mod {
        sidebar_mod_min_height(scale, 5).min(total_h * 0.38)
    } else {
        0.0
    };
    let fx_h = if show_fx {
        let min_fx = sidebar_fx_min_height(scale, slot_count);
        let budget = (total_h - min_osc - mod_h).max(min_fx);
        min_fx.max(budget * 0.55).min(budget)
    } else {
        0.0
    };
    let bottom = fx_h + mod_h;
    let osc_h = (total_h - bottom).max(min_osc);
    OscColumnHeights {
        osc: osc_h,
        fx: fx_h,
        mod_matrix: mod_h,
    }
}

/// Uniform UI scale derived from window and main-column budget.
#[derive(Debug, Clone, Copy)]
pub struct UiScale {
    pub screen: f32,
    pub main: f32,
}

impl UiScale {
    pub fn compute(screen_h: f32, main_h: f32, show_osc_column: bool) -> Self {
        let screen = (screen_h / APP_HEIGHT_FULL).clamp(0.72, 1.0);
        let needed = if show_osc_column {
            MAIN_NEEDED_FULL
        } else {
            MAIN_NEEDED_COMPACT
        };
        let main = (main_h / needed).clamp(0.72, 1.0);
        Self { screen, main }
    }

    /// Combined scale for knobs, rows, cards.
    pub fn ui(&self) -> f32 {
        (self.screen * self.main).clamp(0.72, 1.0)
    }

    pub fn px(&self, design: f32) -> f32 {
        design * self.ui()
    }
}

/// Layout options for the performance / full shell.
#[derive(Debug, Clone, Copy)]
pub struct ShellLayoutOptions {
    pub piano_visible: bool,
    pub show_osc_column: bool,
    pub show_mod_matrix: bool,
    pub mod_matrix_open: bool,
    pub show_fx_rack: bool,
    pub fx_rack_open: bool,
}

impl Default for ShellLayoutOptions {
    fn default() -> Self {
        Self {
            piano_visible: true,
            show_osc_column: false,
            show_mod_matrix: false,
            mod_matrix_open: true,
            show_fx_rack: false,
            fx_rack_open: true,
        }
    }
}

/// Computed rects for the performance shell.
#[derive(Debug, Clone, Copy)]
pub struct ShellLayout {
    pub header: Rect,
    pub main: Rect,
    pub osc: Rect,
    pub center: Rect,
    pub rail: Rect,
    pub mod_matrix: Rect,
    pub fx_rack: Rect,
    pub piano_wrap: Rect,
    pub footer: Rect,
    pub scale: UiScale,
}

/// Mod matrix + FX are never embedded in the center column (sidebars when osc visible).
pub fn embed_mod_fx_in_center(_options: ShellLayoutOptions) -> bool {
    false
}

/// FX rack lives in the left osc column when the osc column is visible.
pub fn embed_fx_in_osc_column(options: ShellLayoutOptions) -> bool {
    options.show_osc_column && options.show_fx_rack
}

/// Mod matrix lives in the left osc column when the osc column is visible.
pub fn embed_mod_in_osc_column(options: ShellLayoutOptions) -> bool {
    options.show_osc_column && options.show_mod_matrix
}

/// Deprecated alias — mod matrix is no longer in the right rail.
pub fn embed_mod_in_rail(options: ShellLayoutOptions) -> bool {
    embed_mod_in_osc_column(options)
}

/// Piano sits at the bottom of the center column when the osc column is visible.
pub fn embed_piano_in_center(options: ShellLayoutOptions) -> bool {
    options.piano_visible && options.show_osc_column
}

impl ShellLayout {
    pub fn compute(screen: Rect, piano_visible: bool) -> Self {
        Self::compute_with_options(
            screen,
            ShellLayoutOptions {
                piano_visible,
                ..Default::default()
            },
        )
    }

    /// S3+: reserve 280px osc column between main left edge and center.
    pub fn compute_with_osc(screen: Rect, piano_visible: bool) -> Self {
        Self::compute_with_options(
            screen,
            ShellLayoutOptions {
                piano_visible,
                show_osc_column: true,
                ..Default::default()
            },
        )
    }

    pub fn compute_with_options(screen: Rect, options: ShellLayoutOptions) -> Self {
        let screen_scale = (screen.height() / APP_HEIGHT_FULL).clamp(0.72, 1.0);

        let piano_wrap_h = if options.piano_visible && !embed_piano_in_center(options) {
            (GRID_UNIT + PIANO_HEIGHT) * screen_scale
        } else {
            0.0
        };

        let mod_h = if options.show_mod_matrix && !embed_mod_in_rail(options) {
            let base = if options.mod_matrix_open {
                MOD_MATRIX_HEIGHT
            } else {
                SECTION_HEADER_HEIGHT
            };
            base * screen_scale
        } else {
            0.0
        };

        let fx_h = if options.show_fx_rack && !embed_fx_in_osc_column(options) {
            let base = if options.fx_rack_open {
                FX_RACK_HEIGHT
            } else {
                SECTION_HEADER_HEIGHT
            };
            base * screen_scale
        } else {
            0.0
        };

        let header = Rect::from_min_size(
            screen.min,
            egui::vec2(screen.width(), HEADER_HEIGHT * screen_scale),
        );

        let footer = Rect::from_min_size(
            egui::pos2(screen.min.x, screen.max.y - FOOTER_HEIGHT * screen_scale),
            egui::vec2(screen.width(), FOOTER_HEIGHT * screen_scale),
        );

        let mut stack_top = footer.min.y;
        if options.piano_visible {
            stack_top -= piano_wrap_h;
        }
        if options.show_fx_rack {
            stack_top -= fx_h;
        }
        if options.show_mod_matrix {
            stack_top -= mod_h;
        }

        let main_top = header.max.y;
        let main_h = (stack_top - main_top).max(0.0);

        let main = Rect::from_min_size(
            egui::pos2(screen.min.x, main_top),
            egui::vec2(screen.width(), main_h),
        );

        let rail_w = RAIL_WIDTH.min(main.width());
        let osc_w = if options.show_osc_column {
            OSC_COLUMN_WIDTH.min((main.width() - rail_w).max(0.0))
        } else {
            0.0
        };
        let center_w = (main.width() - rail_w - osc_w).max(0.0);

        let osc = if options.show_osc_column && osc_w > 0.0 {
            Rect::from_min_size(main.min, egui::vec2(osc_w, main.height()))
        } else {
            Rect::NOTHING
        };

        let center = Rect::from_min_size(
            egui::pos2(main.min.x + osc_w, main.min.y),
            egui::vec2(center_w, main.height()),
        );
        let rail = Rect::from_min_size(
            egui::pos2(main.min.x + osc_w + center_w, main.min.y),
            egui::vec2(rail_w, main.height()),
        );

        let mut section_y = main.max.y;
        let mod_matrix = if options.show_mod_matrix {
            let r = Rect::from_min_size(
                egui::pos2(screen.min.x, section_y),
                egui::vec2(screen.width(), mod_h),
            );
            section_y += mod_h;
            r
        } else {
            Rect::NOTHING
        };

        let fx_rack = if options.show_fx_rack {
            let r = Rect::from_min_size(
                egui::pos2(screen.min.x, section_y),
                egui::vec2(screen.width(), fx_h),
            );
            section_y += fx_h;
            r
        } else {
            Rect::NOTHING
        };

        let piano_wrap = if options.piano_visible {
            Rect::from_min_size(
                egui::pos2(screen.min.x, section_y),
                egui::vec2(screen.width(), piano_wrap_h),
            )
        } else {
            Rect::NOTHING
        };

        let scale = UiScale::compute(screen.height(), main_h, options.show_osc_column);

        Self {
            header,
            main,
            osc,
            center,
            rail,
            mod_matrix,
            fx_rack,
            piano_wrap,
            footer,
            scale,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s1_layout_no_osc_column() {
        let screen = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 720.0));
        let layout = ShellLayout::compute(screen, true);
        assert!(!layout.osc.is_positive());
        assert_eq!(layout.rail.width(), RAIL_WIDTH);
        assert!((layout.center.width() - (1280.0 - RAIL_WIDTH)).abs() < 0.5);
    }

    #[test]
    fn s3_layout_with_osc_column() {
        let screen = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 720.0));
        let layout = ShellLayout::compute_with_osc(screen, true);
        assert_eq!(layout.osc.width(), OSC_COLUMN_WIDTH);
        assert_eq!(layout.rail.width(), RAIL_WIDTH);
        let used = layout.osc.width() + layout.center.width() + layout.rail.width();
        assert!((used - 1280.0).abs() < 0.5);
        assert_eq!(layout.osc.min.x, screen.min.x);
        assert_eq!(layout.center.min.x, screen.min.x + OSC_COLUMN_WIDTH);
        assert_eq!(layout.rail.min.x, screen.min.x + OSC_COLUMN_WIDTH + layout.center.width());
    }

    #[test]
    fn s4_s5_full_layout_sidebar_mod_fx() {
        let screen = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, APP_HEIGHT_FULL));
        let options = ShellLayoutOptions {
            piano_visible: true,
            show_osc_column: true,
            show_mod_matrix: true,
            mod_matrix_open: true,
            show_fx_rack: true,
            fx_rack_open: true,
        };
        let layout = ShellLayout::compute_with_options(screen, options);
        // Mod/FX in sidebars, piano in center — no full-width bottom strips.
        assert!(!layout.mod_matrix.is_positive());
        assert!(!layout.fx_rack.is_positive());
        assert!(!layout.piano_wrap.is_positive());
        assert!(embed_fx_in_osc_column(options));
        assert!(embed_mod_in_rail(options));
        assert!(embed_piano_in_center(options));
        assert!(layout.main.height() > 400.0);
        assert_eq!(layout.footer.max.y, screen.max.y);
    }

    #[test]
    fn min_window_no_overlap() {
        let screen = Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(APP_MIN_WIDTH, APP_MIN_HEIGHT),
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
        crate::layout_audit::audit_shell(&layout, screen, options);
        assert!(layout.main.height() > 100.0);
    }

    #[test]
    fn osc_column_split_reserves_fx_at_bottom() {
        let total = 796.0;
        let stack = osc_column_split_heights(total, 1.0, 3, true, false);
        assert!(stack.fx >= sidebar_fx_min_height(1.0, 3));
        assert_eq!(stack.mod_matrix, 0.0);
        assert!((stack.osc + stack.fx - total).abs() < 0.5);
        assert!(stack.fx > stack.osc * 0.45);
    }

    #[test]
    fn compact_layout_keeps_bottom_mod_fx() {
        let screen = Rect::from_min_size(
            egui::pos2(0.0, 0.0),
            egui::vec2(APP_MIN_WIDTH, APP_MIN_HEIGHT),
        );
        let options = ShellLayoutOptions {
            piano_visible: true,
            show_osc_column: false,
            show_mod_matrix: true,
            mod_matrix_open: true,
            show_fx_rack: true,
            fx_rack_open: true,
        };
        let layout = ShellLayout::compute_with_options(screen, options);
        crate::layout_audit::audit_shell(&layout, screen, options);
        assert!(layout.mod_matrix.is_positive());
        assert!(layout.fx_rack.is_positive());
    }
}
