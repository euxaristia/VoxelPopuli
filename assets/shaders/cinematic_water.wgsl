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
    @location(0) world_pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.world_pos = (u.model * vec4(in.position, 1.0)).xyz;
    let slope = sin(out.world_pos.xz * max(u.screen_size.x, 1.0) + u.u_time) * u.u_color.a;
    out.normal = normalize(in.normal + vec3(slope.x, 0.0, slope.y));
    out.pos = u.mvp * vec4(out.world_pos, 1.0);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let n = normalize(in.normal);
    let view = normalize(u.view_pos - in.world_pos);
    let reflected_dir = reflect(-view, n);
    let hit = scene_reflection(in.world_pos, reflected_dir, 0.08);
    let sky = volume_at(in.world_pos).g;
    let environment = sky_environment(in.world_pos, reflected_dir) * sky;
    let reflection = mix(environment, hit.rgb, hit.a);
    let clip = u.mvp * vec4(in.world_pos, 1.0);
    let uv = vec2(clip.x / clip.w * 0.5 + 0.5, 0.5 - clip.y / clip.w * 0.5);
    let size = vec2<i32>(textureDimensions(opaque_scene));
    let shifted = clamp(vec2<i32>((uv + n.xz * 0.015) * vec2<f32>(size)), vec2(0), size - 1);
    let center = clamp(vec2<i32>(uv * vec2<f32>(size)), vec2(0), size - 1);
    let water_z = textureLoad(water_depth_map, vec2<i32>(in.pos.xy), 0);
    let sample_z = textureLoad(opaque_depth, shifted, 0);
    let refracted = select(
        textureLoad(opaque_scene, center, 0).rgb,
        textureLoad(opaque_scene, shifted, 0).rgb,
        sample_z > water_z,
    );
    var thickness = 0.0;
    let inward = normalize(in.world_pos - u.view_pos);
    for (var i = 1; i <= 8; i++) {
        if (!liquid_at(in.world_pos + inward * f32(i) * 0.5)) {
            break;
        }
        thickness += 0.5;
    }
    let through = refracted * exp(-vec3(0.42, 0.11, 0.07) * thickness);
    let fresnel = 0.02 + 0.98 * pow(1.0 - max(dot(n, view), 0.0), 5.0);
    let glint = pow(max(dot(reflected_dir, normalize(u.sun_dir)), 0.0), 96.0);
    var color = mix(through, reflection, fresnel);
    color += glint * fx.light.sun_color.rgb * fx.active_light.w * directional_shadow(in.world_pos, n) * 0.015;
    let foam = (1.0 - smoothstep(0.04, 0.4, thickness)) * sky * max(n.y, 0.0);
    color = mix(color, vec3(0.62, 0.66, 0.64) * u.hdr_scale, foam * 0.28);
    return vec4(min(color, vec3(60000.0)), 1.0);
}
