// SPDX-License-Identifier: GPL-3.0-or-later

//! `canvas-wgpu`: a small, toolkit-independent wgpu-30 renderer for the
//! shared M0.2 scene (instanced rects, hairline segments, glyph-atlas
//! text). Used by `spike-slint` to back its `Image` element. No
//! automated test: it needs a real GPU device, which the Nix sandbox
//! does not provide; correctness is checked by eye through the Slint
//! spike's windowed mode. See docs/adr/001-ui-toolkit.md.

use std::collections::HashMap;

use ab_glyph::{Font as _, FontArc, ScaleFont as _};

const ATLAS_SIZE: u32 = 2048;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    viewport_size: [f32; 2],
    camera_center: [f32; 2],
    zoom: f32,
    _pad: [f32; 3],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct RectInstance {
    world_min: [f32; 2],
    world_max: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct LineInstance {
    world_a: [f32; 2],
    world_b: [f32; 2],
    color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct GlyphInstance {
    screen_min: [f32; 2],
    screen_max: [f32; 2],
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    color: [f32; 4],
}

/// One rasterized glyph's place in the atlas, plus the pen-relative
/// metrics needed to place it: `offset` is the (left bearing, vertical
/// offset from the baseline) of the glyph bitmap's top-left corner in
/// pixels, `size` is the bitmap size in pixels, `advance` is the
/// horizontal pen advance for this glyph at this px size.
#[derive(Clone, Copy)]
struct AtlasEntry {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    offset: [f32; 2],
    size: [f32; 2],
    advance: f32,
}

/// Grow-only instance buffer: a GPU buffer that is only reallocated when
/// it needs to grow larger than its current capacity (the ADR's
/// edge-case note: allocate instance buffers once per run, not once per
/// frame, even though `upload` re-fills the buffer's contents every
/// frame per the shared rendering-fairness rule).
struct InstanceBuffer {
    buffer: wgpu::Buffer,
    capacity_bytes: u64,
    usage: wgpu::BufferUsages,
    label: &'static str,
}

impl InstanceBuffer {
    fn new(device: &wgpu::Device, label: &'static str, usage: wgpu::BufferUsages) -> Self {
        let capacity_bytes = 4096;
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: capacity_bytes,
            usage,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            capacity_bytes,
            usage,
            label,
        }
    }

    fn upload<T: bytemuck::Pod>(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, data: &[T]) {
        let bytes = bytemuck::cast_slice(data);
        let needed = bytes.len() as u64;
        if needed > self.capacity_bytes {
            let new_capacity = needed.max(self.capacity_bytes * 2);
            self.buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(self.label),
                size: new_capacity,
                usage: self.usage,
                mapped_at_creation: false,
            });
            self.capacity_bytes = new_capacity;
        }
        if !bytes.is_empty() {
            queue.write_buffer(&self.buffer, 0, bytes);
        }
    }
}

pub struct CanvasRenderer {
    rect_pipeline: wgpu::RenderPipeline,
    line_pipeline: wgpu::RenderPipeline,
    text_pipeline: wgpu::RenderPipeline,

    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    rects: InstanceBuffer,
    rect_count: u32,
    lines: InstanceBuffer,
    line_count: u32,
    glyphs: InstanceBuffer,
    glyph_count: u32,

    atlas_texture: wgpu::Texture,
    atlas_bind_group: wgpu::BindGroup,
    atlas_cursor: (u32, u32),
    atlas_row_height: u32,
    glyph_cache: HashMap<(char, u16), AtlasEntry>,
    font: FontArc,
}

impl CanvasRenderer {
    pub fn new(
        device: &wgpu::Device,
        // Part of the documented public signature (docs/adr/001-ui-toolkit.md);
        // not needed yet because glyphs are rasterized lazily in `upload`,
        // not at construction time.
        _queue: &wgpu::Queue,
        target_format: wgpu::TextureFormat,
        font_ttf: &[u8],
    ) -> anyhow::Result<Self> {
        let font = FontArc::try_from_vec(font_ttf.to_vec())
            .map_err(|e| anyhow::anyhow!("invalid font: {e}"))?;

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("canvas-wgpu uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("canvas-wgpu uniform layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("canvas-wgpu uniform bind group"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let atlas_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("canvas-wgpu glyph atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let atlas_view = atlas_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let atlas_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("canvas-wgpu glyph atlas sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let atlas_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("canvas-wgpu atlas layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("canvas-wgpu atlas bind group"),
            layout: &atlas_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_sampler),
                },
            ],
        });

