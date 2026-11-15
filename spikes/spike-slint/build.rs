// SPDX-License-Identifier: GPL-3.0-or-later

fn main() -> Result<(), Box<dyn std::error::Error>> {
    slint_build::compile("ui/app.slint")?;
    Ok(())
}
