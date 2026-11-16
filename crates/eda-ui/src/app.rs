// SPDX-License-Identifier: GPL-3.0-or-later

//! Wires the generated Slint window to `eda-cmd`: owns a `CommandHost`,
//! populates the palette from the registry, runs the chosen command,
//! re-syncs the `theme`/`status-message` properties after every run, and
//! dispatches key presses through the loaded `Keymap`.

use std::cell::RefCell;
use std::rc::Rc;

use eda_cmd::{CommandContext, CommandHost, CommandId, Theme};
use slint::ComponentHandle;

use crate::canvas::Canvas;
use crate::generated::{MainWindow, PaletteEntry};
use crate::keymap::{Keymap, XSCHEM_PRESET};

/// Not an `eda-cmd` command: opening the palette is UI chrome (it does
/// not change `CommandContext`), so it is handled here rather than by
/// adding a command to `eda-cmd`'s registry.
const OPEN_PALETTE: &str = "ui.open_palette";

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("failed to create the Slint window: {0}")]
    Platform(#[from] slint::PlatformError),
    #[error("failed to set the Slint rendering notifier: {0}")]
    RenderingNotifier(String),
    #[error("failed to load the keymap: {0}")]
    Keymap(#[from] crate::keymap::KeymapError),
}

/// The top-level `eda-ui` application: a Slint window plus the
/// `CommandHost` it drives. `eda-app`'s `main` constructs one of these
/// after selecting the Slint wgpu backend and calls `run`.
pub struct App {
    window: MainWindow,
}

impl App {
    /// Builds the window, wires every callback, and loads the
    /// xschem-style keymap preset (`docs/PLAN.md` names Virtuoso-style
    /// and xschem-style; M0.3 ships both as data but activates this one
    /// by default -- see the M0.3 report for how to switch).
    pub fn new() -> Result<Self, AppError> {
        Self::with_keymap(XSCHEM_PRESET)
    }

    /// As `new`, but with an explicit keymap source (exposed for tests
    /// and for a future preset-switcher command).
    pub fn with_keymap(keymap_source: &str) -> Result<Self, AppError> {
        let window = MainWindow::new()?;
        let keymap = Keymap::parse(keymap_source)?;

        // `Rc<RefCell<_>>`: Slint's callbacks are plain `Fn` closures
        // driven by a single-threaded event loop, so there is never
        // concurrent access, but several independent closures
        // (run-command, palette filter, key press, the rendering
        // notifier) all need their own handle to the same host. This is
        // the shape ADR-001 flagged as expected but unverified; it
        // worked as-is, no `RefCell::try_borrow` dance was needed
        // because none of these callbacks re-enters another one.
        let host = Rc::new(RefCell::new(CommandHost::with_builtins()));
        let palette_source = palette_source_entries(&host.borrow());

        sync_from_context(&window, host.borrow().context());
        window.set_palette_entries(filtered_entries(&palette_source, ""));

        {
            let window_weak = window.as_weak();
            let host = Rc::clone(&host);
            window.on_run_command(move |id| {
                let Some(window) = window_weak.upgrade() else {
                    return;
                };
                let mut host = host.borrow_mut();
                let command_id = CommandId::from(id.to_string());
                if let Err(err) = host.run(&command_id) {
                    let mut cx = host.context().clone();
                    cx.status_message = Some(err.to_string());
                    sync_from_context(&window, &cx);
                    return;
                }
                sync_from_context(&window, host.context());
            });
        }

        {
            let window_weak = window.as_weak();
            let palette_source = palette_source.clone();
            window.on_palette_filter_changed(move |text| {
                let Some(window) = window_weak.upgrade() else {
                    return;
                };
                window.set_palette_entries(filtered_entries(&palette_source, text.as_str()));
            });
        }

        {
            let window_weak = window.as_weak();
            let host = Rc::clone(&host);
            window.on_key_pressed(move |text, ctrl, shift, alt, meta| {
                let Some(command_id) = keymap.lookup(text.as_str(), ctrl, shift, alt, meta) else {
                    return false;
                };
                let Some(window) = window_weak.upgrade() else {
                    return false;
                };
                if command_id.as_str() == OPEN_PALETTE {
                    window.set_palette_open(true);
                    return true;
                }
                let mut host = host.borrow_mut();
                let ran = host.run(command_id).is_ok();
                sync_from_context(&window, host.context());
                ran
            });
        }

        setup_canvas(&window, &host);

        Ok(Self { window })
    }

    pub fn run(&self) -> Result<(), AppError> {
        self.window.run().map_err(AppError::from)
    }
}

/// Wires the texture-import harness (`crate::canvas::Canvas`) to the
/// window's rendering notifier. Split out of `App::new` only to keep
/// that function's closure count readable.
fn setup_canvas(window: &MainWindow, host: &Rc<RefCell<CommandHost>>) {
    let canvas = Rc::new(RefCell::new(Canvas::new()));
    let window_weak = window.as_weak();
    let host = Rc::clone(host);

    // `set_rendering_notifier` returns `Err` only when the current
    // backend has no GPU renderer selected; `eda-app` always selects
    // one before constructing `App`, but a test harness running without
    // a display will hit this path, so it is logged rather than
    // unwrapped.
    if let Err(err) = window.window().set_rendering_notifier(move |state, api| {
        let Some(window) = window_weak.upgrade() else {
            return;
        };
        match state {
            slint::RenderingState::RenderingSetup => canvas.borrow_mut().setup(api),
            slint::RenderingState::BeforeRendering => {
                let width = window.get_requested_texture_width().max(0) as u32;
                let height = window.get_requested_texture_height().max(0) as u32;
                let theme = host.borrow().context().theme;
                canvas
                    .borrow_mut()
                    .set_clear_color(canvas_clear_color(theme));
                if let Some(Ok(image)) = canvas.borrow_mut().before_rendering(width, height) {
                    window.set_texture(image);
                }
                window.window().request_redraw();
            }
            slint::RenderingState::RenderingTeardown => canvas.borrow_mut().teardown(),
            _ => {}
        }
    }) {
        eprintln!("eda-ui: failed to set the Slint rendering notifier: {err}");
    }
}

/// Mirrors `ui/theme.slint`'s `canvas-clear` token. Kept in one place
/// (this function) rather than duplicated at each call site; the
/// values themselves necessarily live twice (Slint color literal, wgpu
/// `Color`) because the canvas is cleared by our own render pass, not
/// by Slint.
fn canvas_clear_color(theme: Theme) -> wgpu::Color {
    match theme {
        Theme::Dark => wgpu::Color {
            r: 0x10 as f64 / 255.0,
            g: 0x10 as f64 / 255.0,
            b: 0x14 as f64 / 255.0,
            a: 1.0,
        },
        Theme::Light => wgpu::Color {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    }
}

fn sync_from_context(window: &MainWindow, cx: &CommandContext) {
    window.set_theme(cx.theme.as_str().into());
    window.set_status_message(
        cx.status_message
            .clone()
            .unwrap_or_else(|| "Ready".to_owned())
            .into(),
    );
}

/// `(id, title)` for every registered command, captured once:
/// `CommandRegistry` never gains or loses entries at runtime in M0.3.
#[derive(Clone)]
struct PaletteSourceEntry {
    id: String,
    title: String,
}

fn palette_source_entries(host: &CommandHost) -> Rc<Vec<PaletteSourceEntry>> {
    Rc::new(
        host.registry()
            .iter()
            .map(|cmd| PaletteSourceEntry {
                id: cmd.id().to_string(),
                title: cmd.title().to_owned(),
            })
            .collect(),
    )
}

fn filtered_entries(source: &[PaletteSourceEntry], filter: &str) -> slint::ModelRc<PaletteEntry> {
    let filter = filter.to_ascii_lowercase();
    let matches: Vec<PaletteEntry> = source
        .iter()
        .filter(|entry| {
            filter.is_empty()
                || entry.id.to_ascii_lowercase().contains(&filter)
                || entry.title.to_ascii_lowercase().contains(&filter)
        })
        .map(|entry| PaletteEntry {
            id: entry.id.clone().into(),
            title: entry.title.clone().into(),
        })
        .collect();
    slint::ModelRc::new(slint::VecModel::from(matches))
}

#[cfg(test)]
mod tests {
    use super::*;
    use slint::Model;

    /// `App::new` itself needs a display (not available in this
    /// sandbox); these tests exercise the display-independent helpers
    /// that back it.

    #[test]
    fn canvas_clear_color_matches_theme_slint_tokens() {
        // ui/theme.slint: canvas-clear is #101014 (dark) / #ffffff (light).
        let dark = canvas_clear_color(Theme::Dark);
        assert!((dark.r - 0x10 as f64 / 255.0).abs() < f64::EPSILON);
        assert!((dark.g - 0x10 as f64 / 255.0).abs() < f64::EPSILON);
        assert!((dark.b - 0x14 as f64 / 255.0).abs() < f64::EPSILON);

        let light = canvas_clear_color(Theme::Light);
        assert_eq!(light.r, 1.0);
        assert_eq!(light.g, 1.0);
        assert_eq!(light.b, 1.0);
    }

    #[test]
    fn filtered_entries_matches_id_or_title_case_insensitively() {
        let source = vec![
            PaletteSourceEntry {
                id: "app.hello".to_owned(),
                title: "Hello".to_owned(),
            },
            PaletteSourceEntry {
                id: "view.toggle_theme".to_owned(),
                title: "Toggle theme".to_owned(),
            },
        ];

        let all = filtered_entries(&source, "");
        assert_eq!(all.row_count(), 2);

        let by_title = filtered_entries(&source, "HELLO");
        assert_eq!(by_title.row_count(), 1);
        assert_eq!(by_title.row_data(0).unwrap().id, "app.hello");

        let by_id = filtered_entries(&source, "toggle_theme");
        assert_eq!(by_id.row_count(), 1);

        let none = filtered_entries(&source, "nonexistent");
        assert_eq!(none.row_count(), 0);
    }

    #[test]
    fn palette_source_entries_lists_every_builtin() {
        let host = CommandHost::with_builtins();
        let entries = palette_source_entries(&host);
        assert_eq!(entries.len(), host.registry().len());
        assert!(entries.iter().any(|e| e.id == "app.hello"));
        assert!(entries.iter().any(|e| e.id == "view.toggle_theme"));
        assert!(entries.iter().any(|e| e.id == "edit.undo"));
        assert!(entries.iter().any(|e| e.id == "edit.redo"));
    }
}
