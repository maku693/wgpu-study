var<private> blur_sample_texel_offsets: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
  vec2<f32>(-0.5, -0.5),
  vec2<f32>(0.5, -0.5),
  vec2<f32>(-0.5, 0.5),
  vec2<f32>(0.5, 0.5),
);

@group(0) @binding(0)
var r_texture: texture_2d<f32>;
@group(0) @binding(1)
var r_sampler: sampler;

@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
  let resolution = textureDimensions(r_texture);
  let texel_size = vec2<f32>(1.0) / vec2<f32>(resolution);

  var color = vec4<f32>(0.0);
  for (var i = 0; i < 4; i++) {
    color += textureSample(
      r_texture,
      r_sampler,
      tex_coord + blur_sample_texel_offsets[i] * texel_size
    );
  }
  color *= 0.25;

  var color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
  return color;
}
