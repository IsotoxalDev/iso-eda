// SPDX-License-Identifier: GPL-3.0-or-later

//! The `--bench` output: one JSON object per run, written by hand (no
//! `serde`) so `scene` keeps its zero-dependency policy.

use crate::camera::WARMUP_FRAMES;
use crate::metrics::{stats, FrameTimer, Stats};

pub struct ReportMeta {
    pub toolkit: String,
    pub toolkit_version: String,
    pub scale: u32,
    pub viewport_px: (u32, u32),
    pub scale_factor: f64,
    pub display_server: String,
    pub present_mode_requested: String,
    pub notes: String,
}

pub struct Report {
    pub toolkit: String,
    pub toolkit_version: String,
    pub scale: u32,
    pub frames: u32,
    pub viewport_px: (u32, u32),
    pub scale_factor: f64,
    pub display_server: String,
    pub present_mode_requested: String,
    pub frame_ms: Stats,
    pub build_ms: Stats,
    pub fps: f64,
    pub notes: String,
}

fn drop_warmup(samples: &[f64]) -> &[f64] {
    let drop = (WARMUP_FRAMES as usize).min(samples.len());
    &samples[drop..]
}

fn escape_json(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out
}

fn stats_to_json(s: &Stats) -> String {
    format!(
        "{{\"mean\":{},\"p50\":{},\"p95\":{},\"p99\":{},\"max\":{}}}",
        s.mean, s.p50, s.p95, s.p99, s.max
    )
}

impl Report {
    /// Builds from the timer, dropping the first `WARMUP_FRAMES` samples
    /// from both the `frame_ms` and `build_ms` series. `frames` is the
    /// number of `build_ms` samples that remain after the drop.
    pub fn from_timer(timer: &FrameTimer, meta: ReportMeta) -> Report {
        let frame_ms = drop_warmup(timer.frame_ms_samples());
        let build_ms = drop_warmup(timer.build_ms_samples());
        let frame_stats = stats(frame_ms);
        let build_stats = stats(build_ms);
        let fps = if frame_stats.mean > 0.0 {
            1000.0 / frame_stats.mean
        } else {
            0.0
        };

        Report {
            toolkit: meta.toolkit,
            toolkit_version: meta.toolkit_version,
            scale: meta.scale,
            frames: build_ms.len() as u32,
            viewport_px: meta.viewport_px,
            scale_factor: meta.scale_factor,
            display_server: meta.display_server,
            present_mode_requested: meta.present_mode_requested,
            frame_ms: frame_stats,
            build_ms: build_stats,
            fps,
            notes: meta.notes,
        }
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"toolkit\":\"{}\",\"toolkit_version\":\"{}\",\"scale\":{},\"frames\":{},\"viewport_px\":[{},{}],\"scale_factor\":{},\"display_server\":\"{}\",\"present_mode_requested\":\"{}\",\"frame_ms\":{},\"build_ms\":{},\"fps\":{},\"notes\":\"{}\"}}",
            escape_json(&self.toolkit),
            escape_json(&self.toolkit_version),
            self.scale,
            self.frames,
            self.viewport_px.0,
            self.viewport_px.1,
            self.scale_factor,
            escape_json(&self.display_server),
            escape_json(&self.present_mode_requested),
            stats_to_json(&self.frame_ms),
            stats_to_json(&self.build_ms),
            self.fps,
            escape_json(&self.notes),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_meta() -> ReportMeta {
        ReportMeta {
            toolkit: "test".to_string(),
            toolkit_version: "0.0.0".to_string(),
            scale: 1,
            viewport_px: (1600, 1000),
            scale_factor: 1.0,
            display_server: "wayland".to_string(),
            present_mode_requested: "mailbox".to_string(),
            notes: "synthetic".to_string(),
        }
    }

    #[test]
    fn report_drops_warmup_and_emits_required_keys() {
        let mut timer = FrameTimer::new();
        for _ in 0..600 {
            timer.begin_frame();
            timer.end_build();
        }
        assert_eq!(timer.frames_recorded(), 600);

        let report = Report::from_timer(&timer, test_meta());
        assert_eq!(report.frames, 540);

        let json = report.to_json();
        for key in [
            "toolkit",
            "toolkit_version",
            "scale",
            "frames",
            "viewport_px",
            "scale_factor",
            "display_server",
            "present_mode_requested",
            "frame_ms",
            "build_ms",
            "fps",
            "notes",
            "mean",
            "p50",
            "p95",
            "p99",
            "max",
        ] {
            assert!(
                json.contains(&format!("\"{key}\"")),
                "missing key {key} in {json}"
            );
        }

        // Hand-written balanced-brace check (no serde): every `{` must be
        // closed, and the nesting depth must never go negative or end
        // above zero.
        let mut depth = 0i32;
        for c in json.chars() {
            match c {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
            assert!(depth >= 0, "unbalanced braces in {json}");
        }
        assert_eq!(depth, 0, "unbalanced braces in {json}");
    }
}
