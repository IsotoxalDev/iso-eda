# ADR-001: UI toolkit for the iso-eda shell and canvas

- Status: accepted
- Date: 2026-11-15 (proposed 2026-11-14; accepted 2026-11-15 after the
  benchmarks in `spikes/results/` were committed in `74830ac`)
- Milestone: M0.2 — UI spike (GPUI, Slint, iced, egui) drawing 200k rects,
  50k segments, 5k labels with pan/zoom

## Context

### What forces the decision

iso-eda needs one Rust UI toolkit for the whole application: dockable panels,
command palette, property editors *and* a schematic/layout canvas that must
stay interactive with hundreds of thousands of primitives in view. The canvas
is the hard part. Panels are a solved problem in every candidate; a canvas
that pans and zooms smoothly over an analog layout is not. Every later
milestone (M0.3 shell, M1.4 `eda-render`, Phase 3 layout) builds on this
choice, so it must be made on measurements, not taste.

`docs/PLAN.md` records a working assumption: "GPUI + gpui-component, pinned
revision; confirmed by M0.2; Slint shell + wgpu canvas is the fallback".
That is a prior, not a decision. This ADR exists to test it.

### Constraints

- License: iso-eda is GPL-3.0-or-later. Any toolkit we link must be
  GPL-3.0-or-later-compatible (CLAUDE.md allowed list: GPL-2.0-or-later,
  GPL-3.0, LGPL, BSD, MIT, Apache-2.0).
- Build: everything must build inside the Nix flake (`nix develop`,
  `nix flake check`). A toolkit that cannot be pinned and built offline in
  the Nix sandbox is disqualified regardless of speed.
- Architecture (CLAUDE.md): only `eda-ui` and `eda-app` may depend on the
  toolkit. `eda-render` produces a toolkit-independent draw list. So the
  toolkit's job on the canvas is to rasterize a per-frame draw list of
  rects, segments and labels quickly; it does not own the scene.
- Platform: Linux first (Wayland and X11). macOS/Windows are Phase 6.

### The prior, stated honestly

The project owner already runs gpui in production in a separate project
(`~/Dev/Rachana`, a screenplay editor), knows its API, and has a working Nix
shell for it. He said gpui "is kinda the best" given the licensing angle
(Apache-2.0, cleanest fit). He first said "don't want egui tbh", then
reversed and asked for it to be included anyway. These are real inputs and
they are recorded here so the reader can judge the analysis against them.
They are not evidence about canvas throughput. The point of the spike is to
catch a surprise: gpui being poor at this specific heavy-canvas workload, or
an unfixable Nix packaging problem, or one of the others being dramatically
faster. The Decision section below depends on the measurements, not on
this prior.

### Facts verified during design (2026-11-14)

All of the following were read from the upstream repositories or docs.rs on
the date above, not recalled from memory.