        let rect_pipeline = Self::build_pipeline(
            device,
            target_format,
            &uniform_layout,
            None,
            include_str!("rects.wgsl"),
            "canvas-wgpu rect pipeline",
            &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 2,
                },
            ],
            std::mem::size_of::<RectInstance>() as u64,
        );
        let line_pipeline = Self::build_pipeline(
            device,
            target_format,
            &uniform_layout,
            None,
            include_str!("lines.wgsl"),
            "canvas-wgpu line pipeline",
            &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 2,
                },
            ],
            std::mem::size_of::<LineInstance>() as u64,
        );
        let text_pipeline = Self::build_pipeline(
            device,
            target_format,
            &uniform_layout,
            Some(&atlas_layout),
            include_str!("text.wgsl"),
            "canvas-wgpu text pipeline",
            &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 1,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 16,
                    shader_location: 2,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 24,
                    shader_location: 3,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 32,
                    shader_location: 4,
                },
            ],
            std::mem::size_of::<GlyphInstance>() as u64,
        );

        Ok(Self {
            rect_pipeline,
            line_pipeline,
            text_pipeline,
            uniform_buffer,
            uniform_bind_group,
            rects: InstanceBuffer::new(
                device,
                "canvas-wgpu rect instances",
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            ),
            rect_count: 0,
            lines: InstanceBuffer::new(
                device,
                "canvas-wgpu line instances",
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            ),
            line_count: 0,
            glyphs: InstanceBuffer::new(
                device,
                "canvas-wgpu glyph instances",
                wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            ),
            glyph_count: 0,
            atlas_texture,
            atlas_bind_group,
            atlas_cursor: (0, 0),
            atlas_row_height: 0,
            glyph_cache: HashMap::new(),
            font,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn build_pipeline(
        device: &wgpu::Device,
        target_format: wgpu::TextureFormat,
        uniform_layout: &wgpu::BindGroupLayout,
        atlas_layout: Option<&wgpu::BindGroupLayout>,
        wgsl_source: &str,
        label: &str,
        attributes: &[wgpu::VertexAttribute],
        stride: u64,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(wgsl_source.into()),
        });

        let mut bind_group_layouts: Vec<Option<&wgpu::BindGroupLayout>> =
            vec![Some(uniform_layout)];
        if let Some(atlas) = atlas_layout {
            bind_group_layouts.push(Some(atlas));
        }
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: &bind_group_layouts,
            immediate_size: 0,
        });

        let buffer_layout = wgpu::VertexBufferLayout {
            array_stride: stride,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes,
        };

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(buffer_layout)],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        })
    }

    /// Rasterizes `(c, px_size)` into the atlas on first use and caches
    /// the result; a glyph that no longer fits the atlas is skipped
    /// (recorded as `None` would be the honest outcome, but in practice
    /// the 61 distinct label sizes across the ASCII/underscore alphabet
    /// fit comfortably in a 2048x2048 R8 atlas for a single bench run).
    fn glyph_entry(&mut self, queue: &wgpu::Queue, c: char, px_size: u16) -> Option<AtlasEntry> {
        if let Some(entry) = self.glyph_cache.get(&(c, px_size)) {
            return Some(*entry);
        }

        let scaled = self.font.as_scaled(f32::from(px_size));
        let glyph_id = self.font.glyph_id(c);
        let advance = scaled.h_advance(glyph_id);
        let glyph = glyph_id.with_scale(f32::from(px_size));

        let Some(outlined) = self.font.outline_glyph(glyph) else {
            // No outline (e.g. space): cache a zero-size entry with the
            // correct advance so layout still works.
            let entry = AtlasEntry {
                uv_min: [0.0, 0.0],
                uv_max: [0.0, 0.0],
                offset: [0.0, 0.0],
                size: [0.0, 0.0],
                advance,
            };
            self.glyph_cache.insert((c, px_size), entry);
            return Some(entry);
        };

        let bounds = outlined.px_bounds();
        let width = bounds.width().ceil().max(1.0) as u32;
        let height = bounds.height().ceil().max(1.0) as u32;

        if self.atlas_cursor.0 + width > ATLAS_SIZE {
            self.atlas_cursor.0 = 0;
            self.atlas_cursor.1 += self.atlas_row_height;
            self.atlas_row_height = 0;
        }
        if self.atlas_cursor.1 + height > ATLAS_SIZE {
            // Atlas exhausted for this run; skip this glyph rather than
            // panic or corrupt another glyph's region.
            return None;
        }

        let mut pixels = vec![0u8; (width * height) as usize];
        outlined.draw(|x, y, coverage| {
            let idx = (y * width + x) as usize;
            if idx < pixels.len() {
                pixels[idx] = (coverage.clamp(0.0, 1.0) * 255.0) as u8;
            }
        });

        let (x0, y0) = self.atlas_cursor;
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.atlas_texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: x0, y: y0, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let atlas_size_f = ATLAS_SIZE as f32;
        let entry = AtlasEntry {
            uv_min: [x0 as f32 / atlas_size_f, y0 as f32 / atlas_size_f],
            uv_max: [
                (x0 + width) as f32 / atlas_size_f,
                (y0 + height) as f32 / atlas_size_f,
            ],
            offset: [bounds.min.x, bounds.min.y],
            size: [width as f32, height as f32],
            advance,
        };

        self.atlas_cursor.0 += width;
        self.atlas_row_height = self.atlas_row_height.max(height);
        self.glyph_cache.insert((c, px_size), entry);
        Some(entry)
    }

    /// Uploads the whole scene as instances for this frame (rule 1:
    /// everything, every frame; see docs/adr/001-ui-toolkit.md). The
    /// underlying GPU buffers are grown, not reallocated, when they
    /// already have enough capacity.
    pub fn upload(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        scene: &scene::Scene,
        camera: scene::Camera,
        viewport: scene::Viewport,
    ) {
        let uniforms = Uniforms {
            viewport_size: [viewport.width_px as f32, viewport.height_px as f32],
            camera_center: [camera.center_x as f32, camera.center_y as f32],
            zoom: camera.zoom as f32,
            _pad: [0.0; 3],
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let rect_data: Vec<RectInstance> = scene
            .rects
            .iter()
            .map(|r| {
                let color = scene::PALETTE[r.color as usize];
                RectInstance {
                    world_min: [r.x0 as f32, r.y0 as f32],
                    world_max: [r.x1 as f32, r.y1 as f32],
                    color: [
                        f32::from(color.r) / 255.0,
                        f32::from(color.g) / 255.0,
                        f32::from(color.b) / 255.0,
                        f32::from(color.a) / 255.0,
                    ],
                }
            })
            .collect();
        self.rect_count = rect_data.len() as u32;
        self.rects.upload(device, queue, &rect_data);

        let seg_color = scene::PALETTE[8];
        let seg_color_f = [
            f32::from(seg_color.r) / 255.0,
            f32::from(seg_color.g) / 255.0,
            f32::from(seg_color.b) / 255.0,
            f32::from(seg_color.a) / 255.0,
        ];
        let line_data: Vec<LineInstance> = scene
            .segments
            .iter()
            .map(|s| LineInstance {
                world_a: [s.x0 as f32, s.y0 as f32],
                world_b: [s.x1 as f32, s.y1 as f32],
                color: seg_color_f,
            })
            .collect();
        self.line_count = line_data.len() as u32;
        self.lines.upload(device, queue, &line_data);

        let mut glyph_data: Vec<GlyphInstance> = Vec::new();
        if let Some(px_size) = scene::label_px(camera.zoom) {
            for label in &scene.labels {
                let (anchor_x, anchor_y) = camera.to_screen(viewport, label.x, label.y);
                let mut pen_x = anchor_x;
                for c in label.text.chars() {
                    let Some(entry) = self.glyph_entry(queue, c, px_size) else {
                        continue;
                    };
                    if entry.size[0] > 0.0 && entry.size[1] > 0.0 {
                        let min_x = pen_x + entry.offset[0];
                        let min_y = anchor_y + entry.offset[1];
                        glyph_data.push(GlyphInstance {
                            screen_min: [min_x, min_y],
                            screen_max: [min_x + entry.size[0], min_y + entry.size[1]],
                            uv_min: entry.uv_min,
                            uv_max: entry.uv_max,
                            color: [1.0, 1.0, 1.0, 1.0],
                        });
                    }
                    pen_x += entry.advance;
                }
            }
        }
        self.glyph_count = glyph_data.len() as u32;
        self.glyphs.upload(device, queue, &glyph_data);
    }

    /// Records one render pass into `target`.
    pub fn render(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        clear: wgpu::Color,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("canvas-wgpu pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_bind_group(0, &self.uniform_bind_group, &[]);

        if self.rect_count > 0 {
            pass.set_pipeline(&self.rect_pipeline);
            pass.set_vertex_buffer(0, self.rects.buffer.slice(..));
            pass.draw(0..6, 0..self.rect_count);
        }
        if self.line_count > 0 {
            pass.set_pipeline(&self.line_pipeline);
            pass.set_vertex_buffer(0, self.lines.buffer.slice(..));
            pass.draw(0..6, 0..self.line_count);
        }
        if self.glyph_count > 0 {
            pass.set_pipeline(&self.text_pipeline);
            pass.set_bind_group(1, &self.atlas_bind_group, &[]);
            pass.set_vertex_buffer(0, self.glyphs.buffer.slice(..));
            pass.draw(0..6, 0..self.glyph_count);
        }
    }
}
