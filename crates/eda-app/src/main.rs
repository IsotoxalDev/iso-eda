// SPDX-License-Identifier: GPL-3.0-or-later

//! `iso-eda`: the GUI binary. Selects Slint's wgpu 30 backend (needed by
//! `eda_ui::canvas`'s texture-import harness) before constructing the
//! shell, then runs the event loop. See `docs/adr/001-ui-toolkit.md`.

fn main() -> anyhow::Result<()> {
    slint::BackendSelector::new()
        .require_wgpu_30(slint::wgpu_30::WGPUConfiguration::Automatic(
            slint::wgpu_30::WGPUSettings::default(),
        ))
        .select()
        .map_err(|err| anyhow::anyhow!("failed to select the Slint WGPU backend: {err}"))?;

    let app = eda_ui::App::new()?;
    app.run()?;
    Ok(())
}
