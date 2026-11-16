// SPDX-License-Identifier: GPL-3.0-or-later

//! A bounded, linear undo/redo history.

use std::collections::VecDeque;
use std::num::NonZeroUsize;

use crate::command::Edit;
use crate::context::CommandContext;
use crate::error::CommandError;

pub const DEFAULT_UNDO_LIMIT: usize = 1_000;

/// Linear history: `undo` side (oldest evicted past `limit`) + `redo` side.
pub struct UndoStack {
    undo: VecDeque<Box<dyn Edit>>,
    redo: Vec<Box<dyn Edit>>,
    limit: NonZeroUsize,
}

impl Default for UndoStack {
    fn default() -> Self {
        // DEFAULT_UNDO_LIMIT is a nonzero literal; `unwrap_or` keeps this
        // infallible without `unwrap`/`expect`.
        Self::with_limit(NonZeroUsize::new(DEFAULT_UNDO_LIMIT).unwrap_or(NonZeroUsize::MIN))
    }
}

impl UndoStack {
    pub fn with_limit(limit: NonZeroUsize) -> Self {
        Self {
            undo: VecDeque::new(),
            redo: Vec::new(),
            limit,
        }
    }

    /// Records an applied edit: clears the redo side, evicts the oldest
    /// undo entry when the undo side would exceed `limit`.
    pub fn push(&mut self, edit: Box<dyn Edit>) {
        self.redo.clear();
        self.undo.push_back(edit);
        while self.undo.len() > self.limit.get() {
            self.undo.pop_front();
        }
    }

    /// `Ok(false)` when there is nothing to undo. On `Err`, the edit stays
    /// on the undo side (stack unchanged).
    pub fn undo(&mut self, cx: &mut CommandContext) -> Result<bool, CommandError> {
        let Some(mut edit) = self.undo.pop_back() else {
            return Ok(false);
        };
        match edit.undo(cx) {
            Ok(()) => {
                self.redo.push(edit);
                Ok(true)
            }
            Err(err) => {
                self.undo.push_back(edit);
                Err(err)
            }
        }
    }

    /// `Ok(false)` when there is nothing to redo. On `Err`, the edit stays
    /// on the redo side.
    pub fn redo(&mut self, cx: &mut CommandContext) -> Result<bool, CommandError> {
        let Some(mut edit) = self.redo.pop() else {
            return Ok(false);
        };
        match edit.redo(cx) {
            Ok(()) => {
                self.undo.push_back(edit);
                Ok(true)
            }
            Err(err) => {
                self.redo.push(edit);
                Err(err)
            }
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_label(&self) -> Option<&str> {
        self.undo.back().map(|edit| edit.label())
    }

    pub fn redo_label(&self) -> Option<&str> {
        self.redo.last().map(|edit| edit.label())
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn limit(&self) -> NonZeroUsize {
        self.limit
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
    }
}
