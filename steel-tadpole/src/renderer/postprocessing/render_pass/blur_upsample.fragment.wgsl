struct Uniforms {
  resolution: vec2<f32>,
}

@group(0) @binding(0)
var r_sampler: sampler;
@group(0) @binding(1)
var r_texture: texture_2d<f32>;
@group(0) @binding(2)
var<uniform> r_uniforms: Uniforms;

@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
  let half_texel = vec2<f32>(1.0) / vec2<f32>(r_uniforms.resolution);

  var color = textureSample(
    r_texture,
    r_sampler,
    tex_coord + vec2<f32>(-half_texel.x * 2.0, 0.0)
  );
  color += textureSample(
    r_texture,
    r_sampler,
    tex_coord + vec2<f32>(-half_texel.x, half_texel.y)
  ) * 2.0;
  color += textureSample(
    r_texture,
    r_sampler,
    tex_coord + vec2<f32>(0.0, half_texel.y * 2.0)
  );
  color += textureSample(
    r_texture,
    r_sampler,
    tex_coord + vec2<f32>(half_texel.x, half_texel.y)
  ) * 2.0;
  color += textureSample(
    r_texture,
    r_sampler,
    tex_coord + vec2<f32>(half_texel.x * 2.0, 0.0)
  );
  color += textureSample(
    r_texture,
    r_sampler,
    tex_coord + vec2<f32>(half_texel.x, -half_texel.y)
  ) * 2.0;
  color += textureSample(
    r_texture,
    r_sampler,
    tex_coord + vec2<f32>(0.0, -half_texel.y * 2.0)
  );
  color += textureSample(
    r_texture,
    r_sampler,
    tex_coord + vec2<f32>(-half_texel.x, -half_texel.y)
  ) * 2.0;

  return color / 12.0;
}

