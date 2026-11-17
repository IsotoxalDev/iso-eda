// SPDX-License-Identifier: GPL-3.0-or-later

//! Builtin commands: `app.hello`, `view.toggle_theme`, `edit.undo`,
//! `edit.redo`.

use crate::command::{Command, Edit, Outcome};
use crate::context::{CommandContext, Theme};
use crate::error::CommandError;
use crate::id::CommandId;

pub const HELLO_MESSAGE: &str = "Hello from eda-cmd";

pub struct HelloCommand;

impl Command for HelloCommand {
    fn id(&self) -> CommandId {
        CommandId::new("app.hello")
    }

    fn title(&self) -> &str {
        "Hello"
    }

    fn run(&self, cx: &mut CommandContext) -> Result<Outcome, CommandError> {
        cx.status_message = Some(HELLO_MESSAGE.to_owned());
        Ok(Outcome::Done)
    }
}

pub struct ToggleThemeCommand;

impl Command for ToggleThemeCommand {
    fn id(&self) -> CommandId {
        CommandId::new("view.toggle_theme")
    }

    fn title(&self) -> &str {
        "Toggle theme"
    }

    fn run(&self, cx: &mut CommandContext) -> Result<Outcome, CommandError> {
        let from = cx.theme;
        let to = from.toggled();
        cx.theme = to;
        Ok(Outcome::Edit(Box::new(ThemeEdit::new(from, to))))
    }
}

pub struct UndoCommand;

impl Command for UndoCommand {
    fn id(&self) -> CommandId {
        CommandId::new("edit.undo")
    }

    fn title(&self) -> &str {
        "Undo"
    }

    fn run(&self, _cx: &mut CommandContext) -> Result<Outcome, CommandError> {
        Ok(Outcome::Undo)
    }
}

pub struct RedoCommand;

impl Command for RedoCommand {
    fn id(&self) -> CommandId {
        CommandId::new("edit.redo")
    }

    fn title(&self) -> &str {
        "Redo"
    }

    fn run(&self, _cx: &mut CommandContext) -> Result<Outcome, CommandError> {
        Ok(Outcome::Redo)
    }
}

/// The edit `ToggleThemeCommand` returns. Captures both ends so redo sets
/// `to` rather than toggling again.
pub struct ThemeEdit {
    from: Theme,
    to: Theme,
}

impl ThemeEdit {
    pub fn new(from: Theme, to: Theme) -> Self {
        Self { from, to }
    }
}

impl Edit for ThemeEdit {
    fn label(&self) -> &str {
        "Toggle theme"
    }

    fn undo(&mut self, cx: &mut CommandContext) -> Result<(), CommandError> {
        cx.theme = self.from;
        Ok(())
    }

    fn redo(&mut self, cx: &mut CommandContext) -> Result<(), CommandError> {
        cx.theme = self.to;
        Ok(())
    }
}

pub fn builtin_commands() -> Vec<Box<dyn Command>> {
    vec![
        Box::new(HelloCommand),
        Box::new(ToggleThemeCommand),
        Box::new(UndoCommand),
        Box::new(RedoCommand),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Guards invariant 3: `redo` must replay captured data, not re-toggle.
    /// `Theme::toggled()` is self-inverse, so the identical-state test alone
    /// cannot distinguish "replay `to`" from "toggle again" — this test can.
    #[test]
    fn theme_edit_redo_sets_captured_target_not_toggle() {
        let mut edit = ThemeEdit::new(Theme::Dark, Theme::Light);
        let mut cx = CommandContext {
            theme: Theme::Light,
            status_message: None,
            ..Default::default()
        };

        edit.redo(&mut cx).unwrap();

        assert_eq!(cx.theme, Theme::Light);
    }
}
