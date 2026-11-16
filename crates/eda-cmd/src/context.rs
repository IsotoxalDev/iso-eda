// SPDX-License-Identifier: GPL-3.0-or-later

//! The application state commands read and mutate.

/// The UI theme. Two variants today; M0.3's acceptance check is "theme
/// switch works". If the token file grows more themes, a `view.set_theme`
/// command and a richer `Theme` replace `toggled()`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Theme {
    #[default]
    Dark,
    Light,
}

impl Theme {
    pub fn toggled(self) -> Theme {
        match self {
            Theme::Dark => Theme::Light,
            Theme::Light => Theme::Dark,
        }
    }

    /// "dark" | "light" — the key `eda-ui` uses to pick a token set.
    pub fn as_str(self) -> &'static str {
        match self {
            Theme::Dark => "dark",
            Theme::Light => "light",
        }
    }
}

/// Everything a command may read or mutate. Owned by `CommandHost`.
/// M1.1 adds the design (`eda_db`) here; nothing else about this type changes.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct CommandContext {
    pub theme: Theme,
    /// Last message a command asked the UI to show (status line). Not undoable.
    pub status_message: Option<String>,
}
