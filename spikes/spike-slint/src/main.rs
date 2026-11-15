// SPDX-License-Identifier: GPL-3.0-or-later

//! M0.2 spike: Slint shell, our own `canvas-wgpu` renderer backing the
//! `Image` element via `slint::Image::try_from`. See
//! docs/adr/001-ui-toolkit.md: this spike measures "Slint shell + our
//! wgpu canvas", not Slint primitives drawing the scene.

use std::io::Write;

slint::include_modules!();

use slint::wgpu_30::{wgpu, WGPUConfiguration, WGPUSettings};

fn main() -> anyhow::Result<()> {
    let args = scene::Args::parse(std::env::args().skip(1))
        .map_err(|e| anyhow::anyhow!("argument error: {e}"))?;

    let font_path = std::env::var("SPIKE_FONT")
        .map_err(|_| anyhow::anyhow!("SPIKE_FONT is not set; see spikes/README.md"))?;
    let font_bytes = std::fs::read(&font_path)?;

    slint::BackendSelector::new()
        .require_wgpu_30(WGPUConfiguration::Automatic(WGPUSettings::default()))
        .select()
        .map_err(|e| anyhow::anyhow!("failed to select the Slint WGPU backend: {e}"))?;

    let app = App::new()?;
    app.on_escape_pressed(|| std::process::exit(0));

    let scene_data = scene::Scene::generate(args.scale);

    let mut renderer: Option<canvas_wgpu::CanvasRenderer> = None;
    let mut gpu: Option<(wgpu::Device, wgpu::Queue)> = None;
    let mut output_texture: Option<wgpu::Texture> = None;
    let mut timer = scene::FrameTimer::new();
    let mut frame_index: u32 = 0;
    let mut camera = scene::Camera::fit(scene::Viewport {
        width_px: args.width,
        height_px: args.height,
    });
    let mut prev_pointer: Option<(f32, f32)> = None;
    let mut bench_done = false;

    let app_weak = app.as_weak();

    app.window()
        .set_rendering_notifier(move |state, graphics_api| match state {
            slint::RenderingState::RenderingSetup => {
                if let slint::GraphicsAPI::WGPU30 { device, queue, .. } = graphics_api {
                    gpu = Some((device.clone(), queue.clone()));
                    renderer = match canvas_wgpu::CanvasRenderer::new(
                        device,
                        queue,
                        wgpu::TextureFormat::Rgba8UnormSrgb,
                        &font_bytes,
                    ) {
                        Ok(r) => Some(r),
                        Err(err) => {
                            eprintln!("failed to create CanvasRenderer: {err:#}");
                            None
                        }
                    };
                }
            }
            slint::RenderingState::BeforeRendering => {
                let Some(app) = app_weak.upgrade() else {
                    return;
                };
                let (Some(renderer), Some((device, queue))) = (renderer.as_mut(), gpu.as_ref())
                else {
                    return;
                };

                let width = app.get_requested_texture_width().max(1) as u32;
                let height = app.get_requested_texture_height().max(1) as u32;
                let viewport = scene::Viewport {
                    width_px: width,
                    height_px: height,
                };

                timer.begin_frame();

                if args.bench {
                    camera = scene::camera_at(frame_index, viewport);
                } else {
                    let px = app.get_pointer_x();
                    let py = app.get_pointer_y();
                    if app.get_pointer_pressed() {
                        if let Some((last_x, last_y)) = prev_pointer {
                            camera.center_x -= f64::from(px - last_x) / camera.zoom;
                            camera.center_y -= f64::from(py - last_y) / camera.zoom;
                        }
                        prev_pointer = Some((px, py));
                    } else {
                        prev_pointer = None;
                    }

                    let scroll = app.get_scroll_delta_y();
                    if scroll != 0.0 {
                        app.set_scroll_delta_y(0.0);
                        let half_w = f64::from(width) / 2.0;
                        let half_h = f64::from(height) / 2.0;
                        let world_x = camera.center_x + (f64::from(px) - half_w) / camera.zoom;
                        let world_y = camera.center_y + (f64::from(py) - half_h) / camera.zoom;
                        let factor = (f64::from(scroll) * 0.0015).exp();
                        camera.zoom *= factor;
                        camera.center_x = world_x - (f64::from(px) - half_w) / camera.zoom;
                        camera.center_y = world_y - (f64::from(py) - half_h) / camera.zoom;
                    }
                }

                let wanted_size = wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                };
                let needs_new_texture =
                    !matches!(&output_texture, Some(t) if t.size() == wanted_size);
                if needs_new_texture {
                    output_texture = Some(device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("spike-slint output texture"),
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
                let Some(texture) = output_texture.as_ref() else {
                    return;
                };
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

                renderer.upload(device, queue, &scene_data, camera, viewport);

                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("spike-slint encoder"),
                });
                renderer.render(
                    &mut encoder,
                    &view,
                    wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    },
                );
                queue.submit(Some(encoder.finish()));

                timer.end_build();

                if let Ok(image) = slint::Image::try_from(texture.clone()) {
                    app.set_texture(image);
                }
                app.window().request_redraw();

                if args.bench {
                    frame_index += 1;
                    if frame_index > args.frames && !bench_done {
                        bench_done = true;
                        let meta = scene::ReportMeta {
                            toolkit: "slint".to_string(),
                            toolkit_version: "1.18.1+canvas-wgpu".to_string(),
                            scale: args.scale,
                            viewport_px: (width, height),
                            scale_factor: 1.0,
                            display_server: scene::display_server(),
                            present_mode_requested: "not verified (see docs/adr/001-ui-toolkit.md)"
                                .to_string(),
                            notes: "Slint shell + canvas-wgpu, not Slint primitives".to_string(),
                        };
                        let report = scene::Report::from_timer(&timer, meta);
                        println!("{}", report.to_json());
                        let _ = std::io::stdout().flush();
                        std::process::exit(0);
                    }
                }
            }
            slint::RenderingState::RenderingTeardown => {
                renderer = None;
                gpu = None;
                output_texture = None;
            }
            _ => {}
        })
        .map_err(|e| anyhow::anyhow!("failed to set Slint rendering notifier: {e}"))?;

    app.run()?;
    Ok(())
}