**gpui** (Zed). Apache-2.0. Not usable from crates.io: `crates/gpui/Cargo.toml`
path-depends on Zed workspace crates (`collections`, `gpui_macros`,
`gpui_shared_string`, `gpui_util`, `http_client`, `scheduler`, `refineable`,
`sum_tree`, ...). Consumed as a git dependency pinned to a revision; Cargo
resolves the internal path deps itself. This is the pattern Rachana uses and
it works. Since Rachana's pin the crate has been split: on Linux an app
depends on `gpui` plus `gpui_platform` (features `wayland`, `x11`), which
pulls `gpui_linux` and `gpui_wgpu`. The Linux renderer is **wgpu** (Zed's
fork, `wgpu 29.0.3` at Rachana's lock), not blade. Zed's
`rust-toolchain.toml` pins `1.98.1`. Rachana's `Cargo.lock` pins
`zed-industries/zed#137e677a0561eb7284cfad1bedccea70155e2473` and builds on
this machine with the Nix shell in `~/Dev/Rachana/flake.nix`.

- Drawing API (from `crates/gpui/examples/painting.rs` and `paths_bench.rs`):
  `canvas(prepaint, paint)` element; inside paint,
  `window.paint_quad(quad(bounds, corner_radii, background, border_widths,
  border_color, border_style))`, `PathBuilder::stroke(px(1.))` /
  `PathBuilder::fill()` with `move_to`/`line_to`/`build()` then
  `window.paint_path(path, color)`; continuous redraw via
  `window.request_animation_frame()`.
- Text API (from `crates/gpui/src/text_system.rs`, `text_system/line.rs`):
  `window.text_system().shape_line(text: SharedString, font_size: Pixels,
  runs: &[TextRun], force_width: Option<Pixels>) -> ShapedLine`, then
  `ShapedLine::paint(&self, origin: Point<Pixels>, line_height: Pixels,
  align: TextAlign, align_width: Option<Pixels>, window: &mut Window,
  cx: &mut App) -> Result<()>`.
- Headless: at the pinned rev the entry point is
  `gpui_platform::headless() -> gpui::Application`
  (`crates/gpui_platform/src/gpui_platform.rs`). The
  `WindowingRequest::Headless` name quoted here on 2026-11-14 came from Zed
  `main` and does not exist at the pin (corrected 2026-11-15 by the
  implementer reading the checkout). Headless mode **opens no window and
  renders nothing**; it exists so an app can keep running and later attach
  to a display. It is not an offscreen renderer. Consequence for
  methodology: a true offscreen GPU benchmark is not available for gpui, so
  all four spikes are benchmarked in an unattended *windowed* mode (see
  Methodology).
- Texture import (read 2026-11-15 from the pinned checkout, for the shell
  question in Decision): `Window::paint_image(bounds, corner_radii,
  Arc<RenderImage>, frame_index, grayscale)` takes a `RenderImage`, which
  wraps **CPU-side** `image::Frame` pixel buffers; gpui copies them into
  its sprite atlas with `queue.write_texture` each time a new image id is
  seen. `Window::paint_surface(bounds, CVPixelBuffer)` is
  `#[cfg(target_os = "macos")]` only. `gpui_wgpu::WgpuContext` has `pub
  device: Arc<wgpu::Device>` / `pub queue`, but nothing in gpui's public
  `Window`/`App` API hands an application that context (`gpu_specs()`
  returns metadata only), and it is Zed's fork of wgpu 29, not crates.io
  wgpu 30. There is no custom-render-pass hook. So on Linux the only way
  to show an externally rendered frame inside gpui is GPU -> CPU readback
  -> `RenderImage` -> `paint_image` -> GPU upload, one round trip per frame.
- Present mode (from `gpui_wgpu/src/wgpu_renderer.rs`,
  `gpui_linux/src/linux/{wayland,x11}/window.rs`): `WgpuSurfaceConfig
  { preferred_present_mode }` is hard-coded by the platform layer —
  `Some(PresentMode::Mailbox)` on Wayland, `None` (falls back to `Fifo`,
  i.e. vsync) on X11. `desired_maximum_frame_latency` is hard-coded to 2.
  No env var or public setter. Consequence: the gpui bench must run under
  **Wayland** to get unthrottled frame intervals; under X11 it saturates at
  the refresh rate.
- System libraries: Zed's `nix/build.nix` lists the full-editor set. The
  narrower set that is **verified to build gpui-only** on this machine is
  Rachana's `flake.nix` buildInputs (listed under Nix below). We adopt that
  list rather than guess a smaller one.

**egui + eframe** 0.37.0 (released 2026-10-15). MIT OR Apache-2.0, crates.io,
workspace `rust-version = "1.95"`, edition 2024, wgpu 30, winit 0.30.
`eframe::NativeOptions` has `renderer: Renderer` (must be set to `Wgpu`; the
default feature set uses glow) and `wgpu_options: WgpuConfiguration` whose
`surface: SurfaceConfig` has `present_mode: wgpu::PresentMode` and
`desired_maximum_frame_latency: Option<u32>`. So vsync can be disabled from
code. egui tessellates every `Shape` on the CPU every frame; 200k rects
through `egui::Painter` measures exactly that.

**iced** 0.14.0 (released 2026-10-27). MIT, crates.io, workspace
`rust-version = "1.88"`, edition 2024, **wgpu 27**, winit 0.30, lyon 1.0
tessellation for `canvas`, tiny-skia software fallback. Present mode is
controlled by the `ICED_PRESENT_MODE` env var (`vsync`, `no_vsync`,
`immediate`, `fifo`, `fifo_relaxed`, `mailbox`; `iced_wgpu::window::
compositor::present_mode_from_env`). Per-frame tick via
`iced::window::frames()`. The `canvas` widget (`Program::draw(&self, state,
renderer, theme, bounds, cursor) -> Vec<Geometry>`, `Frame::fill_rectangle`,
`Frame::stroke`, `Frame::fill_text`) tessellates on CPU. A `shader` widget
exists for custom wgpu passes (requires the `advanced`/`wgpu` features).

**Slint** 1.18.1 (released 2026-09-21). License string in the workspace
manifest: `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR
LicenseRef-Slint-Software-3.0`. We would use the **GPL-3.0-only** option.
That is compatible with linking into a GPL-3.0-or-later program, but the
combined binary can then only be distributed under GPL-3.0 **exactly**, not
"or later". This is a real constraint and is recorded under Consequences.
`rust-version = "1.92"`, edition 2024. wgpu integration behind unstable
features `unstable-wgpu-29` / `unstable-wgpu-30`:
`slint::BackendSelector::new().require_wgpu_30(WGPUConfiguration::...)
.select()`, then `window.set_rendering_notifier(|state: RenderingState,
api: GraphicsAPI| ...)`. `GraphicsAPI::WGPU30 { instance, device, queue }`
provides **no render target**: the documented pattern
(`examples/wgpu_texture/lib.rs`) is to render into your own `wgpu::Texture`
in `RenderingState::BeforeRendering`, import it with
`slint::Image::try_from(texture)`, assign it to an `Image` element, and call
`window.request_redraw()`. Required features: `backend-winit`,
`renderer-femtovg-wgpu`, `unstable-wgpu-30`. Consequence: Slint cannot draw
200k primitives as Slint elements; the Slint spike necessarily measures
"Slint shell + our own wgpu canvas", which is exactly the fallback
architecture named in PLAN.md. This makes its number not directly comparable
to the three toolkits drawing with their own primitives, and the results
table must say so.

**Toolchain.** The main workspace pins `1.82.0`. gpui at Zed main needs
1.98.1; egui 1.95; slint 1.92; iced 1.88. `oxalica/rust-overlay`
`manifests/stable/default.nix` lists `1.98.1` (latest `1.100.0`), so the
spikes can pin `1.98.1` exactly as Zed does. The main workspace pin is not
changed by this milestone.

**Nix sandbox.** `nix build` has no GPU and no display. Nix checks for the
spikes can only prove that each spike **compiles** against the pinned
toolchain and libraries. **`nix flake check` passing is not the benchmark.**
Frame times come only from running the binaries on a developer machine with
a real GPU and a Wayland (preferred) or X11 session.

### Methodology: what "frame times recorded" means

The acceptance check for M0.2 is "Frame times recorded; ADR-001 states the
choice". The following definitions are binding for the implementer; the
results table in the Decision section is filled from them and nothing else.

**Shared scene (crate `spikes/scene`).** All four spikes render the *same*
scene, generated by the same code from the same seed, so no toolkit gets a
different workload.

- World: integer coordinates in `[0, 1_000_000]` on both axes (database
  units; the spikes may convert to `f32`/`f64` for drawing — the no-floats
  rule applies to `eda-db`, not to throwaway spikes).
- PRNG: SplitMix64, implemented in `scene` (no `rand` dependency; avoids
  version drift between spikes). Seed `0x0150_EDA0_0000_0001`.
- Rects: `200_000 * scale` axis-aligned, filled, no outline. Width and
  height each uniform in `[200, 20_000]`; position uniform such that the
  rect stays inside the world. Color = `PALETTE[i % 8]` (8 fixed opaque
  colours defined in `scene`).
- Segments: `50_000 * scale` two-point segments, hairline (1 device pixel
  wide regardless of zoom). 70% axis-aligned (alternating H/V by a PRNG
  coin), 30% arbitrary angle. Length uniform in `[500, 50_000]`. Colour
  fixed (`PALETTE[8]`, a 9th entry).
- Labels: `5_000 * scale` strings of 4–12 characters from
  `[A-Za-z0-9_]`, anchored at a world point, left-aligned baseline,
  nominal height `3_000` world units. The on-screen pixel size is
  `label_px(zoom) = round(3_000 * zoom)` clamped to `[4, 64]`; if
  `3_000 * zoom < 4` the label is **not drawn** (shared LOD rule, applied
  identically by all spikes through the `scene` helper). This bounds the
  number of distinct font sizes any toolkit must rasterize to 61.
- Font: all spikes load the same TrueType file from `$SPIKE_FONT`
  (exported by the `spikes` devShell as DejaVu Sans from nixpkgs) and
  register it with the toolkit (gpui `add_fonts`, egui `set_fonts`, iced
  `font::load`, canvas-wgpu `ab_glyph`). No system font lookup.
- `scale` is `1` for the acceptance run and `4` for the "break the vsync
  cap" run (see Metric). Both are recorded.

**Shared camera path (crate `spikes/scene`).** `Camera { center_x: f64,
center_y: f64, zoom: f64 }` where `zoom` is screen pixels per world unit.
`camera_at(frame: u32, viewport: Viewport) -> Camera` is a pure function
of the frame index, identical for all spikes. `N = 600` frames:

- Frames `0..60`: warm-up at fit-to-viewport zoom (`fit = min(w, h) /
  1_000_000`), centre `(500_000, 500_000)`. Excluded from statistics.
- Frames `60..240` (pan): zoom `fit`, centre moves linearly from
  `(250_000, 500_000)` to `(750_000, 500_000)`.
- Frames `240..420` (zoom): centre fixed at `(600_000, 400_000)`, zoom
  grows exponentially from `fit` to `64 * fit`.
- Frames `420..600` (pan zoomed): zoom `64 * fit`, centre moves linearly
  from `(600_000, 400_000)` to `(640_000, 430_000)`.

**Rendering rules (fairness).**

1. Every frame, each spike submits **every** rect and segment in the scene
   through the toolkit's immediate paint API (gpui `paint_quad`/`paint_path`,
   egui `Painter::extend(Vec<Shape>)`, iced `canvas::Frame`, canvas-wgpu
   instance buffers). No view-frustum culling in the spike; culling is
   `eda-render`'s job later and we want the toolkit's raw throughput.
2. No cross-frame caching of rect/segment geometry (no iced `canvas::Cache`,
   no retained egui meshes). Caching **shaped text** per
   `(string, px_size)` is allowed in every spike, because every toolkit
   would otherwise be dominated by shaping and real editors cache it.
3. Axis-aligned segments may be drawn as 1-px rects if the toolkit has no
   cheaper hairline (note it in the results); non-axis-aligned segments
   must use the toolkit's line/path primitive.
4. Window: `1600 x 1000` logical pixels, requested identically. The
   actual device-pixel size and scale factor are recorded.
5. Vsync off wherever the toolkit allows: eframe
   `SurfaceConfig::present_mode = AutoNoVsync`; iced `ICED_PRESENT_MODE=
   mailbox` (set by `bench.sh`); gpui — not controllable, run on Wayland
   (Mailbox); Slint — not verified to be controllable (see "Not verified").
