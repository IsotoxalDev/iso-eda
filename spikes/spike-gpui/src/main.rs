// SPDX-License-Identifier: GPL-3.0-or-later

//! M0.2 spike: GPUI drawing the shared `scene::Scene` via `canvas` +
//! `paint_quad`/`paint_path`/`shape_line`. See docs/adr/001-ui-toolkit.md
//! for methodology, and that ADR's "Not verified" section for the status
//! of the exact APIs used here at the pinned rev.

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use gpui::{
    canvas, div, font, point, prelude::*, px, quad, size, App, Bounds, Context, Hsla, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point as GpuiPoint, Render, Rgba,
    ScrollDelta, ScrollWheelEvent, TextAlign, TextRun, TitlebarOptions, Window, WindowBounds,
    WindowOptions,
};
use gpui_platform::application;

struct SpikeView {
    scene: Rc<scene::Scene>,
    args: scene::Args,
    frame_index: u32,
    // `canvas`'s paint closure is `'static FnOnce`, executed later by
    // gpui's element tree outside of `render`, so it cannot borrow `self`.
    // The timer lives behind `Rc<RefCell<..>>`, shared with the closure,
    // and `begin_frame`/`end_build` wrap the closure's actual drawing work
    // (not `render` itself, which only builds the element tree).
    timer: Rc<RefCell<scene::FrameTimer>>,
    camera: scene::Camera,
    viewport: scene::Viewport,
    dragging_from: Option<GpuiPoint<Pixels>>,
    font: gpui::Font,
}

/// Called from inside the `canvas` paint closure once the timer has
/// recorded enough frames. `process::exit` is final, so there is no need
/// to guard against a second call.
fn finish_bench(
    args: &scene::Args,
    viewport: scene::Viewport,
    timer: &scene::FrameTimer,
    scale_factor: f64,
) {
    let meta = scene::ReportMeta {
        toolkit: "gpui".to_string(),
        toolkit_version: "zed@137e677a".to_string(),
        scale: args.scale,
        viewport_px: (viewport.width_px, viewport.height_px),
        scale_factor,
        display_server: scene::display_server(),
        present_mode_requested: "uncontrollable (Mailbox on Wayland, Fifo on X11)".to_string(),
        notes: String::new(),
    };
    let report = scene::Report::from_timer(timer, meta);
    println!("{}", report.to_json());
    let _ = std::io::stdout().flush();
    std::process::exit(0);
}

fn rgba_from(c: scene::Rgba8) -> Rgba {
    Rgba {
        r: f32::from(c.r) / 255.0,
        g: f32::from(c.g) / 255.0,
        b: f32::from(c.b) / 255.0,
        a: f32::from(c.a) / 255.0,
    }
}

impl SpikeView {
    fn new(args: scene::Args, text_font: gpui::Font) -> Self {
        let viewport = scene::Viewport {
            width_px: args.width,
            height_px: args.height,
        };
        Self {
            scene: Rc::new(scene::Scene::generate(args.scale)),
            args,
            frame_index: 0,
            timer: Rc::new(RefCell::new(scene::FrameTimer::new())),
            camera: scene::Camera::fit(viewport),
            viewport,
            dragging_from: None,
            font: text_font,
        }
    }

    fn pan(&mut self, to: GpuiPoint<Pixels>) {
        if let Some(from) = self.dragging_from {
            let dx = f64::from(f32::from(to.x - from.x));
            let dy = f64::from(f32::from(to.y - from.y));
            self.camera.center_x -= dx / self.camera.zoom;
            self.camera.center_y -= dy / self.camera.zoom;
        }
        self.dragging_from = Some(to);
    }

    fn zoom_at(&mut self, at: GpuiPoint<Pixels>, delta_lines: f32) {
        let cx_px = f64::from(f32::from(at.x));
        let cy_px = f64::from(f32::from(at.y));
        let half_w = f64::from(self.viewport.width_px) / 2.0;
        let half_h = f64::from(self.viewport.height_px) / 2.0;
        let world_x = self.camera.center_x + (cx_px - half_w) / self.camera.zoom;
        let world_y = self.camera.center_y + (cy_px - half_h) / self.camera.zoom;
        let factor = (f64::from(delta_lines) * 0.08).exp();
        self.camera.zoom *= factor;
        self.camera.center_x = world_x - (cx_px - half_w) / self.camera.zoom;
        self.camera.center_y = world_y - (cy_px - half_h) / self.camera.zoom;
    }
}

