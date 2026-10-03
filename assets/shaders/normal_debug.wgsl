@group(0) @binding(1) var normals: texture_2d<f32>;
@group(0) @binding(4) var depth: texture_depth_2d;

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let x = f32((index << 1u) & 2u);
    let y = f32(index & 2u);
    return vec4(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
}

@fragment
fn fs_main(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(position.xy);
    if (textureLoad(depth, pixel, 0) >= 1.0) {
        return vec4(0.0, 0.0, 0.0, 1.0);
    }
    let n = textureLoad(normals, pixel, 0).xyz;
    let squared = dot(n, n);
    if (!(squared > 0.000001 && squared < 10000.0)) {
        return vec4(1.0, 0.0, 1.0, 1.0);
    }
    return vec4(n * 0.5 + 0.5, 1.0);
}