6. Continuous redraw requested every frame (`request_animation_frame`,
   `ctx.request_repaint()`, `window::frames()`, `window.request_redraw()`).

**Metric.** Inside each spike's per-frame callback, take
`std::time::Instant::now()` once at entry. `frame_ms[i]` is the interval
between entries of frame `i` and frame `i-1` (frame-to-frame wall time, the
thing the user feels). `build_ms[i]` is the time spent inside the paint
callback for frame `i` (CPU submission cost; GPU cost is hidden by
asynchronous presentation and is only visible through `frame_ms`). Over
frames `60..600` report for both series: `mean`, `p50`, `p95`, `p99`,
`max` in milliseconds, and `fps = 1000 / mean(frame_ms)`. The `scale = 4`
run exists because a toolkit that cannot disable vsync will show a flat
~16.7 ms at `scale = 1` on a 60 Hz display; at 4× load every toolkit is
expected to be below refresh rate, so the interval becomes informative.

**Output.** `--bench` prints one JSON object to stdout and exits 0 after
frame `N`. Keys: `toolkit`, `toolkit_version`, `scale`, `frames`,
`viewport_px`, `scale_factor`, `display_server` (`wayland`/`x11`),
`present_mode_requested`, `frame_ms: {mean,p50,p95,p99,max}`,
`build_ms: {...}`, `fps`, `notes` (free text: e.g. "segments as 1px
quads"). `spikes/bench.sh` runs every spike at both scales, writes
`spikes/results/<toolkit>-x<scale>.json`, and writes
`spikes/results/environment.txt` (`uname -a`, `vulkaninfo --summary`
adapter line, `$XDG_SESSION_TYPE`, monitor refresh rate if obtainable,
spikes git commit). The results files **are committed** as the evidence
this ADR cites.

**Pass/fail bar for the canvas workload.** A toolkit is "viable" if at
`scale = 1` it reaches `p95(frame_ms) <= 16.7` (60 fps with headroom) on
the owner's machine, and "comfortable" if it also reaches `p95 <= 33` at
`scale = 4`. Toolkits that are viable are then separated by the
non-performance criteria in Options considered (license, Nix build risk,
widget completeness, maintenance). If none is viable with native
primitives, the decision defaults to the "shell + wgpu canvas"
architecture and the toolkit choice becomes a widget/licensing question
only.

## Options considered

Filled in 2026-11-15 from `spikes/results/*.json` as committed in `74830ac`
(spikes source at `8bb87d3`). Machine (`spikes/results/environment.txt`):
NixOS, Linux 7.2.8, Intel Graphics (ARL) integrated GPU (`vulkaninfo`),
Wayland session (Hyprland), monitor refresh rate not obtainable. 600 frames
per run, warm-up excluded. The Slint row measures Slint shell + our wgpu
canvas, not Slint primitives (see Context). All times in ms.

| Toolkit | scale | display | frame_ms p50 / p95 / max | build_ms p95 | fps | notes |
| --- | --- | --- | --- | --- | --- | --- |
| gpui `zed@137e677a` | 1 | wayland, 784x711 @1.0, Mailbox (uncontrollable) | 111.5 / **1408.5** / 1627.8 | 1229.8 | 1.88 | build_ms mean 452; all paint on the UI thread |
| gpui | 4 | wayland | **failed** | — | — | SIGKILL (exit 137) by the compositor's "not responding" watchdog; reproduced on a standalone retry with an 1800 s timeout; not OOM, not a GPU reset (dmesg/journalctl clean) |
| egui 0.37.0 | 1 | wayland, 383x463 @1.6, AutoNoVsync | 51.3 / **205.1** / 13076.4 | 49.9 | 8.19 | max is a single outlier (p99 248.5) |
| egui | 4 | wayland | **failed** | — | — | wgpu validation error: `egui_vertex_buffer` size 308 215 840 B > device `max_buffer_size` 268 435 456 B; panicked in `Device::create_buffer` |
| iced 0.14.0 | 1 | wayland, 1600x1000 @1.0, mailbox | 53.6 / **85.6** / 1515.2 | 42.4 | 16.6 | max is a single outlier (p99 110.3) |
| iced | 4 | wayland, 1600x1000 @1.0, mailbox | 278.7 / **396.8** / 475.1 | 188.9 | 3.42 | completed; clean standalone retry after a transient first attempt (empty stdout, exit 0) |
| slint 1.18.1 + canvas-wgpu | 1 | wayland, 388x463 @1.0, present mode not set | 4.25 / **5.33** / 9.83 | 2.86 | 226.7 | **viable** (p95 <= 16.7) |
| slint 1.18.1 + canvas-wgpu | 4 | wayland, 388x463 @1.0, present mode not set | 19.2 / **30.1** / 35.0 | 27.3 | 49.7 | **comfortable** (p95 <= 33) |

### Reading the results

**Against the bar.** Only Slint + canvas-wgpu is viable (p95 5.33 ms at
scale 1 against a 16.7 ms bar) and it is also comfortable (30.1 ms at
scale 4 against 33 ms). No toolkit drawing with its **own** primitives
comes within an order of magnitude of the bar: iced is 5x over, egui 12x
over, gpui 84x over. The ADR's fallback clause therefore applies: the
canvas is our own wgpu renderer and the toolkit question becomes a shell
question. Independent re-runs by the orchestrating agent on 2026-11-15
reproduced the Slint numbers (scale 1: 219.9 fps / p95 5.47 ms; scale 4:
50.7 fps / p95 29.3 ms) and the undersized windows for egui/gpui/slint
(same sizes each run).

**gpui at scale 4 is a structural result, not a fluke.** gpui builds its
scene on the main (UI) thread inside `paint`; `build_ms` at scale 1 was
already 452 ms mean / 1230 ms p95 for 200k `paint_quad` + 50k `paint_path`
calls. At 800k/200k the per-frame paint blocks the UI thread long enough
that the Wayland compositor's not-responding watchdog terminates the
process before 600 frames complete. There is no custom wgpu pass, no
off-thread paint, and no GPU texture import on Linux to route around it
(see Context, "Texture import"), so there is no cheap fix inside gpui.

**egui at scale 4 fails on a hard device limit.** egui re-tessellates
every `Shape` on the CPU every frame (fairness rule 2 forbids retained
meshes) and uploads one vertex buffer; at 800k rects that buffer is
308 MB, above the 256 MB `max_buffer_size` of this device. This is a
ceiling of the native-primitive path, not a bug in the spike. egui's
`egui_wgpu::Callback` path (our own pass inside egui's render pass) was not
measured; it would not hit this limit because it does not go through egui's
tessellator.

**iced** completed both runs without crashing but is 5x over the bar at
scale 1 and 12x over the comfortable bar at scale 4; its `build_ms` (CPU
lyon tessellation) alone exceeds the frame budget.

**Window-size caveat (resolved, not an open question).** All four spikes
request a 1600x1000 logical window identically (gpui
`WindowBounds::Windowed(Bounds::centered(..))`, egui
`ViewportBuilder::with_inner_size`, iced `window::Settings { size }`, Slint
`preferred-width: 1600px; preferred-height: 1000px`). Only iced got it. egui (383x463 @1.6), gpui
(784x711) and slint (388x463) were measured on much smaller viewports. The
owner's session is **Hyprland**, a tiling Wayland compositor: a new
toplevel that does not request floating is tiled into whatever slot the
layout has free, and the compositor's configure event overrides the
client's requested size. iced's winit setup happened to receive the full
size; the other three were tiled. The behaviour is deterministic across
re-runs. This is an environment interaction, not a defect in any toolkit or
spike, and it does **not** change the ranking: iced, measured on the
*largest* viewport, is still 16x slower than slint on the smallest, and
gpui/egui failed outright at scale 4 on the small viewports (a larger
viewport could only make them slower). The one thing it does weaken is the
absolute margin of the slint + canvas-wgpu row: its CPU-side `build_ms`
(2.86 ms p95) is viewport-independent, but its GPU fill cost would grow
with a 1600x1000 (8.9x more pixels) target; the GPU share of the measured
5.33 ms is at most ~2.5 ms, which would need to grow ~5.5x to breach the
16.7 ms bar. We do not re-run the spikes for this; the shell built in M0.3
has a real full-size window and the canvas frame time is measured there as
a matter of course (see Consequences).

### Option A — gpui (Zed's UI framework), pinned git revision

- What: retained-mode-ish element tree with an immediate `canvas` escape
  hatch; wgpu renderer on Linux; text, SVG, images, layout (taffy) built in;
  `gpui-component` provides docks, tabs, trees, inputs. This is the
  toolkit that most resembles "an editor shell", which is what iso-eda is.
- Cost: git dependency on a very large repo (whole Zed clone per lock
  update, several minutes in Nix); toolchain follows Zed (1.98.1 today);
  API churn between revisions (the `gpui_platform` split happened since
  Rachana's pin); Linux font stack via font-kit/cosmic-text; must learn
  `canvas` + `paint_quad`/`paint_path`/`shape_line` for the heavy canvas
  because gpui exposes **no custom wgpu pass** — the canvas must go
  through gpui's own primitives. If gpui's quad/path path is slow for
  200k objects there is no escape hatch short of patching gpui.
- Risk: medium build/packaging risk (mitigated: Rachana already builds it
  under Nix on this machine); unknown canvas throughput — this is the
  number the spike exists to produce; present mode uncontrollable (Fifo on
  X11).
- Prior experience: the owner runs gpui in production in Rachana and knows
  it. That lowers the learning cost materially; it says nothing about the
  canvas workload.
- License: Apache-2.0. Cleanest fit for GPL-3.0-or-later.

### Option B — egui + eframe

- What: pure immediate-mode GUI; every widget and shape is re-tessellated
  on the CPU each frame; `egui-wgpu` uploads meshes. Mature, small,
  trivially embeddable, excellent docs; `egui_dock` for panels.
- Cost: immediate-mode tessellation of 200k shapes per frame is CPU-bound
  by construction; text atlas re-rasterizes per font size; dockable panels
  and complex editors are possible but the widget set is thinner than
  gpui-component/Slint for an IDE-like shell. A custom `egui_wgpu::Callback`
  can host our own wgpu canvas, which would sidestep the tessellation cost
  (optional stretch measurement, not required).
- Risk: low build risk (crates.io, winit, wgpu 30); high probability that
  native-primitive throughput fails the bar — which the spike will show
  honestly. The owner's stated dislike is recorded above and is not an
  input to the measurement.
- License: MIT OR Apache-2.0.

### Option C — iced

- What: Elm-architecture retained widget tree with a `canvas` widget (lyon
  tessellation on CPU, GPU upload) and a `shader` widget for custom wgpu
  passes; wgpu 27 renderer with tiny-skia fallback.
- Cost: Elm message-passing fits "every action is an `eda-cmd` command"
  well but makes incremental UI (docking, many panels) verbose; `canvas`
  re-tessellates per frame unless cached (and our rules forbid geometry
  caching); wgpu one major behind egui/slint, so a shared custom canvas
  would need its own wgpu 27 build; smaller ecosystem for dock/tab widgets.
- Risk: low build risk (crates.io); medium throughput risk for native
  primitives; present mode controllable via env.
- License: MIT.

### Option D — Slint (GPL-3.0-only option) shell + our wgpu canvas

- What: declarative `.slint` UI compiled at build time, mature widget set,
  professional accessibility and theming; canvas via wgpu texture import
  (`renderer-femtovg-wgpu` + `unstable-wgpu-30`). This is PLAN.md's named
  fallback architecture.
- Cost: we write and own the whole canvas renderer (instanced rects,
  hairlines, glyph atlas) — more code now, but that code is
  toolkit-independent and would be reused if any other toolkit's primitives
  turn out too slow; wgpu integration is explicitly `unstable-*` and may
  break on minor releases; `.slint` build step; two UI paradigms (Slint
  DSL + Rust canvas) to teach.
- Risk: license lock-in to GPL-3.0-**only** for distributed binaries (see
  Consequences); unstable API surface; vsync control not verified.
- License: `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR
  LicenseRef-Slint-Software-3.0`; we would take GPL-3.0-only.

### Spike layout the implementer builds

Everything lives under a top-level `spikes/` directory, **outside**
`crates/` and outside the root Cargo workspace. The spikes are throwaway;
none of them is `eda-ui` or `eda-app`, so CLAUDE.md's "only eda-ui/eda-app
depend on the toolkit" rule remains true, and none of these dependencies
enters the `eda-*` build graph or lockfile.

```text
spikes/
  README.md                 what this is, how to run, "throwaway" notice
  rust-toolchain.toml       channel = "1.98.1", components rustfmt+clippy
  bench.sh                  runs all four spikes at scale 1 and 4, writes results/
  scene/                    lib crate, no deps, unit-tested, own Cargo.lock
    Cargo.toml  src/lib.rs  src/rng.rs  src/scene.rs  src/camera.rs
    src/metrics.rs  src/report.rs
  canvas-wgpu/              lib crate (wgpu 30 + bytemuck + ab_glyph), used by spike-slint
    Cargo.toml  src/lib.rs  src/rects.wgsl  src/lines.wgsl  src/text.wgsl
  spike-gpui/   Cargo.toml  Cargo.lock  src/main.rs
  spike-egui/   Cargo.toml  Cargo.lock  src/main.rs
  spike-iced/   Cargo.toml  Cargo.lock  src/main.rs
  spike-slint/  Cargo.toml  Cargo.lock  build.rs  ui/app.slint  src/main.rs
  results/      gpui-x1.json ... slint-x4.json  environment.txt
```

Rules for the layout:

- Each spike is its **own Cargo package with its own `Cargo.lock`** and an
  empty `[workspace]` table in its `Cargo.toml` (so Cargo does not try to
  attach it to the root workspace). `scene` and `canvas-wgpu` likewise
  carry `[workspace]` and are consumed as `path = "../scene"` deps. The
  root `Cargo.toml` gets `exclude = ["spikes"]`. Rationale: the four spikes
  pull three different wgpu majors (Zed fork of 29, crates.io 30, crates.io
  27); one shared lockfile would be fragile and would make every spike
  rebuild when any one changes.
- Every `.rs`, `.sh` file starts with
  `// SPDX-License-Identifier: GPL-3.0-or-later` (the existing
  `scripts/check-spdx.sh` scans all tracked files, including `spikes/`).
  `.slint` and `.wgsl` files carry the same line as a comment.
- No `unsafe`, no `unwrap`/`expect` outside tests, `anyhow` in the
  binaries, `thiserror` is unnecessary in `scene` (its functions are
  total). CLI parsing is hand-rolled over `std::env::args()` (flags:
  `--bench`, `--scale N`, `--frames N`, `--width N`, `--height N`); no
  `clap`.

### Nix changes (in `flake.nix`)

- `spikesToolchain = pkgs.rust-bin.fromRustupToolchainFile
  ./spikes/rust-toolchain.toml;` and `spikesCraneLib = (crane.mkLib
  pkgs).overrideToolchain (_: spikesToolchain);`. The main workspace keeps
  `1.82.0`.
- `spikeNativeBuildInputs = [ pkg-config clang lld cmake ]` and
  `spikeBuildInputs` = Rachana's verified list: `openssl fontconfig
  freetype libxkbcommon wayland wayland-protocols vulkan-loader
  vulkan-headers libGL mesa alsa-lib libxcb xorg.libX11 xorg.libXcursor
  xorg.libXrandr xorg.libXi xorg.libxcb xorg.xcbutilimage
  xorg.xcbutilkeysyms xorg.xcbutilwm xorg.xcbutilrenderutil`. Env:
  `LIBCLANG_PATH = "${llvmPackages.libclang.lib}/lib"`,
  `VULKAN_SDK = "${vulkan-headers}"`. (Zed's full list additionally has
  `libdrm libgbm libva libxcomposite libxdamage libxext libxfixes git
  libgit2 nodejs protobuf sqlite zstd` — those serve the editor, not gpui;
  if a gpui build in the sandbox fails on a missing library, add from that
  list rather than searching further.)
- `devShells.spikes`: `spikesToolchain`, `cargo-nextest`, `vulkan-tools`
  (for `vulkaninfo`), the inputs above, and a `shellHook` exporting
  `LD_LIBRARY_PATH` for `vulkan-loader libGL wayland libxkbcommon libxcb
  xorg.libX11` (runtime dlopen) and `SPIKE_FONT="${dejavu_fonts}/share/
  fonts/truetype/DejaVuSans.ttf"`.
- `checks`: `spike-scene-test` (`spikesCraneLib.cargoNextest` on
  `spikes/scene`), and **build-only** `spike-gpui-build`,
  `spike-egui-build`, `spike-iced-build`, `spike-slint-build`
  (`spikesCraneLib.buildPackage` with `doCheck = false`). Each uses
  `src = spikesCraneLib.cleanCargoSource ./spikes` with `cargoToml`,
  `cargoLock` pointing at the spike's files and `postUnpack = ''cd
  $sourceRoot/<spike>; sourceRoot="."''` (crane's documented pattern for a
  package in a subdirectory with sibling path deps). Git dependencies
  (gpui → Zed repo and Zed's wgpu/xim forks) are vendored by crane from
  `Cargo.lock`; expect the first `nix flake check` after adding the gpui
  spike to take long because it clones Zed.
- **These checks prove compilation only.** The sandbox has no GPU and no
  display. The acceptance check "frame times recorded" is satisfied only by
  `spikes/results/*.json` produced by `spikes/bench.sh` on a real machine.
- The main workspace's `cleanCargoSource ./.` will now also include
  `spikes/**` (Cargo ignores excluded dirs, but edits under `spikes/`
  invalidate the main `src` hash). Optional: filter `spikes/` out of the
  main `src` with `lib.cleanSourceWith`. Not required for M0.2.

### New dependencies requiring CLAUDE.md sign-off

All are confined to `spikes/` and none enters the `eda-*` graph. Versions
are exact pins; `Cargo.lock` files are committed.

| Crate | Source | License | Reason |
| --- | --- | --- | --- |
| `gpui` | git `zed-industries/zed` rev `137e677a0561eb7284cfad1bedccea70155e2473` (Rachana's verified pin; bump only if an API the spike needs is missing) | Apache-2.0 | Option A toolkit; transitively pulls Zed workspace crates and Zed's wgpu 29 / xim forks (Apache-2.0 / MIT-or-Apache) |
| `gpui_platform` | same git rev, features `wayland`, `x11` | Apache-2.0 | gpui Linux platform + wgpu renderer entry point (`gpui_platform::application()`) |
| `eframe` 0.37.0 | crates.io, `default-features = false`, features `wgpu`, `wayland`, `x11`, `default_fonts` | MIT OR Apache-2.0 | Option B window + wgpu backend |
| `egui` 0.37.0 | crates.io | MIT OR Apache-2.0 | Option B painter API |
| `iced` 0.14.0 | crates.io, features `canvas` (+ defaults `wgpu`, `wayland`, `x11`) | MIT | Option C |
| `slint` 1.18.1 | crates.io, features `backend-winit`, `renderer-femtovg-wgpu`, `unstable-wgpu-30` | GPL-3.0-only (chosen among the triple license) | Option D shell |
| `slint-build` 1.18.1 | crates.io, build-dependency | GPL-3.0-only (same triple) | compiles `ui/app.slint` |
| `wgpu` 30.x | crates.io | MIT OR Apache-2.0 | `canvas-wgpu` renderer; must match Slint's `wgpu-30` |
| `bytemuck` 1.x (`derive`) | crates.io | Zlib OR Apache-2.0 OR MIT | instance buffer `Pod` types in `canvas-wgpu` |
| `ab_glyph` 0.2.x | crates.io | Apache-2.0 | glyph rasterization into the `canvas-wgpu` text atlas (chosen over `glyphon` to avoid coupling the shared canvas to a wgpu major) |
| `anyhow` 1.x | crates.io | MIT OR Apache-2.0 | error handling in the four binaries (already policy for binaries) |

`scene` has **no** dependencies by design (own SplitMix64, hand-written JSON
writer), so its tests run in the sandbox with nothing to vendor.

### Public types and functions of `spikes/scene` (signatures only)

```rust
// rng.rs
pub struct SplitMix64 { state: u64 }
impl SplitMix64 {
    pub fn new(seed: u64) -> Self;
    pub fn next_u64(&mut self) -> u64;
    /// Uniform in [lo, hi] inclusive; lo <= hi.
    pub fn range_i32(&mut self, lo: i32, hi: i32) -> i32;
    pub fn chance(&mut self, numerator: u32, denominator: u32) -> bool;
}

// scene.rs
pub const WORLD: i32 = 1_000_000;
pub const SEED: u64 = 0x0150_EDA0_0000_0001;
pub const LABEL_WORLD_HEIGHT: i32 = 3_000;
pub const LABEL_MIN_PX: u16 = 4;
pub const LABEL_MAX_PX: u16 = 64;
pub const PALETTE: [Rgba8; 9];

#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct Rgba8 { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct Rect { pub x0: i32, pub y0: i32, pub x1: i32, pub y1: i32, pub color: u8 }
#[derive(Clone, Copy, Debug, PartialEq, Eq)] pub struct Segment { pub x0: i32, pub y0: i32, pub x1: i32, pub y1: i32 }
#[derive(Clone, Debug, PartialEq, Eq)]      pub struct Label { pub x: i32, pub y: i32, pub text: String }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scene { pub rects: Vec<Rect>, pub segments: Vec<Segment>, pub labels: Vec<Label> }
impl Scene {
    /// scale >= 1. Counts are 200_000*scale, 50_000*scale, 5_000*scale.
    pub fn generate(scale: u32) -> Scene;
}
/// None when the label would be < LABEL_MIN_PX; else round(3_000*zoom) clamped to LABEL_MAX_PX.
pub fn label_px(zoom: f64) -> Option<u16>;

// camera.rs
pub const FRAMES: u32 = 600;
pub const WARMUP_FRAMES: u32 = 60;
#[derive(Clone, Copy, Debug, PartialEq)] pub struct Viewport { pub width_px: u32, pub height_px: u32 }
#[derive(Clone, Copy, Debug, PartialEq)] pub struct Camera { pub center_x: f64, pub center_y: f64, pub zoom: f64 }
impl Camera {
    pub fn fit(viewport: Viewport) -> Camera;
    pub fn to_screen(&self, viewport: Viewport, x: i32, y: i32) -> (f32, f32);
}
/// Total over 0..=FRAMES; frames >= FRAMES clamp to the last pose.
pub fn camera_at(frame: u32, viewport: Viewport) -> Camera;

// metrics.rs
pub struct FrameTimer { /* last Instant, Vec<f64> frame_ms, Vec<f64> build_ms */ }
impl FrameTimer {
    pub fn new() -> Self;
    /// Call once at the start of each frame callback.
    pub fn begin_frame(&mut self);
    /// Call once at the end of the paint callback.
    pub fn end_build(&mut self);
    pub fn frames_recorded(&self) -> u32;
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats { pub mean: f64, pub p50: f64, pub p95: f64, pub p99: f64, pub max: f64 }
/// Nearest-rank percentiles over the slice; empty slice -> all zeros.
pub fn stats(samples_ms: &[f64]) -> Stats;

// report.rs
pub struct Report {
    pub toolkit: String, pub toolkit_version: String, pub scale: u32, pub frames: u32,
    pub viewport_px: (u32, u32), pub scale_factor: f64, pub display_server: String,
    pub present_mode_requested: String, pub frame_ms: Stats, pub build_ms: Stats,
    pub fps: f64, pub notes: String,
}
impl Report {
    /// Builds from the timer, dropping the first WARMUP_FRAMES samples.
    pub fn from_timer(timer: &FrameTimer, meta: ReportMeta) -> Report;
    pub fn to_json(&self) -> String;
}
pub struct ReportMeta { /* the non-stat fields above */ }

// lib.rs
pub struct Args { pub bench: bool, pub scale: u32, pub frames: u32, pub width: u32, pub height: u32 }
impl Args { pub fn parse(args: impl Iterator<Item = String>) -> Result<Args, String>; }
/// "wayland" if WAYLAND_DISPLAY is set, else "x11" if DISPLAY is set, else "none".
pub fn display_server() -> String;
```

Invariants: `Scene::generate(s)` is a pure function of `s` (same bytes every
call, on every machine); every coordinate lies in `[0, WORLD]`; rects have
`x0 < x1`, `y0 < y1`; `camera_at` is total and finite for every `u32`
frame; `stats` never panics (empty input gives zeros); `Args::parse` never
panics on bad input (returns `Err`).

### Public types of `spikes/canvas-wgpu` (signatures only)

```rust
pub struct CanvasRenderer { /* pipelines, instance buffers, glyph atlas */ }
pub struct Uniforms { /* viewport size, camera center, zoom */ }
impl CanvasRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, target_format: wgpu::TextureFormat, font_ttf: &[u8]) -> anyhow::Result<Self>;
    /// Uploads the whole scene as instances for this frame (rule 1: everything, every frame).
    pub fn upload(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, scene: &scene::Scene, camera: scene::Camera, viewport: scene::Viewport);
    /// Records one render pass into `target`.
    pub fn render(&mut self, encoder: &mut wgpu::CommandEncoder, target: &wgpu::TextureView, clear: wgpu::Color);
}
```

Pipelines: instanced unit-quad for rects (instance = world rect + color,
camera transform in the vertex shader); instanced 1-px screen-space quads
for segments (expanded in the vertex shader); instanced textured quads for
glyphs from an `ab_glyph` R8 atlas keyed by `(char, px_size)`.

### Test list

`spikes/scene` (run by `cargo nextest` locally and by `spike-scene-test`
in Nix):

| Test | Proves |
| --- | --- |
| `splitmix64_matches_reference_vector` | `SplitMix64::new(1234567)` yields the published reference sequence (`6457827717110365317, 3203168211198807973, 9817491932198370423, 4593380528125082431` — Rosetta Code task "Pseudo-random numbers/Splitmix64"; implementer re-checks the vector against that page before committing). The PRNG is the standard algorithm, not a lookalike. |
| `range_i32_is_inclusive_and_in_bounds` | 10 000 draws of `range_i32(-5, 5)` all in `[-5, 5]`, both ends hit. |
| `scene_counts_follow_scale` | `generate(1)` has 200 000/50 000/5 000; `generate(4)` has 4x each. |
| `scene_is_deterministic` | `generate(1) == generate(1)` and `generate(1).rects[..1000] == generate(4).rects[..1000]` (prefix stability: a bigger scale extends, not reshuffles). |
| `scene_geometry_is_inside_world` | every coordinate in `[0, WORLD]`; every rect has `x0 < x1`, `y0 < y1`; every label text length in `4..=12` and alphabet-only. |
| `segments_are_70_percent_axis_aligned` | share of segments with `x0 == x1 || y0 == y1` is within `0.68..=0.72` at scale 1. |
| `label_px_lod_thresholds` | `label_px(0.001) == None` (3 px), `label_px(0.002) == Some(6)`, `label_px(1.0) == Some(64)` (clamped), rounding is nearest. |
| `camera_fit_shows_whole_world` | at frame 0 with `1600x1000`, `to_screen` of all four world corners lies inside the viewport, and the world's shorter axis spans exactly the viewport height. |
| `camera_path_is_total_and_piecewise_monotonic` | for all frames `0..=FRAMES`: finite fields; zoom equals `fit` on `0..240`; zoom strictly increasing on `240..420`; zoom equals `64*fit` on `420..600`; `camera_at(FRAMES+10) == camera_at(FRAMES)`. |
| `camera_path_is_deterministic_across_viewports_in_world_space` | centre path does not depend on viewport; only zoom does. |
| `stats_nearest_rank_percentiles` | `stats(&[1,2,...,100])` gives `p50 = 50`, `p95 = 95`, `p99 = 99`, `max = 100`, `mean = 50.5`; `stats(&[])` is all zeros, no panic. |
| `report_drops_warmup_and_emits_required_keys` | a timer fed 600 synthetic frames yields `frames == 540` samples; `to_json()` contains every key listed under Output; parsing with a 10-line hand-written checker finds balanced braces (no serde). |
| `args_parse_rejects_bad_input_without_panic` | `--scale 0`, `--scale x`, unknown flags return `Err`; defaults are `scale 1, frames 600, 1600x1000, bench false`. |

`spikes/canvas-wgpu`: no automated test (needs a GPU); correctness is
checked by eye in the Slint spike's windowed mode. State this in the
README.

Per spike: no automated tests beyond compiling in Nix. Each spike's
windowed mode is a manual check (`docs/manual-tests/M0.2.md`): launches,
shows the scene, mouse drag pans, wheel zooms about the cursor, `Esc`
quits; `--bench` auto-quits with JSON on stdout.

### Edge cases the implementer must handle

- `--scale 4` means 800k rects: instance buffers of ~16–32 MB; allocate
  once per run, not per frame, in `canvas-wgpu`. Toolkits that re-tessellate
  will simply be slow — that is the measurement, not a bug.
- HiDPI: request logical `1600x1000`; read back the device size and scale
  factor; the camera works in logical pixels for every toolkit.
- Wayland vs X11: `display_server()` is recorded; `bench.sh` refuses to
  run (exit 2 with a message) unless `$WAYLAND_DISPLAY` or `$DISPLAY` is
  set, and warns if gpui is run under X11 (vsync-capped).
- Labels that fall below the LOD threshold are skipped by all spikes
  through the same helper; labels partly off-screen are still submitted.
- Hairline segments longer than the viewport at 64x zoom: fine for GPU
  line/quad primitives; for egui/iced path strokes, submit as-is (the
  toolkit clips).
- The bench must exit 0 after frame `N` even if a toolkit keeps event
  loops alive (gpui: `cx.quit()`; eframe: `ctx.send_viewport_cmd(Close)`;
  iced: `window::close`/`exit`; Slint: `slint::quit_event_loop()`).
- If a toolkit panics on the dev machine (driver quirks), record it in
  `results/` as a failed run with the panic text instead of silently
  omitting the row.

### Ordered implementation checklist (one commit per step)

1. `Cargo.toml` (root): add `exclude = ["spikes"]`. Create
   `spikes/README.md`, `spikes/rust-toolchain.toml` (1.98.1, rustfmt,
   clippy), `spikes/results/.gitkeep`. Verify `nix flake check` still
   passes unchanged.
2. `spikes/scene`: write the 13 tests above first, then `rng.rs`,
   `scene.rs`, `camera.rs`, `metrics.rs`, `report.rs`, `lib.rs` until
   they pass with `cargo nextest run` inside `nix develop .#spikes`
   (devShell added in step 3; until then use `cargo +1.98.1` locally or
   do step 3 first — order 2/3 is interchangeable).
3. `flake.nix`: add `spikesToolchain`, `spikesCraneLib`, the inputs lists,
   `devShells.spikes`, and `checks.spike-scene-test`. `nix flake check`
   green.
4. `spikes/spike-egui`: eframe window, `Renderer::Wgpu`, `AutoNoVsync`,
   load `$SPIKE_FONT`, draw via `Painter::extend`, pan/zoom input,
   `--bench` path with `FrameTimer` + `Report`. Add
   `checks.spike-egui-build`. Record a first result locally to validate the
   JSON shape end to end before the heavier toolkits.
5. `spikes/spike-iced`: `iced::application`, `canvas` widget, `frames()`
   subscription, `--bench`. Add `checks.spike-iced-build`.
6. `spikes/spike-gpui`: `gpui_platform::application()`, `canvas` element
   with `paint_quad`/`paint_path`/`shape_line`, `request_animation_frame`,
   `add_fonts` from `$SPIKE_FONT`, `--bench`. Pin the git rev in the
   dependency list. Add `checks.spike-gpui-build` (expect a long first
   build).
7. `spikes/canvas-wgpu`: rects, lines, glyph atlas pipelines; `upload` /
   `render` as specified; compile-only.
8. `spikes/spike-slint`: `BackendSelector::require_wgpu_30`,
   `set_rendering_notifier` state machine from the upstream example,
   render `canvas-wgpu` into a texture sized to the `Image` element,
   `Image::try_from`, `request_redraw`, `--bench`. Add
   `checks.spike-slint-build`.
9. `spikes/bench.sh`: display check, `ICED_PRESENT_MODE=mailbox`, run all
   four at `--scale 1` and `--scale 4`, write `results/*.json` and
   `results/environment.txt`. Run it on the owner's machine; commit the
   results.
10. `docs/manual-tests/M0.2.md` from the template: windowed check per
    spike, `bench.sh` run, Wayland/X11 noted.
11. Hand back to the architect: fill the results table and the Decision
    section; flip Status to `accepted`; update `docs/PLAN.md` UI row and
    `docs/MILESTONES.md` M0.2 status.

Escalate (stop and report) if: a spike needs `unsafe` or a dependency not
in the table; gpui at the pinned rev lacks `canvas`/`paint_quad`/
`shape_line` (then propose bumping to a newer rev rather than hacking
around it); the Nix build of any spike fails on a missing system library
not in Zed's full list; Slint's `unstable-wgpu-30` API differs from the
`wgpu_texture` example in a way that needs design.

## Decision

**iso-eda's canvas is our own wgpu renderer (`canvas-wgpu`, to become the
draw-list sink in `eda-ui`), and the shell that hosts it is Slint 1.18.x
under its GPL-3.0-only license option, integrated through
`BackendSelector::require_wgpu_30` + `set_rendering_notifier` +
`slint::Image::try_from(wgpu::Texture)` — exactly the configuration the
Slint spike measured.** PLAN.md's working assumption (gpui + gpui-component,
Slint as fallback) is reversed: the fallback is the decision.

