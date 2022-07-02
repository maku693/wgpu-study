@group(0) @binding(0)
var r_sampler: sampler;
@group(0) @binding(1)
var r_texture: texture_2d<f32>;

@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
  let resolution = textureDimensions(r_texture);
  let texel_size = vec2<f32>(1.0) / vec2<f32>(resolution);

  var color = textureSample(r_texture, r_sampler, tex_coord) * 4.0;
  color += textureSample(
    r_texture,
    r_sampler,
    tex_coord + texel_size * vec2<f32>(-0.5, -0.5)
  );
  color += textureSample(
    r_texture,
    r_sampler,
    tex_coord + texel_size * vec2<f32>(0.5, -0.5)
  );
  color += textureSample(
    r_texture,
    r_sampler,
    tex_coord + texel_size * vec2<f32>(-0.5, 0.5)
  );
  color += textureSample(
    r_texture,
    r_sampler,
    tex_coord + texel_size * vec2<f32>(0.5, 0.5)
  );

  return color * 0.125;
}
