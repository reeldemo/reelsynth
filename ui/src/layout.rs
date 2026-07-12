//! Fixed grid layout constants — synced with `brand/mockups/COMPONENT_SPEC.md`.

use egui::Rect;

pub const GRID_UNIT: f32 = 8.0;
pub const SPACE_SM: f32 = 12.0;
pub const SPACE_MD: f32 = 20.0;

pub const APP_WIDTH: f32 = 1280.0;
pub const APP_HEIGHT_S1: f32 = 720.0;
pub const APP_HEIGHT_FULL: f32 = 820.0;

pub const HEADER_HEIGHT: f32 = 48.0;
pub const FOOTER_HEIGHT: f32 = 36.0;
pub const OSC_COLUMN_WIDTH: f32 = 280.0;
pub const RAIL_WIDTH: f32 = 240.0;

pub const KNOB_SM: f32 = 48.0;
pub const KNOB_MD: f32 = 56.0;
pub const KNOB_LG: f32 = 64.0;

pub const WT_STRIP_HEIGHT: f32 = 72.0;

pub const PIANO_HEIGHT: f32 = 72.0;
pub const PIANO_WHITE_KEY_WIDTH: f32 = 15.0;
pub const PIANO_BLACK_WIDTH_RATIO: f32 = 0.58;
pub const PIANO_BLACK_HEIGHT_RATIO: f32 = 0.56;
pub const PIANO_OCTAVES: usize = 2;
pub const PIANO_START_NOTE: u8 = 48; // C3

pub const MOD_MATRIX_HEIGHT: f32 = 160.0;
pub const FX_RACK_HEIGHT: f32 = 120.0;

/// Computed rects for the S1 performance shell (no osc/mod/FX columns).
#[derive(Debug, Clone, Copy)]
pub struct S1Layout {
    pub header: Rect,
    pub main: Rect,
    pub center: Rect,
    pub rail: Rect,
    pub piano_wrap: Rect,
    pub footer: Rect,
}

impl S1Layout {
    pub fn compute(screen: Rect, piano_visible: bool) -> Self {
        let piano_wrap_h = if piano_visible {
            GRID_UNIT * 2.0 + PIANO_HEIGHT
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

        let main_top = header.max.y;
        let main_bottom = footer.min.y - piano_wrap_h;
        let main_h = (main_bottom - main_top).max(0.0);

        let main = Rect::from_min_size(
            egui::pos2(screen.min.x, main_top),
            egui::vec2(screen.width(), main_h),
        );

        let rail_w = RAIL_WIDTH.min(main.width());
        let center_w = (main.width() - rail_w).max(0.0);

        let center = Rect::from_min_size(main.min, egui::vec2(center_w, main.height()));
        let rail = Rect::from_min_size(
            egui::pos2(main.min.x + center_w, main.min.y),
            egui::vec2(rail_w, main.height()),
        );

        let piano_wrap = if piano_visible {
            Rect::from_min_size(
                egui::pos2(screen.min.x, footer.min.y - piano_wrap_h),
                egui::vec2(screen.width(), piano_wrap_h),
            )
        } else {
            Rect::NOTHING
        };

        Self {
            header,
            main,
            center,
            rail,
            piano_wrap,
            footer,
        }
    }
}