The one reason that mattered most: **on this workload the canvas is the
whole problem, and Slint + canvas-wgpu is the only configuration that was
measured to pass** — p95 5.33 ms at scale 1 and 30.1 ms at scale 4
(`spikes/results/slint-x1.json`, `slint-x4.json`, commit `74830ac`), while
every toolkit drawing with its own primitives was 5x to 84x over the bar or
failed to finish (`iced-x1.json` 85.6 ms, `egui-x1.json` 205.1 ms,
`gpui-x1.json` 1408.5 ms; `egui-x4.json` and `gpui-x4.json` are recorded
failures). Once the canvas is ours, the shell's only hard requirement is
that it can present an externally rendered GPU texture every frame without
adding cost. Slint does this zero-copy today and that path is what the
number above was measured through.

Why not the other shells, now that they are being judged as shells only:

- **gpui as shell** (the prior; Apache-2.0; owner's production experience in
  Rachana; `gpui-component` 0.9.1 — now published under the `gpui-kit`
  umbrella, Apache-2.0 — has dock layout, tabs, trees, tables). Rejected
  because, verified from the pinned source on 2026-11-15, gpui on Linux has
  **no zero-copy texture import and no custom-pass hook**: `paint_surface`
  is macOS-only, `paint_image` takes CPU pixel buffers, and the wgpu device
  is not reachable from the public API (and is a wgpu-29 fork, not the
  wgpu 30 `canvas-wgpu` uses). Hosting the canvas in gpui would mean a
  GPU -> CPU -> GPU round trip of the whole canvas every frame (6.4 MB at
  1600x1000, ~16 MB at 1.6x scale, 33 MB on a 4K display), a pipeline
  stall on the readback, and per-frame atlas churn, none of which was
  measured — or a patch to gpui's renderer that we carry on a git-pinned
  fork. The ADR's own rule is to decide on measurements; choosing gpui
  would be choosing an unmeasured integration path over a measured one on
  the strength of the prior. The owner's gpui experience and the richer
  shell widget set are real, and they are what this decision gives up.
