// SPDX-License-Identifier: GPL-3.0-or-later

//! M0.2 spike: iced drawing the shared `scene::Scene` via the `canvas`
//! widget. See docs/adr/001-ui-toolkit.md for methodology.

use std::cell::{Cell, RefCell};
use std::io::Write;

use iced::mouse;
use iced::widget::canvas::{self, Canvas, Frame, Geometry, Path, Stroke, Text};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Theme};

#[derive(Debug, Clone)]
enum Message {
    Tick,
    Dragged(Point),
    DragEnded,
    Scrolled { at: Point, delta: f32 },
    Escape,
    Window(iced::window::Id, iced::window::Event),
    ScaleFactorMeasured(f32),
}

struct State {
    scene: scene::Scene,
    args: scene::Args,
    frame_index: u32,
    // `canvas::Program::draw` takes `&self`, but it is the only place that
    // does the real per-frame work (building `Vec<Geometry>` from the
    // whole scene), so the timer lives behind a `RefCell`: `begin_frame`/
    // `end_build` must wrap `draw`, not `update`, or `build_ms` would
    // measure the trivial message-handling cost instead of the paint cost.
    timer: RefCell<scene::FrameTimer>,
    camera: scene::Camera,
    // The CLI `--width`/`--height` are only the *requested* window size;
    // this machine's compositor (Hyprland, tiling) frequently gives the
    // window a different real size. `draw` is the only place iced reports
    // the actual rendered `Rectangle`, so this is refreshed from there
    // every frame (via `Cell`, since `draw` takes `&self`) and is the
    // only value ever written to a `ReportMeta`/used for `camera_at`.
    viewport: Cell<scene::Viewport>,
    dragging_from: Option<Point>,
    scale_factor: f64,
}

fn color_from(c: scene::Rgba8) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, f32::from(c.a) / 255.0)
}

impl State {
    fn new(args: scene::Args) -> Self {
        let viewport = scene::Viewport {
            width_px: args.width,
            height_px: args.height,
        };
        Self {
            scene: scene::Scene::generate(args.scale),
            args,
            frame_index: 0,
            timer: RefCell::new(scene::FrameTimer::new()),
            camera: scene::Camera::fit(viewport),
            viewport: Cell::new(viewport),
            dragging_from: None,
            scale_factor: 1.0,
        }
    }

    fn update(&mut self, message: Message) -> iced::Task<Message> {
        match message {
            Message::Tick => {
                if self.args.bench {
                    self.frame_index += 1;
                }
                iced::Task::none()
            }
            Message::Dragged(pos) => {
                if let Some(from) = self.dragging_from {
                    let dx = pos.x - from.x;
                    let dy = pos.y - from.y;
                    self.camera.center_x -= f64::from(dx) / self.camera.zoom;
                    self.camera.center_y -= f64::from(dy) / self.camera.zoom;
                }
                self.dragging_from = Some(pos);
                iced::Task::none()
            }
            Message::DragEnded => {
                self.dragging_from = None;
                iced::Task::none()
            }
            Message::Scrolled { at, delta } => {
                let viewport = self.viewport.get();
                let world_x = self.camera.center_x
                    + (f64::from(at.x) - f64::from(viewport.width_px) / 2.0) / self.camera.zoom;
                let world_y = self.camera.center_y
                    + (f64::from(at.y) - f64::from(viewport.height_px) / 2.0) / self.camera.zoom;
                let factor = (f64::from(delta) * 0.0015).exp();
                self.camera.zoom *= factor;
                self.camera.center_x = world_x
                    - (f64::from(at.x) - f64::from(viewport.width_px) / 2.0) / self.camera.zoom;
                self.camera.center_y = world_y
                    - (f64::from(at.y) - f64::from(viewport.height_px) / 2.0) / self.camera.zoom;
                iced::Task::none()
            }
            Message::Escape => std::process::exit(0),
            // The window's real (OS-reported) HiDPI scale factor is not
            // settled the instant the window opens: on Wayland it is
            // negotiated via the fractional-scale protocol shortly after,
            // and winit reports the pre-negotiation value (1.0) until
            // then. So this both asks for it once at `Opened` (covers
            // backends that report it correctly immediately) and keeps
            // listening for `Rescaled`, which iced emits whenever winit's
            // own `ScaleFactorChanged` fires (covers the Wayland
            // negotiation landing after the window is already up).
            Message::Window(id, iced::window::Event::Opened { .. }) => {
                iced::window::scale_factor(id).map(Message::ScaleFactorMeasured)
            }
            Message::Window(_, iced::window::Event::Rescaled(sf)) => {
                self.scale_factor = f64::from(sf);
                iced::Task::none()
            }
            Message::Window(..) => iced::Task::none(),
            Message::ScaleFactorMeasured(sf) => {
                self.scale_factor = f64::from(sf);
                iced::Task::none()
            }
        }
    }

