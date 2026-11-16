// SPDX-License-Identifier: GPL-3.0-or-later

//! Command and undo model for iso-eda. See
//! `docs/adr/002-command-undo-model.md` for the design.

mod command;
mod context;
mod error;
mod id;
mod registry;

pub use command::*;
pub use context::*;
pub use error::*;
pub use id::*;
pub use registry::*;

#[test]
fn crate_name_is_set() {
    assert_eq!(env!("CARGO_PKG_NAME"), "eda-cmd");
}
