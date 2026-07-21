// M2 test shader (hashed-splashing-haven plan): ports the CPU "standard
// block" meshing path (chunk.rs calculate_mesh_data, opaque cube blocks
// only — water/lava/wheat are explicitly out of scope here) to a compute
// shader that reads directly from the persistent GPU voxel pool. Output
// vertex layout intentionally matches the CPU mesher's interleaved format
// (pos f32x3 + uv f32x2 + normal f32x3 + color unorm8x4 = 9 u32 words) so
// no reformatting is needed once M3 wires this into real draw calls.

const AIR: u32 = 0u;
const BEDROCK: u32 = 6u;
const WATER: u32 = 7u;
const LAVA: u32 = 74u;
const WHEAT: u32 = 78u;
const TS: f32 = 1.0 / 16.0;
const PAD: f32 = 0.5 / 256.0;

struct Params {
    chunk_x: i32,
    chunk_z: i32,
    pool_width: i32,
    // Vertical offset added to every emitted vertex. 0.0 for the M2 CPU-diff
    // test (exact parity); nonzero for the M3 visual check, which renders the
    // compute-meshed chunk floating above its CPU-meshed twin.
    y_offset: f32,
    // TEMPORARY debug: when nonzero, write_vertex overrides the packed color
    // with a position-derived rainbow instead of real AO/light, to visually
    // separate "wrong geometry" from "right geometry, wrong shading".
    debug_tint: u32,
};

struct AtlasEntry {
    default_tx: u32,
    default_ty: u32,
    top_tx: u32,
    top_ty: u32,
    top_height: f32,
    is_solid: u32,
    is_transparent: u32,
    _pad0: f32,
};

@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read> blocks_pool: array<u32>;
@group(0) @binding(2) var<storage, read> light_pool: array<u32>;
@group(0) @binding(3) var<storage, read> slot_meta: array<vec2<i32>>;
@group(0) @binding(4) var<storage, read> atlas_table: array<AtlasEntry>;
@group(0) @binding(5) var<storage, read_write> out_vertices: array<u32>;
@group(0) @binding(6) var<storage, read_write> vertex_counter: atomic<u32>;

fn rem_euclid(a: i32, b: i32) -> i32 {
    let r = a % b;
    return select(r, r + b, r < 0);
}

fn floor_div(a: i32, b: i32) -> i32 {
    var q = a / b;
    if ((a % b) != 0 && ((a < 0) != (b < 0))) {
        q = q - 1;
    }
    return q;
}

fn pool_slot(cx: i32, cz: i32) -> i32 {
    let ix = rem_euclid(cx, params.pool_width);
    let iz = rem_euclid(cz, params.pool_width);
    return ix + iz * params.pool_width;
}

// General neighbor read: Air outside the loaded vertical/pool range, matching
// the CPU's MeshSnapshot::get_block/get_light (chunk.rs:409-417,419-427).
// naga can't take a storage-buffer pointer as a function argument, so the
// byte-extraction is inlined in both readers rather than shared.
fn read_block(wx: i32, wy: i32, wz: i32) -> u32 {
    if (wy < 0 || wy >= 256) {
        return AIR;
    }
    let cx = floor_div(wx, 16);
    let cz = floor_div(wz, 16);
    let slot = pool_slot(cx, cz);
    let slot_owner = slot_meta[slot];
    if (slot_owner.x != cx || slot_owner.y != cz) {
        return AIR;
    }
    let lx = u32(rem_euclid(wx, 16));
    let lz = u32(rem_euclid(wz, 16));
    let local_idx = lx * 4096u + u32(wy) * 16u + lz;
    let word = blocks_pool[u32(slot) * 16384u + local_idx / 4u];
    let shift = (local_idx % 4u) * 8u;
    return (word >> shift) & 0xFFu;
}

fn read_light(wx: i32, wy: i32, wz: i32) -> u32 {
    if (wy < 0 || wy >= 256) {
        return 15u;
    }
    let cx = floor_div(wx, 16);
    let cz = floor_div(wz, 16);
    let slot = pool_slot(cx, cz);
    let slot_owner = slot_meta[slot];
    if (slot_owner.x != cx || slot_owner.y != cz) {
        return 15u;
    }
    let lx = u32(rem_euclid(wx, 16));
    let lz = u32(rem_euclid(wz, 16));
    let local_idx = lx * 4096u + u32(wy) * 16u + lz;
    let word = light_pool[u32(slot) * 16384u + local_idx / 4u];
    let shift = (local_idx % 4u) * 8u;
    return (word >> shift) & 0xFFu;
}

