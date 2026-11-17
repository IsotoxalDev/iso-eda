// SPDX-License-Identifier: GPL-3.0-or-later

use crate::attr::AttributeBag;

/// One straight net segment — matching xschem's model, not a general
/// polyline. Zero-length (`p0 == p1`) is legal; nothing here rejects it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Wire {
    pub p0: eda_geom::Point,
    pub p1: eda_geom::Point,
    pub attributes: AttributeBag,
}
