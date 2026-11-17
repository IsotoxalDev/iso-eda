// SPDX-License-Identifier: GPL-3.0-or-later

//! Integer-only database geometry for iso-eda. See
//! `docs/adr/003-schematic-data-model.md` for the design. Nothing in this
//! crate panics: all `Unit` arithmetic saturates.

mod point;
mod rect;
mod transform;
mod unit;

pub use point::*;
pub use rect::*;
pub use transform::*;
pub use unit::*;

#[test]
fn crate_name_is_set() {
    assert_eq!(env!("CARGO_PKG_NAME"), "eda-geom");
}
