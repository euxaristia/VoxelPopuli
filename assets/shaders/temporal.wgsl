// POST
@fragment
fn fs_main(in: CinematicVsOut) -> @location(0) vec4<f32> {
    let current = load_color(post_scene, in.uv).rgb;
    if (fx.time.z < 0.5) {
        return vec4(current, 1.0);
    }
    let size = vec2<i32>(textureDimensions(post_scene));
    let p = clamp(vec2<i32>(in.pos.xy), vec2(0), size - 1);
    let depth = textureLoad(post_depth, p, 0);
    if (depth >= 1.0) {
        return vec4(current, 1.0);
    }
    let world = cinematic_world(in.uv, depth);
    let prev = fx.previous_view_proj * vec4(world, 1.0);
    if (prev.w <= 0.001) {
        return vec4(current, 1.0);
    }
    let uv = vec2(prev.x / prev.w * 0.5 + 0.5, 0.5 - prev.y / prev.w * 0.5);
    if (any(uv < vec2(0.0)) || any(uv > vec2(1.0))) {
        return vec4(current, 1.0);
    }
    let hsize = vec2<i32>(textureDimensions(post_history_depth));
    let hp = clamp(vec2<i32>(uv * vec2<f32>(hsize)), vec2(0), hsize - 1);
    let history_depth = textureLoad(post_history_depth, hp, 0);
    let normal = textureLoad(post_normal, p, 0).xyz;
    let history_normal = textureLoad(post_history_normal, hp, 0).xyz;
    let depth_gap = abs(prev.z / prev.w - history_depth);
    let facing = dot(normalize(normal), normalize(history_normal + vec3(0.0001)));
    if (depth_gap > 0.015 || facing < 0.75) {
        return vec4(current, 1.0);
    }
    var low = current;
    var high = current;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let sample = textureLoad(post_scene, clamp(p + vec2(x, y), vec2(0), size - 1), 0).rgb;
            low = min(low, sample);
            high = max(high, sample);
        }
    }
    let history = clamp(bilinear_color(post_history, uv).rgb, low, high);
    let opaque = load_color(post_extra, in.uv).rgb;
    let reactive = smoothstep(0.02, 0.25, length(current - opaque) / max(length(current), 1.0));
    return vec4(mix(current, history, mix(0.9, 0.12, reactive)), 1.0);
}
