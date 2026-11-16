// SPDX-License-Identifier: GPL-3.0-or-later

//! `CommandHost`: the single object `eda-ui` talks to.

use crate::command::{Command, Outcome};
use crate::context::CommandContext;
use crate::error::CommandError;
use crate::id::CommandId;
use crate::registry::CommandRegistry;
use crate::undo::UndoStack;

/// The single object `eda-ui` talks to. Owns registry, stack and context
/// so the invariant "every undoable change went through `run`" is local.
pub struct CommandHost {
    registry: CommandRegistry,
    undo_stack: UndoStack,
    cx: CommandContext,
}

impl CommandHost {
    /// default stack
    pub fn new(registry: CommandRegistry, cx: CommandContext) -> Self {
        Self {
            registry,
            undo_stack: UndoStack::default(),
            cx,
        }
    }

    pub fn with_undo_stack(
        registry: CommandRegistry,
        cx: CommandContext,
        undo_stack: UndoStack,
    ) -> Self {
        Self {
            registry,
            undo_stack,
            cx,
        }
    }

    /// Builtin registry, default context — what `eda-app` constructs.
    pub fn with_builtins() -> Self {
        Self::new(CommandRegistry::with_builtins(), CommandContext::default())
    }

    pub fn registry(&self) -> &CommandRegistry {
        &self.registry
    }

    pub fn context(&self) -> &CommandContext {
        &self.cx
    }

    pub fn undo_stack(&self) -> &UndoStack {
        &self.undo_stack
    }

    /// Look up by id and run (`UnknownCommand` if absent). Applies the
    /// `Outcome`: `Edit` -> push; `Undo`/`Redo` -> pop the stack; `Done` -> nothing.
    pub fn run(&mut self, id: &CommandId) -> Result<(), CommandError> {
        // `cmd` borrows `self.registry`, not all of `self`, so the
        // subsequent `&mut self.cx` borrow (a disjoint field) is fine here.
        // It would NOT be fine to delegate to `self.run_command(cmd)` while
        // `cmd` is still alive, since that takes `&mut self` as a whole; so
        // the outcome is computed in this block and applied afterwards.
        let outcome = {
            let cmd = self
                .registry
                .get(id)
                .ok_or_else(|| CommandError::UnknownCommand(id.clone()))?;
            cmd.run(&mut self.cx)?
        };
        self.apply(outcome)
    }

    /// Same, for a command value the caller constructed (commands with
    /// arguments, from M1.5 on). Not required to be registered.
    pub fn run_command(&mut self, cmd: &dyn Command) -> Result<(), CommandError> {
        let outcome = cmd.run(&mut self.cx)?;
        self.apply(outcome)
    }

    pub fn undo(&mut self) -> Result<bool, CommandError> {
        self.undo_stack.undo(&mut self.cx)
    }

    pub fn redo(&mut self) -> Result<bool, CommandError> {
        self.undo_stack.redo(&mut self.cx)
    }

    fn apply(&mut self, outcome: Outcome) -> Result<(), CommandError> {
        match outcome {
            Outcome::Done => Ok(()),
            Outcome::Edit(edit) => {
                self.undo_stack.push(edit);
                Ok(())
            }
            Outcome::Undo => self.undo_stack.undo(&mut self.cx).map(|_| ()),
            Outcome::Redo => self.undo_stack.redo(&mut self.cx).map(|_| ()),
        }
    }
}
