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
fn main(@builtin(vertex_index) vertex_index: u32) -> VertexOut {
  let position = vec4<f32>(positions[vertex_index], 0.0, 1.0);
  let tex_coord = tex_coords[vertex_index];
  return VertexOut(
    position,
    tex_coord,
  );
}
