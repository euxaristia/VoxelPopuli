// Fullscreen blit of the offscreen render target (port of texture.vs / texture.fs).
// The v coordinate is flipped: GL render targets store row 0 at the bottom,
// wgpu stores row 0 at the top.
@group(1) @binding(0) var texture0: texture_2d<f32>;
@group(1) @binding(1) var sampler0: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.uv = vec2(in.uv.x, 1.0 - in.uv.y);
    out.pos = vec4(in.position, 1.0);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(texture0, sampler0, in.uv);
}
