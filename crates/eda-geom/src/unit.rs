// SPDX-License-Identifier: GPL-3.0-or-later

/// Integer database unit. No floats anywhere in this crate or in `eda-db`.
/// Chosen as a plain alias, not a newtype: schematics are component-scale;
/// layout-scale coordinates are `eda-gds`'s problem (Phase 3), in its own
/// crate, free to pick something wider.
pub type Unit = i32;