- **egui as shell** (MIT OR Apache-2.0): `egui_wgpu::Callback` runs our pass
  inside egui's render pass on egui's own wgpu 30 device, so `canvas-wgpu`
  would work unchanged and zero-copy. Not measured in that configuration;
  shell widgets (`egui_dock`, `egui_extras`) are thinner than Slint's or
  gpui-kit's, immediate-mode means the whole UI is re-laid-out on every
  repaint, and the owner's stated dislike is on record. egui is nonetheless
  the **designated fallback shell** (see Consequences) because it is the
  only other candidate with a zero-copy path that needs no toolkit patch
  and no wgpu-major port.
- **iced as shell** (MIT): the `shader` widget gives a custom wgpu pass,
  but on wgpu 27, so `canvas-wgpu` would need a parallel wgpu-27 build;
  dock/tab widgets are the thinnest of the four; the Elm architecture makes
  a many-panel IDE verbose. Nothing disqualifying, nothing that beats
  Slint or egui on the criteria that remain.

What the decision costs, stated once here and expanded under Consequences:
distributed iso-eda binaries are GPL-3.0-**only** (Slint's free license
option), not "or later"; the Slint wgpu integration is behind an
`unstable-wgpu-30` feature flag; and the shell is written in the `.slint`
DSL, a second UI language next to Rust.

