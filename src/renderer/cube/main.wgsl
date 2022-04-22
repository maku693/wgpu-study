struct Uniforms {
    mvp_matrix: mat4x4<f32>;
};

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct VertexOut {
    @builtin(position) position: vec4<f32>;
    @location(0) color: vec4<f32>;
};

@stage(vertex)
fn vs_main(
    @location(0) position: vec3<f32>,
) -> VertexOut {
    var out: VertexOut;
    out.position = uniforms.mvp_matrix * vec4<f32>(position, 1.0);
    out.color = vec4<f32>(position + vec3<f32>(0.5), 1.0);
    return out;
}

@stage(fragment)
fn fs_main(
  @location(0) color: vec4<f32>
) -> @location(0) vec4<f32> {
    return color;
}
