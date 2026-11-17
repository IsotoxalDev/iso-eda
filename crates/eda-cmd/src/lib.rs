// SPDX-License-Identifier: GPL-3.0-or-later

//! Command and undo model for iso-eda. See
//! `docs/adr/002-command-undo-model.md` for the design.

mod builtins;
mod command;
mod context;
mod error;
mod host;
mod id;
mod registry;
mod schematic;
mod undo;

pub use builtins::*;
pub use command::*;
pub use context::*;
pub use error::*;
pub use host::*;
pub use id::*;
pub use registry::*;
// `Patch` stays out of the public API: it is `pub(crate)`, so this glob
// only re-exports `SchematicEditError` and the four `Command` types.
pub use schematic::*;
pub use undo::*;

#[test]
fn crate_name_is_set() {
    assert_eq!(env!("CARGO_PKG_NAME"), "eda-cmd");
}
