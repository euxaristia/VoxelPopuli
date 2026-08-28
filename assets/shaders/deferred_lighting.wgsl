// Deferred lighting resolve: G-buffer in, HDR scene radiance out.
//
// Implements the Vibrant Visuals lighting model. Illuminance arrives in
// real lux from lighting/global.json, so this shader works in absolute
// units and leaves the rebalancing to tone mapping.

struct Deferred {
    inv_view_proj: mat4x4<f32>,
    // xyz camera position, w exposure
    camera_pos_exposure: vec4<f32>,
    // xyz direction towards the light, w illuminance in lux
    sun_direction_illuminance: vec4<f32>,
    sun_color: vec4<f32>,
    moon_direction_illuminance: vec4<f32>,
    moon_color: vec4<f32>,
    // rgb ambient color, w illuminance in lux
    ambient_color_illuminance: vec4<f32>,
    // x sky intensity, y emissive desaturation, z block-light lux
    sky_params: vec4<f32>,
    zenith_color: vec4<f32>,
    horizon_color: vec4<f32>,
    // rayleigh strength, sun mie, moon mie, sun glare shape
    atmosphere: vec4<f32>,
    // horizon_blend_stops: min, start, mie_start, max
    horizon_stops: vec4<f32>,
    block_light_color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Deferred;

@group(1) @binding(0) var g_albedo: texture_2d<f32>;
@group(1) @binding(1) var g_normal: texture_2d<f32>;
@group(1) @binding(2) var g_mers: texture_2d<f32>;
@group(1) @binding(3) var g_lighting: texture_2d<f32>;
@group(1) @binding(4) var g_depth: texture_depth_2d;

const PI: f32 = 3.14159265359;
// Lambertian surfaces return irradiance / PI as radiance. Illuminance is
// authored in lux, which would blow out an 8-bit target but is exactly
// what the tone mapper expects.
const INV_PI: f32 = 0.31830988618;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

// One oversized triangle. Cheaper than a quad and has no diagonal seam.
@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    var out: VsOut;
    out.uv = vec2(x, y);
    out.pos = vec4(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

// --- BRDF ------------------------------------------------------------

// GGX / Trowbridge-Reitz normal distribution.
fn distribution_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let d = n_dot_h * n_dot_h * (a2 - 1.0) + 1.0;
    return a2 / max(PI * d * d, 1e-7);
}

// Smith height-correlated visibility, already divided by the
// 4 * NdotL * NdotV denominator of the specular term.
fn visibility_smith(n_dot_v: f32, n_dot_l: f32, roughness: f32) -> f32 {
    let a = roughness * roughness;
    let a2 = a * a;
    let v = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - a2) + a2);
    let l = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - a2) + a2);
    return 0.5 / max(v + l, 1e-7);
}

