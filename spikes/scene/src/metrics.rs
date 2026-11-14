// SPDX-License-Identifier: GPL-3.0-or-later

//! Per-frame timing: wall-clock frame-to-frame interval (`frame_ms`, what
//! the user feels) and CPU paint-callback cost (`build_ms`).

use std::time::Instant;

pub struct FrameTimer {
    last_frame_start: Option<Instant>,
    current_frame_start: Option<Instant>,
    frame_ms: Vec<f64>,
    build_ms: Vec<f64>,
}

impl FrameTimer {
    pub fn new() -> Self {
        Self {
            last_frame_start: None,
            current_frame_start: None,
            frame_ms: Vec::new(),
            build_ms: Vec::new(),
        }
    }

    /// Call once at the start of each frame callback.
    pub fn begin_frame(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_frame_start {
            self.frame_ms
                .push(now.duration_since(last).as_secs_f64() * 1000.0);
        }
        self.last_frame_start = Some(now);
        self.current_frame_start = Some(now);
    }

    /// Call once at the end of the paint callback.
    pub fn end_build(&mut self) {
        let now = Instant::now();
        if let Some(start) = self.current_frame_start {
            self.build_ms
                .push(now.duration_since(start).as_secs_f64() * 1000.0);
        }
    }

    /// Number of fully-timed frames (`begin_frame` + `end_build` pairs).
    pub fn frames_recorded(&self) -> u32 {
        self.build_ms.len() as u32
    }

    pub(crate) fn frame_ms_samples(&self) -> &[f64] {
        &self.frame_ms
    }

    pub(crate) fn build_ms_samples(&self) -> &[f64] {
        &self.build_ms
    }
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stats {
    pub mean: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub max: f64,
}

/// Nearest-rank percentiles over the slice; empty slice gives all zeros
/// (never panics).
pub fn stats(samples_ms: &[f64]) -> Stats {
    if samples_ms.is_empty() {
        return Stats {
            mean: 0.0,
            p50: 0.0,
            p95: 0.0,
            p99: 0.0,
            max: 0.0,
        };
    }

    let mut sorted = samples_ms.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let n = sorted.len();

    let percentile = |p: f64| -> f64 {
        let rank = ((p / 100.0) * n as f64).ceil() as usize;
        let idx = rank.clamp(1, n) - 1;
        sorted[idx]
    };

    let mean = sorted.iter().sum::<f64>() / n as f64;

    Stats {
        mean,
        p50: percentile(50.0),
        p95: percentile(95.0),
        p99: percentile(99.0),
        max: sorted[n - 1],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stats_nearest_rank_percentiles() {
        let samples: Vec<f64> = (1..=100).map(f64::from).collect();
        let s = stats(&samples);
        assert_eq!(s.p50, 50.0);
        assert_eq!(s.p95, 95.0);
        assert_eq!(s.p99, 99.0);
        assert_eq!(s.max, 100.0);
        assert_eq!(s.mean, 50.5);

        let empty = stats(&[]);
        assert_eq!(
            empty,
            Stats {
                mean: 0.0,
                p50: 0.0,
                p95: 0.0,
                p99: 0.0,
                max: 0.0
            }
        );
    }
}
