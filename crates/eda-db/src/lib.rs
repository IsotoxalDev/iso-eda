// SPDX-License-Identifier: GPL-3.0-or-later

//! The library/cell/view/schematic data model for iso-eda. See
//! `docs/adr/003-schematic-data-model.md` for the design. Depends on
//! `eda-geom` only; every mutating method here is total (`Option`-
//! returning, never panics) so no `thiserror` dependency is needed.

mod attr;
mod cell;
mod design;
mod id;
mod instance;
mod label;
mod library;
mod pin;
mod view;
mod wire;

pub use attr::*;
pub use cell::*;
pub use design::*;
pub use id::*;
pub use instance::*;
pub use label::*;
pub use library::*;
pub use pin::*;
pub use view::*;
pub use wire::*;

#[test]
fn crate_name_is_set() {
    assert_eq!(env!("CARGO_PKG_NAME"), "eda-db");
}
