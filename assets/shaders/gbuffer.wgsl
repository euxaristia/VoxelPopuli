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
}

struct GBuffer {
    @location(0) albedo: vec4<f32>,
    @location(1) normal: vec4<f32>,
    @location(2) mers: vec4<f32>,
    @location(3) lighting: vec4<f32>,
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
    let normal_length = length(world_normal);
    out.normal = select(vec3(0.0, 1.0, 0.0), world_normal / normal_length, normal_length > 1e-4);
    out.pos = u.mvp * (u.model * vec4(in.position, 1.0));
    return out;
}

@fragment
fn fs_main(in: VsOut) -> GBuffer {
    let texel = textureSample(texture0, sampler0, in.uv);
    if (texel.a < 0.1) {
        discard;
    }

    var out: GBuffer;
    // The albedo attachment is sRGB, so the hardware handles the encode.
    // col_diffuse stays as the per-draw tint (damage flash, biome color).
    out.albedo = vec4(texel.rgb * u.col_diffuse.rgb, texel.a);
    out.normal = vec4(normalize(in.normal), 0.0);
    // MERS defaults for untextured terrain: dielectric, non-emissive,
    // fully rough. Texture sets and pbr/global.json override this once the
    // MERS atlas is bound.
    out.mers = vec4(0.0, 0.0, 1.0, 0.0);
    // Entity draws put world sky/block light in u_color (a = 1). Terrain
    // leaves a = 0 and uses the mesher's per-vertex bake. White cube
    // vertices used to read as full block-light, which night exposure
    // turned into glowing animals.
    let baked = mix(in.color.rgb, u.u_color.rgb, u.u_color.a);
    out.lighting = vec4(baked, 1.0);
    return out;
}
