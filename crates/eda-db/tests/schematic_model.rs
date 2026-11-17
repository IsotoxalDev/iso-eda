// SPDX-License-Identifier: GPL-3.0-or-later

//! Test list for ADR-003 (`docs/adr/003-schematic-data-model.md`),
//! `eda-db` section. Public API only.

use eda_db::{AttributeBag, Cell, Design, Instance, Library, SchematicView, View, ViewKind, Wire};
use eda_geom::{Point, Transform};
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// Id / IdAllocator
// ---------------------------------------------------------------------------

#[test]
fn id_allocator_never_repeats_across_five_allocs() {
    let mut view = SchematicView::new();
    let ids: Vec<_> = (0..5).map(|_| view.alloc_instance_id()).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), ids.len());
}

// ---------------------------------------------------------------------------
// AttributeBag
// ---------------------------------------------------------------------------

fn sample_bag() -> AttributeBag {
    let mut bag = AttributeBag::new();
    bag.push("a", "1");
    bag.push("b", "2");
    bag.push("a", "3");
    bag
}

#[test]
fn attribute_bag_preserves_order_and_duplicates() {
    let bag = sample_bag();
    let entries: Vec<_> = bag.iter().collect();
    assert_eq!(entries, vec![("a", "1"), ("b", "2"), ("a", "3")]);
}

#[test]
fn attribute_bag_get_returns_first_match() {
    let bag = sample_bag();
    assert_eq!(bag.get("a"), Some("1"));
    let all: Vec<_> = bag.get_all("a").collect();
    assert_eq!(all, vec!["1", "3"]);
}

#[test]
fn attribute_bag_set_replaces_all_and_appends_once() {
    let mut bag = sample_bag();
    bag.set("a", "9");
    let entries: Vec<_> = bag.iter().collect();
    assert_eq!(entries, vec![("b", "2"), ("a", "9")]);
}

#[test]
fn attribute_bag_remove_all_reports_count() {
    let mut bag = sample_bag();
    let removed = bag.remove_all("a");
    assert_eq!(removed, 2);
    let entries: Vec<_> = bag.iter().collect();
    assert_eq!(entries, vec![("b", "2")]);
}

// ---------------------------------------------------------------------------
// Library / Cell / View
// ---------------------------------------------------------------------------

#[test]
fn library_insert_and_remove_cell_round_trip() {
    let mut library = Library::new("lib");
    library.insert_cell(Cell::new("x"));

    assert!(library.cell("x").is_some());

    let removed = library.remove_cell("x");
    assert!(removed.is_some());
    assert!(library.cell("x").is_none());
}

#[test]
fn cell_insert_view_keeps_other_kind() {
    let mut cell = Cell::new("x");
    cell.insert_view(View::Schematic(SchematicView::new()));
    cell.insert_view(View::Symbol(Default::default()));

    assert!(cell.schematic().is_some());
    assert!(cell.symbol().is_some());

    let previous = cell.insert_view(View::Schematic(SchematicView::new()));
    assert!(previous.is_some());
    assert_eq!(previous.unwrap().kind(), ViewKind::Schematic);
}

#[test]
fn design_active_schematic_is_none_without_a_matching_cell_and_view() {
    let design = Design::default();
    assert!(design.active_schematic().is_none());

    let mut library = Library::new("lib");
    library.insert_cell(Cell::new("x"));
    let design = Design::new(library.clone(), "not_present");
    assert!(design.active_schematic().is_none());

    let mut cell_with_symbol = Cell::new("y");
    cell_with_symbol.insert_view(View::Symbol(Default::default()));
    let mut library2 = Library::new("lib");
    library2.insert_cell(cell_with_symbol);
    let design = Design::new(library2, "y");
    assert!(design.active_schematic().is_none());
}

// ---------------------------------------------------------------------------
// SchematicView
// ---------------------------------------------------------------------------

#[test]
fn schematic_view_instance_insert_then_remove_round_trips() {
    let mut view = SchematicView::new();
    let id = view.alloc_instance_id();
    let instance = Instance {
        cell_name: "inv".to_owned(),
        transform: Transform::IDENTITY,
        attributes: AttributeBag::new(),
    };
    view.insert_instance(id, instance.clone());

    let removed = view.remove_instance(id);
    assert_eq!(removed, Some(instance));
    assert_eq!(view.remove_instance(id), None);
}

#[test]
fn schematic_view_accepts_zero_length_wire() {
    let mut view = SchematicView::new();
    let id = view.alloc_wire_id();
    let wire = Wire {
        p0: Point::new(3, 3),
        p1: Point::new(3, 3),
        attributes: AttributeBag::new(),
    };
    view.insert_wire(id, wire.clone());
    assert_eq!(view.wire(id), Some(&wire));
}

// ---------------------------------------------------------------------------
// Design equality
// ---------------------------------------------------------------------------

fn build_design() -> Design {
    let mut library = Library::new("lib");
    let mut cell = Cell::new("top");
    let mut schematic = SchematicView::new();
    let id = schematic.alloc_instance_id();
    schematic.insert_instance(
        id,
        Instance {
            cell_name: "inv".to_owned(),
            transform: Transform::IDENTITY,
            attributes: AttributeBag::new(),
        },
    );
    cell.insert_view(View::Schematic(schematic));
    library.insert_cell(cell);
    Design::new(library, "top")
}

#[test]
fn design_round_trips_through_eq() {
    let a = build_design();
    let b = build_design();
    assert_eq!(a, b);
}

// ---------------------------------------------------------------------------
// Property tests
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum Op {
    Alloc,
    Remove(u8),
}

fn arb_op() -> impl Strategy<Value = Op> {
    prop_oneof![Just(Op::Alloc), (0u8..8).prop_map(Op::Remove),]
}

proptest! {
    #[test]
    fn prop_instance_ids_stay_unique_under_random_alloc_insert_remove(ops in prop::collection::vec(arb_op(), 0..64)) {
        let mut view = SchematicView::new();
        let mut live = Vec::new();

        for op in ops {
            match op {
                Op::Alloc => {
                    let id = view.alloc_instance_id();
                    view.insert_instance(
                        id,
                        Instance {
                            cell_name: "x".to_owned(),
                            transform: Transform::IDENTITY,
                            attributes: AttributeBag::new(),
                        },
                    );
                    live.push(id);
                }
                Op::Remove(idx) => {
                    if !live.is_empty() {
                        let i = (idx as usize) % live.len();
                        let id = live.remove(i);
                        view.remove_instance(id);
                    }
                }
            }

            let mut ids: Vec<_> = view.instances().map(|(id, _)| id).collect();
            let before = ids.len();
            ids.sort();
            ids.dedup();
            prop_assert_eq!(ids.len(), before);
        }
    }

    #[test]
    fn prop_attribute_bag_get_after_set_returns_the_set_value(
        key in "[a-z]{1,4}",
        value in "[a-z0-9]{0,4}",
        extra in prop::collection::vec(("[a-z]{1,4}", "[a-z0-9]{0,4}"), 0..5),
    ) {
        let mut bag = AttributeBag::new();
        for (k, v) in extra {
            bag.push(k, v);
        }
        bag.set(key.clone(), value.clone());
        prop_assert_eq!(bag.get(&key), Some(value.as_str()));
    }
}
