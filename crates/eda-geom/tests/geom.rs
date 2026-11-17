// SPDX-License-Identifier: GPL-3.0-or-later

//! Test list for ADR-003 (`docs/adr/003-schematic-data-model.md`),
//! `eda-geom` section. Public API only.

use eda_geom::{Orientation, Point, Rect, Transform, Unit};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Point
// ---------------------------------------------------------------------------

#[test]
fn point_translated_moves_by_delta() {
    assert_eq!(Point::new(1, 2).translated(3, -5), Point::new(4, -3));
}

// ---------------------------------------------------------------------------
// Rect
// ---------------------------------------------------------------------------

#[test]
fn rect_new_normalizes_swapped_corners() {
    assert_eq!(Rect::new(10, 10, 0, 0), Rect::new(0, 0, 10, 10));
}

#[test]
fn rect_from_points_normalizes_either_order() {
    let a = Point::new(5, -5);
    let b = Point::new(-5, 5);
    assert_eq!(Rect::from_points(a, b), Rect::from_points(b, a));
}

#[test]
fn rect_width_height_are_nonnegative() {
    let rects = [
        Rect::new(0, 0, 10, 10),
        Rect::new(10, 10, 0, 0),
        Rect::new(-5, -5, 5, 5),
        Rect::new(3, 3, 3, 3),
    ];
    for r in rects {
        assert!(r.width() >= 0);
        assert!(r.height() >= 0);
    }
}

#[test]
fn rect_union_contains_both_inputs() {
    let a = Rect::new(0, 0, 10, 10);
    let b = Rect::new(5, -5, 15, 5);
    let u = a.union(&b);
    for p in [
        Point::new(a.x0, a.y0),
        Point::new(a.x0, a.y1),
        Point::new(a.x1, a.y0),
        Point::new(a.x1, a.y1),
        Point::new(b.x0, b.y0),
        Point::new(b.x0, b.y1),
        Point::new(b.x1, b.y0),
        Point::new(b.x1, b.y1),
    ] {
        assert!(u.contains_point(p));
    }
}

#[test]
fn rect_contains_point_is_edge_inclusive() {
    let r = Rect::new(0, 0, 10, 10);
    assert!(r.contains_point(Point::new(0, 5)));
    assert!(r.contains_point(Point::new(10, 5)));
    assert!(r.contains_point(Point::new(5, 0)));
    assert!(r.contains_point(Point::new(5, 10)));
}

#[test]
fn rect_intersects_detects_touching_as_intersecting() {
    let a = Rect::new(0, 0, 10, 10);
    let touching = Rect::new(10, 0, 20, 10);
    let disjoint = Rect::new(20, 0, 30, 10);
    assert!(a.intersects(&touching));
    assert!(!a.intersects(&disjoint));
}

// ---------------------------------------------------------------------------
// Orientation
// ---------------------------------------------------------------------------

#[test]
fn orientation_apply_vector_matches_documented_table() {
    let cases: [(Orientation, (Unit, Unit)); 8] = [
        (Orientation::R0, (2, 3)),
        (Orientation::R90, (-3, 2)),
        (Orientation::R180, (-2, -3)),
        (Orientation::R270, (3, -2)),
        (Orientation::FlipR0, (-2, 3)),
        (Orientation::FlipR90, (-3, -2)),
        (Orientation::FlipR180, (2, -3)),
        (Orientation::FlipR270, (3, 2)),
    ];
    for (o, expected) in cases {
        assert_eq!(o.apply_vector(2, 3), expected, "{o:?} at (2,3)");
        assert_eq!(o.apply_vector(0, 0), (0, 0), "{o:?} at (0,0)");
    }
}

#[test]
fn orientation_r90_four_times_is_r0() {
    let four = Orientation::R90
        .compose(Orientation::R90)
        .compose(Orientation::R90)
        .compose(Orientation::R90);
    assert_eq!(four, Orientation::R0);
}

#[test]
fn orientation_inverse_round_trips_for_all_eight() {
    let orientations = [
        Orientation::R0,
        Orientation::R90,
        Orientation::R180,
        Orientation::R270,
        Orientation::FlipR0,
        Orientation::FlipR90,
        Orientation::FlipR180,
        Orientation::FlipR270,
    ];
    for o in orientations {
        for (x, y) in [(3, -7), (-4, 1)] {
            let (vx, vy) = o.apply_vector(x, y);
            assert_eq!(o.inverse().apply_vector(vx, vy), (x, y), "{o:?}");
        }
    }
}