impl Render for SpikeView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        window.request_animation_frame();

        let size_px = window.viewport_size();
        self.viewport = scene::Viewport {
            width_px: (f32::from(size_px.width)).max(1.0) as u32,
            height_px: (f32::from(size_px.height)).max(1.0) as u32,
        };

        if self.args.bench {
            self.camera = scene::camera_at(self.frame_index, self.viewport);
            self.frame_index += 1;
        }

        let camera = self.camera;
        let viewport = self.viewport;
        let scene_data = self.scene.clone();
        let text_font = self.font.clone();
        let timer = self.timer.clone();
        let args = self.args;

        let canvas_el = canvas(
            move |_, _, _| {},
            move |_, _, window: &mut Window, cx: &mut App| {
                timer.borrow_mut().begin_frame();

                for r in scene_data.rects.iter() {
                    let (x0, y0) = camera.to_screen(viewport, r.x0, r.y0);
                    let (x1, y1) = camera.to_screen(viewport, r.x1, r.y1);
                    let bounds = Bounds {
                        origin: point(px(x0.min(x1)), px(y0.min(y1))),
                        size: size(px((x1 - x0).abs()), px((y1 - y0).abs())),
                    };
                    window.paint_quad(quad(
                        bounds,
                        px(0.),
                        rgba_from(scene::PALETTE[r.color as usize]),
                        px(0.),
                        gpui::transparent_black(),
                        Default::default(),
                    ));
                }

                let seg_color: Hsla = rgba_from(scene::PALETTE[8]).into();
                for seg in scene_data.segments.iter() {
                    let (x0, y0) = camera.to_screen(viewport, seg.x0, seg.y0);
                    let (x1, y1) = camera.to_screen(viewport, seg.x1, seg.y1);
                    let mut builder = gpui::PathBuilder::stroke(px(1.));
                    builder.move_to(point(px(x0), px(y0)));
                    builder.line_to(point(px(x1), px(y1)));
                    if let Ok(path) = builder.build() {
                        window.paint_path(path, seg_color);
                    }
                }

                if let Some(px_size) = scene::label_px(camera.zoom) {
                    let run_color: Hsla = Rgba {
                        r: 1.0,
                        g: 1.0,
                        b: 1.0,
                        a: 1.0,
                    }
                    .into();
                    for l in scene_data.labels.iter() {
                        let (x, y) = camera.to_screen(viewport, l.x, l.y);
                        let text: gpui::SharedString = l.text.clone().into();
                        let len = text.len();
                        let run = TextRun {
                            len,
                            font: text_font.clone(),
                            color: run_color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        };
                        let shaped = window.text_system().shape_line(
                            text,
                            px(f32::from(px_size)),
                            &[run],
                            None,
                        );
                        let _ = shaped.paint(
                            point(px(x), px(y)),
                            px(f32::from(px_size)),
                            TextAlign::Left,
                            None,
                            window,
                            cx,
                        );
                    }
                }

                timer.borrow_mut().end_build();
                if args.bench && timer.borrow().frames_recorded() > args.frames {
                    finish_bench(&args, viewport, &timer.borrow(), f64::from(window.scale_factor()));
                }
            },
        )
        .size_full();

        div()
            .size_full()
            .bg(gpui::black())
            .child(canvas_el)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, ev: &MouseDownEvent, _, _| {
                    this.dragging_from = Some(ev.position);
                }),
            )
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _: &MouseUpEvent, _, _| {
                    this.dragging_from = None;
                }),
            )
            .on_mouse_move(cx.listener(|this, ev: &MouseMoveEvent, _, _| {
                if this.dragging_from.is_some() {
                    this.pan(ev.position);
                }
            }))
            .on_scroll_wheel(cx.listener(|this, ev: &ScrollWheelEvent, _, _| {
                let lines = match ev.delta {
                    ScrollDelta::Lines(l) => l.y,
                    ScrollDelta::Pixels(p) => f32::from(p.y) / 20.0,
                };
                this.zoom_at(ev.position, lines);
            }))
            .on_key_down(cx.listener(|_, ev: &gpui::KeyDownEvent, _, _| {
                if ev.keystroke.key == "escape" {
                    std::process::exit(0);
                }
            }))
    }
}

fn load_font_bytes() -> anyhow::Result<Vec<u8>> {
    let path = std::env::var("SPIKE_FONT")
        .map_err(|_| anyhow::anyhow!("SPIKE_FONT is not set; see spikes/README.md"))?;
    Ok(std::fs::read(path)?)
}

fn main() -> anyhow::Result<()> {
    let args = scene::Args::parse(std::env::args().skip(1))
        .map_err(|e| anyhow::anyhow!("argument error: {e}"))?;
    let font_bytes = load_font_bytes()?;

    application().run(move |cx: &mut App| {
        let fonts = vec![std::borrow::Cow::Owned(font_bytes)];
        if let Err(err) = cx.text_system().add_fonts(fonts) {
            eprintln!("warning: failed to register $SPIKE_FONT: {err:#}");
        }
        let text_font = font("DejaVu Sans");

        let width = px(args.width as f32);
        let height = px(args.height as f32);

        let window = cx.open_window(
            WindowOptions {
                titlebar: Some(TitlebarOptions {
                    title: Some("spike-gpui (M0.2)".into()),
                    ..Default::default()
                }),
                focus: true,
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(width, height),
                    cx,
                ))),
                ..Default::default()
            },
            |_, cx| cx.new(|_| SpikeView::new(args, text_font)),
        );
        // `run`'s closure is `FnOnce(&mut App)` with no return value, so a
        // window-open failure can't be propagated as a `Result`; report it
        // and quit cleanly instead of panicking (CLAUDE.md: no
        // `unwrap`/`expect` outside tests).
        if let Err(err) = window {
            eprintln!("error: failed to open spike-gpui window: {err:#}");
            cx.quit();
            return;
        }

        cx.on_window_closed(|cx, _window_id| {
            cx.quit();
        })
        .detach();
        cx.activate(true);
    });

    Ok(())
}
