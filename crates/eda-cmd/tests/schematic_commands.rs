// SPDX-License-Identifier: GPL-3.0-or-later

//! Test list for ADR-003 (`docs/adr/003-schematic-data-model.md`),
//! `eda-cmd` schematic-command section. Public API only; does not modify
//! `tests/undo_model.rs`.

use eda_cmd::*;
use eda_db::{AttributeBag, Cell, Design, InstanceId, Library, View, Wire};
use eda_geom::{Orientation, Point, Transform};
use proptest::prelude::*;

/// A `CommandHost` with one `Library`, one `Cell` named "top" with an
/// empty `SchematicView`, `active_cell: "top"`.
fn fresh_host() -> CommandHost {
    let mut library = Library::new("lib");
    let mut cell = Cell::new("top");
    cell.insert_view(View::Schematic(eda_db::SchematicView::new()));
    library.insert_cell(cell);
    let design = Design::new(library, "top");
    let cx = CommandContext {
        design,
        ..Default::default()
    };
    CommandHost::new(CommandRegistry::new(), cx)
}

fn first_instance_id(host: &CommandHost) -> InstanceId {
    host.context()
        .design
        .active_schematic()
        .unwrap()
        .instances()
        .next()
        .unwrap()
        .0
}

fn current_transform(host: &CommandHost, id: InstanceId) -> Transform {
    host.context()
        .design
        .active_schematic()
        .unwrap()
        .instance(id)
        .unwrap()
        .transform
}

fn add_instance(host: &mut CommandHost, cell_name: &str, transform: Transform) {
    host.run_command(&AddInstanceCommand {
        cell_name: cell_name.to_owned(),
        transform,
        attributes: AttributeBag::new(),
    })
    .unwrap();
}

// ---------------------------------------------------------------------------

#[test]
fn add_instance_allocates_and_is_undoable() {
    let mut host = fresh_host();
    add_instance(&mut host, "inv", Transform::IDENTITY);

    assert_eq!(
        host.context()
            .design
            .active_schematic()
            .unwrap()
            .instances()
            .count(),
        1
    );
    assert!(host.undo_stack().can_undo());

    host.undo().unwrap();

    assert_eq!(
        host.context()
            .design
            .active_schematic()
            .unwrap()
            .instances()
            .count(),
        0
    );
}

#[test]
fn add_instance_fails_cleanly_with_no_active_schematic() {
    let mut host = CommandHost::new(CommandRegistry::new(), CommandContext::default());
    let cmd = AddInstanceCommand {
        cell_name: "inv".to_owned(),
        transform: Transform::IDENTITY,
        attributes: AttributeBag::new(),
    };

    let result = host.run_command(&cmd);

    assert!(result.is_err());
    assert_eq!(host.context(), &CommandContext::default());
    assert!(!host.undo_stack().can_undo());
}

#[test]
fn move_instance_undo_restores_previous_transform() {
    let mut host = fresh_host();
    let t0 = Transform::IDENTITY;
    add_instance(&mut host, "inv", t0);
    let id = first_instance_id(&host);

    let t1 = Transform::translation(5, 7);
    host.run_command(&MoveInstanceCommand { id, to: t1 }).unwrap();
    assert_eq!(current_transform(&host, id), t1);

    host.undo().unwrap();
    assert_eq!(current_transform(&host, id), t0);
}

#[test]
fn move_instance_redo_sets_captured_target_not_a_relative_delta() {
    let mut host = fresh_host();
    let t0 = Transform::IDENTITY;
    add_instance(&mut host, "inv", t0);
    let id = first_instance_id(&host);

    let t1 = Transform::new(Orientation::R90, Point::new(3, -4));
    host.run_command(&MoveInstanceCommand { id, to: t1 }).unwrap();

    host.undo().unwrap();
    assert_eq!(current_transform(&host, id), t0);

    host.redo().unwrap();
    assert_eq!(current_transform(&host, id), t1);
}

