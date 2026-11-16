// SPDX-License-Identifier: GPL-3.0-or-later

//! The texture-import harness: a `wgpu::Texture` sized to the Slint
//! `Image` element, cleared to a solid color every frame and imported
//! into Slint via `slint::Image::try_from`.
//!
//! Adapted from Slint's own `examples/wgpu_texture/lib.rs` (MIT
//! licensed; see `docs/THIRD-PARTY.md`) and from the M0.2
//! `spikes/spike-slint` binary (same pattern, git history — `spikes/`
//! itself was deleted after ADR-001 was accepted). Per ADR-001's M0.3
//! scope decision, this is clear-color only: the rect/line/glyph
//! pipelines from the deleted `spikes/canvas-wgpu` are explicitly
//! deferred to M1.4, when `eda-render` designs the real draw-list
//! interface. `eda-render` stays untouched this milestone.

use std::fmt;

/// Failures from the texture-import path. Not expected in normal
/// operation (both come from the GPU/Slint side); `eda-ui`'s rendering
/// notifier logs and skips the frame rather than panicking.
#[derive(Debug, thiserror::Error)]
pub enum CanvasError {
    #[error("failed to import the wgpu texture into Slint: {0}")]
    ImageImport(String),
}

impl fmt::Debug for Canvas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Canvas")
            .field("has_device", &self.device.is_some())
            .field(
                "texture_size",
                &self.texture.as_ref().map(wgpu::Texture::size),
            )
            .field("clear_color", &self.clear_color)
            .finish()
    }
}

/// Owns the GPU handles captured at `RenderingSetup` and the output
/// texture, across frames. Lives for the lifetime of the Slint window.
pub struct Canvas {
    device: Option<wgpu::Device>,
    queue: Option<wgpu::Queue>,
    texture: Option<wgpu::Texture>,
    clear_color: wgpu::Color,
}

impl Default for Canvas {
    fn default() -> Self {
        Self::new()
    }
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            device: None,
            queue: None,
            texture: None,
            // Overwritten by the first `set_clear_color` call once the
            // shell knows the active theme; this is just a safe default.
            clear_color: wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
        }
    }

    pub fn set_clear_color(&mut self, color: wgpu::Color) {
        self.clear_color = color;
    }

    pub fn has_gpu(&self) -> bool {
        self.device.is_some() && self.queue.is_some()
    }

    /// Captures `(device, queue)` from `GraphicsAPI::WGPU30`. Called on
    /// `RenderingState::RenderingSetup`.
    pub fn setup(&mut self, api: &slint::GraphicsAPI<'_>) {
        if let slint::GraphicsAPI::WGPU30 { device, queue, .. } = api {
            self.device = Some(device.clone());
            self.queue = Some(queue.clone());
        }
    }

    /// Drops GPU state. Called on `RenderingState::RenderingTeardown`.
    pub fn teardown(&mut self) {
        self.device = None;
        self.queue = None;
        self.texture = None;
    }

    /// (Re)creates the output texture if `width`/`height` changed,
    /// clears it, and imports it as a `slint::Image`. Called on
    /// `RenderingState::BeforeRendering`. `width`/`height` come from
    /// the `Image` element's real size (read back from Slint), not from
    /// a requested window size — ADR-001's tiling-compositor lesson.
    /// `None` when no GPU is available yet, or the size is degenerate.
    pub fn before_rendering(
        &mut self,
        width: u32,
        height: u32,
    ) -> Option<Result<slint::Image, CanvasError>> {
        let (device, queue) = match (self.device.as_ref(), self.queue.as_ref()) {
            (Some(d), Some(q)) => (d, q),
            _ => return None,
        };
        if width == 0 || height == 0 {
            return None;
        }

        let wanted_size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let needs_new_texture = !matches!(&self.texture, Some(t) if t.size() == wanted_size);
        if needs_new_texture {
            self.texture = Some(device.create_texture(&wgpu::TextureDescriptor {
                label: Some("eda-ui canvas texture"),
                size: wanted_size,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            }));
        }
        let texture = self.texture.as_ref()?;

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("eda-ui canvas encoder"),
        });
        {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("eda-ui canvas clear pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(self.clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        queue.submit(Some(encoder.finish()));

        Some(
            slint::Image::try_from(texture.clone())
                .map_err(|err| CanvasError::ImageImport(format!("{err:?}"))),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_canvas_has_no_gpu_and_before_rendering_is_a_noop() {
        let mut canvas = Canvas::new();
        assert!(!canvas.has_gpu());
        assert!(canvas.before_rendering(1600, 1000).is_none());
    }

    #[test]
    fn zero_sized_request_is_a_noop_even_with_no_gpu() {
        let mut canvas = Canvas::new();
        assert!(canvas.before_rendering(0, 0).is_none());
    }

    #[test]
    fn teardown_clears_state() {
        let mut canvas = Canvas::new();
        canvas.teardown();
        assert!(!canvas.has_gpu());
    }

    #[test]
    fn set_clear_color_is_stored() {
        let mut canvas = Canvas::new();
        let color = wgpu::Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        canvas.set_clear_color(color);
        assert_eq!(canvas.clear_color, color);
    }
}
