// Water shader (port of water.vs / water.fs).
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
    fog_density: f32,
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
    var out: VsOut;
    out.color = in.color;
    out.uv = in.uv;
    out.world_pos = (u.model * vec4(in.position, 1.0)).xyz;
    out.normal = normalize((u.model * vec4(in.normal, 0.0)).xyz);
    out.pos = u.mvp * vec4(out.world_pos, 1.0);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(texture0, sampler0, in.uv);

    // Normal-only waves preserve the shoreline and cannot open chunk seams.
    // Crossed ripples use world coordinates so adjacent chunks stay in phase.
    let phase_a = dot(in.world_pos.xz, vec2(1.3, 0.8)) + u.u_time * 1.6;
    let phase_b = dot(in.world_pos.xz, vec2(-0.7, 1.9)) - u.u_time * 1.1;
    let slope = vec2(1.3, 0.8) * cos(phase_a) * 0.035
        + vec2(-0.7, 1.9) * cos(phase_b) * 0.018;
    let n = normalize(in.normal + vec3(-slope.x, 0.0, -slope.y) * abs(in.normal.y));
    let v = normalize(u.view_pos - in.world_pos);
    let l = normalize(u.sun_dir);
    let r = reflect(-l, n);

    // Mesh channels encode sky light, block light, and AO, not RGB tint.
    // Multiplying them into RGB removed green in daylight and turned water purple.
    let daylight = 0.15 + max(u.sun_dir.y, 0.0) * 0.85;
    let light = max(max(in.color.r * daylight, in.color.g) * in.color.b, 0.12);
    let diff = max(dot(n, l), 0.0);
    var albedo = texel.rgb;
    if (u.hdr_output != 0) {
        albedo = srgb_to_linear(albedo);
    }
    let diffuse = albedo * light * (diff * 0.7 + 0.3);

    // Specular (Sparkle/Highlights)
    let spec = pow(max(dot(v, r), 0.0), 32.0);
    let specular = vec3(1.0) * spec * 0.25 * in.color.r * max(u.sun_dir.y, 0.0);

    // Fresnel-ish transparency
    // Water's head-on reflectance is about 2%; reflection grows at grazing angles.
    let fresnel = 0.02 + 0.98 * pow(1.0 - clamp(abs(dot(n, v)), 0.0, 1.0), 5.0);
    let alpha = mix(in.color.a * 0.60, 0.94, fresnel);

    // More sky reflection at grazing angles; clear shallows when looking down.
    var color = mix(diffuse, u.sky_col.rgb * light, fresnel) + specular;
    let distance = max(length(in.world_pos - u.view_pos) - 24.0, 0.0);
    let altitude = max((in.world_pos.y + u.view_pos.y) * 0.5 - 96.0, 0.0);
    let haze = (1.0 - exp(-distance * u.fog_density * exp(-altitude * 0.008))) * in.color.r;
    color = mix(color, u.sky_col.rgb, haze);

    return vec4(color * u.hdr_scale, alpha);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}
