struct Uniforms {
  exposure: f32,
}

@group(0) @binding(0)
var r_sampler: sampler;

@group(1) @binding(0)
var<uniform> r_uniforms: Uniforms;
@group(1) @binding(1)
var r_color_target_texture: texture_2d<f32>;
@group(1) @binding(2)
var r_bloom_texture: texture_2d<f32>;

@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
  var dimension = textureDimensions(r_color_target_texture);
  var color = textureLoad(r_color_target_texture, vec2<i32>(vec2<f32>(dimension) * tex_coord), 0);
  color += textureSample(r_bloom_texture, r_sampler, tex_coord); // Add bloom
  return pow(color * r_uniforms.exposure, vec4<f32>(2.2)); // Tonemap
}
