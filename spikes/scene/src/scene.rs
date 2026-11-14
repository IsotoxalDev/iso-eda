// SPDX-License-Identifier: GPL-3.0-or-later

//! The shared scene: the same `Scene::generate(scale)` output is drawn by
//! every UI spike, so no toolkit gets an easier or harder workload.

use crate::rng::SplitMix64;

pub const WORLD: i32 = 1_000_000;
pub const SEED: u64 = 0x0150_EDA0_0000_0001;
pub const LABEL_WORLD_HEIGHT: i32 = 3_000;
pub const LABEL_MIN_PX: u16 = 4;
pub const LABEL_MAX_PX: u16 = 64;

/// 8 fixed opaque rect colours plus a 9th entry reserved for segments.
pub const PALETTE: [Rgba8; 9] = [
    Rgba8 {
        r: 0xE6,
        g: 0x19,
        b: 0x4B,
        a: 0xFF,
    },
    Rgba8 {
        r: 0x3C,
        g: 0xB4,
        b: 0x4B,
        a: 0xFF,
    },
    Rgba8 {
        r: 0xFF,
        g: 0xE1,
        b: 0x19,
        a: 0xFF,
    },
    Rgba8 {
        r: 0x43,
        g: 0x63,
        b: 0xD8,
        a: 0xFF,
    },
    Rgba8 {
        r: 0xF5,
        g: 0x82,
        b: 0x31,
        a: 0xFF,
    },
    Rgba8 {
        r: 0x91,
        g: 0x1E,
        b: 0xB4,
        a: 0xFF,
    },
    Rgba8 {
        r: 0x46,
        g: 0xF0,
        b: 0xF0,
        a: 0xFF,
    },
    Rgba8 {
        r: 0xF0,
        g: 0x32,
        b: 0xE6,
        a: 0xFF,
    },
    Rgba8 {
        r: 0xBF,
        g: 0xBF,
        b: 0xBF,
        a: 0xFF,
    },
];

