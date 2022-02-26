@stage(vertex)
fn vs_main(
    @location(0) model_position: vec3<f32>,
    @location(1) instance_position: vec3<f32>
) -> @builtin(position) vec4<f32> {
    return vec4<f32>(model_position + instance_position, 1.0);
}

@stage(fragment)
fn fs_main() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 1.0);
}
