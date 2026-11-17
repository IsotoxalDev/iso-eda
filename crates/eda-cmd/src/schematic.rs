// SPDX-License-Identifier: GPL-3.0-or-later

//! The first schematic-editing commands: `AddInstanceCommand`,
//! `MoveInstanceCommand`, `DeleteInstanceCommand`, `AddWireCommand`, and
//! the `Patch` that implements `Edit` for all four. See
//! `docs/adr/003-schematic-data-model.md`.
//!
//! None of these four commands is registered in
//! `CommandRegistry::with_builtins()`; they are run via
//! `CommandHost::run_command`. Palette entries and keymap bindings for
//! editing are M1.5's job.

use crate::command::{Command, Edit, Outcome};
use crate::context::CommandContext;
use crate::error::CommandError;
use crate::id::CommandId;

#[derive(Debug, thiserror::Error)]
pub enum SchematicEditError {
    #[error("no cell named `{0}` is active, or it has no schematic view")]
    NoActiveSchematic(String),
    #[error("instance {0:?} not found")]
    NoSuchInstance(eda_db::InstanceId),
    #[error("wire {0:?} not found")]
    NoSuchWire(eda_db::WireId),
}

/// The recorded change for all four schematic commands below. `pub(crate)`:
/// not part of the public API outside `eda-cmd`, only `Outcome::Edit(Box::new(..))`
/// crosses the boundary, as a `Box<dyn Edit>`.
pub(crate) enum Patch {
    AddInstance {
        id: eda_db::InstanceId,
        instance: eda_db::Instance,
    },
    DeleteInstance {
        id: eda_db::InstanceId,
        instance: eda_db::Instance,
    },
    MoveInstance {
        id: eda_db::InstanceId,
        from: eda_geom::Transform,
        to: eda_geom::Transform,
    },
    AddWire {
        id: eda_db::WireId,
        wire: eda_db::Wire,
    },
}

impl Edit for Patch {
    fn label(&self) -> &str {
        match self {
            Patch::AddInstance { .. } => "Add instance",
            Patch::DeleteInstance { .. } => "Delete instance",
            Patch::MoveInstance { .. } => "Move instance",
            Patch::AddWire { .. } => "Add wire",
        }
    }

    fn undo(&mut self, cx: &mut CommandContext) -> Result<(), CommandError> {
        let label = self.label().to_owned();
        let active_cell = cx.design.active_cell.clone();
        let schematic = cx.design.active_schematic_mut().ok_or_else(|| CommandError::UndoFailed {
            label: label.clone(),
            source: Box::new(SchematicEditError::NoActiveSchematic(active_cell)),
        })?;
        match self {
            Patch::AddInstance { id, .. } => {
                schematic.remove_instance(*id).ok_or_else(|| CommandError::UndoFailed {
                    label: label.clone(),
                    source: Box::new(SchematicEditError::NoSuchInstance(*id)),
                })?;
            }
            Patch::DeleteInstance { id, instance } => {
                schematic.insert_instance(*id, instance.clone());
            }
            Patch::MoveInstance { id, from, .. } => {
                let inst = schematic.instance_mut(*id).ok_or_else(|| CommandError::UndoFailed {
                    label: label.clone(),
                    source: Box::new(SchematicEditError::NoSuchInstance(*id)),
                })?;
                inst.transform = *from;
            }
            Patch::AddWire { id, .. } => {
                schematic.remove_wire(*id).ok_or_else(|| CommandError::UndoFailed {
                    label: label.clone(),
                    source: Box::new(SchematicEditError::NoSuchWire(*id)),
                })?;
            }
        }
        Ok(())
    }

    fn redo(&mut self, cx: &mut CommandContext) -> Result<(), CommandError> {
        let label = self.label().to_owned();
        let active_cell = cx.design.active_cell.clone();
        let schematic = cx.design.active_schematic_mut().ok_or_else(|| CommandError::RedoFailed {
            label: label.clone(),
            source: Box::new(SchematicEditError::NoActiveSchematic(active_cell)),
        })?;
        match self {
            Patch::AddInstance { id, instance } => {
                schematic.insert_instance(*id, instance.clone());
            }
            Patch::DeleteInstance { id, .. } => {
                schematic.remove_instance(*id).ok_or_else(|| CommandError::RedoFailed {
                    label: label.clone(),
                    source: Box::new(SchematicEditError::NoSuchInstance(*id)),
                })?;
            }
            Patch::MoveInstance { id, to, .. } => {
                let inst = schematic.instance_mut(*id).ok_or_else(|| CommandError::RedoFailed {
                    label: label.clone(),
                    source: Box::new(SchematicEditError::NoSuchInstance(*id)),
                })?;
                inst.transform = *to;
            }
            Patch::AddWire { id, wire } => {
                schematic.insert_wire(*id, wire.clone());
            }
        }
        Ok(())
    }
}

