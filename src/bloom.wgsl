struct Uniforms {
  threshold: f32,
  intensity: f32,
}

struct VertexOut {
  @builtin(position) position: vec4<f32>,
  @location(0) tex_coord: vec2<f32>,
}

var<private> positions: array<vec2<f32>, 3> = array<vec2<f32>, 3>(
  vec2<f32>(-1.0, -1.0),
  vec2<f32>(3.0, -1.0),
  vec2<f32>(-1.0, 3.0),
);

var<private> tex_coords: array<vec2<f32>, 3> = array<vec2<f32>, 3>(
  vec2<f32>(0.0, 1.0),
  vec2<f32>(2.0, 1.0),
  vec2<f32>(0.0, -1.0),
);

@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
  let position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
  let tex_coord = tex_coords[vertex_index];
  return VertexOut(
    position,
    tex_coord,
  );
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
