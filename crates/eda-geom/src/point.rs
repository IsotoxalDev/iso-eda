// SPDX-License-Identifier: GPL-3.0-or-later

use crate::unit::Unit;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    pub x: Unit,
    pub y: Unit,
}

impl Point {
    pub const ORIGIN: Point = Point { x: 0, y: 0 };

    pub fn new(x: Unit, y: Unit) -> Self {
        Self { x, y }
    }

    /// Saturating in both axes.
    pub fn translated(self, dx: Unit, dy: Unit) -> Self {
        Self {
            x: self.x.saturating_add(dx),
            y: self.y.saturating_add(dy),
        }
    }
}
