// Main block shader (port of ps1.vs / ps1.fs).
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
    _pad: f32,
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
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) world_pos: vec3<f32>,
    @location(3) normal: vec3<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var p = in.position;
    // Simple wave animation for water (alpha 240/255 ~= 0.94)
    if (in.color.a > 0.940 && in.color.a < 0.942) {
        p.y += sin(u.u_time * 1.5 + in.position.x * 0.8 + in.position.z * 0.8) * 0.08 - 0.05;
    }
    var out: VsOut;
    out.color = in.color;
    out.uv = in.uv;
    out.world_pos = (u.model * vec4(p, 1.0)).xyz;
    out.normal = normalize((u.model * vec4(in.normal, 0.0)).xyz);
    out.pos = u.mvp * (u.model * vec4(p, 1.0));
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(texture0, sampler0, in.uv);
    if (texel.a < 0.1) {
        discard;
    }

    // Global ambient based on sun position (time of day)
    let sun_y = max(0.0, u.sun_dir.y);
    let time_light = 0.15 + sun_y * 0.85; // 0.15 at night, 1.0 at noon

    let sky_light = in.color.r * time_light;
    let block_light = in.color.g;
    let light_level = max(sky_light, block_light);
    let ao = in.color.b;
    let light_factor = max(light_level * ao, 0.12);

    var color = texel.rgb * light_factor * u.col_diffuse.rgb;
    color *= mix(vec3(1.0, 0.9, 0.8), vec3(1.0, 1.0, 1.05), sun_y); // slight tinting

    return vec4(color, texel.a * in.color.a * u.col_diffuse.a);
}
