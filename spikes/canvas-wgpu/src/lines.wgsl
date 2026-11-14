// SPDX-License-Identifier: GPL-3.0-or-later

// Instanced hairline segments. Instance = world endpoints (a, b) + color.
// The camera transform happens here (world -> screen), then the segment
// is expanded into a 1-device-pixel-wide screen-space quad, so the line
// stays hairline-thin regardless of zoom.

struct Uniforms {
    viewport_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    // See rects.wgsl: three scalars, not `vec3<f32>`, to match the
    // 32-byte Rust-side Uniforms exactly (`vec3<f32>` has 16-byte
    // alignment in WGSL, which would grow this struct to 48 bytes).
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) world_a: vec2<f32>,
    @location(1) world_b: vec2<f32>,
    @location(2) color: vec4<f32>,
) -> VertexOutput {
    // x: 0 at endpoint a, 1 at endpoint b. y: -0.5..0.5 device pixels of
    // perpendicular half-width.
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, -0.5), vec2<f32>(1.0, -0.5), vec2<f32>(0.0, 0.5),
        vec2<f32>(0.0, 0.5), vec2<f32>(1.0, -0.5), vec2<f32>(1.0, 0.5),
    );
    let corner = corners[vertex_index];

    let screen_a = (world_a - u.camera_center) * u.zoom + u.viewport_size * 0.5;
    let screen_b = (world_b - u.camera_center) * u.zoom + u.viewport_size * 0.5;
    let dir = screen_b - screen_a;
    let len = max(length(dir), 0.0001);
    let unit = dir / len;
    let normal = vec2<f32>(-unit.y, unit.x);

    let along = screen_a + dir * corner.x;
    let screen_pos = along + normal * corner.y;

    let ndc = vec2<f32>(
        screen_pos.x / u.viewport_size.x * 2.0 - 1.0,
        1.0 - screen_pos.y / u.viewport_size.y * 2.0,
    );

    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}