fn fresnel_schlick(cos_theta: f32, f0: vec3<f32>) -> vec3<f32> {
    return f0 + (vec3(1.0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// Fresnel with a roughness term, for the indirect/ambient specular where
// there is no single view-light pair to evaluate against.
fn fresnel_schlick_roughness(cos_theta: f32, f0: vec3<f32>, roughness: f32) -> vec3<f32> {
    let inv_rough = vec3(1.0 - roughness);
    return f0 + (max(inv_rough, f0) - f0) * pow(clamp(1.0 - cos_theta, 0.0, 1.0), 5.0);
}

// One directional light's contribution, in radiance.
fn direct_light(
    n: vec3<f32>,
    v: vec3<f32>,
    l: vec3<f32>,
    albedo: vec3<f32>,
    metalness: f32,
    roughness: f32,
    f0: vec3<f32>,
    illuminance: vec3<f32>,
    shadow: f32,
) -> vec3<f32> {
    let n_dot_l = dot(n, l);
    if (n_dot_l <= 0.0 || shadow <= 0.0) {
        return vec3(0.0);
    }
    let n_dot_v = max(dot(n, v), 1e-4);
    let h = normalize(v + l);
    let n_dot_h = max(dot(n, h), 0.0);
    let v_dot_h = max(dot(v, h), 0.0);

    let d = distribution_ggx(n_dot_h, roughness);
    let vis = visibility_smith(n_dot_v, n_dot_l, roughness);
    let f = fresnel_schlick(v_dot_h, f0);

    let specular = d * vis * f;
    // Metals have no diffuse lobe; what is not reflected is absorbed.
    let kd = (vec3(1.0) - f) * (1.0 - metalness);
    let diffuse = kd * albedo * INV_PI;

    return (diffuse + specular) * illuminance * n_dot_l * shadow;
}

// --- Sky -------------------------------------------------------------

// Analytic sky: a Rayleigh-weighted vertical gradient plus a Mie lobe
// around each celestial body. Driven entirely by atmospherics.json.
fn sky_radiance(dir: vec3<f32>) -> vec3<f32> {
    let height = clamp(dir.y, -1.0, 1.0);
    let stops = u.horizon_stops;
    // Blend from horizon color to zenith color between `start` and `max`,
    // with `min` lifting the floor of the horizon band.
    let lower = min(stops.x, stops.y);
    let upper = max(stops.y + max(stops.w, 1e-3), lower + 1e-3);
    let t = clamp((height - lower) / (upper - lower), 0.0, 1.0);
    // Rayleigh scattering falls off with the fourth power towards the
    // zenith; strength scales how blue the gradient reads.
    let rayleigh = pow(t, 1.0 / max(u.atmosphere.x, 1e-3));
    var color = mix(u.horizon_color.rgb, u.zenith_color.rgb, rayleigh);

    // Forward-scattering lobes around the sun and moon.
    let glare = max(u.atmosphere.w, 1.0);
    let sun_cos = max(dot(dir, u.sun_direction_illuminance.xyz), 0.0);
    let moon_cos = max(dot(dir, u.moon_direction_illuminance.xyz), 0.0);
    let mie_fade = smoothstep(stops.z - 0.5, stops.z + 0.5, height + 1.0);
    color += u.sun_color.rgb * u.atmosphere.y * pow(sun_cos, glare) * mie_fade;
    color += u.moon_color.rgb * u.atmosphere.z * pow(moon_cos, glare) * mie_fade;
    return color;
}

// Rebuilds world position from the depth buffer.
fn world_from_depth(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let world = u.inv_view_proj * ndc;
    return world.xyz / world.w;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let coord = vec2<i32>(in.pos.xy);
    let depth = textureLoad(g_depth, coord, 0);

    // The geometry pass clears depth to 1.0, so an untouched pixel is sky.
    // Sky radiance is scaled by the sun's illuminance so the sky brightens
    // and dims with the day rather than staying a flat painted color.
    let view_dir = normalize(
        world_from_depth(in.uv, 1.0) - u.camera_pos_exposure.xyz
    );
    if (depth >= 1.0) {
        let sun_lux = u.sun_direction_illuminance.w;
        let moon_lux = u.moon_direction_illuminance.w;
        return vec4(sky_radiance(view_dir) * (sun_lux + moon_lux) * INV_PI, 1.0);
    }

    let albedo_sample = textureLoad(g_albedo, coord, 0);
    let albedo = albedo_sample.rgb;
    let normal_sample = textureLoad(g_normal, coord, 0);
    let mers = textureLoad(g_mers, coord, 0);
    let baked = textureLoad(g_lighting, coord, 0);

    let n = normalize(normal_sample.xyz);
    let world_pos = world_from_depth(in.uv, depth);
    let v = normalize(u.camera_pos_exposure.xyz - world_pos);

    let metalness = mers.r;
    let emissive = mers.g;
    // A perfectly smooth surface produces a singular highlight, so hold
    // roughness just off zero.
    let roughness = clamp(mers.b, 0.045, 1.0);
    let subsurface = mers.a;

    let sky_visibility = baked.r;
    let block_light = baked.g;
    let ambient_occlusion = baked.b;

    // Dielectrics reflect about 4% head-on; metals reflect their albedo.
    let f0 = mix(vec3(0.04), albedo, metalness);

    var radiance = vec3(0.0);

    // Sun and moon are opposite points of one orbit and both contribute
    // whenever they are above the horizon. Sky visibility stands in for a
    // shadow term until the shadow maps land.
    radiance += direct_light(
        n, v, u.sun_direction_illuminance.xyz, albedo, metalness, roughness, f0,
        u.sun_color.rgb * u.sun_direction_illuminance.w, sky_visibility
    );
    radiance += direct_light(
        n, v, u.moon_direction_illuminance.xyz, albedo, metalness, roughness, f0,
        u.moon_color.rgb * u.moon_direction_illuminance.w, sky_visibility
    );

    // Indirect diffuse from the sky, weighted by how much of the upper
    // hemisphere the surface faces and how exposed to sky it is.
    let sky_intensity = u.sky_params.x;
    let hemisphere = clamp(n.y * 0.5 + 0.5, 0.0, 1.0);
    let sky_color = mix(u.horizon_color.rgb, u.zenith_color.rgb, hemisphere);
    let sky_lux = u.sun_direction_illuminance.w + u.moon_direction_illuminance.w;
    let sky_irradiance = sky_color * sky_lux * sky_intensity * sky_visibility * INV_PI;
    let kd_sky = (vec3(1.0) - fresnel_schlick_roughness(max(dot(n, v), 0.0), f0, roughness))
        * (1.0 - metalness);
    radiance += kd_sky * albedo * sky_irradiance * ambient_occlusion;

    // Indirect specular: reflect the sky. Rough surfaces pull towards the
    // flat hemisphere color, which is the cheap stand-in for a prefiltered
    // environment map.
    let reflected = reflect(-v, n);
    let sky_specular = mix(sky_radiance(reflected), sky_color, roughness);
    let f_ibl = fresnel_schlick_roughness(max(dot(n, v), 0.0), f0, roughness);
    radiance += f_ibl * sky_specular * sky_lux * sky_intensity * sky_visibility
        * INV_PI * ambient_occlusion;

    // Static local light: torches and the like, baked per vertex by the
    // mesher and tinted by local_lighting.json.
    radiance += albedo * u.block_light_color.rgb * block_light
        * u.sky_params.z * INV_PI * ambient_occlusion;

    // Ambient: the documented fallback so an unlit cave is not pure black.
    let ambient = u.ambient_color_illuminance.rgb * u.ambient_color_illuminance.w;
    radiance += albedo * ambient * ambient_occlusion * INV_PI;

    // Emissive, desaturated towards its own luminance by the amount
    // lighting/global.json asks for.
    let luminance = dot(albedo, vec3(0.2126, 0.7152, 0.0722));
    let emissive_albedo = mix(albedo, vec3(luminance), u.sky_params.y);
    // Scaled into lux so emissive surfaces survive tone mapping in
    // daylight without being painfully bright at night.
    radiance += emissive_albedo * emissive * 2000.0;

    // Subsurface: light that entered the surface and left facing the
    // viewer, so it shows most when the light is behind the geometry.
    if (subsurface > 0.0) {
        let back = max(dot(v, -u.sun_direction_illuminance.xyz), 0.0);
        let wrap = max(dot(n, -u.sun_direction_illuminance.xyz) * 0.5 + 0.5, 0.0);
        radiance += albedo * u.sun_color.rgb * u.sun_direction_illuminance.w
            * subsurface * pow(back, 2.0) * wrap * sky_visibility * INV_PI;
    }

    return vec4(radiance, 1.0);
}
