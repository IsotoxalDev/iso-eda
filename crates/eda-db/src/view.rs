// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use crate::id::{Id, IdAllocator};
use crate::instance::Instance;
use crate::label::Label;
use crate::pin::Pin;
use crate::wire::Wire;

pub type InstanceId = Id<Instance>;
pub type WireId = Id<Wire>;
pub type LabelId = Id<Label>;
pub type PinId = Id<Pin>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ViewKind {
    Schematic,
    Symbol,
}

/// Holds the `Instance`/`Wire`/`Label` objects of one schematic and the
/// three id allocators that own their ids. No `Net` type: net membership
/// is derived from this data by a connectivity pass that does not exist
/// yet (M1.6); this struct only stores what that pass will read.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SchematicView {
    instances: BTreeMap<InstanceId, Instance>,
    wires: BTreeMap<WireId, Wire>,
    labels: BTreeMap<LabelId, Label>,
    instance_ids: IdAllocator<Instance>,
    wire_ids: IdAllocator<Wire>,
    label_ids: IdAllocator<Label>,
}

impl SchematicView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc_instance_id(&mut self) -> InstanceId {
        self.instance_ids.alloc()
    }

    /// Returns the previous occupant of `id`, if any (should normally be
    /// `None`; `Some` signals the caller reused an id, which is a bug).
    pub fn insert_instance(&mut self, id: InstanceId, instance: Instance) -> Option<Instance> {
        self.instances.insert(id, instance)
    }

    pub fn remove_instance(&mut self, id: InstanceId) -> Option<Instance> {
        self.instances.remove(&id)
    }

    pub fn instance(&self, id: InstanceId) -> Option<&Instance> {
        self.instances.get(&id)
    }

    pub fn instance_mut(&mut self, id: InstanceId) -> Option<&mut Instance> {
        self.instances.get_mut(&id)
    }

    /// Ascending by id.
    pub fn instances(&self) -> impl Iterator<Item = (InstanceId, &Instance)> {
        self.instances.iter().map(|(id, i)| (*id, i))
    }

    pub fn alloc_wire_id(&mut self) -> WireId {
        self.wire_ids.alloc()
    }

    pub fn insert_wire(&mut self, id: WireId, wire: Wire) -> Option<Wire> {
        self.wires.insert(id, wire)
    }

    pub fn remove_wire(&mut self, id: WireId) -> Option<Wire> {
        self.wires.remove(&id)
    }

    pub fn wire(&self, id: WireId) -> Option<&Wire> {
        self.wires.get(&id)
    }

    pub fn wire_mut(&mut self, id: WireId) -> Option<&mut Wire> {
        self.wires.get_mut(&id)
    }

    /// Ascending by id.
    pub fn wires(&self) -> impl Iterator<Item = (WireId, &Wire)> {
        self.wires.iter().map(|(id, w)| (*id, w))
    }

    pub fn alloc_label_id(&mut self) -> LabelId {
        self.label_ids.alloc()
    }

    pub fn insert_label(&mut self, id: LabelId, label: Label) -> Option<Label> {
        self.labels.insert(id, label)
    }

    pub fn remove_label(&mut self, id: LabelId) -> Option<Label> {
        self.labels.remove(&id)
    }

    pub fn label(&self, id: LabelId) -> Option<&Label> {
        self.labels.get(&id)
    }

    pub fn label_mut(&mut self, id: LabelId) -> Option<&mut Label> {
        self.labels.get_mut(&id)
    }

    /// Ascending by id.
    pub fn labels(&self) -> impl Iterator<Item = (LabelId, &Label)> {
        self.labels.iter().map(|(id, l)| (*id, l))
    }
}

/// Holds the `Pin` list of one symbol. No graphic primitives (outline,
/// lines, arcs) yet — those are M1.2 (parsing) / M1.4 (rendering) territory.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SymbolView {
    pins: BTreeMap<PinId, Pin>,
    pin_ids: IdAllocator<Pin>,
}

impl SymbolView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc_pin_id(&mut self) -> PinId {
        self.pin_ids.alloc()
    }

    pub fn insert_pin(&mut self, id: PinId, pin: Pin) -> Option<Pin> {
        self.pins.insert(id, pin)
    }

    pub fn remove_pin(&mut self, id: PinId) -> Option<Pin> {
        self.pins.remove(&id)
    }

    pub fn pin(&self, id: PinId) -> Option<&Pin> {
        self.pins.get(&id)
    }

    pub fn pin_mut(&mut self, id: PinId) -> Option<&mut Pin> {
        self.pins.get_mut(&id)
    }

    pub fn pins(&self) -> impl Iterator<Item = (PinId, &Pin)> {
        self.pins.iter().map(|(id, p)| (*id, p))
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum View {
    Schematic(SchematicView),
    Symbol(SymbolView),
}

impl View {
    pub fn kind(&self) -> ViewKind {
        match self {
            View::Schematic(_) => ViewKind::Schematic,
            View::Symbol(_) => ViewKind::Symbol,
        }
    }

    pub fn as_schematic(&self) -> Option<&SchematicView> {
        match self {
            View::Schematic(v) => Some(v),
            View::Symbol(_) => None,
        }
    }

    pub fn as_schematic_mut(&mut self) -> Option<&mut SchematicView> {
        match self {
            View::Schematic(v) => Some(v),
            View::Symbol(_) => None,
        }
    }

    pub fn as_symbol(&self) -> Option<&SymbolView> {
        match self {
            View::Symbol(v) => Some(v),
            View::Schematic(_) => None,
        }
    }

    pub fn as_symbol_mut(&mut self) -> Option<&mut SymbolView> {
        match self {
            View::Symbol(v) => Some(v),
            View::Schematic(_) => None,
        }
    }
}
