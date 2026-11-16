// SPDX-License-Identifier: GPL-3.0-or-later

//! `eda-ui`: the Slint shell (panels, command palette, theme, keymap,
//! input -> `eda-cmd`) and the wgpu canvas it hosts. See
//! `docs/adr/001-ui-toolkit.md`.

mod canvas;
mod keymap;

pub use canvas::*;
pub use keymap::*;

#[test]
fn crate_name_is_set() {
    assert_eq!(env!("CARGO_PKG_NAME"), "eda-ui");
}
