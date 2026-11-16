// SPDX-License-Identifier: GPL-3.0-or-later

//! Errors returned by `eda-cmd`'s public API.

use crate::id::CommandId;

/// Boxed source error for command/edit failures.
pub type BoxError = Box<dyn std::error::Error + Send + Sync + 'static>;

#[derive(Debug, thiserror::Error)]
pub enum CommandError {
    #[error("command `{0}` is already registered")]
    DuplicateId(CommandId),
    #[error("unknown command `{0}`")]
    UnknownCommand(CommandId),
    #[error("command `{id}` failed: {source}")]
    Failed {
        id: CommandId,
        #[source]
        source: BoxError,
    },
    #[error("undo of `{label}` failed: {source}")]
    UndoFailed {
        label: String,
        #[source]
        source: BoxError,
    },
    #[error("redo of `{label}` failed: {source}")]
    RedoFailed {
        label: String,
        #[source]
        source: BoxError,
    },
}

impl CommandError {
    /// Convenience for command bodies: wraps any error as `Failed`.
    pub fn failed(id: &CommandId, source: impl std::error::Error + Send + Sync + 'static) -> Self {
        Self::Failed { id: id.clone(), source: Box::new(source) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;

    #[derive(Debug)]
    struct TestError;

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "synthetic source error")
        }
    }

    impl std::error::Error for TestError {}

    /// Each variant's `Display` contains the id/label it names.
    #[test]
    fn command_error_display_is_readable() {
        let id = CommandId::new("app.hello");

        let duplicate = CommandError::DuplicateId(id.clone());
        assert!(duplicate.to_string().contains("app.hello"));

        let unknown = CommandError::UnknownCommand(id.clone());
        assert!(unknown.to_string().contains("app.hello"));

        let failed = CommandError::Failed { id: id.clone(), source: Box::new(TestError) };
        assert!(failed.to_string().contains("app.hello"));

        let undo_failed =
            CommandError::UndoFailed { label: "Toggle theme".to_owned(), source: Box::new(TestError) };
        assert!(undo_failed.to_string().contains("Toggle theme"));

        let redo_failed =
            CommandError::RedoFailed { label: "Toggle theme".to_owned(), source: Box::new(TestError) };
        assert!(redo_failed.to_string().contains("Toggle theme"));
    }
}
