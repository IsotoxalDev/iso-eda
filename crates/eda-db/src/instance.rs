// SPDX-License-Identifier: GPL-3.0-or-later

use crate::attr::AttributeBag;

/// A placed reference to another `Cell`'s symbol, plus placement and
/// metadata. The instance's own name (xschem's `name=U1`) is NOT a typed
/// field — it lives in `attributes` like every other xschem attribute,
/// per the "don't invent xschem semantics here" rule.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Instance {
    /// Name of the referenced `Cell` within the same `Library`. Not
    /// validated or resolved here (M1.6/M1.9's job); loading or editing a
    /// schematic with a dangling reference must not panic.
    pub cell_name: String,
    pub transform: eda_geom::Transform,
    pub attributes: AttributeBag,
}
