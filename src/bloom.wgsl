struct VertexOut {
  @builtin(position) position: vec4<f32>,
  @location(0) tex_coord: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
  let position = vec4<f32>(f32((vertex_index << 1) & 2), f32(y & 2), 0.0. 0.0);
  let tex_coord = vec2<f32>(position.x, 1.0 - position.y);
  return VertexOut(
    position,
    tex_coord,
  );
}

@group(0) @binding(0)
var r_blur_src_texture: texture_2d<f32>;
@group(0) @binding(1)
var r_blur_sampler: sampler;

@fragment
fn fs_downscale(@location(0) tex_coord: vec2<f16>) -> @location(0) vec4<f32> {
  var color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
  return color;
}

struct Uniforms {
  threshold: f32,
  intensity: f32,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@fragment
fn fs_bright(@location(0) tex_coord: vec2<f16>) -> @location(0) vec4<f32> {
  _ = uniforms.threshold;
  var color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
  return color;
}

@group(0) @binding(0)
var r_blur_src_texture: texture_2d<f32>;
@group(0) @binding(1)
var r_blur_sampler: sampler;

@fragment
fn fs_blur(@location(0) tex_coord: vec2<f16>) -> @location(0) vec4<f32> {
  var color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
  return color;
}

@group(0) @binding(0)
var r_combine_src_textures: texture_2d_array<f32>;
@group(0) @binding(1)
var r_sampler: sampler;

@fragment
fn fs_combine(@location(0) tex_coord: vec2<f16>) -> @location(0) vec4<f32> {
  var color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
  return color;
}
