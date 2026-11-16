// SPDX-License-Identifier: GPL-3.0-or-later

fn main() {
    if let Err(err) = slint_build::compile("ui/app.slint") {
        panic!("failed to compile ui/app.slint: {err}");
    }
    println!("cargo:rerun-if-changed=ui/app.slint");
    println!("cargo:rerun-if-changed=ui/theme.slint");
    println!("cargo:rerun-if-changed=ui/palette.slint");
}
