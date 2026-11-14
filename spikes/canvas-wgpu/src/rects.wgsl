// SPDX-License-Identifier: GPL-3.0-or-later

// Instanced unit-quad for filled, axis-aligned rects. Instance = world
// rect (min/max) + color; the camera transform (world -> screen -> NDC)
// happens here, in the vertex shader, not on the CPU.

struct Uniforms {
    viewport_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    _pad: vec3<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) world_min: vec2<f32>,
    @location(1) world_max: vec2<f32>,
    @location(2) color: vec4<f32>,
) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vertex_index];
    let world_pos = mix(world_min, world_max, corner);
    let screen_pos = (world_pos - u.camera_center) * u.zoom + u.viewport_size * 0.5;
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
