//! Device geometry: millimetre zones from the config mapped onto touchpad coordinates.

use crate::config::{Config, Sides};
use crate::triggers::{Corner, Edge};

#[derive(Clone, Debug)]
pub struct Geometry {
    pub xmin: i32,
    pub ymin: i32,
    /// Logical units per millimetre.
    pub ux: f64,
    pub uy: f64,
    pub width_mm: f64,
    pub height_mm: f64,
    pub area: Sides,
    pub side: Sides,
    pub corner_w: f64,
    pub corner_h: f64,
}

/// Shrink opposite bands proportionally so the centre keeps at least half of the span.
fn clamp_pair(a: f64, b: f64, span: f64) -> (f64, f64) {
    let limit = span * 0.5;
    let sum = a + b;
    if sum <= limit || sum <= 0.0 {
        (a, b)
    } else {
        (a * limit / sum, b * limit / sum)
    }
}

fn clamp_sides(s: Sides, w: f64, h: f64) -> Sides {
    let (top, bottom) = clamp_pair(s.top.max(0.0), s.bottom.max(0.0), h);
    let (left, right) = clamp_pair(s.left.max(0.0), s.right.max(0.0), w);
    Sides { enabled: s.enabled, top, right, bottom, left }
}

impl Geometry {
    pub fn new(xmin: i32, xmax: i32, ymin: i32, ymax: i32, size_mm: (f64, f64), cfg: &Config) -> Self {
        let (w, h) = size_mm;
        let ux = (xmax - xmin).max(1) as f64 / w;
        let uy = (ymax - ymin).max(1) as f64 / h;
        Self {
            xmin,
            ymin,
            ux,
            uy,
            width_mm: w,
            height_mm: h,
            area: clamp_sides(cfg.area, w, h),
            side: clamp_sides(
                Sides {
                    enabled: cfg.side_drag.enabled,
                    top: cfg.side_drag.top,
                    right: cfg.side_drag.right,
                    bottom: cfg.side_drag.bottom,
                    left: cfg.side_drag.left,
                },
                w,
                h,
            ),
            corner_w: cfg.corners.width.min(w * 0.5),
            corner_h: cfg.corners.height.min(h * 0.5),
        }
    }

    pub fn mm(&self, x: i32, y: i32) -> (f64, f64) {
        ((x - self.xmin) as f64 / self.ux, (y - self.ymin) as f64 / self.uy)
    }

    /// Distance in mm between two device points.
    pub fn dist_mm(&self, a: (i32, i32), b: (i32, i32)) -> f64 {
        let dx = (a.0 - b.0) as f64 / self.ux;
        let dy = (a.1 - b.1) as f64 / self.uy;
        (dx * dx + dy * dy).sqrt()
    }

    pub fn corner_at(&self, x: i32, y: i32) -> Option<Corner> {
        let (mx, my) = self.mm(x, y);
        let left = mx < self.corner_w;
        let right = mx >= self.width_mm - self.corner_w;
        let top = my < self.corner_h;
        let bottom = my >= self.height_mm - self.corner_h;
        match (top, bottom, left, right) {
            (true, _, true, _) => Some(Corner::TopLeft),
            (true, _, _, true) => Some(Corner::TopRight),
            (_, true, true, _) => Some(Corner::BottomLeft),
            (_, true, _, true) => Some(Corner::BottomRight),
            _ => None,
        }
    }

    fn in_band(&self, bands: &Sides, edge: Edge, x: i32, y: i32) -> bool {
        let (mx, my) = self.mm(x, y);
        match edge {
            Edge::Top => bands.top > 0.0 && my < bands.top,
            Edge::Bottom => bands.bottom > 0.0 && my >= self.height_mm - bands.bottom,
            Edge::Left => bands.left > 0.0 && mx < bands.left,
            Edge::Right => bands.right > 0.0 && mx >= self.width_mm - bands.right,
        }
    }

    /// Inside the shared area band of that side.
    pub fn in_area(&self, edge: Edge, x: i32, y: i32) -> bool {
        self.in_band(&self.area, edge, x, y)
    }

    /// Inside the thin side-drag band of that side.
    pub fn in_side(&self, edge: Edge, x: i32, y: i32) -> bool {
        self.in_band(&self.side, edge, x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geo() -> Geometry {
        // The user's touchpad: 3996 x 2242 units at 31 units/mm
        Geometry::new(0, 3996, 0, 2242, (3996.0 / 31.0, 2242.0 / 31.0), &Config::default())
    }

    #[test]
    fn corners_match_numberpad_zones() {
        let g = geo();
        assert_eq!(g.corner_at(3900, 100), Some(Corner::TopRight));
        assert_eq!(g.corner_at(100, 100), Some(Corner::TopLeft));
        assert_eq!(g.corner_at(2000, 100), None);
        assert_eq!(g.corner_at(100, 2200), Some(Corner::BottomLeft));
    }

    #[test]
    fn bands() {
        let g = geo();
        assert!(g.in_area(Edge::Right, 3500, 1500)); // 27 mm right band starts at x ~ 3159
        assert!(!g.in_area(Edge::Right, 3000, 1500));
        assert!(!g.in_area(Edge::Bottom, 2000, 2200)); // bottom band disabled by default
        assert!(g.in_side(Edge::Top, 2000, 100)); // 4 mm = 124 units
        assert!(!g.in_side(Edge::Top, 2000, 200));
    }

    #[test]
    fn centre_keeps_half() {
        let mut c = Config::default();
        c.area.top = 40.0;
        c.area.bottom = 40.0;
        let g = Geometry::new(0, 3996, 0, 2242, (128.9, 72.3), &c);
        assert!((g.area.top + g.area.bottom - 72.3 * 0.5).abs() < 1e-9);
    }
}
