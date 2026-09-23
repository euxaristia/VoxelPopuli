struct Uniforms {
    mvp: mat4x4<f32>, model: mat4x4<f32>,
    col_diffuse: vec4<f32>, sky_col: vec4<f32>, u_color: vec4<f32>,
    sun_dir: vec3<f32>, u_time: f32, view_pos: vec3<f32>, time: f32,
    screen_size: vec2<f32>, body_type: i32, hdr_scale: f32, hdr_output: i32,
}
@group(0) @binding(0) var<uniform> u: Uniforms;
@group(1) @binding(0) var texture0: texture_2d<f32>;
@group(1) @binding(1) var sampler0: sampler;
struct Input { @location(0) position: vec3<f32>, @location(1) uv: vec2<f32> }
struct Output { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32> }
@vertex fn vs_main(v: Input) -> Output {
    var o: Output;
    o.position = u.mvp * u.model * vec4(v.position, 1.0);
    o.position.z = o.position.w * 0.999999;
    o.uv = u.u_color.xy + v.uv * u.u_color.zw;
    return o;
}
@fragment fn fs_main(v: Output) -> @location(0) vec4<f32> {
    let texel = textureSample(texture0, sampler0, v.uv);
    // Black texels contribute no light in the additive celestial pass.
    if (max(texel.r, max(texel.g, texel.b)) * texel.a < 0.001) { discard; }
    var rgb = texel.rgb;
    if (u.hdr_output != 0) {
        rgb = select(pow((rgb + 0.055) / 1.055, vec3(2.4)), rgb / 12.92, rgb <= vec3(0.04045));
    }
    return vec4(rgb * u.hdr_scale, texel.a);
}
