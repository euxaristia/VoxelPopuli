// POST
@fragment
fn fs_main(in: CinematicVsOut) -> @location(0) vec4<f32> {
    let scene = load_color(post_scene, in.uv).rgb;
    let bloom = bilinear_color(post_extra, in.uv).rgb;
    let exposure = max(fx.light.camera_pos_exposure.w, 0.000001);
    return vec4(min(scene + bloom * 0.08 / exposure, vec3(60000.0)), 1.0);
}
