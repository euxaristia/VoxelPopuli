// POST
@fragment
fn fs_main(in: CinematicVsOut) -> @location(0) vec4<f32> {
    let base = load_color(post_scene, in.uv).rgb;
    let fog = bilinear_color(post_extra, in.uv);
    let underwater = liquid_at(fx.light.camera_pos_exposure.xyz);
    let tint = select(vec3(fog.a), mix(fx.light.water_color.rgb, vec3(1.0), fog.a), underwater);
    return vec4(min(base * tint + fog.rgb, vec3(60000.0)), 1.0);
}
