// SPDX-License-Identifier: GPL-3.0-or-later

use crate::attr::AttributeBag;

/// A symbol-level connection point, in the symbol's own local coordinate
/// frame (before any instance `Transform` is applied).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    pub name: String,
    pub position: eda_geom::Point,
    pub attributes: AttributeBag,
}
