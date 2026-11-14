// SPDX-License-Identifier: GPL-3.0-or-later

#[test]
fn crate_name_is_set() {
    assert_eq!(env!("CARGO_PKG_NAME"), "eda-script");
}
