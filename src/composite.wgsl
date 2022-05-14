struct Uniforms {
  exposure: f32,
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
@group(0) @binding(1)
var color_texture: texture_2d<f32>;
@group(0) @binding(2)
var color_sampler: sampler;

var<private> blur_sample_texel_offsets: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
  vec2<f32>(-0.5, -0.5),
  vec2<f32>(0.5, -0.5),
  vec2<f32>(-0.5, 0.5),
  vec2<f32>(0.5, 0.5),
);

@fragment
fn fs_main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
  let resolution = textureDimensions(color_texture);
  let texel_size = vec2<f32>(1.0) / vec2<f32>(resolution);

  var color = vec4<f32>(0.0);
  for (var i = 0; i < 4; i++) {
    color += textureSample(
      color_texture,
      color_sampler,
      tex_coord + blur_sample_texel_offsets[i] * texel_size
    );
  }
  color *= 0.25;

  return pow(color * uniforms.exposure, vec4<f32>(2.2));
}
