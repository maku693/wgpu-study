@group(0) @binding(0)
var texture: texture_2d_array<f32>;
@group(0) @binding(1)
var sampler: sampler;

@fragment
fn main(@location(0) tex_coord: vec2<f32>) -> @location(0) vec4<f32> {
  var color = vec4<f32>(1.0, 1.0, 1.0, 1.0);
  return color;
}
