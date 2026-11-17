// SPDX-License-Identifier: GPL-3.0-or-later

use crate::library::Library;
use crate::view::SchematicView;

/// The state one `CommandHost` edits: one `Library` plus which `Cell` is
/// open. M1.1's stack is per-document (ADR-002); multi-cell navigation
/// (descend/ascend) is M1.9's job.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Design {
    pub library: Library,
    pub active_cell: String,
}

impl Design {
    pub fn new(library: Library, active_cell: impl Into<String>) -> Self {
        Self {
            library,
            active_cell: active_cell.into(),
        }
    }

    /// `None` if `active_cell` does not name a cell, or that cell has no
    /// schematic view.
    pub fn active_schematic(&self) -> Option<&SchematicView> {
        self.library
            .cell(&self.active_cell)
            .and_then(|c| c.schematic())
    }

    pub fn active_schematic_mut(&mut self) -> Option<&mut SchematicView> {
        self.library
            .cell_mut(&self.active_cell)
            .and_then(|c| c.schematic_mut())
    }
}
