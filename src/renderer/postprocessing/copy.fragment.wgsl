@group(0) @binding(0)
var r_sampler: sampler;
@group(0) @binding(1)
var r_texture: texture_2d<f32>;

@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
  return textureSample(r_texture, r_sampler, tex_coord);
}
