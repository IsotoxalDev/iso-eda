// SPDX-License-Identifier: GPL-3.0-or-later

//! The palette's view of commands: an ordered, duplicate-free set.

use std::collections::BTreeMap;

use crate::command::Command;
use crate::error::CommandError;
use crate::id::CommandId;

/// Commands the palette can list and run. Ordered by id (BTreeMap) so
/// enumeration is deterministic for the palette and for snapshot tests.
#[derive(Default)]
pub struct CommandRegistry {
    commands: BTreeMap<CommandId, Box<dyn Command>>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// All builtins (`app.hello`, `edit.redo`, `edit.undo`, `view.toggle_theme`).
    pub fn with_builtins() -> Self {
        let mut registry = Self::new();
        for cmd in crate::builtins::builtin_commands() {
            let id = cmd.id();
            let inserted = registry.register(cmd).is_ok();
            debug_assert!(inserted, "builtin command ids must be unique: {id}");
        }
        registry
    }

    /// `Err(DuplicateId)` leaves the registry unchanged.
    pub fn register(&mut self, cmd: Box<dyn Command>) -> Result<(), CommandError> {
        let id = cmd.id();
        if self.commands.contains_key(&id) {
            return Err(CommandError::DuplicateId(id));
        }
        self.commands.insert(id, cmd);
        Ok(())
    }

    pub fn get(&self, id: &CommandId) -> Option<&dyn Command> {
        self.commands.get(id).map(|cmd| cmd.as_ref())
    }

    /// Ascending by id.
    pub fn iter(&self) -> impl Iterator<Item = &dyn Command> + '_ {
        self.commands.values().map(|cmd| cmd.as_ref())
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}
