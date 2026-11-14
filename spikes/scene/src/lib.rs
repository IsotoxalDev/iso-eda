// SPDX-License-Identifier: GPL-3.0-or-later

//! `scene`: the toolkit-independent crate shared by every M0.2 UI spike.
//! Deterministic scene generator, shared camera path, frame-time metrics
//! and a hand-rolled JSON report writer. No dependencies (see
//! docs/adr/001-ui-toolkit.md): the four spikes pull different wgpu
//! majors and must not be able to drift on a shared RNG crate version.

pub mod camera;
pub mod metrics;
pub mod report;
pub mod rng;
pub mod scene;

pub use camera::{camera_at, Camera, Viewport, FRAMES, WARMUP_FRAMES};
pub use metrics::{stats, FrameTimer, Stats};
pub use report::{Report, ReportMeta};
pub use rng::SplitMix64;
pub use scene::{
    label_px, Label, Rect, Rgba8, Scene, Segment, LABEL_MAX_PX, LABEL_MIN_PX, LABEL_WORLD_HEIGHT,
    PALETTE, SEED, WORLD,
};

/// Hand-rolled CLI args, parsed over `std::env::args()`. No `clap`: the
/// spikes are throwaway and the flag set is tiny (`--bench`, `--scale N`,
/// `--frames N`, `--width N`, `--height N`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Args {
    pub bench: bool,
    pub scale: u32,
    pub frames: u32,
    pub width: u32,
    pub height: u32,
}

impl Args {
    /// Never panics on bad input: unknown flags, a missing value, or a
    /// value that fails to parse as `u32` all return `Err` with a message.
    pub fn parse(args: impl Iterator<Item = String>) -> Result<Args, String> {
        let mut out = Args {
            bench: false,
            scale: 1,
            frames: FRAMES,
            width: 1600,
            height: 1000,
        };

        let mut it = args;
        while let Some(arg) = it.next() {
            match arg.as_str() {
                "--bench" => out.bench = true,
                "--scale" => {
                    let v = parse_u32_flag(&mut it, "--scale")?;
                    if v == 0 {
                        return Err("--scale must be >= 1".to_string());
                    }
                    out.scale = v;
                }
                "--frames" => out.frames = parse_u32_flag(&mut it, "--frames")?,
                "--width" => out.width = parse_u32_flag(&mut it, "--width")?,
                "--height" => out.height = parse_u32_flag(&mut it, "--height")?,
                other => return Err(format!("unknown flag: {other}")),
            }
        }

        Ok(out)
    }
}

fn parse_u32_flag(it: &mut impl Iterator<Item = String>, flag: &str) -> Result<u32, String> {
    let value = it
        .next()
        .ok_or_else(|| format!("{flag} requires a value"))?;
    value
        .parse::<u32>()
        .map_err(|_| format!("invalid {flag} value: {value}"))
}

/// `"wayland"` if `WAYLAND_DISPLAY` is set, else `"x11"` if `DISPLAY` is
/// set, else `"none"`.
pub fn display_server() -> String {
    if std::env::var("WAYLAND_DISPLAY").is_ok() {
        "wayland".to_string()
    } else if std::env::var("DISPLAY").is_ok() {
        "x11".to_string()
    } else {
        "none".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_parse_rejects_bad_input_without_panic() {
        let bad_scale = Args::parse(vec!["--scale".to_string(), "0".to_string()].into_iter());
        assert!(bad_scale.is_err());

        let bad_scale_nonnumeric =
            Args::parse(vec!["--scale".to_string(), "x".to_string()].into_iter());
        assert!(bad_scale_nonnumeric.is_err());

        let unknown_flag = Args::parse(vec!["--wat".to_string()].into_iter());
        assert!(unknown_flag.is_err());

        let defaults = Args::parse(std::iter::empty()).expect("defaults must parse");
        assert_eq!(
            defaults,
            Args {
                bench: false,
                scale: 1,
                frames: 600,
                width: 1600,
                height: 1000
            }
        );
    }
}
