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

// Bedrock Vibrant Visuals multi-octave Gerstner-style trochoidal wave slope
fn wave_slope(pos: vec2<f32>, t: f32) -> vec2<f32> {
    let freq = select(1.0, u.screen_size.x, u.screen_size.x > 0.01);
    let pull = select(0.38, u.screen_size.y, abs(u.screen_size.y) > 0.001);

    // Base frequency: swell wavelength ~8 blocks
    var p = pos * (freq * 0.22);
    var slope = vec2<f32>(0.0);
    var amp = 0.075;
    var speed = 1.1;
    var heading = 0.42; // starting angle
    let dir_inc = 1.3962634; // 80.0 degrees (Bedrock standard)

    // 8 octaves of rotating waves: prevents orthogonal interference grids
    for (var i = 0; i < 8; i = i + 1) {
        let dir = vec2<f32>(cos(heading), sin(heading));
        let phase = dot(p, dir) + t * speed;
        let s = sin(phase);
        let c = cos(phase);

        // Bedrock trochoidal wave crest shaping:
        // pull > 0 pulls waves into sharp crests and broader troughs
        let crest = mix(1.0, pow(s * 0.5 + 0.5, 1.4) * 2.0, clamp(pull, 0.0, 1.0));
        slope = slope + dir * (c * crest) * amp;

        p = p * 1.28;
        amp = amp * 0.68;
        speed = speed * 1.05;
        heading = heading + dir_inc;
    }

    return slope;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let texel = textureSample(texture0, sampler0, in.uv);

    // Multi-octave trochoidal wave normal perturbation.
    // Crossed ripples use world coordinates so adjacent chunks stay in phase.
    let wave_depth = select(1.0, u.u_color.w, length(u.u_color) > 0.001);
    let slope = wave_slope(in.world_pos.xz, u.u_time) * wave_depth;
    let n = normalize(in.normal + vec3(-slope.x, 0.0, -slope.y) * abs(in.normal.y));
    let v = normalize(u.view_pos - in.world_pos);

    // Celestial lighting: sun by day, moon by night
    let is_day = u.sun_dir.y >= 0.0;
    let celestial_dir = select(-normalize(u.sun_dir), normalize(u.sun_dir), is_day);
    let celestial_col = select(vec3<f32>(0.70, 0.85, 1.0), vec3<f32>(1.0, 0.96, 0.88), is_day);
    let celestial_alt = max(celestial_dir.y, 0.0);
    let diff = max(dot(n, celestial_dir), 0.0);

    // Mesh channels encode sky light, block light, and AO, not RGB tint.
    let daylight = 0.15 + max(u.sun_dir.y, 0.0) * 0.85;
    let light = max(max(in.color.r * daylight, in.color.g) * in.color.b, 0.12);

    // Bedrock Vibrant Visuals water appearance:
    // Driven by the bio-optical water color and continuous wave optics without 16x16 block seams.
    let custom_color = u.u_color.rgb;
    let ocean_day = select(vec3<f32>(0.090, 0.529, 0.831), custom_color, length(custom_color) > 0.01);
    let ocean_night = vec3<f32>(0.035, 0.120, 0.230) * (ocean_day / vec3<f32>(0.090, 0.529, 0.831));
    let ocean_base = mix(ocean_night, ocean_day, clamp(u.sun_dir.y * 2.0, 0.0, 1.0));
    var albedo = mix(ocean_base, texel.rgb, 0.05);
    if (u.hdr_output != 0) {
        albedo = srgb_to_linear(albedo);
    }

    // Sky reflection: reflected ray samples between zenith and horizon colors
    let r_v = reflect(-v, n);
    let zenith = select(u.col_diffuse.rgb, u.sky_col.rgb, length(u.col_diffuse.rgb) < 0.01);
    let sky_dome = mix(u.sky_col.rgb, zenith, clamp(r_v.y * 1.5, 0.0, 1.0));
    let sky_reflection = select(sky_dome * light, sky_dome, u.hdr_output != 0);

    // Ambient sky and water body scattering: in deferred HDR mode, use physical sky radiance.
    let ambient_light = select(vec3<f32>(light * 0.45), zenith * 0.50 + vec3<f32>(light * 0.15), u.hdr_output != 0);
    let direct_light = albedo * celestial_col * (diff * select(0.20, 0.45, is_day) * celestial_alt);
    let diffuse = albedo * ambient_light + direct_light;

    // Specular (Sun track by day, Moonlight glisten across waves by night)
    let r_l = reflect(-celestial_dir, n);
    let spec = pow(max(dot(v, r_l), 0.0), 32.0);
    let spec_intensity = select(0.35, 0.55, is_day);
    let specular = celestial_col * spec * spec_intensity * in.color.r * celestial_alt;

    // Bedrock Vibrant Visuals water: depth-aware opacity (1.0 for deep ocean),
    // with strong Fresnel reflection at grazing angles.
    let fresnel = 0.04 + 0.96 * pow(1.0 - clamp(abs(dot(n, v)), 0.0, 1.0), 4.5);
    let alpha = mix(in.color.a * 0.92, 1.0, fresnel);

    var color = mix(diffuse, sky_reflection, fresnel) + specular;
    let distance = max(length(in.world_pos - u.view_pos) - 24.0, 0.0);
    let altitude = max((in.world_pos.y + u.view_pos.y) * 0.5 - 96.0, 0.0);
    let haze = (1.0 - exp(-distance * u.fog_density * exp(-altitude * 0.008))) * in.color.r;
    color = mix(color, u.sky_col.rgb, haze);
    return vec4(color * u.hdr_scale, alpha);
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}
