@group(0) @binding(0)
var r_texture1: texture_2d<f32>;
@group(0) @binding(1)
var r_texture2: texture_2d<f32>;
@group(0) @binding(2)
var r_sampler: sampler;

@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
  return textureSample(r_texture1, r_sampler, tex_coord) + textureSample(r_texture2, r_sampler, tex_coord);
}
