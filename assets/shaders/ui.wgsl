// Pixel-coordinate colored UI rects (port of ui.vs / ui.fs).
struct Uniforms {
    mvp: mat4x4<f32>,
    model: mat4x4<f32>,
    col_diffuse: vec4<f32>,
    sky_col: vec4<f32>,
    u_color: vec4<f32>,
    sun_dir: vec3<f32>,
    u_time: f32,
    view_pos: vec3<f32>,
    time: f32,
    screen_size: vec2<f32>,
    body_type: i32,
    hdr_scale: f32,
}
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(3) color: vec4<f32>,
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let p = (in.position.xy / u.screen_size) * 2.0 - 1.0;
    var out: VsOut;
    out.pos = vec4(p.x, -p.y, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