// Matches the CPU's AO-specific `is_solid` closure (chunk.rs:1342), which
// deliberately excludes transparent blocks (leaves, glass) from counting as
// AO occluders — not the plain BlockType::is_solid() method.
fn is_solid_block(b: u32) -> bool {
    let e = atlas_table[b];
    return e.is_solid != 0u && e.is_transparent == 0u;
}

fn should_draw_face(current: u32, neighbor: u32) -> bool {
    if (neighbor == AIR) {
        return true;
    }
    let cur_liquid = (current == WATER || current == LAVA);
    let nbr_liquid = (neighbor == WATER || neighbor == LAVA);
    if (cur_liquid) {
        return neighbor != current;
    }
    if (nbr_liquid) {
        return true;
    }
    if (atlas_table[neighbor].is_transparent != 0u && current != neighbor) {
        return true;
    }
    return false;
}

fn calc_ao(s1: bool, s2: bool, c: bool) -> f32 {
    if (s1 && s2) {
        return 0.4;
    }
    var occ = 0;
    if (s1) { occ = occ + 1; }
    if (s2) { occ = occ + 1; }
    if (c) { occ = occ + 1; }
    if (occ == 0) { return 1.0; }
    if (occ == 1) { return 0.8; }
    if (occ == 2) { return 0.6; }
    return 0.4;
}

fn calc_light_f(light_val: u32) -> f32 {
    let l = f32(light_val);
    return max(pow(0.85, 15.0 - l), 0.1);
}

fn calc_vertex_light(l0: u32, l1: u32, l2: u32, l3: u32) -> f32 {
    let avg = (calc_light_f(l0) + calc_light_f(l1) + calc_light_f(l2) + calc_light_f(l3)) / 4.0;
    return max(avg, 0.1);
}

// Returns light*ao for corners (-1,-1),(1,-1),(1,1),(-1,1) as .x/.y/.z/.w.
fn face_shading(base: vec3<i32>, u_axis: vec3<i32>, v_axis: vec3<i32>) -> vec4<f32> {
    let l_center = read_light(base.x, base.y, base.z);
    return vec4<f32>(
        corner_shade(base, u_axis, v_axis, -1, -1, l_center),
        corner_shade(base, u_axis, v_axis, 1, -1, l_center),
        corner_shade(base, u_axis, v_axis, 1, 1, l_center),
        corner_shade(base, u_axis, v_axis, -1, 1, l_center),
    );
}

fn corner_shade(base: vec3<i32>, u_axis: vec3<i32>, v_axis: vec3<i32>, du: i32, dv: i32, l_center: u32) -> f32 {
    let pu = base + u_axis * du;
    let pv = base + v_axis * dv;
    let puv = base + u_axis * du + v_axis * dv;
    let s1 = is_solid_block(read_block(pu.x, pu.y, pu.z));
    let s2 = is_solid_block(read_block(pv.x, pv.y, pv.z));
    let c = is_solid_block(read_block(puv.x, puv.y, puv.z));
    let ao = calc_ao(s1, s2, c);
    let light = calc_vertex_light(l_center, read_light(pu.x, pu.y, pu.z), read_light(pv.x, pv.y, pv.z), read_light(puv.x, puv.y, puv.z));
    return light * ao;
}

fn reserve_vertices(n: u32) -> u32 {
    return atomicAdd(&vertex_counter, n);
}