#[test]
fn delete_instance_then_undo_reinserts_at_the_same_id() {
    let mut host = fresh_host();
    add_instance(&mut host, "inv", Transform::IDENTITY);
    let id = first_instance_id(&host);

    host.run_command(&DeleteInstanceCommand { id }).unwrap();
    assert!(host
        .context()
        .design
        .active_schematic()
        .unwrap()
        .instance(id)
        .is_none());

    host.undo().unwrap();
    assert!(host
        .context()
        .design
        .active_schematic()
        .unwrap()
        .instance(id)
        .is_some());
}

#[test]
fn deleting_then_adding_never_reuses_the_freed_id() {
    let mut host = fresh_host();
    add_instance(&mut host, "a", Transform::IDENTITY);
    let id_a = first_instance_id(&host);

    host.run_command(&DeleteInstanceCommand { id: id_a }).unwrap();

    add_instance(&mut host, "b", Transform::IDENTITY);
    let id_b = first_instance_id(&host);

    assert_ne!(id_a, id_b);
}

#[test]
fn add_wire_undo_then_redo_restores_wire() {
    let mut host = fresh_host();
    let wire = Wire {
        p0: Point::new(0, 0),
        p1: Point::new(10, 0),
        attributes: AttributeBag::new(),
    };
    host.run_command(&AddWireCommand { wire: wire.clone() }).unwrap();

    let id = host
        .context()
        .design
        .active_schematic()
        .unwrap()
        .wires()
        .next()
        .unwrap()
        .0;
    assert_eq!(
        host.context().design.active_schematic().unwrap().wire(id),
        Some(&wire)
    );

    host.undo().unwrap();
    assert!(host
        .context()
        .design
        .active_schematic()
        .unwrap()
        .wire(id)
        .is_none());

    host.redo().unwrap();
    assert_eq!(
        host.context().design.active_schematic().unwrap().wire(id),
        Some(&wire)
    );
}

#[test]
fn undo_then_redo_returns_identical_state_for_schematic_edits() {
    let mut host = fresh_host();
    add_instance(&mut host, "inv", Transform::IDENTITY);
    let id = first_instance_id(&host);

    host.run_command(&MoveInstanceCommand {
        id,
        to: Transform::translation(2, 2),
    })
    .unwrap();

    host.run_command(&AddWireCommand {
        wire: Wire {
            p0: Point::new(0, 0),
            p1: Point::new(1, 1),
            attributes: AttributeBag::new(),
        },
    })
    .unwrap();

    let snapshot = host.context().design.clone();

    assert!(host.undo().unwrap());
    assert!(host.undo().unwrap());
    assert!(host.undo().unwrap());

    assert!(host.redo().unwrap());
    assert!(host.redo().unwrap());
    assert!(host.redo().unwrap());

    assert_eq!(host.context().design, snapshot);
}

// ---------------------------------------------------------------------------
// Property test
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum SchematicOp {
    AddInstance,
    MoveInstance(u8),
    DeleteInstance(u8),
    AddWire,
    Undo,
    Redo,
}

fn arb_schematic_op() -> impl Strategy<Value = SchematicOp> {
    prop_oneof![
        Just(SchematicOp::AddInstance),
        (0u8..8).prop_map(SchematicOp::MoveInstance),
        (0u8..8).prop_map(SchematicOp::DeleteInstance),
        Just(SchematicOp::AddWire),
        Just(SchematicOp::Undo),
        Just(SchematicOp::Redo),
    ]
}

