// SPDX-License-Identifier: GPL-3.0-or-later

use crate::point::Point;
use crate::rect::Rect;
use crate::unit::Unit;

/// The 8-element dihedral group D4: four 90-degree rotations and their
/// four mirror-then-rotate combinations. Named, enumerable, and total —
/// this is deliberately the shape M1.2 needs to map xschem's `rot`
/// (0..=3 quarter turns) and `flip` (0/1) encoding onto; that mapping is
/// NOT decided here (see ADR-003, "Not solved yet" — `FlipR0` meaning
/// "negate x, then rotate 0" is this crate's own convention, not assumed
/// to match xschem's flip axis).
///
/// `apply_vector` is the linear part only (no translation):
/// - `R0(x,y) = (x,y)`
/// - `R90(x,y) = (-y,x)`
/// - `R180(x,y) = (-x,-y)`
/// - `R270(x,y) = (y,-x)`
/// - `FlipR0(x,y) = (-x,y)`
/// - `FlipR90(x,y) = (-y,-x)`
/// - `FlipR180(x,y) = (x,-y)`
/// - `FlipR270(x,y) = (y,x)`
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Orientation {
    #[default]
    R0,
    R90,
    R180,
    R270,
    FlipR0,
    FlipR90,
    FlipR180,
    FlipR270,
}

impl Orientation {
    /// Internal, canonical decomposition: `(k, mirrored)` where `k` is the
    /// number of 90-degree steps (0..4) applied after an optional mirror
    /// (negate x) step. Not public: `compose`/`inverse` are expressed in
    /// terms of this pair so the group arithmetic is in one place, instead
    /// of a hand-written 8x8 table.
    fn parts(self) -> (u8, bool) {
        match self {
            Orientation::R0 => (0, false),
            Orientation::R90 => (1, false),
            Orientation::R180 => (2, false),
            Orientation::R270 => (3, false),
            Orientation::FlipR0 => (0, true),
            Orientation::FlipR90 => (1, true),
            Orientation::FlipR180 => (2, true),
            Orientation::FlipR270 => (3, true),
        }
    }

    fn from_parts(k: u8, mirrored: bool) -> Orientation {
        match (k % 4, mirrored) {
            (0, false) => Orientation::R0,
            (1, false) => Orientation::R90,
            (2, false) => Orientation::R180,
            (3, false) => Orientation::R270,
            (0, true) => Orientation::FlipR0,
            (1, true) => Orientation::FlipR90,
            (2, true) => Orientation::FlipR180,
            (3, true) => Orientation::FlipR270,
            _ => unreachable!("k % 4 is always in 0..4"),
        }
    }

    pub fn apply_vector(self, x: Unit, y: Unit) -> (Unit, Unit) {
        let neg = |v: Unit| 0i32.saturating_sub(v);
        match self {
            Orientation::R0 => (x, y),
            Orientation::R90 => (neg(y), x),
            Orientation::R180 => (neg(x), neg(y)),
            Orientation::R270 => (y, neg(x)),
            Orientation::FlipR0 => (neg(x), y),
            Orientation::FlipR90 => (neg(y), neg(x)),
            Orientation::FlipR180 => (x, neg(y)),
            Orientation::FlipR270 => (y, x),
        }
    }

    /// Group composition: "apply `self`, then `next`" as one `Orientation`.
    /// `self.compose(next).apply_vector(v) == next.apply_vector(self.apply_vector(v))`
    /// (component-wise on the tuple `v`).
    pub fn compose(self, next: Orientation) -> Orientation {
        let (k1, s1) = self.parts();
        let (k2, s2) = next.parts();
        if !s2 {
            // next has no mirror: rotations add, mirror bit carries from self.
            Orientation::from_parts((k1 + k2) % 4, s1)
        } else {
            // next mirrors: mirroring reverses the sense of self's rotation
            // (reflect-then-rotate commutes with rotation only up to sign),
            // and the two mirror bits cancel.
            Orientation::from_parts((k2 + 4 - k1) % 4, !s1)
        }
    }

    /// The two-sided inverse under `compose`: `o.compose(o.inverse())
    /// == Orientation::R0`.
    pub fn inverse(self) -> Orientation {
        let (k, s) = self.parts();
        if s {
            // Every `FlipR*` variant is its own inverse (a reflection).
            self
        } else {
            Orientation::from_parts((4 - k) % 4, false)
        }
    }

    /// `true` for the four `FlipR*` variants.
    pub fn is_mirrored(self) -> bool {
        self.parts().1
    }
}

/// Rigid (orthogonal) transform: linear part (`orientation`) applied
/// first, then `translation` added. No shear, no scale — not needed for
/// schematics, and deliberately not building layout's general affine case.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Transform {
    pub orientation: Orientation,
    pub translation: Point,
}

impl Transform {
    pub const IDENTITY: Transform = Transform {
        orientation: Orientation::R0,
        translation: Point::ORIGIN,
    };

    pub fn new(orientation: Orientation, translation: Point) -> Self {
        Self {
            orientation,
            translation,
        }
    }

    pub fn translation(dx: Unit, dy: Unit) -> Self {
        Self {
            orientation: Orientation::R0,
            translation: Point::new(dx, dy),
        }
    }

    /// `apply_point(p) = orientation.apply_vector(p) + translation`.
    pub fn apply_point(&self, p: Point) -> Point {
        let (vx, vy) = self.orientation.apply_vector(p.x, p.y);
        Point::new(
            vx.saturating_add(self.translation.x),
            vy.saturating_add(self.translation.y),
        )
    }

    /// Transforms all four corners and re-normalizes via `Rect::from_points`
    /// and `union`. Always axis-aligned again: every `Orientation` is a
    /// 90-degree-step map.
    pub fn apply_rect(&self, r: Rect) -> Rect {
        let p00 = self.apply_point(Point::new(r.x0, r.y0));
        let p10 = self.apply_point(Point::new(r.x1, r.y0));
        let p01 = self.apply_point(Point::new(r.x0, r.y1));
        let p11 = self.apply_point(Point::new(r.x1, r.y1));
        Rect::from_points(p00, p10).union(&Rect::from_points(p01, p11))
    }

    /// `self.then(next)` means "apply `self` first, then `next`":
    /// `self.then(next).apply_point(p) == next.apply_point(self.apply_point(p))`.
    pub fn then(&self, next: &Transform) -> Transform {
        let orientation = self.orientation.compose(next.orientation);
        let (vx, vy) = next
            .orientation
            .apply_vector(self.translation.x, self.translation.y);
        let translation = Point::new(
            vx.saturating_add(next.translation.x),
            vy.saturating_add(next.translation.y),
        );
        Transform {
            orientation,
            translation,
        }
    }

    /// `t.then(&t.inverse()) == Transform::IDENTITY` for every `t`.
    pub fn inverse(&self) -> Transform {
        let inv_orientation = self.orientation.inverse();
        let (vx, vy) = inv_orientation.apply_vector(self.translation.x, self.translation.y);
        let inv_translation = Point::new(0i32.saturating_sub(vx), 0i32.saturating_sub(vy));
        Transform {
            orientation: inv_orientation,
            translation: inv_translation,
        }
    }
}