fn write_vertex(base_word: u32, pos: vec3<f32>, uv: vec2<f32>, normal: vec3<f32>, color: vec4<f32>) {
    out_vertices[base_word + 0u] = bitcast<u32>(pos.x);
    out_vertices[base_word + 1u] = bitcast<u32>(pos.y + params.y_offset);
    out_vertices[base_word + 2u] = bitcast<u32>(pos.z);
    out_vertices[base_word + 3u] = bitcast<u32>(uv.x);
    out_vertices[base_word + 4u] = bitcast<u32>(uv.y);
    out_vertices[base_word + 5u] = bitcast<u32>(normal.x);
    out_vertices[base_word + 6u] = bitcast<u32>(normal.y);
    out_vertices[base_word + 7u] = bitcast<u32>(normal.z);
    var c = color;
    if (params.debug_tint != 0u) {
        c = vec4<f32>(
            fract(pos.x * 0.3) * 255.0,
            fract(pos.y * 0.3) * 255.0,
            fract(pos.z * 0.3) * 255.0,
            255.0,
        );
    }
    let r = u32(clamp(c.x, 0.0, 255.0));
    let g = u32(clamp(c.y, 0.0, 255.0));
    let b = u32(clamp(c.z, 0.0, 255.0));
    let a = u32(clamp(c.w, 0.0, 255.0));
    out_vertices[base_word + 8u] = r | (g << 8u) | (b << 16u) | (a << 24u);
}

fn emit_quad6(positions: array<vec3<f32>, 6>, uvs: array<vec2<f32>, 6>, normal: vec3<f32>, colors: array<f32, 6>) {
    let v0i = reserve_vertices(6u);
    let bw = v0i * 9u;
    for (var k: u32 = 0u; k < 6u; k = k + 1u) {
        let g = colors[k];
        write_vertex(bw + k * 9u, positions[k], uvs[k], normal, vec4<f32>(g, g, g, 255.0));
    }
}

