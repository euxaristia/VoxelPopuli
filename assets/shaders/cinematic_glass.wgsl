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
    @location(1) world_pos: vec3<f32>,
    @location(2) normal: vec3<f32>,
}

fn glass_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.uv = in.uv;
    out.world_pos = (u.model * vec4(in.position, 1.0)).xyz;
    out.normal = normalize((u.model * vec4(in.normal, 0.0)).xyz);
    out.pos = u.mvp * vec4(out.world_pos, 1.0);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(texture0, sampler0, in.uv);
    if (texel.a < 0.1) {
        discard;
    }
    let water_z = textureLoad(water_depth_map, vec2<i32>(in.pos.xy), 0);
    if (u.body_type == 1 && in.pos.z <= water_z) {
        discard;
    }
    if (u.body_type == 2 && in.pos.z >= water_z) {
        discard;
    }
    let n = normalize(in.normal);
    let view = normalize(u.view_pos - in.world_pos);
    let reflected_dir = reflect(-view, n);
    let hit = scene_reflection(in.world_pos, reflected_dir, 0.04);
    let reflection = mix(sky_environment(in.world_pos, reflected_dir), hit.rgb, hit.a);
    let fresnel = 0.04 + 0.96 * pow(1.0 - max(dot(n, view), 0.0), 5.0);
    let transmission = glass_linear(texel.rgb) * glass_linear(u.col_diffuse.rgb) * u.hdr_scale;
    return vec4(mix(transmission, reflection, fresnel), clamp(0.28 + fresnel * 0.55, 0.0, 1.0));
}
