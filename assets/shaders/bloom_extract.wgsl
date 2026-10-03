// POST
fn extract(exposed: vec3<f32>) -> vec3<f32> {
    let luma = dot(exposed, vec3(0.2126, 0.7152, 0.0722));
    let knee = max(luma - 0.92, 0.0);
    let weight = knee * knee / (knee + 0.15);
    return exposed * weight / max(luma, 0.001);
}

@fragment
fn fs_main(in: CinematicVsOut) -> @location(0) vec4<f32> {
    let size = vec2<i32>(textureDimensions(post_scene));
    let base = vec2<i32>(floor(in.uv * vec2<f32>(size) - 0.5));
    var glow = vec3(0.0);
    for (var y = 0; y < 2; y++) {
        for (var x = 0; x < 2; x++) {
            let color = textureLoad(post_scene, clamp(base + vec2(x,y),vec2(0),size-1),0).rgb;
            glow += extract(color * fx.light.camera_pos_exposure.w);
        }
    }
    return vec4(glow * 0.25, 1.0);
}
