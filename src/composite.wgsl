struct Uniforms {
  exposure: f32,
}

struct VertexOut {
  @builtin(position) position: vec4<f32>,
  @location(0) tex_coord: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;
@group(0) @binding(1)
var offscreen_color: texture_2d<f32>;

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

@fragment
fn fs_main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
  var a_ = uniforms;
  var b_ = textureDimensions(offscreen_color);
  return vec4<f32>(tex_coord, 1.0, 1.0);
}