pub struct AddInstanceCommand {
    pub cell_name: String,
    pub transform: eda_geom::Transform,
    pub attributes: eda_db::AttributeBag,
}

impl Command for AddInstanceCommand {
    fn id(&self) -> CommandId {
        CommandId::new("schematic.add_instance")
    }

    fn title(&self) -> &str {
        "Add instance"
    }

    fn run(&self, cx: &mut CommandContext) -> Result<Outcome, CommandError> {
        let active_cell = cx.design.active_cell.clone();
        let schematic = cx.design.active_schematic_mut().ok_or_else(|| {
            CommandError::failed(
                &self.id(),
                SchematicEditError::NoActiveSchematic(active_cell),
            )
        })?;

        let id = schematic.alloc_instance_id();
        let instance = eda_db::Instance {
            cell_name: self.cell_name.clone(),
            transform: self.transform,
            attributes: self.attributes.clone(),
        };
        schematic.insert_instance(id, instance.clone());

        Ok(Outcome::Edit(Box::new(Patch::AddInstance { id, instance })))
    }
}

pub struct MoveInstanceCommand {
    pub id: eda_db::InstanceId,
    pub to: eda_geom::Transform,
}

impl Command for MoveInstanceCommand {
    fn id(&self) -> CommandId {
        CommandId::new("schematic.move_instance")
    }

    fn title(&self) -> &str {
        "Move instance"
    }

    fn run(&self, cx: &mut CommandContext) -> Result<Outcome, CommandError> {
        let active_cell = cx.design.active_cell.clone();
        let schematic = cx.design.active_schematic_mut().ok_or_else(|| {
            CommandError::failed(
                &self.id(),
                SchematicEditError::NoActiveSchematic(active_cell),
            )
        })?;

        let instance = schematic
            .instance_mut(self.id)
            .ok_or_else(|| CommandError::failed(&self.id(), SchematicEditError::NoSuchInstance(self.id)))?;

        let from = instance.transform;
        instance.transform = self.to;

        Ok(Outcome::Edit(Box::new(Patch::MoveInstance {
            id: self.id,
            from,
            to: self.to,
        })))
    }
}

pub struct DeleteInstanceCommand {
    pub id: eda_db::InstanceId,
}

impl Command for DeleteInstanceCommand {
    fn id(&self) -> CommandId {
        CommandId::new("schematic.delete_instance")
    }

    fn title(&self) -> &str {
        "Delete instance"
    }

    fn run(&self, cx: &mut CommandContext) -> Result<Outcome, CommandError> {
        let active_cell = cx.design.active_cell.clone();
        let schematic = cx.design.active_schematic_mut().ok_or_else(|| {
            CommandError::failed(
                &self.id(),
                SchematicEditError::NoActiveSchematic(active_cell),
            )
        })?;

        let instance = schematic
            .remove_instance(self.id)
            .ok_or_else(|| CommandError::failed(&self.id(), SchematicEditError::NoSuchInstance(self.id)))?;

        Ok(Outcome::Edit(Box::new(Patch::DeleteInstance {
            id: self.id,
            instance,
        })))
    }
}

pub struct AddWireCommand {
    pub wire: eda_db::Wire,
}

impl Command for AddWireCommand {
    fn id(&self) -> CommandId {
        CommandId::new("schematic.add_wire")
    }

    fn title(&self) -> &str {
        "Add wire"
    }

    fn run(&self, cx: &mut CommandContext) -> Result<Outcome, CommandError> {
        let active_cell = cx.design.active_cell.clone();
        let schematic = cx.design.active_schematic_mut().ok_or_else(|| {
            CommandError::failed(
                &self.id(),
                SchematicEditError::NoActiveSchematic(active_cell),
            )
        })?;

        let id = schematic.alloc_wire_id();
        let wire = self.wire.clone();
        schematic.insert_wire(id, wire.clone());

        Ok(Outcome::Edit(Box::new(Patch::AddWire { id, wire })))
    }
}
