//! Mouse-wheel zoom / pan for Design WT curve previews (Result / Layers / Selected).

use egui::{Pos2, Rect, Ui, Vec2};

/// Screen-space zoom around the plot center, with pan offset in pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WtCurveViewTransform {
    /// 1.0 = fit full cycle; larger zooms into the curve.
    pub zoom: f32,
    pub pan: Vec2,
}

impl Default for WtCurveViewTransform {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: Vec2::ZERO,
        }
    }
}

impl WtCurveViewTransform {
    pub const MIN_ZOOM: f32 = 1.0;
    pub const MAX_ZOOM: f32 = 16.0;

    /// Map a point painted in unzoomed plot space into zoomed screen space.
    pub fn map_pos(self, p: Pos2, inner: Rect) -> Pos2 {
        let c = inner.center();
        Pos2::new(
            c.x + (p.x - c.x) * self.zoom + self.pan.x,
            c.y + (p.y - c.y) * self.zoom + self.pan.y,
        )
    }

    /// Inverse of [`map_pos`] — for hit-testing with a screen pointer.
    pub fn unmap_pos(self, p: Pos2, inner: Rect) -> Pos2 {
        let z = self.zoom.max(1e-3);
        let c = inner.center();
        Pos2::new(
            c.x + (p.x - c.x - self.pan.x) / z,
            c.y + (p.y - c.y - self.pan.y) / z,
        )
    }

    pub fn map_points(self, pts: &[Pos2], inner: Rect) -> Vec<Pos2> {
        pts.iter().map(|p| self.map_pos(*p, inner)).collect()
    }

    /// Hit radius in unzoomed space so on-screen grab size stays ~`screen_px`.
    pub fn hit_radius(self, screen_px: f32) -> f32 {
        screen_px / self.zoom.max(1e-3)
    }

    /// Zoom toward `pointer` (keeps that screen point stable). Returns true if changed.
    pub fn apply_wheel_zoom(&mut self, scroll_y: f32, pointer: Pos2, inner: Rect) -> bool {
        if scroll_y.abs() < f32::EPSILON {
            return false;
        }
        let old = self.zoom;
        let factor = if scroll_y > 0.0 { 1.12 } else { 1.0 / 1.12 };
        let new = (old * factor).clamp(Self::MIN_ZOOM, Self::MAX_ZOOM);
        if (new - old).abs() < 1e-4 {
            return false;
        }
        let c = inner.center();
        // Keep `pointer` fixed: pan' = pan + (pointer - c) * (old - new)
        self.pan += (pointer.to_vec2() - c.to_vec2()) * (old - new);
        if new <= Self::MIN_ZOOM + 1e-3 {
            self.zoom = Self::MIN_ZOOM;
            self.pan = Vec2::ZERO;
        } else {
            self.zoom = new;
        }
        true
    }

    pub fn apply_pan(&mut self, delta: Vec2) -> bool {
        if delta == Vec2::ZERO {
            return false;
        }
        if self.zoom <= Self::MIN_ZOOM + 1e-3 {
            return false;
        }
        self.pan += delta;
        true
    }
}

/// When the pointer is over `hover_rect`, consume wheel: zoom (default) or Shift/horizontal pan.
///
/// Returns true when the view transform changed.
pub fn consume_plot_scroll(
    ui: &Ui,
    hover_rect: Rect,
    view: &mut WtCurveViewTransform,
) -> bool {
    let Some(pointer) = ui.input(|i| i.pointer.hover_pos()).filter(|p| hover_rect.contains(*p))
    else {
        return false;
    };

    let (scroll, shift) = ui.input(|i| (i.smooth_scroll_delta, i.modifiers.shift));
    if scroll == Vec2::ZERO {
        return false;
    }

    // Consume so parent panes / scroll areas do not steal the gesture.
    ui.ctx().input_mut(|i| {
        i.smooth_scroll_delta = Vec2::ZERO;
    });

    if shift || scroll.x.abs() > scroll.y.abs() * 1.25 {
        view.apply_pan(Vec2::new(scroll.x, scroll.y))
    } else {
        view.apply_wheel_zoom(scroll.y, pointer, hover_rect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_unmap_roundtrip_at_zoom() {
        let inner = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(200.0, 100.0));
        let mut view = WtCurveViewTransform {
            zoom: 2.0,
            pan: Vec2::new(10.0, -5.0),
        };
        let p = Pos2::new(80.0, 40.0);
        let mapped = view.map_pos(p, inner);
        let back = view.unmap_pos(mapped, inner);
        assert!((back.x - p.x).abs() < 1e-3);
        assert!((back.y - p.y).abs() < 1e-3);

        let changed = view.apply_wheel_zoom(10.0, inner.center(), inner);
        assert!(changed);
        assert!(view.zoom > 2.0);
    }

    #[test]
    fn zoom_out_to_min_resets_pan() {
        let inner = Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(100.0, 100.0));
        let mut view = WtCurveViewTransform {
            zoom: 1.2,
            pan: Vec2::new(20.0, 10.0),
        };
        // Scroll out repeatedly
        for _ in 0..20 {
            view.apply_wheel_zoom(-20.0, Pos2::new(50.0, 50.0), inner);
        }
        assert!((view.zoom - 1.0).abs() < 1e-3);
        assert_eq!(view.pan, Vec2::ZERO);
    }

    #[test]
    fn hit_radius_shrinks_with_zoom() {
        let view = WtCurveViewTransform {
            zoom: 2.0,
            pan: Vec2::ZERO,
        };
        assert!((view.hit_radius(14.0) - 7.0).abs() < 1e-4);
    }
}
