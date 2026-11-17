// SPDX-License-Identifier: GPL-3.0-or-later

use std::collections::BTreeMap;

use crate::cell::Cell;

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Library {
    pub name: String,
    cells: BTreeMap<String, Cell>,
}

impl Library {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cells: BTreeMap::new(),
        }
    }

    /// Keyed by `cell.name.clone()`; returns the previous cell of that
    /// name, if any.
    pub fn insert_cell(&mut self, cell: Cell) -> Option<Cell> {
        self.cells.insert(cell.name.clone(), cell)
    }

    pub fn remove_cell(&mut self, name: &str) -> Option<Cell> {
        self.cells.remove(name)
    }

    pub fn cell(&self, name: &str) -> Option<&Cell> {
        self.cells.get(name)
    }

    pub fn cell_mut(&mut self, name: &str) -> Option<&mut Cell> {
        self.cells.get_mut(name)
    }

    /// Ascending by name.
    pub fn cells(&self) -> impl Iterator<Item = &Cell> {
        self.cells.values()
    }
}
