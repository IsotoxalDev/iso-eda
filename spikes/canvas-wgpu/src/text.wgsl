// SPDX-License-Identifier: GPL-3.0-or-later

// Instanced textured quads for glyphs, sampled from an ab_glyph R8 atlas
// keyed by (char, px_size). Instance positions are already in screen
// pixels (the CPU side resolves world -> screen and the LOD px size
// before building the instance, since label visibility itself is a
// per-frame, zoom-dependent decision); only the viewport -> NDC mapping
// happens here.

struct Uniforms {
    viewport_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    _pad: vec3<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var atlas_tex: texture_2d<f32>;
@group(1) @binding(1) var atlas_sampler: sampler;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @location(0) screen_min: vec2<f32>,
    @location(1) screen_max: vec2<f32>,
    @location(2) uv_min: vec2<f32>,
    @location(3) uv_max: vec2<f32>,
    @location(4) color: vec4<f32>,
) -> VertexOutput {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 1.0),
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0),
    );
    let corner = corners[vertex_index];
    let screen_pos = mix(screen_min, screen_max, corner);
    let uv = mix(uv_min, uv_max, corner);

    let ndc = vec2<f32>(
        screen_pos.x / u.viewport_size.x * 2.0 - 1.0,
        1.0 - screen_pos.y / u.viewport_size.y * 2.0,
    );

    var out: VertexOutput;
    out.clip_position = vec4<f32>(ndc, 0.0, 1.0);
    out.uv = uv;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let coverage = textureSample(atlas_tex, atlas_sampler, in.uv).r;
    return vec4<f32>(in.color.rgb, in.color.a * coverage);
}