## Consequences

### Architecture

- The canvas is ours. `canvas-wgpu` (instanced rects, screen-space hairline
  quads, `ab_glyph` R8 atlas, three WGSL pipelines, wgpu 30) is the seed of
  the draw-list sink in `eda-ui`. `eda-render` stays toolkit-independent
  and emits a per-frame draw list; the measured `build_ms` of the canvas
  (2.9 ms p95 for 255k primitives at scale 1, 27 ms at scale 4) is the
  upload cost `eda-render`'s culling/LOD (M1.4) must fit under, and shows
  that at ~1M primitives culling is mandatory, not optional.
- The shell is Slint. `eda-ui` owns the `.slint` files, the
  `set_rendering_notifier` state machine, the texture sized to the `Image`
  element, and the input mapping from Slint events to `eda-cmd` commands.
  Only `eda-ui` and `eda-app` depend on `slint`/`slint-build`/`wgpu`
  (CLAUDE.md rule unchanged).
- The main workspace toolchain pin (`1.82.0`) rises to at least Slint's
  MSRV (`1.92`) when M0.3 wires the shell in; the spikes used `1.98.1` and
  that is the recommended pin. Note it in the M0.3 report; no separate ADR.
- M0.3 ("App shell: window, docks, command palette, keymap, theme tokens,
  empty canvas") now means: Slint window, a dock/split layout written in
  `.slint` (Slint's standard widgets do not ship a dock — see Not verified),
  and the canvas texture plumbing ported from `spike-slint` into `eda-ui`.
  The canvas frame time at the real full-size window is measured in M0.3's
  manual test as the follow-up to the viewport caveat above; if p95 at
  1600x1000, scale 1, exceeds 16.7 ms there, the fix is in `canvas-wgpu`
  (it is our code), not a toolkit change.

### Licensing

- Distributed iso-eda binaries are **GPL-3.0-only**, because Slint is
  linked under its GPL-3.0-only option. iso-eda's own source files stay
  `GPL-3.0-or-later`; the combined work's effective license is GPL-3.0
  exactly. `docs/THIRD-PARTY.md` and release notes must say so. A future
  move of the binary to a later GPL would require dropping Slint (see
  Reversal) or a Slint commercial license. This is the single largest
  cost of the decision and it is accepted deliberately: the alternative
  was an unmeasured canvas path.
- `slint`, `slint-build`, `wgpu`, `bytemuck`, `ab_glyph` move from the
  spikes dependency table into the main workspace at M0.3 under the usual
  CLAUDE.md sign-off; the licenses are already recorded above.

### Risks carried forward

- Slint's wgpu integration is behind `unstable-wgpu-30`. Minor Slint
  releases may change `BackendSelector`/`GraphicsAPI`/`Image::try_from`.
  Mitigation: pin Slint exactly in `Cargo.lock`, upgrade on purpose, and
  keep the texture-import plumbing in one module in `eda-ui`.
- Slint's present mode/vsync is not configurable from our code as far as
  we verified; in practice the spike ran at 227 fps, so it was not
  vsync-capped on this Wayland session.
- Two UI languages (`.slint` + Rust). Mitigated by keeping all behaviour in
  Rust (`eda-cmd` commands) and only layout/styling in `.slint`.

### What the spikes taught, kept here because `spikes/` is deleted

- gpui: immediate painting (`paint_quad`/`paint_path`) is serialized on the
  UI thread; 200k quads cost ~450 ms of CPU per frame at the pinned rev and
  800k trip the compositor watchdog. Any future idea of "just use gpui
  primitives for the canvas" is closed by measurement.
- egui: CPU re-tessellation produces one vertex buffer per frame; at 800k
  rects it is 308 MB and exceeds `max_buffer_size` on this device. egui's
  native painter has a hard ceiling well below layout scale.
- iced: completes at 800k rects but lyon tessellation alone (189 ms p95)
  exceeds any frame budget.
- Two real bugs found during the build, both fixed before the committed
  numbers: a WGSL `vec3<f32>` uniform alignment mismatch in `canvas-wgpu`
  (caught by a wgpu validation error — the 16-byte alignment rule for
  `vec3` in uniform structs), and a `build_ms` timer that wrapped the wrong
  scope in `spike-iced` and `spike-gpui`. Lesson for `eda-ui`: keep
  `Uniforms` `#[repr(C)]` with explicit padding and a `size_of` test.
- On a tiling compositor (Hyprland) a plain toplevel is tiled regardless of
  the requested size. `eda-app` should not rely on an initial window size
  for anything; read the actual size from the toolkit after mapping.

### Lifecycle of `spikes/`

With this ADR accepted, `spikes/` is to be deleted except
`spikes/results/*.json` and `spikes/results/environment.txt`, which stay as
the evidence this ADR cites. The deletion removes the four
`spike-*-build` checks and `spike-scene-test` from `nix flake check` and
with them the Zed clone, three wgpu majors and the Slint compiler from the
check graph. Before deleting, M0.3's implementer copies `spikes/canvas-wgpu`
and `spikes/spike-slint/src/main.rs` (texture plumbing) into `eda-ui`; the
`scene` crate's camera/metrics code is a useful seed for `eda-render`'s
benchmark harness. The deletion is a separate commit after the owner has
reviewed this decision.

### Reversal

Because the canvas goes through `eda-render`'s draw list into our own
wgpu renderer, the shell is replaceable without touching the data model,
commands, renderer, or `canvas-wgpu`. Swapping shells costs re-implementing
`eda-ui`'s panels, input mapping and theme tokens, plus the texture/pass
plumbing. The designated fallback shell is **egui** (`egui_wgpu::Callback`,
same wgpu major, zero-copy, permissive license), which would also lift the
GPL-3.0-only constraint. gpui becomes a candidate shell again only if it
grows a Linux texture-import or custom-pass hook (its macOS-only
`PaintSurface` primitive shows where such a hook would go) and that hook is
measured. The trigger for reversal is one of: Slint's `unstable-wgpu-*`
breaking in a way that is not fixed within one Slint minor release; the
GPL-3.0-only constraint becoming a concrete blocker for distribution; or
the full-size canvas measurement in M0.3 failing for a reason in the
shell's texture path rather than in `canvas-wgpu`.

## Not verified

Resolved since 2026-11-14 (kept for the record):

- gpui at rev `137e677a` does expose `canvas`, `paint_quad`, `paint_path`,
  `shape_line` (the spike compiled and ran against them). `WindowingRequest`
  does not exist at that rev; headless is `gpui_platform::headless()`.
- Rachana's system-library list sufficed for all four spikes under Nix
  (`nix flake check` green at `8bb87d3` with all four build checks).
- The SplitMix64 reference vector was confirmed by the implementer's test.
- Frame-time numbers exist: `spikes/results/*.json`, commit `74830ac`.

Still not verified (state of knowledge on 2026-11-15):

- Whether Slint's `WGPUSettings` or winit backend allow setting the present
  mode. The measured 227 fps shows the spike was not vsync-capped, but we
  did not find or set a control.
- The slint + canvas-wgpu p95 at the full 1600x1000 window. It was measured
  at 388x463 logical (Hyprland tiling, see "Reading the results"). The
  ranking does not depend on it; the margin to the 16.7 ms bar does, and
  M0.3 measures it.
- The cost of gpui's CPU-readback `paint_image` path for a full-window
  canvas. Not measured because the decision does not rest on it; it would
  only matter if gpui were reconsidered as the shell.
- Whether egui's `egui_wgpu::Callback` path hosts `canvas-wgpu` at the
  measured speed. Not measured; egui is the fallback, not the choice.
- Whether Slint's standard widget set or a maintained community component
  provides a dock/split layout; the M0.3 design assumes it must be written
  in `.slint`.
- Monitor refresh rate on the benchmark machine (`environment.txt`: not
  obtainable); irrelevant to the result because no viable run was near the
  refresh rate except slint at scale 4 (49.7 fps), and the uncapped
  227 fps at scale 1 shows no cap was in effect.
- `gpui-component`'s rename: docs.rs shows `gpui-component` 0.9.1
  (2026-11-09) with repository `longbridge/gpui-kit` and recommends
  depending on the `gpui-kit` umbrella crate; whether `gpui-component`
  will keep being published under its own name was not checked further.