    /// Called from `draw` once the timer has recorded enough frames.
    /// `process::exit` is final, so there is no need to guard against a
    /// second call.
    fn finish_bench(&self) {
        let viewport = self.viewport.get();
        let meta = scene::ReportMeta {
            toolkit: "iced".to_string(),
            toolkit_version: "0.14.0".to_string(),
            scale: self.args.scale,
            viewport_px: (viewport.width_px, viewport.height_px),
            scale_factor: self.scale_factor,
            display_server: scene::display_server(),
            present_mode_requested: std::env::var("ICED_PRESENT_MODE")
                .unwrap_or_else(|_| "default".to_string()),
            notes: String::new(),
        };
        let report = scene::Report::from_timer(&self.timer.borrow(), meta);
        println!("{}", report.to_json());
        let _ = std::io::stdout().flush();
        std::process::exit(0);
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        iced::Subscription::batch([
            iced::window::frames().map(|_| Message::Tick),
            iced::window::events().map(|(id, event)| Message::Window(id, event)),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        Canvas::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

impl canvas::Program<Message> for State {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<Geometry> {
        self.timer.borrow_mut().begin_frame();

        // `bounds` is the one place iced reports the canvas's *actual*
        // rendered size; this machine's compositor does not honour the
        // CLI `--width`/`--height` request, so `self.viewport` (used for
        // reporting and, in bench mode, for the camera path) is always
        // refreshed from here rather than from the stale construction
        // argument.
        let viewport = scene::Viewport {
            width_px: bounds.width.max(1.0) as u32,
            height_px: bounds.height.max(1.0) as u32,
        };
        self.viewport.set(viewport);

        let camera = if self.args.bench {
            scene::camera_at(self.frame_index, viewport)
        } else {
            self.camera
        };

        let mut frame = Frame::new(renderer, bounds.size());

        for r in &self.scene.rects {
            let (x0, y0) = camera.to_screen(viewport, r.x0, r.y0);
            let (x1, y1) = camera.to_screen(viewport, r.x1, r.y1);
            let top_left = Point::new(x0.min(x1), y0.min(y1));
            let size = iced::Size::new((x1 - x0).abs(), (y1 - y0).abs());
            frame.fill_rectangle(top_left, size, color_from(scene::PALETTE[r.color as usize]));
        }

        let seg_color = color_from(scene::PALETTE[8]);
        for seg in &self.scene.segments {
            let (x0, y0) = camera.to_screen(viewport, seg.x0, seg.y0);
            let (x1, y1) = camera.to_screen(viewport, seg.x1, seg.y1);
            let path = Path::line(Point::new(x0, y0), Point::new(x1, y1));
            frame.stroke(
                &path,
                Stroke::default().with_color(seg_color).with_width(1.0),
            );
        }

        if let Some(px) = scene::label_px(camera.zoom) {
            for l in &self.scene.labels {
                let (x, y) = camera.to_screen(viewport, l.x, l.y);
                frame.fill_text(Text {
                    content: l.text.clone(),
                    position: Point::new(x, y),
                    color: Color::WHITE,
                    size: iced::Pixels(f32::from(px)),
                    ..Text::default()
                });
            }
        }

        self.timer.borrow_mut().end_build();
        if self.args.bench && self.timer.borrow().frames_recorded() > self.args.frames {
            self.finish_bench();
        }

        vec![frame.into_geometry()]
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: &iced::Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Message>> {
        use iced::Event;
        let cursor_pos = cursor.position_in(bounds);

        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                cursor_pos.map(|pos| canvas::Action::publish(Message::Dragged(pos)))
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                Some(canvas::Action::publish(Message::DragEnded))
            }
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                cursor_pos.map(|pos| canvas::Action::publish(Message::Dragged(pos)))
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let dy = match delta {
                    mouse::ScrollDelta::Lines { y, .. } => *y,
                    mouse::ScrollDelta::Pixels { y, .. } => *y,
                };
                cursor_pos
                    .map(|pos| canvas::Action::publish(Message::Scrolled { at: pos, delta: dy }))
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed {
                key: iced::keyboard::Key::Named(iced::keyboard::key::Named::Escape),
                ..
            }) => Some(canvas::Action::publish(Message::Escape)),
            _ => None,
        }
    }
}

fn main() -> anyhow::Result<()> {
    let args = scene::Args::parse(std::env::args().skip(1))
        .map_err(|e| anyhow::anyhow!("argument error: {e}"))?;

    let width = args.width;
    let height = args.height;

    iced::application(
        move || (State::new(args), iced::Task::none()),
        State::update,
        State::view,
    )
    .title("spike-iced (M0.2)")
    .subscription(State::subscription)
    .window(iced::window::Settings {
        size: iced::Size::new(width as f32, height as f32),
        ..Default::default()
    })
    .run()
    .map_err(|e| anyhow::anyhow!("iced error: {e}"))?;

    Ok(())
}