const LABEL_ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgba8 {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rect {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
    pub color: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Segment {
    pub x0: i32,
    pub y0: i32,
    pub x1: i32,
    pub y1: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub x: i32,
    pub y: i32,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Scene {
    pub rects: Vec<Rect>,
    pub segments: Vec<Segment>,
    pub labels: Vec<Label>,
}

fn gen_rect(rng: &mut SplitMix64, index: usize) -> Rect {
    let width = rng.range_i32(200, 20_000);
    let height = rng.range_i32(200, 20_000);
    let x0 = rng.range_i32(0, WORLD - width);
    let y0 = rng.range_i32(0, WORLD - height);
    Rect {
        x0,
        y0,
        x1: x0 + width,
        y1: y0 + height,
        color: (index % 8) as u8,
    }
}

fn gen_segment(rng: &mut SplitMix64) -> Segment {
    let axis_aligned = rng.chance(7, 10);
    if axis_aligned {
        let horizontal = rng.chance(1, 2);
        let length = rng.range_i32(500, 50_000);
        if horizontal {
            let y = rng.range_i32(0, WORLD);
            let x0 = rng.range_i32(0, WORLD - length);
            Segment {
                x0,
                y0: y,
                x1: x0 + length,
                y1: y,
            }
        } else {
            let x = rng.range_i32(0, WORLD);
            let y0 = rng.range_i32(0, WORLD - length);
            Segment {
                x0: x,
                y0,
                x1: x,
                y1: y0 + length,
            }
        }
    } else {
        let length = rng.range_i32(500, 50_000);
        // 1..179 degrees: arbitrary angle, excludes the exact 0/180
        // endpoints (which would be axis-aligned anyway).
        let angle_deg = rng.range_i32(1, 179);
        let x0 = rng.range_i32(0, WORLD);
        let y0 = rng.range_i32(0, WORLD);
        let rad = f64::from(angle_deg).to_radians();
        let dx = (f64::from(length) * rad.cos()).round() as i32;
        let dy = (f64::from(length) * rad.sin()).round() as i32;
        let x1 = (x0 + dx).clamp(0, WORLD);
        let y1 = (y0 + dy).clamp(0, WORLD);
        Segment { x0, y0, x1, y1 }
    }
}

fn gen_label(rng: &mut SplitMix64) -> Label {
    let x = rng.range_i32(0, WORLD);
    let y = rng.range_i32(0, WORLD);
    let len = rng.range_i32(4, 12) as usize;
    let mut text = String::with_capacity(len);
    for _ in 0..len {
        let idx = rng.range_i32(0, (LABEL_ALPHABET.len() - 1) as i32) as usize;
        text.push(LABEL_ALPHABET[idx] as char);
    }
    Label { x, y, text }
}

impl Scene {
    /// `scale >= 1`. Counts are `200_000 * scale`, `50_000 * scale`,
    /// `5_000 * scale`. Pure function of `scale` (and the fixed `SEED`):
    /// same bytes every call, on every machine.
    pub fn generate(scale: u32) -> Scene {
        let scale = scale.max(1);
        let mut rng = SplitMix64::new(SEED);

        let rect_count = 200_000usize * scale as usize;
        let seg_count = 50_000usize * scale as usize;
        let label_count = 5_000usize * scale as usize;

        let mut rects = Vec::with_capacity(rect_count);
        for i in 0..rect_count {
            rects.push(gen_rect(&mut rng, i));
        }

        let mut segments = Vec::with_capacity(seg_count);
        for _ in 0..seg_count {
            segments.push(gen_segment(&mut rng));
        }

        let mut labels = Vec::with_capacity(label_count);
        for _ in 0..label_count {
            labels.push(gen_label(&mut rng));
        }

        Scene {
            rects,
            segments,
            labels,
        }
    }
}

/// `None` when the label would be `< LABEL_MIN_PX`; else
/// `round(3_000 * zoom)` clamped to `LABEL_MAX_PX`.
pub fn label_px(zoom: f64) -> Option<u16> {
    let px = (f64::from(LABEL_WORLD_HEIGHT) * zoom).round();
    if px < f64::from(LABEL_MIN_PX) {
        None
    } else {
        Some(px.min(f64::from(LABEL_MAX_PX)) as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_counts_follow_scale() {
        let s1 = Scene::generate(1);
        assert_eq!(s1.rects.len(), 200_000);
        assert_eq!(s1.segments.len(), 50_000);
        assert_eq!(s1.labels.len(), 5_000);

        let s4 = Scene::generate(4);
        assert_eq!(s4.rects.len(), 800_000);
        assert_eq!(s4.segments.len(), 200_000);
        assert_eq!(s4.labels.len(), 20_000);
    }

    #[test]
    fn scene_is_deterministic() {
        let a = Scene::generate(1);
        let b = Scene::generate(1);
        assert_eq!(a, b);

        let c = Scene::generate(4);
        assert_eq!(a.rects[..1000], c.rects[..1000]);
    }

    #[test]
    fn scene_geometry_is_inside_world() {
        let s = Scene::generate(1);
        for r in &s.rects {
            assert!((0..=WORLD).contains(&r.x0));
            assert!((0..=WORLD).contains(&r.x1));
            assert!((0..=WORLD).contains(&r.y0));
            assert!((0..=WORLD).contains(&r.y1));
            assert!(r.x0 < r.x1);
            assert!(r.y0 < r.y1);
        }
        for seg in &s.segments {
            assert!((0..=WORLD).contains(&seg.x0));
            assert!((0..=WORLD).contains(&seg.x1));
            assert!((0..=WORLD).contains(&seg.y0));
            assert!((0..=WORLD).contains(&seg.y1));
        }
        for l in &s.labels {
            assert!((0..=WORLD).contains(&l.x));
            assert!((0..=WORLD).contains(&l.y));
            assert!((4..=12).contains(&l.text.len()));
            assert!(l
                .text
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_'));
        }
    }

    #[test]
    fn segments_are_70_percent_axis_aligned() {
        let s = Scene::generate(1);
        let aligned = s
            .segments
            .iter()
            .filter(|seg| seg.x0 == seg.x1 || seg.y0 == seg.y1)
            .count();
        let share = aligned as f64 / s.segments.len() as f64;
        assert!(
            (0.68..=0.72).contains(&share),
            "axis-aligned share {share} out of [0.68, 0.72]"
        );
    }

    #[test]
    fn label_px_lod_thresholds() {
        assert_eq!(label_px(0.001), None);
        assert_eq!(label_px(0.002), Some(6));
        assert_eq!(label_px(1.0), Some(64));
    }
}
