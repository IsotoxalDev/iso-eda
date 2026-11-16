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
