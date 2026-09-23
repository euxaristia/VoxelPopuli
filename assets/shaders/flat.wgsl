// Flat color shader for stars and clouds (port of flat.vs / flat.fs).
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
    hdr_output: i32,
}
@group(0) @binding(0) var<uniform> u: Uniforms;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
}

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.color = in.color;
    out.uv = in.uv;
    out.pos = u.mvp * vec4(in.position, 1.0);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // body_type: 0=flat/stars, 3=clouds
    if (u.body_type == 3) {
        return vec4(scene_color(in.color.rgb) * u.col_diffuse.rgb * u.hdr_scale, in.color.a);
    }
    return vec4(scene_color(in.color.rgb) * u.hdr_scale, in.color.a);
}

fn scene_color(c: vec3<f32>) -> vec3<f32> {
    if (u.hdr_output != 0) {
        return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
    }
    return c;
}
