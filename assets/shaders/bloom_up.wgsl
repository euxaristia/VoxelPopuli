// POST
@fragment
fn fs_main(in: CinematicVsOut) -> @location(0) vec4<f32> {
    return vec4(bilinear_color(post_scene, in.uv).rgb + bilinear_color(post_extra, in.uv).rgb, 1.0);
}
