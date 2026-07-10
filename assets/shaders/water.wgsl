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
    // Slight vertex animation for waves
    let wave = sin(u.u_time * 2.5 + p.x * 1.5 + p.z * 1.5) * 0.05;
    p.y += wave;

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

    let n = normalize(in.normal);
    let v = normalize(u.view_pos - in.world_pos);
    let l = normalize(u.sun_dir);
    let r = reflect(-l, n);

    // Ambient + Diffuse
    let diff = max(dot(n, l), 0.0);
    let diffuse = texel.rgb * in.color.rgb * (diff * 0.7 + 0.3);

    // Specular (Sparkle/Highlights)
    let spec = pow(max(dot(v, r), 0.0), 32.0);
    let specular = vec3(1.0) * spec * 0.6;

    // Fresnel-ish transparency
    let fresnel = pow(1.0 - max(dot(n, v), 0.0), 3.0);
    let alpha = mix(in.color.a, 1.0, fresnel * 0.5);

    // Simple shore foam / depth effect simulation
    let color = mix(diffuse, vec3(0.1, 0.4, 0.8), 0.2) + specular;

    return vec4(color, alpha);
}
