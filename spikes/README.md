<!-- SPDX-License-Identifier: GPL-3.0-or-later -->
# spikes/ — M0.2 UI toolkit throwaway benchmarks

**This directory is throwaway.** It exists only to answer one question for
`docs/adr/001-ui-toolkit.md`: which Rust UI toolkit (GPUI, egui, iced,
Slint-shell-plus-our-own-wgpu-canvas) can draw a heavy schematic/layout-scale
scene (200k rects, 50k segments, 5k labels) with pan/zoom at an interactive
frame rate. None of the code here is `eda-ui` or `eda-app`; nothing here
enters the `eda-*` build graph, the root Cargo workspace, or the root
`Cargo.lock`. Once ADR-001 is `accepted`, `spikes/` is deleted — the winner's
lessons move into `eda-ui`, and the committed `results/*.json` plus the ADR
text are the permanent record.

See `docs/adr/001-ui-toolkit.md` for the full methodology (shared scene,
camera path, metrics, rendering-fairness rules) and the licensing rationale.

## Layout

- `scene/` — toolkit-independent lib crate: deterministic scene generator
  (SplitMix64 PRNG), shared camera path, frame-time metrics, JSON report
  writer. No dependencies. Real unit tests (`cargo nextest run`).
- `canvas-wgpu/` — wgpu-30 renderer (instanced rects, hairline segments,
  glyph atlas) shared by the Slint spike (and usable by any other spike that
  wants it). No automated tests — needs a GPU; correctness is checked by eye
  in the windowed Slint spike.
- `spike-gpui/`, `spike-egui/`, `spike-iced/`, `spike-slint/` — one throwaway
  binary per toolkit. Each draws the same `scene::Scene` for `scene::FRAMES`
  frames along the same `scene::camera_at` path and supports `--bench` (see
  below).
- `bench.sh` — runs every spike at `--scale 1` and `--scale 4`, writes
  `results/<toolkit>-x<scale>.json` and `results/environment.txt`.
- `results/` — committed JSON bench output + environment dump. This is the
  evidence `docs/adr/001-ui-toolkit.md`'s Decision section cites.

Each spike/lib is its own Cargo package with its own `Cargo.lock` and an
empty `[workspace]` table, consuming `scene`/`canvas-wgpu` via `path = ...`.
This is deliberate: the four spikes pull three different wgpu majors (Zed's
fork of 29 via gpui, crates.io 30 via egui/Slint, crates.io 27 via iced), so
one shared lockfile would be fragile.

## Running

Toolchain: `channel = "1.98.1"`, pinned in `spikes/rust-toolchain.toml`
(independent of the main workspace's `1.82.0`). Enter the Nix shell with the
system libraries each spike needs:

```sh
nix develop .#spikes
```

Inside it, each spike binary supports:

- No flags: opens a window, draws the scene, pan with mouse-drag, zoom with
  the scroll wheel (about the cursor), `Esc` quits.
- `--bench [--scale N] [--frames N] [--width N] [--height N]`: runs the
  scripted camera path for `N` frames (default `scene::FRAMES` = 600),
  prints one JSON `Report` to stdout, exits 0. Defaults: `scale=1`,
  `frames=600`, `width=1600`, `height=1000`.

Run everything and record results:

```sh
cd spikes && ./bench.sh
```

`bench.sh` refuses to run (exit 2) unless `$WAYLAND_DISPLAY` or `$DISPLAY`
is set — the Nix sandbox has no GPU/display, so `nix flake check` only
proves each spike **compiles**; frame times only come from running the
binaries on a real machine. See the ADR's "Methodology" section for exactly
what each metric means, and "Not verified" / "Edge cases" for the caveats
(gpui cannot disable vsync off Wayland; Slint's canvas is actually
`canvas-wgpu`, not Slint primitives; a toolkit panic is recorded as a failed
run in `results/`, not silently dropped).

## What is NOT tested here

`canvas-wgpu` has no automated test — it needs a real GPU device, which the
Nix sandbox does not provide. Its correctness is a manual, by-eye check via
the Slint spike's windowed mode (`docs/manual-tests/M0.2.md`).

## Status

ADR-001 is `proposed`. Do not delete this directory until the ADR is
`accepted` and the Decision section is filled in.
