// Flat color shader for stars, clouds, sun and moon (port of flat.vs / flat.fs).
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
    // body_type: 0=flat, 1=sun, 2=moon
    if (u.body_type > 0) {
        let uv8 = in.uv * 8.0;
        let iuv = vec2<i32>(floor(uv8));
        if (iuv.x < 0 || iuv.x > 7 || iuv.y < 0 || iuv.y > 7) {
            discard;
        }

        var shape = array<i32, 8>(60, 126, 255, 255, 255, 255, 126, 60);
        let row_mask = shape[7 - iuv.y];
        if ((row_mask & (1 << u32(7 - iuv.x))) == 0) {
            discard;
        }

        var color = in.color;
        if (u.body_type == 2) {
            // Craters on the moon
            var craters = array<i32, 8>(0, 20, 36, 64, 8, 16, 2, 0);
            let crater_mask = craters[7 - iuv.y];
            if ((crater_mask & (1 << u32(7 - iuv.x))) != 0) {
                color = vec4(color.rgb * 0.65, color.a);
            }
        }
        // hdr_scale is 1.0 on the forward path and the scene's key
        // luminance when drawing into the HDR target, so celestial
        // bodies survive tone mapping instead of being crushed.
        return vec4(color.rgb * u.hdr_scale, color.a);
    }
    return vec4(in.color.rgb * u.hdr_scale, in.color.a);
}
