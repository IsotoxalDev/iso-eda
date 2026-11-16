# Third-party code adapted into iso-eda

iso-eda is GPL-3.0-or-later. Per `CLAUDE.md`, code adapted from a
GPL-3.0-or-later-compatible source is logged here, keeping the original
copyright notice.

| Source | License | What was adapted | Used in |
| --- | --- | --- | --- |
| Slint's `examples/wgpu_texture/lib.rs` (`// Copyright © SixtyFPS GmbH <info@slint.dev>`, `SPDX-License-Identifier: MIT`), from the `slint` repository at the version pinned by `spikes/spike-slint/Cargo.toml` (1.18.1) | MIT | The `window.set_rendering_notifier` state machine pattern: matching on `RenderingState::RenderingSetup` / `BeforeRendering` / `RenderingTeardown`, rendering into an owned `wgpu::Texture` in `BeforeRendering`, importing it via `slint::Image::try_from`, assigning it to an `Image` element, and calling `window.request_redraw()`. No code was copied verbatim; the control-flow shape was followed because it is the only documented way to get a render target out of Slint's `GraphicsAPI::WGPU30`, per ADR-001's "Spikes" section. | `spikes/spike-slint/src/main.rs` (deleted in `8e28d49`/`443dd94`; still readable at git commit `e27f2bc`), `crates/eda-ui/src/canvas.rs` (M0.3, clear-color only; the rect/line/glyph pipelines are deferred to M1.4) |