// ---------------------------------------------------------------------------
// Transform
// ---------------------------------------------------------------------------

#[test]
fn transform_identity_is_a_no_op() {
    for p in [Point::new(0, 0), Point::new(5, -5), Point::new(-100, 42)] {
        assert_eq!(Transform::IDENTITY.apply_point(p), p);
    }
}

#[test]
fn transform_then_order_is_not_commutative() {
    let rotate = Transform::new(Orientation::R90, Point::ORIGIN);
    let translate = Transform::translation(10, 0);
    let p = Point::new(1, 0);

    let rotate_then_translate = rotate.then(&translate).apply_point(p);
    let translate_then_rotate = translate.then(&rotate).apply_point(p);

    assert_eq!(rotate_then_translate, Point::new(10, 1));
    assert_eq!(translate_then_rotate, Point::new(0, 11));
    assert_ne!(rotate_then_translate, translate_then_rotate);
}

#[test]
fn transform_inverse_undoes_apply_worked_example() {
    let t = Transform::new(Orientation::R90, Point::new(5, 7));
    let p = Point::new(2, 3);

    let applied = t.apply_point(p);
    assert_eq!(applied, Point::new(2, 9));

    assert_eq!(t.inverse().apply_point(applied), p);
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

fn arb_orientation() -> impl Strategy<Value = Orientation> {
    prop_oneof![
        Just(Orientation::R0),
        Just(Orientation::R90),
        Just(Orientation::R180),
        Just(Orientation::R270),
        Just(Orientation::FlipR0),
        Just(Orientation::FlipR90),
        Just(Orientation::FlipR180),
        Just(Orientation::FlipR270),
    ]
}

fn arb_unit() -> impl Strategy<Value = Unit> {
    // Avoid i32::MIN so translated/apply_vector sums stay interesting
    // without every case saturating to the same boundary value.
    -1_000_000..=1_000_000
}

fn arb_point() -> impl Strategy<Value = Point> {
    (arb_unit(), arb_unit()).prop_map(|(x, y)| Point::new(x, y))
}

fn arb_transform() -> impl Strategy<Value = Transform> {
    (arb_orientation(), arb_point()).prop_map(|(o, t)| Transform::new(o, t))
}

fn arb_rect() -> impl Strategy<Value = Rect> {
    (arb_point(), arb_point()).prop_map(|(a, b)| Rect::from_points(a, b))
}

proptest! {
    #[test]
    fn prop_orientation_compose_matches_sequential_apply(
        o1 in arb_orientation(),
        o2 in arb_orientation(),
        x in arb_unit(),
        y in arb_unit(),
    ) {
        let (x1, y1) = o1.apply_vector(x, y);
        prop_assert_eq!(o1.compose(o2).apply_vector(x, y), o2.apply_vector(x1, y1));
    }

    #[test]
    fn prop_transform_then_matches_sequential_apply(
        t1 in arb_transform(),
        t2 in arb_transform(),
        p in arb_point(),
    ) {
        prop_assert_eq!(t1.then(&t2).apply_point(p), t2.apply_point(t1.apply_point(p)));
    }

    #[test]
    fn prop_transform_inverse_round_trips(t in arb_transform(), p in arb_point()) {
        prop_assert_eq!(t.inverse().apply_point(t.apply_point(p)), p);
        prop_assert_eq!(t.then(&t.inverse()), Transform::IDENTITY);
    }

    #[test]
    fn prop_rect_apply_transform_stays_normalized(r in arb_rect(), t in arb_transform()) {
        let out = t.apply_rect(r);
        prop_assert!(out.x0 <= out.x1);
        prop_assert!(out.y0 <= out.y1);
    }

    #[test]
    fn prop_rect_union_is_commutative_and_contains_inputs(a in arb_rect(), b in arb_rect()) {
        let u1 = a.union(&b);
        let u2 = b.union(&a);
        prop_assert_eq!(u1, u2);
        for p in [
            Point::new(a.x0, a.y0),
            Point::new(a.x1, a.y1),
            Point::new(b.x0, b.y0),
            Point::new(b.x1, b.y1),
        ] {
            prop_assert!(u1.contains_point(p));
        }
    }
}
