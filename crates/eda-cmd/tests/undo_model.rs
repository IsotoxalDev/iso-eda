// SPDX-License-Identifier: GPL-3.0-or-later

//! Test list for ADR-002 (`docs/adr/002-command-undo-model.md`), public API
//! only: everything here goes through `eda_cmd`'s crate-root re-exports, the
//! same surface `eda-ui` will use. The two rows marked `(*unit*)` in that
//! ADR's test table (`theme_edit_redo_sets_captured_target_not_toggle`,
//! `command_error_display_is_readable`) live inside `builtins.rs`/`error.rs`
//! as `#[cfg(test)]` unit tests instead, and are not duplicated here.

use eda_cmd::*;
use std::fmt;
use std::num::NonZeroUsize;

// ---------------------------------------------------------------------------
// Test-only helpers named by the ADR but not fully specified there.
// ---------------------------------------------------------------------------

/// Minimal `std::error::Error` used as the `source` for synthetic failures
/// below. No new dependency: just enough to satisfy `BoxError`'s bounds.
#[derive(Debug)]
struct TestError(&'static str);

impl fmt::Display for TestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for TestError {}

/// A `Command` whose `run` always fails. Proves the host leaves `cx`
/// untouched and the redo side intact when a command errors (invariant 1).
struct FailingCommand;

impl FailingCommand {
    const ID: &'static str = "test.failing_command";
}

impl Command for FailingCommand {
    fn id(&self) -> CommandId {
        CommandId::new(Self::ID)
    }

    fn title(&self) -> &str {
        "Failing command (test)"
    }

    fn run(&self, _cx: &mut CommandContext) -> Result<Outcome, CommandError> {
        Err(CommandError::failed(
            &self.id(),
            TestError("synthetic command failure"),
        ))
    }
}

/// An `Edit` whose `undo`/`redo` always fail. Proves a failed undo/redo
/// leaves the edit exactly where it was (invariant 8: no silent corruption).
struct FailingEdit;

impl Edit for FailingEdit {
    fn label(&self) -> &str {
        "Failing edit"
    }

    fn undo(&mut self, _cx: &mut CommandContext) -> Result<(), CommandError> {
        Err(CommandError::UndoFailed {
            label: self.label().to_owned(),
            source: Box::new(TestError("synthetic undo failure")),
        })
    }

    fn redo(&mut self, _cx: &mut CommandContext) -> Result<(), CommandError> {
        Err(CommandError::RedoFailed {
            label: self.label().to_owned(),
            source: Box::new(TestError("synthetic redo failure")),
        })
    }
}

/// Test-only `Command` that applies a `FailingEdit` through the normal
/// `run`/`Outcome::Edit` path, so it lands on the undo stack the same way a
/// real command's edit would (rather than calling `UndoStack::push` directly).
struct PushFailingEdit;

impl Command for PushFailingEdit {
    fn id(&self) -> CommandId {
        CommandId::new("test.push_failing_edit")
    }

    fn title(&self) -> &str {
        "Push failing edit (test)"
    }

    fn run(&self, _cx: &mut CommandContext) -> Result<Outcome, CommandError> {
        Ok(Outcome::Edit(Box::new(FailingEdit)))
    }
}

/// A `Command` with an argument (`to`), deliberately not registered in any
/// `CommandRegistry`. Proves the M1.5 "commands with arguments, run via
/// `run_command`, need no registry entry" path. Undoable: returns a
/// `ThemeEdit` exactly as `ToggleThemeCommand` does, just with an explicit
/// target instead of `from.toggled()`.
struct SetThemeCommand {
    to: Theme,
}

impl Command for SetThemeCommand {
    fn id(&self) -> CommandId {
        CommandId::new("test.set_theme")
    }

    fn title(&self) -> &str {
        "Set theme (test)"
    }

    fn run(&self, cx: &mut CommandContext) -> Result<Outcome, CommandError> {
        let from = cx.theme;
        cx.theme = self.to;
        Ok(Outcome::Edit(Box::new(ThemeEdit::new(from, self.to))))
    }
}

// ---------------------------------------------------------------------------
// id.rs
// ---------------------------------------------------------------------------

#[test]
fn command_id_display_eq_and_order() {
    let a = CommandId::new("a.b");
    assert_eq!(a.to_string(), "a.b");
    assert_eq!(a, CommandId::from("a.b".to_string()));

    let hello = CommandId::new("app.hello");
    let edit_redo = CommandId::new("edit.redo");
    let edit_undo = CommandId::new("edit.undo");
    let toggle = CommandId::new("view.toggle_theme");

    assert!(hello < edit_redo);
    assert!(edit_redo < edit_undo);
    assert!(edit_undo < toggle);
}

// ---------------------------------------------------------------------------
// registry.rs
// ---------------------------------------------------------------------------

#[test]
fn registry_lists_builtins_sorted_by_id() {
    let registry = CommandRegistry::with_builtins();
    let ids: Vec<CommandId> = registry.iter().map(|cmd| cmd.id()).collect();

    assert_eq!(
        ids,
        vec![
            CommandId::new("app.hello"),
            CommandId::new("edit.redo"),
            CommandId::new("edit.undo"),
            CommandId::new("view.toggle_theme"),
        ]
    );

    for cmd in registry.iter() {
        assert!(
            !cmd.title().is_empty(),
            "title for {} must not be empty",
            cmd.id()
        );
    }
}

#[test]
fn registry_rejects_duplicate_id() {
    let mut registry = CommandRegistry::with_builtins();
    let len_before = registry.len();

    let err = registry
        .register(Box::new(HelloCommand))
        .expect_err("re-registering app.hello must fail");

    match err {
        CommandError::DuplicateId(id) => assert_eq!(id, CommandId::new("app.hello")),
        other => panic!("expected CommandError::DuplicateId, got {other:?}"),
    }
    assert_eq!(registry.len(), len_before);
}

#[test]
fn registry_get_unknown_is_none() {
    let registry = CommandRegistry::with_builtins();
    assert!(registry.get(&CommandId::new("nope")).is_none());
}

// ---------------------------------------------------------------------------
// host.rs: run / unknown id / atomicity
// ---------------------------------------------------------------------------

#[test]
fn host_run_unknown_id_is_error_and_changes_nothing() {
    let mut host = CommandHost::with_builtins();

    let err = host
        .run(&CommandId::new("nope"))
        .expect_err("unknown id must error");

    match err {
        CommandError::UnknownCommand(id) => assert_eq!(id, CommandId::new("nope")),
        other => panic!("expected CommandError::UnknownCommand, got {other:?}"),
    }
    assert_eq!(host.context(), &CommandContext::default());
    assert!(!host.undo_stack().can_undo());
}

// ---------------------------------------------------------------------------
// builtins.rs behaviour, exercised through the public host API
// ---------------------------------------------------------------------------

#[test]
fn hello_sets_status_and_is_not_undoable() {
    let mut host = CommandHost::with_builtins();
    host.run(&CommandId::new("app.hello")).unwrap();

    assert_eq!(
        host.context().status_message,
        Some(HELLO_MESSAGE.to_owned())
    );
    assert!(!host.undo_stack().can_undo());
    assert_eq!(host.undo().unwrap(), false);
}

#[test]
fn toggle_theme_flips_and_undo_restores() {
    let mut host = CommandHost::with_builtins();
    assert_eq!(host.context().theme, Theme::Dark);

    host.run(&CommandId::new("view.toggle_theme")).unwrap();
    assert_eq!(host.context().theme, Theme::Light);

    assert_eq!(host.undo().unwrap(), true);
    assert_eq!(host.context().theme, Theme::Dark);
    assert!(host.undo_stack().can_redo());
}

#[test]
fn undo_then_redo_returns_identical_state() {
    let mut host = CommandHost::with_builtins();
    host.run(&CommandId::new("view.toggle_theme")).unwrap();
    host.run(&CommandId::new("view.toggle_theme")).unwrap();

    let snapshot = host.context().clone();

    assert_eq!(host.undo().unwrap(), true);
    assert_eq!(host.undo().unwrap(), true);
    assert_eq!(host.redo().unwrap(), true);
    assert_eq!(host.redo().unwrap(), true);

    assert_eq!(host.context(), &snapshot);
}

// ---------------------------------------------------------------------------
// undo.rs: empty stack, linearity, bounding, labels
// ---------------------------------------------------------------------------

#[test]
fn undo_and_redo_on_empty_stack_are_ok_false() {
    let mut host = CommandHost::with_builtins();
    assert_eq!(host.undo().unwrap(), false);
    assert_eq!(host.redo().unwrap(), false);
}

#[test]
fn new_edit_clears_redo() {
    let mut host = CommandHost::with_builtins();
    host.run(&CommandId::new("view.toggle_theme")).unwrap();
    host.undo().unwrap();
    host.run(&CommandId::new("view.toggle_theme")).unwrap();

    assert!(!host.undo_stack().can_redo());
    assert_eq!(host.redo().unwrap(), false);
}

#[test]
fn non_undoable_command_keeps_redo() {
    let mut host = CommandHost::with_builtins();
    host.run(&CommandId::new("view.toggle_theme")).unwrap(); // Dark -> Light
    host.undo().unwrap(); // Light -> Dark, "Toggle theme" now on redo side
    host.run(&CommandId::new("app.hello")).unwrap(); // not undoable

    assert!(host.undo_stack().can_redo());
    assert_eq!(host.redo().unwrap(), true);
    assert_eq!(host.context().theme, Theme::Light);
}

#[test]
fn undo_limit_evicts_oldest() {
    let mut host = CommandHost::with_undo_stack(
        CommandRegistry::with_builtins(),
        CommandContext::default(),
        UndoStack::with_limit(NonZeroUsize::new(3).unwrap()),
    );

    for _ in 0..5 {
        host.run(&CommandId::new("view.toggle_theme")).unwrap();
    }

    assert_eq!(host.undo_stack().undo_len(), 3);

    assert_eq!(host.undo().unwrap(), true);
    assert_eq!(host.undo().unwrap(), true);
    assert_eq!(host.undo().unwrap(), true);
    assert_eq!(host.undo().unwrap(), false);

    // Five toggles from Dark end at Light; undoing the three surviving
    // edits (toggles 3, 4, 5) lands back on the state after two toggles,
    // i.e. Dark. Toggles 1 and 2 were evicted and are gone for good.
    assert_eq!(host.context().theme, Theme::Dark);
}

#[test]
fn undo_redo_commands_drive_the_stack() {
    let mut host = CommandHost::with_builtins();

    host.run(&CommandId::new("view.toggle_theme")).unwrap();
    assert_eq!(host.context().theme, Theme::Light);

    host.run(&CommandId::new("edit.undo")).unwrap();
    assert_eq!(host.context().theme, Theme::Dark);
    assert!(!host.undo_stack().can_undo());
    assert!(host.undo_stack().can_redo());

    host.run(&CommandId::new("edit.redo")).unwrap();
    assert_eq!(host.context().theme, Theme::Light);
    assert!(host.undo_stack().can_undo());
    assert!(!host.undo_stack().can_redo());
}

#[test]
fn failed_command_is_not_recorded_and_keeps_redo() {
    let mut host = CommandHost::with_builtins();
    host.run(&CommandId::new("view.toggle_theme")).unwrap();
    host.undo().unwrap();
    let context_before = host.context().clone();

    let err = host
        .run_command(&FailingCommand)
        .expect_err("FailingCommand must fail");

    match err {
        CommandError::Failed { id, .. } => {
            assert_eq!(id, CommandId::new(FailingCommand::ID));
        }
        other => panic!("expected CommandError::Failed, got {other:?}"),
    }
    assert_eq!(host.undo_stack().undo_len(), 0);
    assert_eq!(host.undo_stack().redo_len(), 1);
    assert_eq!(host.context(), &context_before);
}

#[test]
fn failed_undo_leaves_edit_on_undo_side() {
    let mut host = CommandHost::with_builtins();
    host.run_command(&PushFailingEdit).unwrap();

    let err = host.undo().expect_err("FailingEdit::undo must fail");
    match err {
        CommandError::UndoFailed { label, .. } => assert_eq!(label, "Failing edit"),
        other => panic!("expected CommandError::UndoFailed, got {other:?}"),
    }
    assert!(host.undo_stack().can_undo());
    assert!(!host.undo_stack().can_redo());
}

#[test]
fn undo_labels_follow_stack() {
    let mut host = CommandHost::with_builtins();
    host.run(&CommandId::new("view.toggle_theme")).unwrap();

    assert_eq!(host.undo_stack().undo_label(), Some("Toggle theme"));
    assert_eq!(host.undo_stack().redo_label(), None);

    host.undo().unwrap();

    assert_eq!(host.undo_stack().undo_label(), None);
    assert_eq!(host.undo_stack().redo_label(), Some("Toggle theme"));
}

// ---------------------------------------------------------------------------
// host.rs: run_command with an unregistered, argument-carrying command
// ---------------------------------------------------------------------------

#[test]
fn run_command_accepts_unregistered_values() {
    let mut host = CommandHost::with_builtins();
    assert!(host
        .registry()
        .get(&CommandId::new("test.set_theme"))
        .is_none());

    host.run_command(&SetThemeCommand { to: Theme::Light })
        .unwrap();
    assert_eq!(host.context().theme, Theme::Light);
    assert!(host.undo_stack().can_undo());

    assert_eq!(host.undo().unwrap(), true);
    assert_eq!(host.context().theme, Theme::Dark);
}

// ---------------------------------------------------------------------------
// Model test: 2000 random steps against a hand-rolled reference model.
// ---------------------------------------------------------------------------

#[test]
fn undo_stack_matches_linear_history_model() {
    let mut host = CommandHost::with_builtins();

    // Reference model: the sequence of theme states a linear undo/redo
    // history would hold, plus a cursor into it. `toggle` truncates
    // everything after the cursor and appends the new state (mirrors
    // `UndoStack::push` clearing the redo side); `undo`/`redo` move the
    // cursor; `hello` touches neither. Starts matching `CommandHost`'s
    // default context (`Theme::Dark`).
    let mut states: Vec<Theme> = vec![Theme::Dark];
    let mut cursor: usize = 0;

    // Hand-rolled LCG (Numerical Recipes constants); no new dependency.
    let mut lcg_state: u32 = 0x1234_5678;
    let mut next_u32 = |state: &mut u32| -> u32 {
        *state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        *state
    };

    for step in 0..2000u32 {
        match next_u32(&mut lcg_state) % 4 {
            0 => {
                // toggle
                host.run(&CommandId::new("view.toggle_theme")).unwrap();
                let new_theme = states[cursor].toggled();
                states.truncate(cursor + 1);
                states.push(new_theme);
                cursor = states.len() - 1;
            }
            1 => {
                // undo
                let did_undo = host.undo().unwrap();
                if cursor > 0 {
                    assert!(did_undo, "step {step}: model expected undo to succeed");
                    cursor -= 1;
                } else {
                    assert!(!did_undo, "step {step}: model expected undo to be a no-op");
                }
            }
            2 => {
                // redo
                let did_redo = host.redo().unwrap();
                if cursor + 1 < states.len() {
                    assert!(did_redo, "step {step}: model expected redo to succeed");
                    cursor += 1;
                } else {
                    assert!(!did_redo, "step {step}: model expected redo to be a no-op");
                }
            }
            _ => {
                // hello: no effect on theme or either stack side
                host.run(&CommandId::new("app.hello")).unwrap();
            }
        }

        assert_eq!(
            host.context().theme,
            states[cursor],
            "step {step}: real host and reference model disagree"
        );
    }

    // Sanity check on the "default limit, so no eviction" premise: a
    // 2000-step random walk over {toggle, undo, redo, hello} should never
    // push the undo side anywhere near DEFAULT_UNDO_LIMIT (1000).
    assert!(host.undo_stack().undo_len() <= DEFAULT_UNDO_LIMIT);
}
