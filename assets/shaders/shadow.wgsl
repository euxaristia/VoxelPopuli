struct Uniforms {
    mvp: mat4x4<f32>,
    model: mat4x4<f32>,
}
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var texture0: texture_2d<f32>;
@group(1) @binding(1) var sampler0: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) color: vec4<f32>,
}
struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.uv = in.uv;
    out.pos = u.mvp * (u.model * vec4(in.position, 1.0));
    return out;
}

@fragment
fn fs_main(in: VsOut) {
    if (textureSample(texture0, sampler0, in.uv).a < 0.1) {
        discard;
    }
}
