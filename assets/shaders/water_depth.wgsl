struct Uniforms {
    mvp: mat4x4<f32>,
    model: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) color: vec4<f32>,
}

@vertex
fn vs_main(in: VsIn) -> @builtin(position) vec4<f32> {
    return u.mvp * (u.model * vec4(in.position, 1.0));
}

@fragment
fn fs_main() {}
