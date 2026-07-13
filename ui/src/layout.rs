//! Fixed grid layout constants — synced with `brand/mockups/COMPONENT_SPEC.md`.

use egui::Rect;

pub const GRID_UNIT: f32 = 8.0;
pub const SPACE_SM: f32 = 8.0;
pub const SPACE_MD: f32 = 16.0;

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
pub const APP_HEIGHT_FULL: f32 = 820.0;

pub const HEADER_HEIGHT: f32 = 40.0;
pub const FOOTER_HEIGHT: f32 = 28.0;
pub const OSC_COLUMN_WIDTH: f32 = 252.0;
pub const RAIL_WIDTH: f32 = 216.0;

pub const KNOB_SM: f32 = 40.0;
pub const KNOB_MD: f32 = 44.0;
pub const KNOB_LG: f32 = 52.0;
pub const KNOB_COL_WIDTH: f32 = 56.0;

pub const WT_STRIP_HEIGHT: f32 = 60.0;
pub const WT_MORPH_HEIGHT: f32 = 24.0;
pub const WT_TOOLBAR_HEIGHT: f32 = 24.0;
pub const WT_VIEW_MIN_HEIGHT: f32 = 128.0;

pub const PIANO_HEIGHT: f32 = 80.0;
pub const PIANO_WHITE_KEY_WIDTH: f32 = 18.0;
pub const PIANO_BLACK_WIDTH_RATIO: f32 = 0.58;
pub const PIANO_BLACK_HEIGHT_RATIO: f32 = 0.56;
pub const PIANO_OCTAVES: usize = 2;
pub const PIANO_START_NOTE: u8 = 48; // C3

pub const MOD_MATRIX_HEIGHT: f32 = 136.0;
pub const FX_RACK_HEIGHT: f32 = 104.0;
pub const SECTION_HEADER_HEIGHT: f32 = 24.0;

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
        let piano_wrap_h = if options.piano_visible {
            GRID_UNIT * 2.0 + PIANO_HEIGHT
        } else {
            0.0
        };

        let mod_h = if options.show_mod_matrix {
            if options.mod_matrix_open {
                MOD_MATRIX_HEIGHT
            } else {
                SECTION_HEADER_HEIGHT
            }
        } else {
            0.0
        };

        let fx_h = if options.show_fx_rack {
            if options.fx_rack_open {
                FX_RACK_HEIGHT
            } else {
                SECTION_HEADER_HEIGHT
            }
        } else {
            0.0
        };

        let header = Rect::from_min_size(
            screen.min,
            egui::vec2(screen.width(), HEADER_HEIGHT),
        );

        let footer = Rect::from_min_size(
            egui::pos2(screen.min.x, screen.max.y - FOOTER_HEIGHT),
            egui::vec2(screen.width(), FOOTER_HEIGHT),
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
    fn s4_s5_full_layout_sections() {
        let screen = Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(1280.0, 820.0));
        let layout = ShellLayout::compute_with_options(
            screen,
            ShellLayoutOptions {
                piano_visible: true,
                show_osc_column: true,
                show_mod_matrix: true,
                mod_matrix_open: true,
                show_fx_rack: true,
                fx_rack_open: true,
            },
        );
        assert_eq!(layout.mod_matrix.height(), MOD_MATRIX_HEIGHT);
        assert_eq!(layout.fx_rack.height(), FX_RACK_HEIGHT);
        assert_eq!(layout.mod_matrix.min.y, layout.main.max.y);
        assert_eq!(layout.fx_rack.min.y, layout.mod_matrix.max.y);
        assert_eq!(layout.piano_wrap.min.y, layout.fx_rack.max.y);
        assert_eq!(layout.footer.max.y, screen.max.y);
    }
}
