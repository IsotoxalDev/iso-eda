// SPDX-License-Identifier: GPL-3.0-or-later

//! M0.2 spike: egui + eframe drawing the shared `scene::Scene`.
//! See docs/adr/001-ui-toolkit.md for methodology.

use std::io::Write;

use eframe::egui;

struct SpikeApp {
    scene: scene::Scene,
    args: scene::Args,
    frame_index: u32,
    timer: scene::FrameTimer,
    camera: scene::Camera,
    bench_done: bool,
}

fn load_font(ctx: &egui::Context) -> anyhow::Result<()> {
    let path = std::env::var("SPIKE_FONT")
        .map_err(|_| anyhow::anyhow!("SPIKE_FONT is not set; see spikes/README.md"))?;
    let bytes = std::fs::read(&path)?;

    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert(
        "spike-font".to_owned(),
        std::sync::Arc::new(egui::FontData::from_owned(bytes)),
    );
    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "spike-font".to_owned());
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "spike-font".to_owned());
    ctx.set_fonts(fonts);
    Ok(())
}

fn color32(c: scene::Rgba8) -> egui::Color32 {
    egui::Color32::from_rgba_unmultiplied(c.r, c.g, c.b, c.a)
}

impl SpikeApp {
    fn new(cc: &eframe::CreationContext<'_>, args: scene::Args) -> Self {
        if let Err(err) = load_font(&cc.egui_ctx) {
            eprintln!("warning: failed to load $SPIKE_FONT: {err:#}");
        }
        let scene = scene::Scene::generate(args.scale);
        let viewport = scene::Viewport {
            width_px: args.width,
            height_px: args.height,
        };
        Self {
            scene,
            args,
            frame_index: 0,
            timer: scene::FrameTimer::new(),
            camera: scene::Camera::fit(viewport),
            bench_done: false,
        }
    }

    fn finish_bench(&mut self, scale_factor: f64, viewport_px: (u32, u32)) {
        if self.bench_done {
            return;
        }
        self.bench_done = true;
        let meta = scene::ReportMeta {
            toolkit: "egui".to_string(),
            toolkit_version: "0.37.0".to_string(),
            scale: self.args.scale,
            viewport_px,
            scale_factor,
            display_server: scene::display_server(),
            present_mode_requested: "AutoNoVsync".to_string(),
            notes: String::new(),
        };
        let report = scene::Report::from_timer(&self.timer, meta);
        println!("{}", report.to_json());
        let _ = std::io::stdout().flush();
        std::process::exit(0);
    }
}

impl eframe::App for SpikeApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.timer.begin_frame();

        let ctx = ui.ctx().clone();
        let screen = ui.max_rect();
        let scale_factor = f64::from(ctx.pixels_per_point());
        let viewport = scene::Viewport {
            width_px: screen.width().max(1.0) as u32,
            height_px: screen.height().max(1.0) as u32,
        };

        if self.args.bench {
            self.camera = scene::camera_at(self.frame_index, viewport);
        } else {
            ctx.input(|i| {
                if i.key_pressed(egui::Key::Escape) {
                    std::process::exit(0);
                }
                let pointer = &i.pointer;
                if pointer.primary_down() {
                    let delta = pointer.delta();
                    if delta != egui::Vec2::ZERO {
                        self.camera.center_x -= f64::from(delta.x) / self.camera.zoom;
                        self.camera.center_y -= f64::from(delta.y) / self.camera.zoom;
                    }
                }
                let scroll = i.smooth_scroll_delta.y;
                if scroll != 0.0 {
                    if let Some(cursor) = pointer.hover_pos() {
                        let world_x = self.camera.center_x
                            + (f64::from(cursor.x) - f64::from(viewport.width_px) / 2.0)
                                / self.camera.zoom;
                        let world_y = self.camera.center_y
                            + (f64::from(cursor.y) - f64::from(viewport.height_px) / 2.0)
                                / self.camera.zoom;
                        let factor = (f64::from(scroll) * 0.0015).exp();
                        self.camera.zoom *= factor;
                        self.camera.center_x = world_x
                            - (f64::from(cursor.x) - f64::from(viewport.width_px) / 2.0)
                                / self.camera.zoom;
                        self.camera.center_y = world_y
                            - (f64::from(cursor.y) - f64::from(viewport.height_px) / 2.0)
                                / self.camera.zoom;
                    }
                }
            });
        }

        let camera = self.camera;
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(egui::Color32::BLACK))
            .show(ui, |ui| {
                let painter = ui.painter();
                for r in &self.scene.rects {
                    let (x0, y0) = camera.to_screen(viewport, r.x0, r.y0);
                    let (x1, y1) = camera.to_screen(viewport, r.x1, r.y1);
                    painter.rect_filled(
                        egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1)),
                        0.0,
                        color32(scene::PALETTE[r.color as usize]),
                    );
                }
                let seg_color = color32(scene::PALETTE[8]);
                for seg in &self.scene.segments {
                    let (x0, y0) = camera.to_screen(viewport, seg.x0, seg.y0);
                    let (x1, y1) = camera.to_screen(viewport, seg.x1, seg.y1);
                    painter.line_segment(
                        [egui::pos2(x0, y0), egui::pos2(x1, y1)],
                        egui::Stroke::new(1.0, seg_color),
                    );
                }
                if let Some(px) = scene::label_px(camera.zoom) {
                    for l in &self.scene.labels {
                        let (x, y) = camera.to_screen(viewport, l.x, l.y);
                        painter.text(
                            egui::pos2(x, y),
                            egui::Align2::LEFT_BOTTOM,
                            &l.text,
                            egui::FontId::monospace(f32::from(px)),
                            egui::Color32::WHITE,
                        );
                    }
                }
            });

        self.timer.end_build();

        if self.args.bench {
            self.frame_index += 1;
            if self.frame_index > self.args.frames {
                self.finish_bench(scale_factor, (viewport.width_px, viewport.height_px));
            }
        }

        ctx.request_repaint();
    }
}

fn main() -> anyhow::Result<()> {
    let args = scene::Args::parse(std::env::args().skip(1))
        .map_err(|e| anyhow::anyhow!("argument error: {e}"))?;

    let viewport = egui::ViewportBuilder::default()
        .with_inner_size([args.width as f32, args.height as f32])
        .with_title("spike-egui (M0.2)");

    let native_options = eframe::NativeOptions {
        viewport,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            surface: eframe::egui_wgpu::SurfaceConfig {
                present_mode: wgpu::PresentMode::AutoNoVsync,
                desired_maximum_frame_latency: None,
            },
            ..Default::default()
        },
        ..Default::default()
    };

    eframe::run_native(
        "spike-egui",
        native_options,
        Box::new(move |cc| Ok(Box::new(SpikeApp::new(cc, args)))),
    )
    .map_err(|e| anyhow::anyhow!("eframe error: {e}"))?;

    Ok(())
}
