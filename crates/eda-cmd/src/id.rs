// SPDX-License-Identifier: GPL-3.0-or-later

//! Stable command identifiers.

use std::borrow::Cow;
use std::fmt;

/// Stable, user-visible command identifier: `<area>.<verb_noun>`, ASCII
/// lower-case snake case, e.g. `app.hello`, `view.toggle_theme`,
/// `edit.undo`. Keymap files and the palette refer to commands by this.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandId(Cow<'static, str>);

impl CommandId {
    pub const fn new(id: &'static str) -> Self {
        Self(Cow::Borrowed(id))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<String> for CommandId {
    fn from(id: String) -> Self {
        Self(Cow::Owned(id))
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
