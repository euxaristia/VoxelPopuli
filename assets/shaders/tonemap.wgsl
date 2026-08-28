// Tone mapping and color grading: HDR scene radiance in lux down to a
// displayable image.
//
// Exposure is computed on the CPU from the illuminance the pack declares
// and arrives in camera_pos_exposure.w, so a scene lit at 100,000 lux and
// one lit at 0.27 lux both land in range without a readback.

struct Deferred {
    inv_view_proj: mat4x4<f32>,
    camera_pos_exposure: vec4<f32>,
    sun_direction_illuminance: vec4<f32>,
    sun_color: vec4<f32>,
    moon_direction_illuminance: vec4<f32>,
    moon_color: vec4<f32>,
    ambient_color_illuminance: vec4<f32>,
    sky_params: vec4<f32>,
    zenith_color: vec4<f32>,
    horizon_color: vec4<f32>,
    atmosphere: vec4<f32>,
    horizon_stops: vec4<f32>,
    block_light_color: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Deferred;
@group(1) @binding(0) var scene: texture_2d<f32>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VsOut {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    var out: VsOut;
    out.uv = vec2(x, y);
    out.pos = vec4(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    return out;
}

// ACES filmic tone mapping, Narkowicz's fit. Rolls highlights off without
// the hard clip a Reinhard curve leaves on a bright sky.
fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51;
    let b = 0.03;
    let c = 2.43;
    let d = 0.59;
    let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3(0.0), vec3(1.0));
}

fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let cutoff = c <= vec3(0.0031308);
    let low = c * 12.92;
    let high = 1.055 * pow(max(c, vec3(0.0031308)), vec3(1.0 / 2.4)) - 0.055;
    return select(high, low, cutoff);
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let coord = vec2<i32>(in.pos.xy);
    var color = textureLoad(scene, coord, 0).rgb;

    color *= u.camera_pos_exposure.w;
    color = aces(color);
    // The tone-map target is a plain UNORM surface, so the transfer
    // function has to be applied here rather than by the hardware.
    color = linear_to_srgb(color);

    return vec4(color, 1.0);
}
