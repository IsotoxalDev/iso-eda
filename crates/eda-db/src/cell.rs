// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use crate::view::{SchematicView, SymbolView, View, ViewKind};

/// At most one view per `ViewKind` (the Cadence/xschem cellview
/// convention: a cell has a `.sch` and/or a `.sym`).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Cell {
    pub name: String,
    views: BTreeMap<ViewKind, View>,
}

impl Cell {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            views: BTreeMap::new(),
        }
    }

    /// Keyed by `view.kind()`; returns the previous view of that kind, if any.
    pub fn insert_view(&mut self, view: View) -> Option<View> {
        self.views.insert(view.kind(), view)
    }

    pub fn remove_view(&mut self, kind: ViewKind) -> Option<View> {
        self.views.remove(&kind)
    }

    pub fn schematic(&self) -> Option<&SchematicView> {
        self.views
            .get(&ViewKind::Schematic)
            .and_then(View::as_schematic)
    }

    pub fn schematic_mut(&mut self) -> Option<&mut SchematicView> {
        self.views
            .get_mut(&ViewKind::Schematic)
            .and_then(View::as_schematic_mut)
    }

    pub fn symbol(&self) -> Option<&SymbolView> {
        self.views.get(&ViewKind::Symbol).and_then(View::as_symbol)
    }

    pub fn symbol_mut(&mut self) -> Option<&mut SymbolView> {
        self.views
            .get_mut(&ViewKind::Symbol)
            .and_then(View::as_symbol_mut)
    }
}
