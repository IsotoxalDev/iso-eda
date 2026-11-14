// SPDX-License-Identifier: GPL-3.0-or-later

//! The shared camera path: a pure function of the frame index, identical
//! for all four spikes, so pan/zoom load is comparable across toolkits.

pub const FRAMES: u32 = 600;
pub const WARMUP_FRAMES: u32 = 60;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Viewport {
    pub width_px: u32,
    pub height_px: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub center_x: f64,
    pub center_y: f64,
    pub zoom: f64,
}

impl Camera {
    /// `zoom = min(w, h) / WORLD`, centred on the world.
    pub fn fit(viewport: Viewport) -> Camera {
        let zoom =
            f64::from(viewport.width_px.min(viewport.height_px)) / f64::from(crate::scene::WORLD);
        Camera {
            center_x: 500_000.0,
            center_y: 500_000.0,
            zoom,
        }
    }

    /// Maps a world point to a logical-pixel screen coordinate, world
    /// origin to the top-left, camera centre to the viewport centre.
    pub fn to_screen(&self, viewport: Viewport, x: i32, y: i32) -> (f32, f32) {
        let sx = f64::from(viewport.width_px) / 2.0 + (f64::from(x) - self.center_x) * self.zoom;
        let sy = f64::from(viewport.height_px) / 2.0 + (f64::from(y) - self.center_y) * self.zoom;
        (sx as f32, sy as f32)
    }
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Total over `0..=FRAMES`; frames `>= FRAMES` clamp to the last pose.
pub fn camera_at(frame: u32, viewport: Viewport) -> Camera {
    let fit = Camera::fit(viewport).zoom;
    let f = frame.min(FRAMES);

    if f < WARMUP_FRAMES {
        // Warm-up: fit-to-viewport zoom, centre on the world.
        Camera {
            center_x: 500_000.0,
            center_y: 500_000.0,
            zoom: fit,
        }
    } else if f < 240 {
        // Pan at fit zoom.
        let t = f64::from(f - WARMUP_FRAMES) / f64::from(240 - WARMUP_FRAMES);
        Camera {
            center_x: lerp(250_000.0, 750_000.0, t),
            center_y: 500_000.0,
            zoom: fit,
        }
    } else if f < 420 {
        // Zoom in at a fixed centre, exponentially from fit to 64*fit.
        let t = f64::from(f - 240) / f64::from(420 - 240);
        Camera {
            center_x: 600_000.0,
            center_y: 400_000.0,
            zoom: fit * 64f64.powf(t),
        }
    } else {
        // Pan at 64x zoom. f is clamped to FRAMES above, so t saturates
        // at exactly 1.0 for every frame >= FRAMES (the "last pose").
        let t = f64::from(f - 420) / f64::from(600 - 420);
        Camera {
            center_x: lerp(600_000.0, 640_000.0, t),
            center_y: lerp(400_000.0, 430_000.0, t),
            zoom: fit * 64.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::WORLD;

    #[test]
    fn camera_fit_shows_whole_world() {
        let viewport = Viewport {
            width_px: 1600,
            height_px: 1000,
        };
        let camera = camera_at(0, viewport);
        let corners = [(0, 0), (0, WORLD), (WORLD, 0), (WORLD, WORLD)];
        let mut min_y = f32::MAX;
        let mut max_y = f32::MIN;
        for (x, y) in corners {
            let (sx, sy) = camera.to_screen(viewport, x, y);
            assert!((0.0..=f32::from(1600u16)).contains(&sx), "sx={sx}");
            assert!((0.0..=f32::from(1000u16)).contains(&sy), "sy={sy}");
            min_y = min_y.min(sy);
            max_y = max_y.max(sy);
        }
        assert!((max_y - min_y - 1000.0).abs() < 0.01);
    }

    #[test]
    fn camera_path_is_total_and_piecewise_monotonic() {
        let viewport = Viewport {
            width_px: 1600,
            height_px: 1000,
        };
        let fit = Camera::fit(viewport).zoom;
        let mut prev_zoom_in_zoom_phase: Option<f64> = None;
        for frame in 0..=FRAMES {
            let cam = camera_at(frame, viewport);
            assert!(cam.center_x.is_finite());
            assert!(cam.center_y.is_finite());
            assert!(cam.zoom.is_finite());

            if frame < 240 {
                assert!((cam.zoom - fit).abs() < 1e-9, "frame {frame}");
            } else if frame < 420 {
                if let Some(prev) = prev_zoom_in_zoom_phase {
                    assert!(cam.zoom > prev, "zoom not increasing at frame {frame}");
                }
                prev_zoom_in_zoom_phase = Some(cam.zoom);
            } else {
                assert!((cam.zoom - 64.0 * fit).abs() < 1e-6, "frame {frame}");
            }
        }

        assert_eq!(
            camera_at(FRAMES + 10, viewport),
            camera_at(FRAMES, viewport)
        );
    }

    #[test]
    fn camera_path_is_deterministic_across_viewports_in_world_space() {
        let v1 = Viewport {
            width_px: 1600,
            height_px: 1000,
        };
        let v2 = Viewport {
            width_px: 3200,
            height_px: 2000,
        };
        for frame in [0, 60, 150, 240, 300, 419, 420, 500, 599] {
            let c1 = camera_at(frame, v1);
            let c2 = camera_at(frame, v2);
            assert_eq!(c1.center_x, c2.center_x, "frame {frame}");
            assert_eq!(c1.center_y, c2.center_y, "frame {frame}");
        }
    }
}