proptest! {
    #[test]
    fn prop_schematic_edit_sequence_preserves_invariants(
        ops in prop::collection::vec(arb_schematic_op(), 0..40)
    ) {
        let mut host = fresh_host();

        for op in ops {
            let before_undo = host.undo_stack().undo_len();
            let before_redo = host.undo_stack().redo_len();

            let live_ids: Vec<InstanceId> = host
                .context()
                .design
                .active_schematic()
                .unwrap()
                .instances()
                .map(|(id, _)| id)
                .collect();

            match op {
                SchematicOp::AddInstance => {
                    add_instance(&mut host, "x", Transform::IDENTITY);
                    prop_assert_eq!(host.undo_stack().undo_len(), before_undo + 1);
                    prop_assert_eq!(host.undo_stack().redo_len(), 0);
                }
                SchematicOp::AddWire => {
                    host.run_command(&AddWireCommand {
                        wire: Wire {
                            p0: Point::new(0, 0),
                            p1: Point::new(1, 1),
                            attributes: AttributeBag::new(),
                        },
                    })
                    .unwrap();
                    prop_assert_eq!(host.undo_stack().undo_len(), before_undo + 1);
                    prop_assert_eq!(host.undo_stack().redo_len(), 0);
                }
                SchematicOp::MoveInstance(idx) => {
                    if live_ids.is_empty() {
                        prop_assert_eq!(host.undo_stack().undo_len(), before_undo);
                        prop_assert_eq!(host.undo_stack().redo_len(), before_redo);
                    } else {
                        let id = live_ids[(idx as usize) % live_ids.len()];
                        host.run_command(&MoveInstanceCommand {
                            id,
                            to: Transform::translation(1, -1),
                        })
                        .unwrap();
                        prop_assert_eq!(host.undo_stack().undo_len(), before_undo + 1);
                        prop_assert_eq!(host.undo_stack().redo_len(), 0);
                    }
                }
                SchematicOp::DeleteInstance(idx) => {
                    if live_ids.is_empty() {
                        prop_assert_eq!(host.undo_stack().undo_len(), before_undo);
                        prop_assert_eq!(host.undo_stack().redo_len(), before_redo);
                    } else {
                        let id = live_ids[(idx as usize) % live_ids.len()];
                        host.run_command(&DeleteInstanceCommand { id }).unwrap();
                        prop_assert_eq!(host.undo_stack().undo_len(), before_undo + 1);
                        prop_assert_eq!(host.undo_stack().redo_len(), 0);
                    }
                }
                SchematicOp::Undo => {
                    let did_undo = host.undo().unwrap();
                    if before_undo > 0 {
                        prop_assert!(did_undo);
                        prop_assert_eq!(host.undo_stack().undo_len(), before_undo - 1);
                        prop_assert_eq!(host.undo_stack().redo_len(), before_redo + 1);
                    } else {
                        prop_assert!(!did_undo);
                        prop_assert_eq!(host.undo_stack().undo_len(), before_undo);
                        prop_assert_eq!(host.undo_stack().redo_len(), before_redo);
                    }
                }
                SchematicOp::Redo => {
                    let did_redo = host.redo().unwrap();
                    if before_redo > 0 {
                        prop_assert!(did_redo);
                        prop_assert_eq!(host.undo_stack().redo_len(), before_redo - 1);
                        prop_assert_eq!(host.undo_stack().undo_len(), before_undo + 1);
                    } else {
                        prop_assert!(!did_redo);
                        prop_assert_eq!(host.undo_stack().undo_len(), before_undo);
                        prop_assert_eq!(host.undo_stack().redo_len(), before_redo);
                    }
                }
            }

            // All live instance ids stay distinct.
            let ids: Vec<InstanceId> = host
                .context()
                .design
                .active_schematic()
                .unwrap()
                .instances()
                .map(|(id, _)| id)
                .collect();
            let mut sorted = ids.clone();
            sorted.sort();
            sorted.dedup();
            prop_assert_eq!(sorted.len(), ids.len());

            // undo() immediately followed by redo() is a net no-op on both
            // the design and the stack lengths (the same captured Edit
            // moves undo-side -> redo-side -> undo-side), so running this
            // check after every step never perturbs the rest of the
            // sequence, while still proving the pair restores state
            // exactly "at any point" in the run.
            if host.undo_stack().can_undo() {
                let before_pair = host.context().design.clone();
                prop_assert!(host.undo().unwrap());
                prop_assert!(host.redo().unwrap());
                prop_assert_eq!(&host.context().design, &before_pair);
            }
        }
    }
}
