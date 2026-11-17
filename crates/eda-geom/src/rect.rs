// SPDX-License-Identifier: GPL-3.0-or-later

use crate::point::Point;
use crate::unit::Unit;

/// Axis-aligned. Invariant: `x0 <= x1 && y0 <= y1`, always, by
/// construction — there is no way to build an invalid `Rect`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rect {
    pub x0: Unit,
    pub y0: Unit,
    pub x1: Unit,
    pub y1: Unit,
}

impl Rect {
    /// Normalizes (swaps) so the invariant holds; never panics.
    pub fn new(x0: Unit, y0: Unit, x1: Unit, y1: Unit) -> Self {
        let (x0, x1) = if x0 <= x1 { (x0, x1) } else { (x1, x0) };
        let (y0, y1) = if y0 <= y1 { (y0, y1) } else { (y1, y0) };
        Self { x0, y0, x1, y1 }
    }

    /// `Rect::new` from two opposite corners in either order.
    pub fn from_points(a: Point, b: Point) -> Self {
        Self::new(a.x, a.y, b.x, b.y)
    }

    /// `x1 - x0`, always >= 0.
    pub fn width(&self) -> Unit {
        self.x1.saturating_sub(self.x0)
    }

    /// `y1 - y0`, always >= 0.
    pub fn height(&self) -> Unit {
        self.y1.saturating_sub(self.y0)
    }

    /// Smallest rect containing both inputs.
    pub fn union(&self, other: &Rect) -> Rect {
        Rect {
            x0: self.x0.min(other.x0),
            y0: self.y0.min(other.y0),
            x1: self.x1.max(other.x1),
            y1: self.y1.max(other.y1),
        }
    }

    /// Edge-inclusive: a point on the boundary counts as contained.
    pub fn contains_point(&self, p: Point) -> bool {
        p.x >= self.x0 && p.x <= self.x1 && p.y >= self.y0 && p.y <= self.y1
    }

    /// Edge-inclusive: rects that only touch at an edge/corner intersect.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x0 <= other.x1 && other.x0 <= self.x1 && self.y0 <= other.y1 && other.y0 <= self.y1
    }

    pub fn translated(&self, dx: Unit, dy: Unit) -> Rect {
        Rect {
            x0: self.x0.saturating_add(dx),
            y0: self.y0.saturating_add(dy),
            x1: self.x1.saturating_add(dx),
            y1: self.y1.saturating_add(dy),
        }
    }
}
