// SPDX-License-Identifier: GPL-3.0-or-later

use crate::attr::AttributeBag;

/// Free text anchored at a point — serves both plain annotation and
/// net-name labelling; which role a given `Label` plays is xschem/M1.2
/// semantics, not decided here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub position: eda_geom::Point,
    pub text: String,
    pub attributes: AttributeBag,
}
