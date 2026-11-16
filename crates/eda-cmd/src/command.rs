// SPDX-License-Identifier: GPL-3.0-or-later

//! The `Command` / `Edit` split: a stateless verb versus the recorded,
//! undoable change it produces.

use crate::context::CommandContext;
use crate::error::CommandError;
use crate::id::CommandId;

/// What a command's run produced.
pub enum Outcome {
    /// State was not changed in any undoable way (or not at all).
    Done,
    /// An undoable change was applied; the host records it.
    Edit(Box<dyn Edit>),
    /// Ask the host to pop the undo / redo stack (returned only by the
    /// `edit.undo` / `edit.redo` builtins).
    Undo,
    Redo,
}

/// A user-invocable verb. Stateless: all state is in `cx`; all change is
/// in the returned `Edit`. Must be atomic: on `Err`, `cx` is unchanged.
pub trait Command {
    fn id(&self) -> CommandId;
    /// Palette text, e.g. "Toggle theme".
    fn title(&self) -> &str;
    fn run(&self, cx: &mut CommandContext) -> Result<Outcome, CommandError>;
}

/// A recorded change. `redo` must reproduce exactly the state `run` left
/// behind, using data captured at run time (ids, old/new values); it must
/// not recompute, re-allocate, or re-read the selection.
pub trait Edit {
    /// Shown as "Undo <label>" / "Redo <label>".
    fn label(&self) -> &str;
    fn undo(&mut self, cx: &mut CommandContext) -> Result<(), CommandError>;
    fn redo(&mut self, cx: &mut CommandContext) -> Result<(), CommandError>;
}