fn emit_standard_block(wx: i32, wy: i32, wz: i32, block: u32) {
    let entry = atlas_table[block];
    let fx = f32(wx);
    let fy = f32(wy);
    let fz = f32(wz);
    let u0 = f32(entry.default_tx) * TS + PAD;
    let v0 = f32(entry.default_ty) * TS + PAD;
    let u1 = f32(entry.default_tx + 1u) * TS - PAD;
    let v1 = f32(entry.default_ty + 1u) * TS - PAD;
    let tu0 = f32(entry.top_tx) * TS + PAD;
    let tv0 = f32(entry.top_ty) * TS + PAD;
    let tu1 = f32(entry.top_tx + 1u) * TS - PAD;
    let tv1 = f32(entry.top_ty + 1u) * TS - PAD;
    let bt = entry.top_height;

    // Top face
    let neighbor_top = read_block(wx, wy + 1, wz);
    if (should_draw_face(block, neighbor_top)) {
        let l00 = is_solid_block(read_block(wx - 1, wy + 1, wz - 1));
        let l10 = is_solid_block(read_block(wx, wy + 1, wz - 1));
        let l20 = is_solid_block(read_block(wx + 1, wy + 1, wz - 1));
        let l01 = is_solid_block(read_block(wx - 1, wy + 1, wz));
        let l21 = is_solid_block(read_block(wx + 1, wy + 1, wz));
        let l02 = is_solid_block(read_block(wx - 1, wy + 1, wz + 1));
        let l12 = is_solid_block(read_block(wx, wy + 1, wz + 1));
        let l22 = is_solid_block(read_block(wx + 1, wy + 1, wz + 1));

        let ao00 = calc_ao(l01, l10, l00);
        let ao10 = calc_ao(l21, l10, l20);
        let ao01 = calc_ao(l01, l12, l02);
        let ao11 = calc_ao(l21, l12, l22);

        let lb = read_light(wx, wy + 1, wz);
        let ll01 = read_light(wx - 1, wy + 1, wz);
        let ll10 = read_light(wx, wy + 1, wz - 1);
        let ll21 = read_light(wx + 1, wy + 1, wz);
        let ll12 = read_light(wx, wy + 1, wz + 1);

        let light00 = calc_vertex_light(lb, ll01, ll10, read_light(wx - 1, wy + 1, wz - 1));
        let light10 = calc_vertex_light(lb, ll21, ll10, read_light(wx + 1, wy + 1, wz - 1));
        let light01 = calc_vertex_light(lb, ll01, ll12, read_light(wx - 1, wy + 1, wz + 1));
        let light11 = calc_vertex_light(lb, ll21, ll12, read_light(wx + 1, wy + 1, wz + 1));

        let c00 = max(255.0 * light00 * ao00, 35.0);
        let c10 = max(255.0 * light10 * ao10, 35.0);
        let c01 = max(255.0 * light01 * ao01, 35.0);
        let c11 = max(255.0 * light11 * ao11, 35.0);

        emit_quad6(
            array<vec3<f32>, 6>(
                vec3<f32>(fx, fy + bt, fz), vec3<f32>(fx, fy + bt, fz + 1.0), vec3<f32>(fx + 1.0, fy + bt, fz + 1.0),
                vec3<f32>(fx, fy + bt, fz), vec3<f32>(fx + 1.0, fy + bt, fz + 1.0), vec3<f32>(fx + 1.0, fy + bt, fz),
            ),
            array<vec2<f32>, 6>(
                vec2<f32>(tu0, tv0), vec2<f32>(tu0, tv1), vec2<f32>(tu1, tv1),
                vec2<f32>(tu0, tv0), vec2<f32>(tu1, tv1), vec2<f32>(tu1, tv0),
            ),
            vec3<f32>(0.0, 1.0, 0.0),
            array<f32, 6>(c00, c01, c11, c00, c11, c10),
        );
    }

    // Bottom face — y==0 uses a synthetic Bedrock neighbor (chunk.rs:1888-1892),
    // not a general property of read_block, so it's special-cased here only.
    var neighbor_bottom: u32;
    if (wy > 0) {
        neighbor_bottom = read_block(wx, wy - 1, wz);
    } else {
        neighbor_bottom = BEDROCK;
    }
    if (should_draw_face(block, neighbor_bottom)) {
        let s = face_shading(vec3<i32>(wx, wy - 1, wz), vec3<i32>(1, 0, 0), vec3<i32>(0, 0, 1));
        let sv = array<f32, 4>(s.x, s.y, s.z, s.w);
        let g0 = max(255.0 * sv[0] * 0.5, 35.0);
        let g1 = max(255.0 * sv[1] * 0.5, 35.0);
        let g2 = max(255.0 * sv[2] * 0.5, 35.0);
        let g3 = max(255.0 * sv[3] * 0.5, 35.0);
        emit_quad6(
            array<vec3<f32>, 6>(
                vec3<f32>(fx, fy, fz), vec3<f32>(fx + 1.0, fy, fz + 1.0), vec3<f32>(fx, fy, fz + 1.0),
                vec3<f32>(fx, fy, fz), vec3<f32>(fx + 1.0, fy, fz), vec3<f32>(fx + 1.0, fy, fz + 1.0),
            ),
            array<vec2<f32>, 6>(
                vec2<f32>(u0, v0), vec2<f32>(u1, v1), vec2<f32>(u0, v1),
                vec2<f32>(u0, v0), vec2<f32>(u1, v0), vec2<f32>(u1, v1),
            ),
            vec3<f32>(0.0, -1.0, 0.0),
            array<f32, 6>(g0, g2, g3, g0, g1, g2),
        );
    }

    // Sides: Z+, Z-, X+, X-
    for (var i: u32 = 0u; i < 4u; i = i + 1u) {
        var neighbor: u32;
        var norm: vec3<f32>;
        var normi: vec3<i32>;
        var s_mul: f32;
        if (i == 0u) {
            neighbor = read_block(wx, wy, wz + 1);
            norm = vec3<f32>(0.0, 0.0, 1.0);
            normi = vec3<i32>(0, 0, 1);
            s_mul = 0.6;
        } else if (i == 1u) {
            neighbor = read_block(wx, wy, wz - 1);
            norm = vec3<f32>(0.0, 0.0, -1.0);
            normi = vec3<i32>(0, 0, -1);
            s_mul = 0.6;
        } else if (i == 2u) {
            neighbor = read_block(wx + 1, wy, wz);
            norm = vec3<f32>(1.0, 0.0, 0.0);
            normi = vec3<i32>(1, 0, 0);
            s_mul = 0.8;
        } else {
            neighbor = read_block(wx - 1, wy, wz);
            norm = vec3<f32>(-1.0, 0.0, 0.0);
            normi = vec3<i32>(-1, 0, 0);
            s_mul = 0.8;
        }
        if (!should_draw_face(block, neighbor)) {
            continue;
        }

        var positions: array<vec3<f32>, 6>;
        if (i == 0u) {
            positions = array<vec3<f32>, 6>(
                vec3<f32>(fx, fy, fz + 1.0), vec3<f32>(fx + 1.0, fy, fz + 1.0), vec3<f32>(fx + 1.0, fy + bt, fz + 1.0),
                vec3<f32>(fx, fy, fz + 1.0), vec3<f32>(fx + 1.0, fy + bt, fz + 1.0), vec3<f32>(fx, fy + bt, fz + 1.0),
            );
        } else if (i == 1u) {
            positions = array<vec3<f32>, 6>(
                vec3<f32>(fx + 1.0, fy, fz), vec3<f32>(fx, fy, fz), vec3<f32>(fx, fy + bt, fz),
                vec3<f32>(fx + 1.0, fy, fz), vec3<f32>(fx, fy + bt, fz), vec3<f32>(fx + 1.0, fy + bt, fz),
            );
        } else if (i == 2u) {
            positions = array<vec3<f32>, 6>(
                vec3<f32>(fx + 1.0, fy, fz + 1.0), vec3<f32>(fx + 1.0, fy, fz), vec3<f32>(fx + 1.0, fy + bt, fz),
                vec3<f32>(fx + 1.0, fy, fz + 1.0), vec3<f32>(fx + 1.0, fy + bt, fz), vec3<f32>(fx + 1.0, fy + bt, fz + 1.0),
            );
        } else {
            positions = array<vec3<f32>, 6>(
                vec3<f32>(fx, fy, fz), vec3<f32>(fx, fy, fz + 1.0), vec3<f32>(fx, fy + bt, fz + 1.0),
                vec3<f32>(fx, fy, fz), vec3<f32>(fx, fy + bt, fz + 1.0), vec3<f32>(fx, fy + bt, fz),
            );
        }
        let uvs = array<vec2<f32>, 6>(
            vec2<f32>(u0, v1), vec2<f32>(u1, v1), vec2<f32>(u1, v0),
            vec2<f32>(u0, v1), vec2<f32>(u1, v0), vec2<f32>(u0, v0),
        );

        let base = vec3<i32>(wx + normi.x, wy, wz + normi.z);
        var u_axis: vec3<i32>;
        if (i < 2u) {
            u_axis = vec3<i32>(1, 0, 0);
        } else {
            u_axis = vec3<i32>(0, 0, 1);
        }
        let s = face_shading(base, u_axis, vec3<i32>(0, 1, 0));
        let sv = array<f32, 4>(s.x, s.y, s.z, s.w);
        var colors: array<f32, 6>;
        if (i == 0u || i == 3u) {
            colors = array<f32, 6>(
                max(255.0 * sv[0] * s_mul, 35.0), max(255.0 * sv[1] * s_mul, 35.0), max(255.0 * sv[2] * s_mul, 35.0),
                max(255.0 * sv[0] * s_mul, 35.0), max(255.0 * sv[2] * s_mul, 35.0), max(255.0 * sv[3] * s_mul, 35.0),
            );
        } else {
            colors = array<f32, 6>(
                max(255.0 * sv[1] * s_mul, 35.0), max(255.0 * sv[0] * s_mul, 35.0), max(255.0 * sv[3] * s_mul, 35.0),
                max(255.0 * sv[1] * s_mul, 35.0), max(255.0 * sv[3] * s_mul, 35.0), max(255.0 * sv[2] * s_mul, 35.0),
            );
        }
        emit_quad6(positions, uvs, norm, colors);
    }
}

@compute @workgroup_size(16, 1, 16)
fn cs_main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let x = i32(gid.x);
    let z = i32(gid.z);
    if (x >= 16 || z >= 16) {
        return;
    }
    let base_wx = params.chunk_x * 16;
    let base_wz = params.chunk_z * 16;
    let wx = base_wx + x;
    let wz = base_wz + z;
    for (var y: i32 = 0; y < 256; y = y + 1) {
        let block = read_block(wx, y, wz);
        if (block == AIR || block == WATER || block == LAVA || block == WHEAT) {
            continue;
        }
        emit_standard_block(wx, y, wz, block);
    }
}
