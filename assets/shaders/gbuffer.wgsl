// Deferred geometry pass. Replaces ps1.wgsl's forward shading: instead of
// resolving a color, it writes the surface description the deferred
// lighting pass consumes.
//
// Vertex colors carry what the mesher baked: R sky light, G block light,
// B ambient occlusion. Those pass through untouched; the lighting pass
// decides what they mean.
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
@group(1) @binding(2) var material_mers: texture_2d<f32>;
@group(1) @binding(3) var material_normal: texture_2d<f32>;

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
    @location(2) normal: vec3<f32>,
    @location(3) world: vec3<f32>,
}

struct GBuffer {
    @location(0) albedo: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) mers: vec4<f32>,
    @location(3) lighting: vec4<f32>,
}

fn unit_or(value:vec3<f32>,fallback:vec3<f32>)->vec3<f32> {
    let squared = dot(value, value);
    if (squared > 0.000000000001 && squared < 100000000000000000000.0) {
        return value * inverseSqrt(squared);
    }
    return fallback;
}

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    out.color = in.color;
    out.uv = in.uv;
    // Normals are transformed by the model matrix directly: chunk models
    // are translation-only, so there is no non-uniform scale to correct.
    // Some effect meshes are built with zero normals; normalizing those
    // would put a NaN in the G-buffer and blow a hole in the lighting, so
    // they fall back to facing up.
    let world_normal = (u.model * vec4(in.normal, 0.0)).xyz;
    out.normal = unit_or(world_normal, vec3(0.0, 1.0, 0.0));
    out.world = (u.model * vec4(in.position, 1.0)).xyz;
    out.pos = u.mvp * (u.model * vec4(in.position, 1.0));
    return out;
}

@fragment
fn fs_main(in: VsOut) -> GBuffer {
    let texel = textureSample(texture0, sampler0, in.uv);
    var out: GBuffer;
    // The shared atlas and authored tints are sRGB values in UNORM storage.
    // Decode before the sRGB attachment encodes them again; otherwise the
    // lighting pass receives inflated albedo and washes out every material.
    out.albedo = vec4(srgb_to_linear(texel.rgb) * srgb_to_linear(u.col_diffuse.rgb), texel.a);
    let dims = vec2<i32>(textureDimensions(material_mers));
    let material_texel = clamp(vec2<i32>(in.uv * vec2<f32>(dims)), vec2(0), dims - 1);
    let mapped = textureLoad(material_normal, material_texel, 0).xyz * 2.0 - 1.0;
    let geom = unit_or(in.normal, vec3(0.0, 1.0, 0.0));
    let du = dpdx(in.uv);
    let dv = dpdy(in.uv);
    let det = du.x * dv.y - du.y * dv.x;
    var normal = geom;
    let dx = dpdx(in.world);
    let dy = dpdy(in.world);
    if (abs(det) > 0.000000000001 && abs(det) < 10000.0 && length(mapped.xy) > 0.006) {
        let tangent = unit_or((dx * dv.y - dy * du.y) / det, vec3(0.0));
        let bitangent = unit_or((dy * du.x - dx * dv.x) / det, vec3(0.0));
        if (dot(tangent,tangent) > 0.5 && dot(bitangent,bitangent) > 0.5) {
            normal = unit_or(tangent * mapped.x + bitangent * mapped.y + geom * mapped.z, geom);
        }
    }
    // Keep derivatives valid across cutout edges, then discard uncovered pixels.
    if (texel.a < 0.1) { discard; }
    out.normal = vec4(normal, 0.0);
    out.mers = textureLoad(material_mers, material_texel, 0);
    // Entity draws put world sky/block light in u_color (a = 1). Terrain
    // leaves a = 0 and uses the mesher's per-vertex bake. White cube
    // vertices used to read as full block-light, which night exposure
    // turned into glowing animals.
    let baked = mix(in.color.rgb, u.u_color.rgb, u.u_color.a);
    out.lighting = vec4(baked, 1.0);
    return out;
}

fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    return select(pow((c + 0.055) / 1.055, vec3(2.4)), c / 12.92, c <= vec3(0.04045));
}
