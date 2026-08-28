use crate::block::BlockType;
use crate::noise::perlin_2d;
use crate::renderer::Mesh;
use std::collections::VecDeque;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    Plains,
    Desert,
    SnowyTundra,
    SnowyTaiga,
    Mountains,
    HighHills,
}

fn mix64(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

fn seed_offset(seed: u64, salt: u64) -> f32 {
    let bucket = mix64(seed ^ salt) % 1_000_000;
    bucket as f32 / 10.0 - 50_000.0
}

fn seeded_perlin_2d(
    world_x: f32,
    world_z: f32,
    scale: f32,
    octaves: usize,
    seed: u64,
    salt: u64,
) -> f32 {
    perlin_2d(
        world_x + seed_offset(seed, salt),
        world_z + seed_offset(seed, salt ^ 0xA5A5_A5A5_A5A5_A5A5),
        scale,
        octaves,
    )
}

fn get_biome_seeded(world_x: f32, world_z: f32, seed: u64) -> Biome {
    let temperature = seeded_perlin_2d(world_x, world_z, 0.002, 2, seed, 10);
    let humidity = seeded_perlin_2d(world_x, world_z, 0.002, 2, seed, 20);
    let mountain_noise = seeded_perlin_2d(world_x, world_z, 0.001, 3, seed, 30);
    let hills_noise = seeded_perlin_2d(world_x, world_z, 0.005, 2, seed, 40);

    if mountain_noise > 0.45 {
        Biome::Mountains
    } else if hills_noise > 0.35 {
        Biome::HighHills
    } else if temperature < -0.15 {
        if humidity > 0.0 {
            Biome::SnowyTaiga
        } else {
            Biome::SnowyTundra
        }
    } else if temperature > 0.15 {
        Biome::Desert
    } else {
        Biome::Plains
    }
}

pub fn biome_at(world_x: f32, world_z: f32, seed: u64) -> Biome {
    get_biome_seeded(world_x, world_z, seed)
}

fn smoothstep(edge0: f32, edge1: f32, value: f32) -> f32 {
    let t = ((value - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

pub(crate) fn chunk_hash(seed: u64, x: i32, z: i32, salt: i32) -> u32 {
    let mixed = mix64(
        seed ^ (x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (z as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ (salt as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB),
    );
    (mixed ^ (mixed >> 32)) as u32
}

pub(crate) fn hash_unit(seed: u64, x: i32, z: i32, salt: i32) -> f32 {
    chunk_hash(seed, x, z, salt) as f32 / u32::MAX as f32
}

pub(crate) struct ChunkRng {
    state: u64,
}

impl ChunkRng {
    pub(crate) fn new(seed: u64, x: i32, z: i32, salt: u64) -> Self {
        let state = mix64(
            seed ^ (x as i64 as u64).wrapping_mul(0xD1B5_4A32_D192_ED03)
                ^ (z as i64 as u64).wrapping_mul(0xABC9_83DB_5F35_53B5)
                ^ salt,
        );
        Self { state }
    }

    pub(crate) fn next_u32(&mut self) -> u32 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        (mix64(self.state) >> 32) as u32
    }

    fn range_usize(&mut self, start: usize, end: usize) -> usize {
        debug_assert!(start < end);
        start + (self.next_u32() as usize % (end - start))
    }

    fn range_i32(&mut self, start: i32, end: i32) -> i32 {
        debug_assert!(start < end);
        start + (self.next_u32() % (end - start) as u32) as i32
    }

    fn range_i32_inclusive(&mut self, start: i32, end: i32) -> i32 {
        debug_assert!(start <= end);
        start + (self.next_u32() % (end - start + 1) as u32) as i32
    }

    pub(crate) fn next_f64(&mut self) -> f64 {
        self.next_u32() as f64 / 4294967296.0
    }

    pub(crate) fn range_f32(&mut self, start: f32, end: f32) -> f32 {
        start + (end - start) * (self.next_u32() as f32 / 4294967296.0)
    }
}

pub const CHUNK_WIDTH: usize = 16;
pub const CHUNK_HEIGHT: usize = 256;
pub const CHUNK_DEPTH: usize = 16;
pub const WATER_VERTEX_ALPHA: u8 = 180;
const SEA_LEVEL: usize = 124;
const MAX_TERRAIN_HEIGHT: i32 = CHUNK_HEIGHT as i32 - 18;

pub(crate) fn terrain_height_and_biome(world_x: f32, world_z: f32, seed: u64) -> (usize, Biome) {
    let biome = get_biome_seeded(world_x, world_z, seed);
    let continental = seeded_perlin_2d(world_x, world_z, 0.005, 3, seed, 100);
    let detail = seeded_perlin_2d(world_x, world_z, 0.03, 4, seed, 110);
    let mountain_noise = seeded_perlin_2d(world_x, world_z, 0.001, 3, seed, 30);
    let hills_noise = seeded_perlin_2d(world_x, world_z, 0.005, 2, seed, 40);
    let ridged = (-seeded_perlin_2d(world_x, world_z, 0.01, 3, seed, 120).abs() + 1.0).powi(2);

    let mountain_factor = smoothstep(0.34, 0.58, mountain_noise);
    let hill_factor = smoothstep(0.25, 0.48, hills_noise) * (1.0 - mountain_factor * 0.55);
    let base_h = continental * (40.0 + mountain_factor * 8.0)
        + 128.0
        + hill_factor * 7.0
        + mountain_factor * (15.0 + ridged * 42.0);
    let height_scale = 15.0 + hill_factor * 8.0 + mountain_factor * 12.0;
    let mut height = base_h + detail * height_scale;

    let rough_factor = mountain_factor.max(hill_factor);
    if rough_factor > 0.1 && continental < 0.1 {
        let fjord_noise = seeded_perlin_2d(world_x, world_z, 0.02, 2, seed, 130).abs();
        if fjord_noise < 0.15 {
            let depth_factor = (0.15 - fjord_noise) / 0.15 * rough_factor;
            height = height * (1.0 - depth_factor) + 110.0 * depth_factor;
        }
    }

    ((height as i32).clamp(1, MAX_TERRAIN_HEIGHT) as usize, biome)
}

// MC 1.0 water level encoding:
// 0 = no water
// 1 = source (MC metadata 0) — permanent, strongest
// 2..=8 = flowing (MC metadata 1..7) — higher = weaker
// 9..=16 = falling water (same decay levels but falling)
pub const WATER_SOURCE: u8 = 1;
pub const WATER_FALL_BIT: u8 = 8;

pub fn water_is_source(level: u8) -> bool {
    level == WATER_SOURCE
}
pub fn water_is_falling(level: u8) -> bool {
    (9..=16).contains(&level)
}
pub fn water_decay(level: u8) -> u8 {
    if level == 0 { 0 } else { (level - 1) & 7 }
}
#[allow(dead_code)]
pub fn water_height(level: u8) -> f32 {
    if level == 0 {
        return 0.0;
    }
    if level > WATER_FALL_BIT {
        return 1.0;
    } // falling
    let decay = water_decay(level);
    (8 - decay) as f32 / 9.0
}
// Height used for meshing: returns 1.0 for source blocks to eliminate gap artifacts
pub fn water_render_height(level: u8) -> f32 {
    if level == 0 {
        return 0.0;
    }
    if level > WATER_FALL_BIT || level == WATER_SOURCE {
        return 1.0;
    }
    let decay = water_decay(level);
    (8 - decay) as f32 / 9.0
}
pub fn water_level_from_decay(decay: u8, falling: bool) -> u8 {
    if falling {
        WATER_FALL_BIT + decay + 1
    } else {
        decay + 1
    }
}

pub struct MeshData {
    pub v: Vec<f32>,
    pub t: Vec<f32>,
    pub n: Vec<f32>,
    pub c: Vec<u8>,
}

pub type BlockArray = Box<[[[BlockType; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]>;
pub type LightArray = Box<[[[u8; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]>;

// Owned copy of a chunk's voxel data, handed to a meshing worker so the
// live chunk can keep being mutated on the main thread while meshing runs.
pub struct ChunkData {
    pub x: i32,
    pub z: i32,
    pub blocks: BlockArray,
    pub light: LightArray,
    pub liquid_levels: LightArray,
}

// Finished meshing job, sent back to the main thread over a channel.
// job_id ties the result to a specific dispatch: if the chunk was replaced
// and re-dispatched while this job was in flight, a stale result must not
// upload over the newer job's output.
pub struct MeshResult {
    pub x: i32,
    pub z: i32,
    pub job_id: u64,
    pub light: LightArray,
    pub opaque: MeshData,
    pub transparent: MeshData,
    pub water: MeshData,
}

// Snapshot of the chunk plus a 1-block border from its neighbors,
// captured on the main thread. Meshing never reads further than 1 block
// outside the chunk (face culling, AO corners, water corner heights),
// so this is everything a worker needs for cross-chunk queries.
pub const SNAP_W: usize = CHUNK_WIDTH + 2;
pub const SNAP_D: usize = CHUNK_DEPTH + 2;

#[inline]
pub fn pack_light(sky: u8, block: u8) -> u8 {
    ((sky & 0x0F) << 4) | (block & 0x0F)
}

#[inline]
pub fn unpack_light(val: u8) -> (u8, u8) {
    (val >> 4, val & 0x0F)
}

/// Converts a 0-15 sky or block light level to the 0-1 factor the mesher
/// writes into vertex colour. Entities sample the same curve so they match
/// the terrain they stand on.
#[inline]
pub fn light_factor(level: u8) -> f32 {
    if level == 0 {
        0.0
    } else {
        0.85f32.powf(15.0 - level as f32)
    }
}

// Skylight + BFS light propagation over a chunk's own blocks. Shared by the
// live chunk (generation) and by worker-owned copies (background meshing).
fn compute_lighting(
    blocks: &[[[BlockType; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH],
    light: &mut [[[u8; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH],
) {
    let mut sky = [[[0u8; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH];
    let mut block_light = [[[0u8; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH];

    // PASS 1: Skylight top-down
    for x in 0..CHUNK_WIDTH {
        for z in 0..CHUNK_DEPTH {
            let mut sunlight: u8 = 15;
            for y in (0..CHUNK_HEIGHT).rev() {
                let b = blocks[x][y][z];
                if b == BlockType::Air
                    || b == BlockType::Water
                    || b == BlockType::Lava
                    || b.is_transparent()
                {
                    sky[x][y][z] = sunlight;
                } else {
                    sky[x][y][z] = 0;
                    sunlight = 0;
                }
            }
        }
    }

    // Horizontal Sky Light Propagation
    let mut sky_queue = VecDeque::new();
    for x in 0..CHUNK_WIDTH {
        for y in 0..CHUNK_HEIGHT {
            for z in 0..CHUNK_DEPTH {
                if sky[x][y][z] > 1 {
                    sky_queue.push_back((x, y, z));
                }
            }
        }
    }

    while let Some((x, y, z)) = sky_queue.pop_front() {
        let current_light = sky[x][y][z];
        let drop_light = current_light.saturating_sub(1);
        if drop_light == 0 {
            continue;
        }

        let neighbors = [
            (x as i32 + 1, y as i32, z as i32),
            (x as i32 - 1, y as i32, z as i32),
            (x as i32, y as i32 + 1, z as i32),
            (x as i32, y as i32 - 1, z as i32),
            (x as i32, y as i32, z as i32 + 1),
            (x as i32, y as i32, z as i32 - 1),
        ];

        for (nx, ny, nz) in neighbors {
            if nx >= 0
                && nx < CHUNK_WIDTH as i32
                && ny >= 0
                && ny < CHUNK_HEIGHT as i32
                && nz >= 0
                && nz < CHUNK_DEPTH as i32
            {
                let (ux, uy, uz) = (nx as usize, ny as usize, nz as usize);
                let nb = blocks[ux][uy][uz];
                let occluding = nb.is_solid() && !nb.is_transparent();

                if !occluding && sky[ux][uy][uz] < drop_light {
                    sky[ux][uy][uz] = drop_light;
                    sky_queue.push_back((ux, uy, uz));
                }
            }
        }
    }

    // PASS 2: Block light propagation
    let mut block_queue = VecDeque::new();
    for x in 0..CHUNK_WIDTH {
        for y in 0..CHUNK_HEIGHT {
            for z in 0..CHUNK_DEPTH {
                let emitted = match blocks[x][y][z] {
                    BlockType::Torch => 14,
                    BlockType::RedstoneTorch => 7,
                    BlockType::Lava => 15,
                    _ => 0,
                };
                if emitted > 0 {
                    block_light[x][y][z] = emitted;
                    block_queue.push_back((x, y, z));
                }
            }
        }
    }

    while let Some((x, y, z)) = block_queue.pop_front() {
        let current_light = block_light[x][y][z];
        let drop_light = current_light.saturating_sub(1);
        if drop_light == 0 {
            continue;
        }

        let neighbors = [
            (x as i32 + 1, y as i32, z as i32),
            (x as i32 - 1, y as i32, z as i32),
            (x as i32, y as i32 + 1, z as i32),
            (x as i32, y as i32 - 1, z as i32),
            (x as i32, y as i32, z as i32 + 1),
            (x as i32, y as i32, z as i32 - 1),
        ];

        for (nx, ny, nz) in neighbors {
            if nx >= 0
                && nx < CHUNK_WIDTH as i32
                && ny >= 0
                && ny < CHUNK_HEIGHT as i32
                && nz >= 0
                && nz < CHUNK_DEPTH as i32
            {
                let (ux, uy, uz) = (nx as usize, ny as usize, nz as usize);
                let nb = blocks[ux][uy][uz];
                let occluding = nb.is_solid() && !nb.is_transparent();

                if !occluding && block_light[ux][uy][uz] < drop_light {
                    block_light[ux][uy][uz] = drop_light;
                    block_queue.push_back((ux, uy, uz));
                }
            }
        }
    }

    // Combine into light array
    for x in 0..CHUNK_WIDTH {
        for y in 0..CHUNK_HEIGHT {
            for z in 0..CHUNK_DEPTH {
                light[x][y][z] = pack_light(sky[x][y][z], block_light[x][y][z]);
            }
        }
    }
}

pub struct MeshSnapshot {
    pub x: i32,
    pub z: i32,
    // Flat, x-major/y-mid/z-minor buffers (see `flat_idx`) instead of nested
    // fixed-size arrays. `Box::new([[[T; SNAP_D]; CHUNK_HEIGHT]; SNAP_W])`
    // measured at ~1.5ms/chunk on its own (see capture-probe telemetry from
    // the flying-forward stutter investigation): the compiler doesn't turn a
    // ~83k-element nested-array literal into a heap-direct memset, so it
    // gets materialized somewhere first and then copied into the Box. A
    // flat `vec![default; N].into_boxed_slice()` allocates and fills the
    // heap buffer directly (LLVM lowers a uniform fill to memset), which is
    // the actual majority of `capture`'s per-chunk cost, dwarfing the
    // neighbor-copy loop below it.
    blocks: Box<[BlockType]>,
    light: Box<[u8]>,
    liquid: Box<[u8]>,
}

const SNAP_LEN: usize = SNAP_W * CHUNK_HEIGHT * SNAP_D;

#[inline]
fn flat_idx(sx: usize, y: usize, sz: usize) -> usize {
    (sx * CHUNK_HEIGHT + y) * SNAP_D + sz
}

impl MeshSnapshot {
    pub fn capture(world: &crate::world::World, cx: i32, cz: i32) -> Self {
        let mut neighbors: [[Option<&Chunk>; 3]; 3] = [[None; 3]; 3];
        for (dx, row) in neighbors.iter_mut().enumerate() {
            for (dz, slot) in row.iter_mut().enumerate() {
                *slot = world.get_chunk(cx + dx as i32 - 1, cz + dz as i32 - 1);
            }
        }
        Self::build(cx, cz, neighbors, |wx, wy, wz| world.get_block(wx, wy, wz))
    }

    /// Assembles a snapshot from an already-resolved 3x3 neighbor grid (indices
    /// 0..3 map to world-chunk offsets -1..1 on each axis) plus a fallback for
    /// blocks outside any loaded neighbor. Split out from `capture` so the
    /// indexing logic can be unit tested without a live `World`.
    fn build(
        cx: i32,
        cz: i32,
        neighbors: [[Option<&Chunk>; 3]; 3],
        fallback_block: impl Fn(i32, i32, i32) -> BlockType,
    ) -> Self {
        let mut snap = Self {
            x: cx,
            z: cz,
            blocks: vec![BlockType::Air; SNAP_LEN].into_boxed_slice(),
            light: vec![pack_light(15, 0); SNAP_LEN].into_boxed_slice(),
            liquid: vec![0u8; SNAP_LEN].into_boxed_slice(),
        };

        // Fast path: the dz=0 neighbor row (west/center/east) contributes a
        // full 16-wide run of contiguous z values at every (bx, y), matching
        // storage's z-innermost layout, so it can be bulk-copied with slice
        // copies instead of per-element assignment. This is the dominant
        // share of the snapshot (the center chunk alone is ~79% of it).
        for (dxn, row) in neighbors.iter().enumerate() {
            let (sx_start, bx_range) = match dxn {
                0 => (0usize, (CHUNK_WIDTH - 1)..CHUNK_WIDTH),
                1 => (1usize, 0..CHUNK_WIDTH),
                _ => (SNAP_W - 1, 0..1),
            };
            match row[1] {
                Some(c) => {
                    for (i, bx) in bx_range.clone().enumerate() {
                        let sx = sx_start + i;
                        for y in 0..CHUNK_HEIGHT {
                            let base = flat_idx(sx, y, 1);
                            snap.blocks[base..base + CHUNK_DEPTH].copy_from_slice(&c.blocks[bx][y]);
                            snap.light[base..base + CHUNK_DEPTH].copy_from_slice(&c.light[bx][y]);
                            snap.liquid[base..base + CHUNK_DEPTH]
                                .copy_from_slice(&c.liquid_levels[bx][y]);
                        }
                    }
                }
                None => {
                    for (i, _bx) in bx_range.clone().enumerate() {
                        let sx = sx_start + i;
                        let lx = sx as i32 - 1;
                        let wx = cx * CHUNK_WIDTH as i32 + lx;
                        for sz in 1..(1 + CHUNK_DEPTH) {
                            let lz = sz as i32 - 1;
                            let wz = cz * CHUNK_DEPTH as i32 + lz;
                            for y in 0..CHUNK_HEIGHT {
                                let block = fallback_block(wx, y as i32, wz);
                                let idx = flat_idx(sx, y, sz);
                                snap.blocks[idx] = block;
                                snap.liquid[idx] =
                                    if block == BlockType::Water || block == BlockType::Lava {
                                        WATER_SOURCE
                                    } else {
                                        0
                                    };
                            }
                        }
                    }
                }
            }
        }

        // Remaining cells: the two z-border rows (sz=0 and sz=SNAP_D-1,
        // including corners). A fixed bz makes the source non-contiguous
        // across bx here, so these stay per-element like the original loop.
        for sz in [0usize, SNAP_D - 1] {
            let lz = sz as i32 - 1;
            let dz = lz.div_euclid(CHUNK_DEPTH as i32);
            let bz = lz.rem_euclid(CHUNK_DEPTH as i32) as usize;
            for sx in 0..SNAP_W {
                let lx = sx as i32 - 1;
                let dx = lx.div_euclid(CHUNK_WIDTH as i32);
                let bx = lx.rem_euclid(CHUNK_WIDTH as i32) as usize;
                match neighbors[(dx + 1) as usize][(dz + 1) as usize] {
                    Some(c) => {
                        for y in 0..CHUNK_HEIGHT {
                            let idx = flat_idx(sx, y, sz);
                            snap.blocks[idx] = c.blocks[bx][y][bz];
                            snap.light[idx] = c.light[bx][y][bz];
                            snap.liquid[idx] = c.liquid_levels[bx][y][bz];
                        }
                    }
                    None => {
                        // Unloaded neighbor: fall back to the edits map,
                        // matching what World::get_block would return. Edited
                        // liquids get source level, as Chunk::set_block would.
                        let wx = cx * CHUNK_WIDTH as i32 + lx;
                        let wz = cz * CHUNK_DEPTH as i32 + lz;
                        for y in 0..CHUNK_HEIGHT {
                            let block = fallback_block(wx, y as i32, wz);
                            let idx = flat_idx(sx, y, sz);
                            snap.blocks[idx] = block;
                            snap.liquid[idx] =
                                if block == BlockType::Water || block == BlockType::Lava {
                                    WATER_SOURCE
                                } else {
                                    0
                                };
                        }
                    }
                }
            }
        }
        snap
    }

    #[inline]
    fn local(&self, wx: i32, wz: i32) -> Option<(usize, usize)> {
        let lx = wx - self.x * CHUNK_WIDTH as i32 + 1;
        let lz = wz - self.z * CHUNK_DEPTH as i32 + 1;
        if lx < 0 || lx >= SNAP_W as i32 || lz < 0 || lz >= SNAP_D as i32 {
            None
        } else {
            Some((lx as usize, lz as usize))
        }
    }

    pub fn get_block(&self, wx: i32, wy: i32, wz: i32) -> BlockType {
        if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
            return BlockType::Air;
        }
        match self.local(wx, wz) {
            Some((lx, lz)) => self.blocks[flat_idx(lx, wy as usize, lz)],
            None => BlockType::Air,
        }
    }

    pub fn get_light(&self, wx: i32, wy: i32, wz: i32) -> u8 {
        if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
            return pack_light(15, 0);
        }
        match self.local(wx, wz) {
            Some((lx, lz)) => self.light[flat_idx(lx, wy as usize, lz)],
            None => pack_light(15, 0),
        }
    }

    pub fn get_liquid_level(&self, wx: i32, wy: i32, wz: i32) -> u8 {
        if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
            return 0;
        }
        match self.local(wx, wz) {
            Some((lx, lz)) => self.liquid[flat_idx(lx, wy as usize, lz)],
            None => 0,
        }
    }
}

pub struct Chunk {
    pub blocks: Box<[[[BlockType; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]>,
    pub light: Box<[[[u8; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]>,
    pub liquid_levels: Box<[[[u8; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]>,
    pub mesh_opaque: Option<Mesh>,
    pub mesh_transparent: Option<Mesh>,
    pub mesh_water: Option<Mesh>,
    pub dirty: bool,
    pub meshing_in_progress: bool,
    // Identity of the most recent mesh dispatch for this chunk; results
    // carrying a different id are stale and must be dropped.
    pub mesh_job_id: u64,
    pub x: i32,
    pub z: i32,
    pub seed: u64,
}

impl Chunk {
    pub fn new(x: i32, z: i32, seed: u64) -> Self {
        Self {
            blocks: Box::new([[[BlockType::Air; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]),
            light: Box::new([[[0; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]),
            liquid_levels: Box::new([[[0; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]),
            mesh_opaque: None,
            mesh_transparent: None,
            mesh_water: None,
            dirty: true,
            meshing_in_progress: false,
            mesh_job_id: 0,
            x,
            z,
            seed,
        }
    }

    pub fn copy_terrain_from(&mut self, other: &Chunk) {
        self.blocks = other.blocks.clone();
        self.light = other.light.clone();
        self.liquid_levels = other.liquid_levels.clone();
        self.x = other.x;
        self.z = other.z;
        self.dirty = true;
        self.meshing_in_progress = false;
        self.mesh_job_id = 0;
        self.mesh_opaque = None;
        self.mesh_transparent = None;
        self.mesh_water = None;
    }

    pub fn hydrate_fluids(&mut self) {
        for x in 0..CHUNK_WIDTH {
            for y in 0..CHUNK_HEIGHT {
                for z in 0..CHUNK_DEPTH {
                    let block = self.blocks[x][y][z];
                    if matches!(block, BlockType::Water | BlockType::Lava)
                        && self.liquid_levels[x][y][z] == 0
                    {
                        self.liquid_levels[x][y][z] = WATER_SOURCE;
                    }
                }
            }
        }
    }

    /// Flat byte views of blocks/light/liquid, in `x*4096 + y*16 + z` order,
    /// for uploading to the GPU voxel pool (see hashed-splashing-haven plan).
    pub fn gpu_bytes(&self) -> (&[u8], &[u8], &[u8]) {
        const N: usize = CHUNK_WIDTH * CHUNK_HEIGHT * CHUNK_DEPTH;
        // SAFETY: BlockType is #[repr(u8)] and Copy; light/liquid_levels are
        // already u8. All three are fixed-size nested arrays, contiguous in
        // memory, so a flat byte view over the whole array is valid.
        let blocks = unsafe { std::slice::from_raw_parts(self.blocks.as_ptr() as *const u8, N) };
        let light = unsafe { std::slice::from_raw_parts(self.light.as_ptr() as *const u8, N) };
        let liquid =
            unsafe { std::slice::from_raw_parts(self.liquid_levels.as_ptr() as *const u8, N) };
        (blocks, light, liquid)
    }

    pub(crate) fn surface_y(&self, x: usize, z: usize) -> Option<usize> {
        for y in (1..CHUNK_HEIGHT - 2).rev() {
            let b = self.blocks[x][y][z];
            if b.is_solid() && b != BlockType::SnowLayer {
                return Some(y);
            }
        }
        None
    }

    fn set_local(&mut self, x: i32, y: i32, z: i32, block: BlockType) {
        if x >= 0
            && x < CHUNK_WIDTH as i32
            && y >= 0
            && y < CHUNK_HEIGHT as i32
            && z >= 0
            && z < CHUNK_DEPTH as i32
        {
            let (ux, uy, uz) = (x as usize, y as usize, z as usize);
            self.blocks[ux][uy][uz] = block;
            self.liquid_levels[ux][uy][uz] =
                if block == BlockType::Water || block == BlockType::Lava {
                    WATER_SOURCE
                } else {
                    0
                };
        }
    }

    fn generate_lava_pockets(&mut self) {
        if hash_unit(self.seed, self.x, self.z, 5001) > 0.38 {
            return;
        }

        let cx = 4 + (chunk_hash(self.seed, self.x, self.z, 5002) % 8) as i32;
        let cy = 8 + (chunk_hash(self.seed, self.x, self.z, 5003) % 24) as i32;
        let cz = 4 + (chunk_hash(self.seed, self.x, self.z, 5004) % 8) as i32;
        let radius = 2 + (chunk_hash(self.seed, self.x, self.z, 5005) % 2) as i32;
        let r2 = radius * radius;

        for dx in -radius..=radius {
            for dy in -2i32..=2 {
                for dz in -radius..=radius {
                    let dist = dx * dx + dz * dz + dy.abs() * 2;
                    if dist > r2 + 1 {
                        continue;
                    }
                    let block = if dy <= 0 {
                        BlockType::Lava
                    } else {
                        BlockType::Air
                    };
                    self.set_local(cx + dx, cy + dy, cz + dz, block);
                }
            }
        }
    }

    fn generate_dungeon_room(&mut self) {
        if hash_unit(self.seed, self.x, self.z, 6101) > 0.10 {
            return;
        }

        let cx = 5 + (chunk_hash(self.seed, self.x, self.z, 6102) % 6) as i32;
        let cy = 18 + (chunk_hash(self.seed, self.x, self.z, 6103) % 42) as i32;
        let cz = 5 + (chunk_hash(self.seed, self.x, self.z, 6104) % 6) as i32;

        for dx in -4i32..=4 {
            for dy in 0..=4 {
                for dz in -4i32..=4 {
                    let wall = dx.abs() == 4 || dz.abs() == 4 || dy == 0 || dy == 4;
                    let block = if wall {
                        if (chunk_hash(self.seed, self.x + dx, self.z + dz, 6110 + dy) & 1) == 0 {
                            BlockType::MossyCobblestone
                        } else {
                            BlockType::Cobblestone
                        }
                    } else {
                        BlockType::Air
                    };
                    self.set_local(cx + dx, cy + dy, cz + dz, block);
                }
            }
        }

        self.set_local(cx, cy + 1, cz, BlockType::MobSpawner);
        self.set_local(cx - 2, cy + 1, cz, BlockType::Chest);
        self.set_local(cx + 2, cy + 1, cz, BlockType::Chest);
        self.set_local(cx, cy + 1, cz - 3, BlockType::Torch);
        self.set_local(cx, cy + 1, cz + 3, BlockType::Torch);
    }

    fn generate_biome_decorations(&mut self) {
        for x in 1..CHUNK_WIDTH - 1 {
            for z in 1..CHUNK_DEPTH - 1 {
                let world_x = self.x * CHUNK_WIDTH as i32 + x as i32;
                let world_z = self.z * CHUNK_DEPTH as i32 + z as i32;
                let biome = get_biome_seeded(world_x as f32, world_z as f32, self.seed);
                let Some(surface_y) = self.surface_y(x, z) else {
                    continue;
                };

                if biome == Biome::Desert
                    && self.blocks[x][surface_y][z] == BlockType::Sand
                    && self.blocks[x][surface_y + 1][z] == BlockType::Air
                    && hash_unit(self.seed, world_x, world_z, 8101) < 0.018
                {
                    let height = 2 + (chunk_hash(self.seed, world_x, world_z, 8102) % 2) as usize;
                    for h in 1..=height {
                        if surface_y + h < CHUNK_HEIGHT
                            && self.blocks[x][surface_y + h][z] == BlockType::Air
                        {
                            self.blocks[x][surface_y + h][z] = BlockType::Cactus;
                        }
                    }
                }

                let near_water = [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .any(|(dx, dz)| {
                        let nx = (x as i32 + dx) as usize;
                        let nz = (z as i32 + dz) as usize;
                        self.blocks[nx][surface_y + 1][nz] == BlockType::Water
                            || self.blocks[nx][surface_y][nz] == BlockType::Water
                    });
                if near_water
                    && (116..=126).contains(&surface_y)
                    && self.blocks[x][surface_y][z] == BlockType::Sand
                    && hash_unit(self.seed, world_x, world_z, 8201) < 0.35
                {
                    self.blocks[x][surface_y][z] = BlockType::Clay;
                }
            }
        }
    }

    pub fn generate(&mut self) {
        let mut rng = ChunkRng::new(self.seed, self.x, self.z, 1001);

        // PASS 1: Terrain (biome-aware)
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_DEPTH {
                let world_x = (self.x * CHUNK_WIDTH as i32 + x as i32) as f32;
                let world_z = (self.z * CHUNK_DEPTH as i32 + z as i32) as f32;
                let (height, biome) = terrain_height_and_biome(world_x, world_z, self.seed);

                for y in 0..CHUNK_HEIGHT {
                    let b = if y == 0 {
                        BlockType::Bedrock
                    } else if y < height.saturating_sub(4) {
                        BlockType::Stone
                    } else if y < height {
                        match biome {
                            Biome::Desert => BlockType::Sand,
                            Biome::Mountains => {
                                if y > 180 {
                                    BlockType::Stone
                                } else {
                                    BlockType::Dirt
                                }
                            }
                            _ => {
                                if height < 126 {
                                    BlockType::Sand
                                } else {
                                    BlockType::Dirt
                                }
                            }
                        }
                    } else if y == height {
                        if height < 126 {
                            BlockType::Sand
                        } else {
                            match biome {
                                Biome::Desert => BlockType::Sand,
                                Biome::SnowyTundra | Biome::SnowyTaiga => BlockType::SnowyGrass,
                                Biome::Mountains => {
                                    if height > 185 {
                                        BlockType::PowderedSnow
                                    } else if height > 160 {
                                        BlockType::Stone
                                    } else {
                                        BlockType::Grass
                                    }
                                }
                                Biome::HighHills => BlockType::Grass,
                                Biome::Plains => BlockType::Grass,
                            }
                        }
                    } else if y < SEA_LEVEL {
                        BlockType::Water
                    } else {
                        BlockType::Air
                    };
                    self.blocks[x][y][z] = b;
                    if b == BlockType::Water {
                        self.liquid_levels[x][y][z] = WATER_SOURCE;
                    }
                }
            }
        }

        // PASS 3: Minecraft 1.0 Authentic WorldGenMinable Ore Generation
        let ore_specs: [(BlockType, usize, usize, i32, i32); 8] = [
            // (BlockType, count_per_chunk, vein_size, min_y, max_y)
            (BlockType::Dirt, 20, 32, 0, 128),
            (BlockType::Gravel, 10, 32, 0, 128),
            (BlockType::CoalOre, 20, 16, 0, 128),
            (BlockType::IronOre, 20, 8, 0, 64),
            (BlockType::GoldOre, 2, 8, 0, 32),
            (BlockType::RedstoneOre, 8, 7, 0, 16),
            (BlockType::DiamondOre, 1, 7, 1, 15),
            (BlockType::LapisOre, 1, 7, 1, 32),
        ];

        for (block, count, vein_size, min_y, max_y) in ore_specs {
            for _ in 0..count {
                let local_x = rng.range_i32(0, CHUNK_WIDTH as i32);
                let y = rng.range_i32(min_y, max_y);
                let local_z = rng.range_i32(0, CHUNK_DEPTH as i32);
                generate_minable_vein(self, block, vein_size, local_x, y, local_z, &mut rng);
            }
        }

        // PASS 4: Gravel
        for _ in 0..8 {
            let x = rng.range_usize(0, CHUNK_WIDTH) as i32;
            let y = rng.range_i32(64, 128);
            let z = rng.range_usize(0, CHUNK_DEPTH) as i32;

            let vein_size = rng.range_i32_inclusive(16, 47);
            let mut current_x = x;
            let mut current_y = y;
            let mut current_z = z;

            for _ in 0..vein_size {
                if current_x >= 0
                    && current_x < CHUNK_WIDTH as i32
                    && current_z >= 0
                    && current_z < CHUNK_DEPTH as i32
                    && current_y >= 0
                    && current_y < CHUNK_HEIGHT as i32
                {
                    let cx = current_x as usize;
                    let cy = current_y as usize;
                    let cz = current_z as usize;
                    let b = self.blocks[cx][cy][cz];
                    if b == BlockType::Stone || b == BlockType::Dirt {
                        self.blocks[cx][cy][cz] = BlockType::Gravel;
                    }
                }

                let dir = rng.range_i32(0, 6);
                match dir {
                    0 => current_x += 1,
                    1 => current_x -= 1,
                    2 => current_y += 1,
                    3 => current_y -= 1,
                    4 => current_z += 1,
                    5 => current_z -= 1,
                    _ => {}
                }
            }
        }

        let cave_rng = |mut seed: u64| -> f32 {
            seed = mix64(seed);
            ((seed >> 40) as f32) / ((1u64 << 24) as f32)
        };

        // Carve caves coming from neighboring chunks to ensure seamless tunnels
        for cx in -1i32..=1 {
            for cz in -1i32..=1 {
                let neighbor_x = self.x + cx;
                let neighbor_z = self.z + cz;

                // Deterministic seed for this chunk
                let base_seed = mix64(
                    self.seed
                        ^ (neighbor_x as i64 as u64).wrapping_mul(341_873_128_712)
                        ^ (neighbor_z as i64 as u64).wrapping_mul(132_897_987_541)
                        ^ 0xC0A5_E51D_5EED_0001,
                );

                let num_caves = ((cave_rng(base_seed) * 11.0) as i32).max(0);

                for i in 0..num_caves {
                    let cave_seed = base_seed.wrapping_add((i * 9283711) as u64);

                    let start_x =
                        (neighbor_x * CHUNK_WIDTH as i32) as f32 + cave_rng(cave_seed) * 16.0;
                    let depth_roll = cave_rng(cave_seed.wrapping_add(1));
                    let start_y = 8.0 + depth_roll * depth_roll * 86.0;
                    let start_z = (neighbor_z * CHUNK_DEPTH as i32) as f32
                        + cave_rng(cave_seed.wrapping_add(2)) * 16.0;

                    let mut length = cave_rng(cave_seed.wrapping_add(3)) * 82.0 + 18.0; // 18 to 100 steps
                    let mut radius = cave_rng(cave_seed.wrapping_add(4)) * 1.35 + 1.0; // 1.0 to 2.35 base radius

                    // Occasional underground rooms, kept deeper so hillside mouths stay small.
                    if start_y < 54.0 && cave_rng(cave_seed.wrapping_add(5)) < 0.10 {
                        radius *= 1.75;
                        length = 0.0; // Rooms don't immediately worm
                    }

                    // Rare wider tunnel starting, but not a surface-breaking cavern.
                    if cave_rng(cave_seed.wrapping_add(6)) < 0.10 {
                        radius *= cave_rng(cave_seed.wrapping_add(7)) * 0.85 + 1.0;
                    }

                    let mut current_x = start_x;
                    let mut current_y = start_y;
                    let mut current_z = start_z;

                    let mut yaw = cave_rng(cave_seed.wrapping_add(8)) * std::f32::consts::PI * 2.0;
                    let mut pitch =
                        (cave_rng(cave_seed.wrapping_add(9)) - 0.5) * std::f32::consts::PI * 0.5;

                    let mut step = 0.0;
                    let mut s = cave_seed.wrapping_add(10);

                    while step < length || length == 0.0 {
                        let progress = if length > 0.0 { step / length } else { 0.5 };
                        let taper = (std::f32::consts::PI * progress).sin();
                        let rad = radius * (0.65 + taper * 0.70);

                        // Carve the sphere at the current point
                        let min_cx =
                            ((current_x - rad).floor() as i32) - (self.x * CHUNK_WIDTH as i32);
                        let max_cx =
                            ((current_x + rad).ceil() as i32) - (self.x * CHUNK_WIDTH as i32);
                        let min_cy = (current_y - rad).floor() as i32;
                        let max_cy = (current_y + rad).ceil() as i32;
                        let min_cz =
                            ((current_z - rad).floor() as i32) - (self.z * CHUNK_DEPTH as i32);
                        let max_cz =
                            ((current_z + rad).ceil() as i32) - (self.z * CHUNK_DEPTH as i32);

                        for bx in min_cx.max(0)..=max_cx.min(CHUNK_WIDTH as i32 - 1) {
                            for by in min_cy.max(0)..=max_cy.min(CHUNK_HEIGHT as i32 - 1) {
                                for bz in min_cz.max(0)..=max_cz.min(CHUNK_DEPTH as i32 - 1) {
                                    let global_x = (self.x * CHUNK_WIDTH as i32 + bx) as f32;
                                    let global_y = by as f32;
                                    let global_z = (self.z * CHUNK_DEPTH as i32 + bz) as f32;

                                    let vertical_scale = if current_y > 64.0 { 1.9 } else { 1.35 };
                                    let dist_sq = (global_x - current_x).powi(2)
                                        + ((global_y - current_y) * vertical_scale).powi(2)
                                        + (global_z - current_z).powi(2);

                                    let edge_noise = seeded_perlin_2d(
                                        global_x + global_y * 3.0,
                                        global_z + global_y * 11.0,
                                        0.18,
                                        2,
                                        self.seed,
                                        9101,
                                    );
                                    let fine_noise = seeded_perlin_2d(
                                        global_x,
                                        global_z + global_y * 5.0,
                                        0.33,
                                        1,
                                        self.seed,
                                        9102,
                                    );
                                    let mut rough_rad = (rad
                                        * (0.86 + edge_noise * 0.18 + fine_noise * 0.08))
                                        .max(0.65);
                                    let ux = bx as usize;
                                    let uz = bz as usize;
                                    let current_block = self.blocks[ux][by as usize][uz];
                                    let surface =
                                        self.surface_y(ux, uz).unwrap_or(CHUNK_HEIGHT - 1);
                                    let depth_below_surface =
                                        (surface as i32 - by).clamp(0, CHUNK_HEIGHT as i32);
                                    let near_surface = global_y > 44.0 && depth_below_surface <= 14;
                                    if near_surface {
                                        rough_rad = rough_rad.min(0.95);
                                    }

                                    if dist_sq <= rough_rad * rough_rad {
                                        if near_surface && depth_below_surface <= 6 {
                                            continue;
                                        }
                                        if near_surface
                                            && !matches!(
                                                current_block,
                                                BlockType::Stone | BlockType::Gravel
                                            )
                                        {
                                            continue;
                                        }
                                        if matches!(
                                            current_block,
                                            BlockType::Stone
                                                | BlockType::Dirt
                                                | BlockType::Gravel
                                                | BlockType::Grass
                                                | BlockType::SnowyGrass
                                                | BlockType::PowderedSnow
                                                | BlockType::SnowLayer
                                        ) {
                                            self.blocks[bx as usize][by as usize][bz as usize] =
                                                BlockType::Air;
                                        }
                                    }
                                }
                            }
                        }

                        if length == 0.0 {
                            break;
                        } // It was just a room

                        // Step the worm forward
                        current_x += yaw.cos() * pitch.cos();
                        current_y += pitch.sin();
                        current_z += yaw.sin() * pitch.cos();

                        s = s.wrapping_add(1);
                        yaw += (cave_rng(s) - 0.5) * 0.4;
                        s = s.wrapping_add(1);
                        pitch += (cave_rng(s) - 0.5) * 0.4;
                        pitch =
                            pitch.clamp(-std::f32::consts::PI * 0.4, std::f32::consts::PI * 0.4);

                        step += 1.0;
                    }
                }
            }
        }

        self.generate_lava_pockets();
        self.generate_dungeon_room();

        // PASS 5: Trees (Moved after carvers to avoid floating)
        let mut tree_positions: Vec<(usize, usize)> = Vec::new();

        let (tree_attempts, min_dist) = match get_biome_seeded(
            (self.x * CHUNK_WIDTH as i32 + 8) as f32,
            (self.z * CHUNK_DEPTH as i32 + 8) as f32,
            self.seed,
        ) {
            Biome::Plains | Biome::HighHills => (2, 4),
            Biome::SnowyTaiga => (8, 5), // Reduced attempts and increased distance to prevent bunching
            _ => (0, 0),
        };

        for i in 0..tree_attempts {
            let x = rng.range_usize(5, CHUNK_WIDTH - 5); // Increased padding
            let z = rng.range_usize(5, CHUNK_DEPTH - 5);

            // Distance check to prevent bunching
            let mut too_close = false;
            for &(tx, tz) in &tree_positions {
                let dx = x as i32 - tx as i32;
                let dz = z as i32 - tz as i32;
                if dx * dx + dz * dz < min_dist * min_dist {
                    too_close = true;
                    break;
                }
            }
            if too_close {
                continue;
            }

            let world_x = (self.x * CHUNK_WIDTH as i32 + x as i32) as f32;
            let world_z = (self.z * CHUNK_DEPTH as i32 + z as i32) as f32;
            let biome = get_biome_seeded(world_x, world_z, self.seed);
            let seed = chunk_hash(self.seed, world_x as i32, world_z as i32, 8301 + i) as i32;

            // Find surface
            let mut surface_y = 0;
            for y in (60..CHUNK_HEIGHT - 32).rev() {
                let b = self.blocks[x][y][z];
                if b == BlockType::Grass || b == BlockType::SnowyGrass {
                    surface_y = y;
                    break;
                } else if b.is_solid() && b != BlockType::SnowLayer {
                    // Not valid ground
                    break;
                }
            }

            if surface_y > 0 {
                match biome {
                    Biome::Plains => {
                        // Oak tree
                        let tree_h = 4 + seed.rem_euclid(3) as usize;
                        for h in 1..=tree_h {
                            if surface_y + h < CHUNK_HEIGHT {
                                self.blocks[x][surface_y + h][z] = BlockType::OakLog;
                            }
                        }
                        for ly in -2_i32..=1 {
                            let rad: i32 = if ly < 0 { 2 } else { 1 };
                            for lx in -rad..=rad {
                                for lz in -rad..=rad {
                                    if ly < 0
                                        && lx.abs() == rad
                                        && lz.abs() == rad
                                        && (seed + lx + lz) % 3 == 0
                                    {
                                        continue;
                                    }
                                    if ly == 1 && (lx.abs() + lz.abs() > 1) {
                                        continue;
                                    }
                                    let ly_idx = surface_y as i32 + tree_h as i32 + ly + 1;
                                    let bx = (x as i32 + lx) as usize;
                                    let bz = (z as i32 + lz) as usize;
                                    if ly_idx >= 0
                                        && ly_idx < CHUNK_HEIGHT as i32
                                        && bx < CHUNK_WIDTH
                                        && bz < CHUNK_DEPTH
                                        && self.blocks[bx][ly_idx as usize][bz] == BlockType::Air
                                    {
                                        self.blocks[bx][ly_idx as usize][bz] = BlockType::OakLeaves;
                                    }
                                }
                            }
                        }
                        tree_positions.push((x, z));
                    }
                    Biome::SnowyTaiga => {
                        // 1:1 Authentic Minecraft 1.0 spruce tree pattern
                        let tree_h = 7 + seed.rem_euclid(4) as usize;
                        // Trunk
                        for h in 1..=tree_h {
                            if surface_y + h < CHUNK_HEIGHT {
                                self.blocks[x][surface_y + h][z] = BlockType::SpruceLog;
                            }
                        }
                        // Leaves
                        for y_off in 0..=tree_h {
                            let ly = surface_y + tree_h + 1 - y_off;
                            if ly >= CHUNK_HEIGHT || ly <= surface_y + 1 {
                                continue;
                            }
                            let rad: i32 = match y_off {
                                0 => 0,
                                1 => 1,
                                2 => 0,
                                3 => 1,
                                4 => 2,
                                5 => 1,
                                6 => 2,
                                7 => 2,
                                _ => 2,
                            };
                            for lx in -rad..=rad {
                                for lz in -rad..=rad {
                                    if rad > 0 && lx.abs() == rad && lz.abs() == rad {
                                        continue;
                                    }
                                    let bx = x as i32 + lx;
                                    let bz = z as i32 + lz;
                                    if bx >= 0
                                        && bx < CHUNK_WIDTH as i32
                                        && bz >= 0
                                        && bz < CHUNK_DEPTH as i32
                                    {
                                        let bx = bx as usize;
                                        let bz = bz as usize;
                                        if self.blocks[bx][ly][bz] == BlockType::Air {
                                            self.blocks[bx][ly][bz] = BlockType::SpruceLeaves;
                                        }
                                    }
                                }
                            }
                        }
                        tree_positions.push((x, z));
                    }
                    _ => {}
                }
            }
        }

        self.generate_biome_decorations();

        // PASS 6: Snow layer for snowy biomes
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_DEPTH {
                let world_x = (self.x * CHUNK_WIDTH as i32 + x as i32) as f32;
                let world_z = (self.z * CHUNK_DEPTH as i32 + z as i32) as f32;
                let biome = get_biome_seeded(world_x, world_z, self.seed);
                if biome == Biome::SnowyTundra
                    || biome == Biome::SnowyTaiga
                    || biome == Biome::Mountains
                {
                    // Find topmost solid block and place snow on top
                    for y in (1..CHUNK_HEIGHT - 1).rev() {
                        let b = self.blocks[x][y][z];
                        if b.is_solid() && b != BlockType::SnowLayer {
                            let above = self.blocks[x][y + 1][z];
                            if above == BlockType::Air {
                                // Don't place snow on logs, leaves, or snowy grass (already snow-topped)
                                if b != BlockType::OakLog
                                    && b != BlockType::SpruceLog
                                    && b != BlockType::SnowyGrass
                                {
                                    self.blocks[x][y + 1][z] = BlockType::SnowLayer;
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }

        // PASS 8: Villages (multi-chunk, seed-deterministic)
        crate::village::stamp_chunk(self);

        self.calculate_lighting();
    }

    pub fn calculate_lighting(&mut self) {
        compute_lighting(&self.blocks, &mut self.light);
    }

    pub fn snapshot_data(&self) -> ChunkData {
        ChunkData {
            x: self.x,
            z: self.z,
            blocks: self.blocks.clone(),
            light: self.light.clone(),
            liquid_levels: self.liquid_levels.clone(),
        }
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block: BlockType) {
        if x < CHUNK_WIDTH && y < CHUNK_HEIGHT && z < CHUNK_DEPTH {
            let old_block = self.blocks[x][y][z];
            self.blocks[x][y][z] = block;
            if block == BlockType::Water || block == BlockType::Lava {
                self.liquid_levels[x][y][z] = WATER_SOURCE;
            } else {
                self.liquid_levels[x][y][z] = 0;
            }
            self.dirty = true;
            if matches!(
                block,
                BlockType::Torch | BlockType::RedstoneTorch | BlockType::Lava
            ) || matches!(
                old_block,
                BlockType::Torch | BlockType::RedstoneTorch | BlockType::Lava
            ) {
                self.calculate_lighting();
            }
        }
    }

    pub fn get_block(&self, x: usize, y: usize, z: usize) -> BlockType {
        if x < CHUNK_WIDTH && y < CHUNK_HEIGHT && z < CHUNK_DEPTH {
            self.blocks[x][y][z]
        } else {
            BlockType::Air
        }
    }
}

impl ChunkData {
    pub fn calculate_lighting(&mut self) {
        compute_lighting(&self.blocks, &mut self.light);
    }

    pub fn calculate_mesh_data(&self, snap: &MeshSnapshot) -> (MeshData, MeshData, MeshData) {
        let mut v_op = Vec::new();
        let mut t_op = Vec::new();
        let mut n_op = Vec::new();
        let mut c_op = Vec::new();

        let mut v_tr = Vec::new();
        let mut t_tr = Vec::new();
        let mut n_tr = Vec::new();
        let mut c_tr = Vec::new();

        let mut v_wa = Vec::new();
        let mut t_wa = Vec::new();
        let mut n_wa = Vec::new();
        let mut c_wa = Vec::new();

        let should_draw_face = |current: BlockType, neighbor: BlockType| -> bool {
            if current == BlockType::Torch || current == BlockType::Bell {
                return true;
            }
            if neighbor == BlockType::Air {
                return true;
            }
            let current_is_liquid = current == BlockType::Water || current == BlockType::Lava;
            let neighbor_is_liquid = neighbor == BlockType::Water || neighbor == BlockType::Lava;
            if current_is_liquid {
                // Liquid shell: only hide faces against the same liquid.
                return neighbor != current;
            }
            if neighbor_is_liquid && !current_is_liquid {
                return true;
            }
            if neighbor.is_transparent() && current != neighbor {
                return true;
            }
            false
        };

        let get_block_safe = |wx: i32, wy: i32, wz: i32| -> BlockType {
            // Snapshot covers the chunk plus a 1-block border (including diagonals)
            snap.get_block(wx, wy, wz)
        };

        // Standard Voxel AO calculation
        // Side1, Side2, Corner are the three blocks adjacent to the vertex
        let calc_ao = |s1: bool, s2: bool, c: bool| -> f32 {
            if s1 && s2 {
                return 0.4;
            }
            let mut occ = 0;
            if s1 {
                occ += 1;
            }
            if s2 {
                occ += 1;
            }
            if c {
                occ += 1;
            }
            match occ {
                0 => 1.0,
                1 => 0.8,
                2 => 0.6,
                _ => 0.4,
            }
        };

        let get_light_safe = |wx: i32, wy: i32, wz: i32| -> u8 {
            if wy < 0 || wy >= CHUNK_HEIGHT as i32 {
                return pack_light(15, 0);
            }

            // Local chunk coordinates
            let cx = wx - (self.x * CHUNK_WIDTH as i32);
            let cz = wz - (self.z * CHUNK_DEPTH as i32);

            if cx >= 0 && cx < CHUNK_WIDTH as i32 && cz >= 0 && cz < CHUNK_DEPTH as i32 {
                // Own chunk: freshly recomputed light
                return self.light[cx as usize][wy as usize][cz as usize];
            }

            // Neighbor light from the snapshot border
            snap.get_light(wx, wy, wz)
        };

        let calc_light_f = light_factor;

        let calc_vertex_light = |l0: u8, l1: u8, l2: u8, l3: u8| -> (f32, f32) {
            let (s0, b0) = unpack_light(l0);
            let (s1, b1) = unpack_light(l1);
            let (s2, b2) = unpack_light(l2);
            let (s3, b3) = unpack_light(l3);

            let sky_avg =
                (calc_light_f(s0) + calc_light_f(s1) + calc_light_f(s2) + calc_light_f(s3)) * 0.25;
            let block_avg =
                (calc_light_f(b0) + calc_light_f(b1) + calc_light_f(b2) + calc_light_f(b3)) * 0.25;

            (sky_avg, block_avg)
        };

        let is_solid = |b: BlockType| b.is_solid() && !b.is_transparent();

        // Smooth (per-vertex) lighting + AO for a face, generalizing the
        // corner sampling the top face already used by hand. `base` is the
        // neighbor cell one step along the face normal; `u_axis`/`v_axis`
        // are the two in-plane world-space unit steps. Returns (sky, block, AO) for
        // corners (-,-), (+,-), (+,+), (-,+) in that order; callers still
        // apply their own directional darkening factor and floor.
        let face_shading = |base: (i32, i32, i32),
                            u_axis: (i32, i32, i32),
                            v_axis: (i32, i32, i32)|
         -> [(f32, f32, f32); 4] {
            let offset = |du: i32, dv: i32| -> (i32, i32, i32) {
                (
                    base.0 + u_axis.0 * du + v_axis.0 * dv,
                    base.1 + u_axis.1 * du + v_axis.1 * dv,
                    base.2 + u_axis.2 * du + v_axis.2 * dv,
                )
            };
            let solid_at = |du: i32, dv: i32| -> bool {
                let (px, py, pz) = offset(du, dv);
                is_solid(get_block_safe(px, py, pz))
            };
            let light_at = |du: i32, dv: i32| -> u8 {
                let (px, py, pz) = offset(du, dv);
                get_light_safe(px, py, pz)
            };
            let l_center = light_at(0, 0);
            let corner = |du: i32, dv: i32| -> (f32, f32, f32) {
                let ao = calc_ao(solid_at(du, 0), solid_at(0, dv), solid_at(du, dv));
                let (sky, block) =
                    calc_vertex_light(l_center, light_at(du, 0), light_at(0, dv), light_at(du, dv));
                (sky, block, ao)
            };
            [corner(-1, -1), corner(1, -1), corner(1, 1), corner(-1, 1)]
        };

        for x in 0..CHUNK_WIDTH {
            for y in 0..CHUNK_HEIGHT {
                for z in 0..CHUNK_DEPTH {
                    let block = self.blocks[x][y][z];
                    if block == BlockType::Air {
                        continue;
                    }

                    let fx = (x as i32 + self.x * CHUNK_WIDTH as i32) as f32;
                    let fy = y as f32;
                    let fz = (z as i32 + self.z * CHUNK_DEPTH as i32) as f32;
                    let ts = 1.0 / 16.0;
                    let pad = 0.5 / 256.0;

                    if block == BlockType::Water || block == BlockType::Lava {
                        let is_lava = block == BlockType::Lava;
                        let level = if is_lava {
                            WATER_SOURCE
                        } else {
                            self.liquid_levels[x][y][z]
                        };
                        if level == 0 {
                            continue;
                        }

                        let wx = x as i32 + self.x * CHUNK_WIDTH as i32;
                        let wy = y as i32;
                        let wz = z as i32 + self.z * CHUNK_DEPTH as i32;
                        let (tx, ty) = if is_lava { (7, 7) } else { (13, 12) };
                        let liquid_alpha = if is_lava { 255 } else { WATER_VERTEX_ALPHA };
                        let u0 = tx as f32 * ts + pad;
                        let v0 = ty as f32 * ts + pad;
                        let u1 = (tx + 1) as f32 * ts - pad;
                        let v1 = (ty + 1) as f32 * ts - pad;

                        // MC 1.0 corner height averaging for smooth water surface
                        let corner_h = |cx: i32, cz: i32| -> f32 {
                            let mut total = 0.0f32;
                            let mut count = 0i32;
                            for ddx in [-1i32, 0] {
                                for ddz in [-1i32, 0] {
                                    let bx = cx + ddx;
                                    let bz = cz + ddz;
                                    if snap.get_block(bx, wy + 1, bz) == block {
                                        return 1.0;
                                    }
                                    let b = snap.get_block(bx, wy, bz);
                                    if b == block {
                                        let l = snap.get_liquid_level(bx, wy, bz);
                                        total += if is_lava { 1.0 } else { water_render_height(l) };
                                        count += 1;
                                    } else if !b.is_solid() {
                                        count += 1;
                                    }
                                }
                            }
                            if count > 0 { total / count as f32 } else { 1.0 }
                        };

                        let h00 = corner_h(wx, wz); // (x, z) corner
                        let h10 = corner_h(wx + 1, wz); // (x+1, z) corner
                        let h11 = corner_h(wx + 1, wz + 1); // (x+1, z+1) corner
                        let h01 = corner_h(wx, wz + 1); // (x, z+1) corner

                        // Top face
                        let neighbor_top = if y < CHUNK_HEIGHT - 1 {
                            self.blocks[x][y + 1][z]
                        } else {
                            BlockType::Air
                        };
                        if neighbor_top != block && !is_solid(neighbor_top) {
                            v_wa.extend_from_slice(&[
                                fx,
                                fy + h00,
                                fz,
                                fx,
                                fy + h01,
                                fz + 1.0,
                                fx + 1.0,
                                fy + h11,
                                fz + 1.0,
                                fx,
                                fy + h00,
                                fz,
                                fx + 1.0,
                                fy + h11,
                                fz + 1.0,
                                fx + 1.0,
                                fy + h10,
                                fz,
                            ]);
                            t_wa.extend_from_slice(&[
                                u0, v0, u0, v1, u1, v1, u0, v0, u1, v1, u1, v0,
                            ]);
                            for _ in 0..6 {
                                n_wa.extend_from_slice(&[0.0, 1.0, 0.0]);
                            }
                            let (sky_light, block_light) =
                                unpack_light(get_light_safe(wx, wy + 1, wz));
                            let sky_val = (255.0 * calc_light_f(sky_light)) as u8;
                            let block_val = (255.0 * calc_light_f(block_light)) as u8;
                            for _ in 0..6 {
                                c_wa.extend_from_slice(&[sky_val, block_val, 255, liquid_alpha]);
                            }
                        }
                        // Bottom face
                        let neighbor_bottom = if y > 0 {
                            self.blocks[x][y - 1][z]
                        } else {
                            BlockType::Bedrock
                        };
                        if neighbor_bottom != block && !is_solid(neighbor_bottom) {
                            v_wa.extend_from_slice(&[
                                fx,
                                fy,
                                fz,
                                fx + 1.0,
                                fy,
                                fz + 1.0,
                                fx,
                                fy,
                                fz + 1.0,
                                fx,
                                fy,
                                fz,
                                fx + 1.0,
                                fy,
                                fz,
                                fx + 1.0,
                                fy,
                                fz + 1.0,
                            ]);
                            t_wa.extend_from_slice(&[
                                u0, v0, u1, v1, u0, v1, u0, v0, u1, v0, u1, v1,
                            ]);
                            for _ in 0..6 {
                                n_wa.extend_from_slice(&[0.0, -1.0, 0.0]);
                            }
                            let (sky_light, block_light) =
                                unpack_light(get_light_safe(wx, wy - 1, wz));
                            let sky_val = (255.0 * 0.5 * calc_light_f(sky_light)) as u8;
                            let block_val = (255.0 * 0.5 * calc_light_f(block_light)) as u8;
                            for _ in 0..6 {
                                c_wa.extend_from_slice(&[sky_val, block_val, 255, liquid_alpha]);
                            }
                        }
                        // Side faces with MC-style corner heights
                        // Z+ face
                        let n_zpb = if z < CHUNK_DEPTH - 1 {
                            self.blocks[x][y][z + 1]
                        } else {
                            snap.get_block(wx, wy, wz + 1)
                        };
                        if n_zpb != block && !is_solid(n_zpb) {
                            v_wa.extend_from_slice(&[
                                fx,
                                fy,
                                fz + 1.0,
                                fx + 1.0,
                                fy,
                                fz + 1.0,
                                fx + 1.0,
                                fy + h11,
                                fz + 1.0,
                                fx,
                                fy,
                                fz + 1.0,
                                fx + 1.0,
                                fy + h11,
                                fz + 1.0,
                                fx,
                                fy + h01,
                                fz + 1.0,
                            ]);
                            t_wa.extend_from_slice(&[
                                u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0,
                            ]);
                            for _ in 0..6 {
                                n_wa.extend_from_slice(&[0.0, 0.0, 1.0]);
                            }
                            let (sky_light, block_light) =
                                unpack_light(get_light_safe(wx, wy, wz + 1));
                            let sky_val = (255.0 * 0.6 * calc_light_f(sky_light)) as u8;
                            let block_val = (255.0 * 0.6 * calc_light_f(block_light)) as u8;
                            for _ in 0..6 {
                                c_wa.extend_from_slice(&[sky_val, block_val, 255, liquid_alpha]);
                            }
                        }
                        // Z- face
                        let n_znb = if z > 0 {
                            self.blocks[x][y][z - 1]
                        } else {
                            snap.get_block(wx, wy, wz - 1)
                        };
                        if n_znb != block && !is_solid(n_znb) {
                            v_wa.extend_from_slice(&[
                                fx + 1.0,
                                fy,
                                fz,
                                fx,
                                fy,
                                fz,
                                fx,
                                fy + h00,
                                fz,
                                fx + 1.0,
                                fy,
                                fz,
                                fx,
                                fy + h00,
                                fz,
                                fx + 1.0,
                                fy + h10,
                                fz,
                            ]);
                            t_wa.extend_from_slice(&[
                                u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0,
                            ]);
                            for _ in 0..6 {
                                n_wa.extend_from_slice(&[0.0, 0.0, -1.0]);
                            }
                            let (sky_light, block_light) =
                                unpack_light(get_light_safe(wx, wy, wz - 1));
                            let sky_val = (255.0 * 0.6 * calc_light_f(sky_light)) as u8;
                            let block_val = (255.0 * 0.6 * calc_light_f(block_light)) as u8;
                            for _ in 0..6 {
                                c_wa.extend_from_slice(&[sky_val, block_val, 255, liquid_alpha]);
                            }
                        }
                        // X+ face
                        let n_xpb = if x < CHUNK_WIDTH - 1 {
                            self.blocks[x + 1][y][z]
                        } else {
                            snap.get_block(wx + 1, wy, wz)
                        };
                        if n_xpb != block && !is_solid(n_xpb) {
                            v_wa.extend_from_slice(&[
                                fx + 1.0,
                                fy,
                                fz + 1.0,
                                fx + 1.0,
                                fy,
                                fz,
                                fx + 1.0,
                                fy + h10,
                                fz,
                                fx + 1.0,
                                fy,
                                fz + 1.0,
                                fx + 1.0,
                                fy + h10,
                                fz,
                                fx + 1.0,
                                fy + h11,
                                fz + 1.0,
                            ]);
                            t_wa.extend_from_slice(&[
                                u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0,
                            ]);
                            for _ in 0..6 {
                                n_wa.extend_from_slice(&[1.0, 0.0, 0.0]);
                            }
                            let (sky_light, block_light) =
                                unpack_light(get_light_safe(wx + 1, wy, wz));
                            let sky_val = (255.0 * 0.8 * calc_light_f(sky_light)) as u8;
                            let block_val = (255.0 * 0.8 * calc_light_f(block_light)) as u8;
                            for _ in 0..6 {
                                c_wa.extend_from_slice(&[sky_val, block_val, 255, liquid_alpha]);
                            }
                        }
                        // X- face
                        let n_xnb = if x > 0 {
                            self.blocks[x - 1][y][z]
                        } else {
                            snap.get_block(wx - 1, wy, wz)
                        };
                        if n_xnb != block && !is_solid(n_xnb) {
                            v_wa.extend_from_slice(&[
                                fx,
                                fy,
                                fz,
                                fx,
                                fy,
                                fz + 1.0,
                                fx,
                                fy + h01,
                                fz + 1.0,
                                fx,
                                fy,
                                fz,
                                fx,
                                fy + h01,
                                fz + 1.0,
                                fx,
                                fy + h00,
                                fz,
                            ]);
                            t_wa.extend_from_slice(&[
                                u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0,
                            ]);
                            for _ in 0..6 {
                                n_wa.extend_from_slice(&[-1.0, 0.0, 0.0]);
                            }
                            let (sky_light, block_light) =
                                unpack_light(get_light_safe(wx - 1, wy, wz));
                            let sky_val = (255.0 * 0.8 * calc_light_f(sky_light)) as u8;
                            let block_val = (255.0 * 0.8 * calc_light_f(block_light)) as u8;
                            for _ in 0..6 {
                                c_wa.extend_from_slice(&[sky_val, block_val, 255, liquid_alpha]);
                            }
                        }
                        continue;
                    }

                    if block == BlockType::Wheat {
                        let (v, t, n, c) = (&mut v_tr, &mut t_tr, &mut n_tr, &mut c_tr);
                        let (tx, ty) = crate::item::atlas_uv(block);
                        let u0 = tx as f32 * ts + pad;
                        let v0 = ty as f32 * ts + pad;
                        let u1 = (tx as f32 + 1.0) * ts - pad;
                        let v1 = (ty as f32 + 1.0) * ts - pad;
                        let wx = x as i32 + self.x * CHUNK_WIDTH as i32;
                        let wy = y as i32;
                        let wz = z as i32 + self.z * CHUNK_DEPTH as i32;
                        let (sky0, block0) = unpack_light(get_light_safe(wx, wy, wz));
                        let (sky1, block1) = unpack_light(get_light_safe(wx, wy + 1, wz));
                        let sky_light = sky0.max(sky1);
                        let block_light = block0.max(block1);
                        let sky_val = (255.0 * calc_light_f(sky_light)).max(70.0) as u8;
                        let block_val = (255.0 * calc_light_f(block_light)) as u8;
                        let color = [sky_val, block_val, 255, 255];
                        let y0 = fy;
                        let y1 = fy + 0.875;
                        let x0 = fx + 0.12;
                        let x1 = fx + 0.88;
                        let z0 = fz + 0.12;
                        let z1 = fz + 0.88;
                        let planes = [
                            [[x0, y0, z0], [x1, y0, z1], [x1, y1, z1], [x0, y1, z0]],
                            [[x1, y0, z0], [x0, y0, z1], [x0, y1, z1], [x1, y1, z0]],
                        ];

                        for verts in planes {
                            v.extend_from_slice(&[
                                verts[0][0],
                                verts[0][1],
                                verts[0][2],
                                verts[1][0],
                                verts[1][1],
                                verts[1][2],
                                verts[2][0],
                                verts[2][1],
                                verts[2][2],
                                verts[0][0],
                                verts[0][1],
                                verts[0][2],
                                verts[2][0],
                                verts[2][1],
                                verts[2][2],
                                verts[3][0],
                                verts[3][1],
                                verts[3][2],
                                verts[2][0],
                                verts[2][1],
                                verts[2][2],
                                verts[1][0],
                                verts[1][1],
                                verts[1][2],
                                verts[0][0],
                                verts[0][1],
                                verts[0][2],
                                verts[3][0],
                                verts[3][1],
                                verts[3][2],
                                verts[2][0],
                                verts[2][1],
                                verts[2][2],
                                verts[0][0],
                                verts[0][1],
                                verts[0][2],
                            ]);
                            t.extend_from_slice(&[
                                u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0, u1, v0, u1, v1, u0,
                                v1, u0, v0, u1, v0, u0, v1,
                            ]);
                            for _ in 0..12 {
                                n.extend_from_slice(&[0.0, 1.0, 0.0]);
                                c.extend_from_slice(&color);
                            }
                        }
                        continue;
                    }

                    if block == BlockType::Torch || block == BlockType::RedstoneTorch {
                        let (v, t, n, c) = (&mut v_tr, &mut t_tr, &mut n_tr, &mut c_tr);
                        let (tx, ty) = crate::item::atlas_uv(block);
                        let ts = 1.0 / 16.0;
                        let pad = 0.5 / 256.0;

                        // 3D Torch Post bounds (2x2 voxels wide, 10 voxels tall)
                        let min_x = fx + 0.4375;
                        let max_x = fx + 0.5625;
                        let min_z = fz + 0.4375;
                        let max_z = fz + 0.5625;
                        let min_y = fy;
                        let max_y = fy + 0.625;

                        // Side UVs (pixels 7..9 in U, 3..13 in V)
                        let su0 = (tx as f32 + 7.0 / 16.0) * ts + pad;
                        let su1 = (tx as f32 + 9.0 / 16.0) * ts - pad;
                        let sv0 = (ty as f32 + 3.0 / 16.0) * ts + pad;
                        let sv1 = (ty as f32 + 13.0 / 16.0) * ts - pad;

                        // Top UVs (pixels 7..9 in U, 3..5 in V)
                        let tu0 = (tx as f32 + 7.0 / 16.0) * ts + pad;
                        let tu1 = (tx as f32 + 9.0 / 16.0) * ts - pad;
                        let tv0 = (ty as f32 + 3.0 / 16.0) * ts + pad;
                        let tv1 = (ty as f32 + 5.0 / 16.0) * ts - pad;

                        // Torch color: torch model itself is self-luminous!
                        let torch_color = [150, 255, 255, 255];

                        // Top face
                        v.extend_from_slice(&[
                            min_x, max_y, min_z, min_x, max_y, max_z, max_x, max_y, max_z, min_x,
                            max_y, min_z, max_x, max_y, max_z, max_x, max_y, min_z,
                        ]);
                        t.extend_from_slice(&[
                            tu0, tv0, tu0, tv1, tu1, tv1, tu0, tv0, tu1, tv1, tu1, tv0,
                        ]);
                        for _ in 0..6 {
                            n.extend_from_slice(&[0.0, 1.0, 0.0]);
                            c.extend_from_slice(&torch_color);
                        }

                        // Side Z+
                        v.extend_from_slice(&[
                            min_x, min_y, max_z, max_x, min_y, max_z, max_x, max_y, max_z, min_x,
                            min_y, max_z, max_x, max_y, max_z, min_x, max_y, max_z,
                        ]);
                        t.extend_from_slice(&[
                            su0, sv1, su1, sv1, su1, sv0, su0, sv1, su1, sv0, su0, sv0,
                        ]);
                        for _ in 0..6 {
                            n.extend_from_slice(&[0.0, 0.0, 1.0]);
                            c.extend_from_slice(&torch_color);
                        }

                        // Side Z-
                        v.extend_from_slice(&[
                            max_x, min_y, min_z, min_x, min_y, min_z, min_x, max_y, min_z, max_x,
                            min_y, min_z, min_x, max_y, min_z, max_x, max_y, min_z,
                        ]);
                        t.extend_from_slice(&[
                            su0, sv1, su1, sv1, su1, sv0, su0, sv1, su1, sv0, su0, sv0,
                        ]);
                        for _ in 0..6 {
                            n.extend_from_slice(&[0.0, 0.0, -1.0]);
                            c.extend_from_slice(&torch_color);
                        }

                        // Side X+
                        v.extend_from_slice(&[
                            max_x, min_y, max_z, max_x, min_y, min_z, max_x, max_y, min_z, max_x,
                            min_y, max_z, max_x, max_y, min_z, max_x, max_y, max_z,
                        ]);
                        t.extend_from_slice(&[
                            su0, sv1, su1, sv1, su1, sv0, su0, sv1, su1, sv0, su0, sv0,
                        ]);
                        for _ in 0..6 {
                            n.extend_from_slice(&[1.0, 0.0, 0.0]);
                            c.extend_from_slice(&torch_color);
                        }

                        // Side X-
                        v.extend_from_slice(&[
                            min_x, min_y, min_z, min_x, min_y, max_z, min_x, max_y, max_z, min_x,
                            min_y, min_z, min_x, max_y, max_z, min_x, max_y, min_z,
                        ]);
                        t.extend_from_slice(&[
                            su0, sv1, su1, sv1, su1, sv0, su0, sv1, su1, sv0, su0, sv0,
                        ]);
                        for _ in 0..6 {
                            n.extend_from_slice(&[-1.0, 0.0, 0.0]);
                            c.extend_from_slice(&torch_color);
                        }

                        continue;
                    }

                    // STANDARD BLOCK MESHING (OPAQUE)
                    let (v, t, n, c) = (&mut v_op, &mut t_op, &mut n_op, &mut c_op);
                    let (tx, ty) = {
                        let (t, s) = crate::item::atlas_uv(block);
                        (t as i32, s as i32)
                    };

                    let u0 = tx as f32 * ts + pad;
                    let v0 = ty as f32 * ts + pad;
                    let u1 = (tx + 1) as f32 * ts - pad;
                    let v1 = (ty + 1) as f32 * ts - pad;
                    let (min_x, max_x, min_z, max_z, block_top) = if block == BlockType::SnowLayer {
                        (fx, fx + 1.0, fz, fz + 1.0, 0.125)
                    } else if block == BlockType::Bell {
                        (fx + 0.2, fx + 0.8, fz + 0.2, fz + 0.8, 0.6)
                    } else {
                        (fx, fx + 1.0, fz, fz + 1.0, 1.0)
                    };
                    let wx = x as i32 + self.x * CHUNK_WIDTH as i32;
                    let wy = y as i32;
                    let wz = z as i32 + self.z * CHUNK_DEPTH as i32;

                    // Top
                    let neighbor_top = if y < CHUNK_HEIGHT - 1 {
                        self.blocks[x][y + 1][z]
                    } else {
                        BlockType::Air
                    };
                    if should_draw_face(block, neighbor_top) {
                        let (mut tu0, mut tv0, mut tu1, mut tv1) = (u0, v0, u1, v1);
                        {
                            let (ttx, tty) = crate::item::atlas_uv_top(block);
                            if (ttx as i32, tty as i32) != (tx, ty) {
                                tu0 = ttx as f32 * ts + pad;
                                tv0 = tty as f32 * ts + pad;
                                tu1 = (ttx as f32 + 1.0) * ts - pad;
                                tv1 = (tty as f32 + 1.0) * ts - pad;
                            }
                        }

                        let l_00 = is_solid(get_block_safe(wx - 1, wy + 1, wz - 1));
                        let l_10 = is_solid(get_block_safe(wx, wy + 1, wz - 1));
                        let l_20 = is_solid(get_block_safe(wx + 1, wy + 1, wz - 1));
                        let l_01 = is_solid(get_block_safe(wx - 1, wy + 1, wz));
                        let l_21 = is_solid(get_block_safe(wx + 1, wy + 1, wz));
                        let l_02 = is_solid(get_block_safe(wx - 1, wy + 1, wz + 1));
                        let l_12 = is_solid(get_block_safe(wx, wy + 1, wz + 1));
                        let l_22 = is_solid(get_block_safe(wx + 1, wy + 1, wz + 1));

                        let ao00 = calc_ao(l_01, l_10, l_00);
                        let ao10 = calc_ao(l_21, l_10, l_20);
                        let ao01 = calc_ao(l_01, l_12, l_02);
                        let ao11 = calc_ao(l_21, l_12, l_22);

                        v.extend_from_slice(&[
                            min_x,
                            fy + block_top,
                            min_z,
                            min_x,
                            fy + block_top,
                            max_z,
                            max_x,
                            fy + block_top,
                            max_z,
                            min_x,
                            fy + block_top,
                            min_z,
                            max_x,
                            fy + block_top,
                            max_z,
                            max_x,
                            fy + block_top,
                            min_z,
                        ]);
                        t.extend_from_slice(&[
                            tu0, tv0, tu0, tv1, tu1, tv1, tu0, tv0, tu1, tv1, tu1, tv0,
                        ]);

                        let l_b = get_light_safe(wx, wy + 1, wz);
                        let ll_01 = get_light_safe(wx - 1, wy + 1, wz);
                        let ll_10 = get_light_safe(wx, wy + 1, wz - 1);
                        let ll_21 = get_light_safe(wx + 1, wy + 1, wz);
                        let ll_12 = get_light_safe(wx, wy + 1, wz + 1);
                        let (sky00, blk00) = calc_vertex_light(
                            l_b,
                            ll_01,
                            ll_10,
                            get_light_safe(wx - 1, wy + 1, wz - 1),
                        );
                        let (sky10, blk10) = calc_vertex_light(
                            l_b,
                            ll_21,
                            ll_10,
                            get_light_safe(wx + 1, wy + 1, wz - 1),
                        );
                        let (sky01, blk01) = calc_vertex_light(
                            l_b,
                            ll_01,
                            ll_12,
                            get_light_safe(wx - 1, wy + 1, wz + 1),
                        );
                        let (sky11, blk11) = calc_vertex_light(
                            l_b,
                            ll_21,
                            ll_12,
                            get_light_safe(wx + 1, wy + 1, wz + 1),
                        );

                        for _ in 0..6 {
                            n.extend_from_slice(&[0.0, 1.0, 0.0]);
                        }
                        let c00 = [
                            (255.0 * sky00).clamp(0.0, 255.0) as u8,
                            (255.0 * blk00).clamp(0.0, 255.0) as u8,
                            (255.0 * ao00).clamp(0.0, 255.0) as u8,
                            255,
                        ];
                        let c10 = [
                            (255.0 * sky10).clamp(0.0, 255.0) as u8,
                            (255.0 * blk10).clamp(0.0, 255.0) as u8,
                            (255.0 * ao10).clamp(0.0, 255.0) as u8,
                            255,
                        ];
                        let c01 = [
                            (255.0 * sky01).clamp(0.0, 255.0) as u8,
                            (255.0 * blk01).clamp(0.0, 255.0) as u8,
                            (255.0 * ao01).clamp(0.0, 255.0) as u8,
                            255,
                        ];
                        let c11 = [
                            (255.0 * sky11).clamp(0.0, 255.0) as u8,
                            (255.0 * blk11).clamp(0.0, 255.0) as u8,
                            (255.0 * ao11).clamp(0.0, 255.0) as u8,
                            255,
                        ];
                        c.extend_from_slice(&[
                            c00[0], c00[1], c00[2], 255, c01[0], c01[1], c01[2], 255, c11[0],
                            c11[1], c11[2], 255, c00[0], c00[1], c00[2], 255, c11[0], c11[1],
                            c11[2], 255, c10[0], c10[1], c10[2], 255,
                        ]);
                    }

                    // Bottom
                    let neighbor_bottom = if y > 0 {
                        self.blocks[x][y - 1][z]
                    } else {
                        BlockType::Bedrock
                    };
                    if should_draw_face(block, neighbor_bottom) {
                        v.extend_from_slice(&[
                            min_x, fy, min_z, max_x, fy, max_z, min_x, fy, max_z, min_x, fy, min_z,
                            max_x, fy, min_z, max_x, fy, max_z,
                        ]);
                        t.extend_from_slice(&[u0, v0, u1, v1, u0, v1, u0, v0, u1, v0, u1, v1]);
                        for _ in 0..6 {
                            n.extend_from_slice(&[0.0, -1.0, 0.0]);
                        }
                        let s = face_shading((wx, wy - 1, wz), (1, 0, 0), (0, 0, 1));
                        for idx in [0usize, 2, 3, 0, 1, 2] {
                            let (sky_f, block_f, ao_val) = s[idx];
                            let r = (255.0 * sky_f * 0.5).clamp(0.0, 255.0) as u8;
                            let g = (255.0 * block_f * 0.5).clamp(0.0, 255.0) as u8;
                            let b = (255.0 * ao_val).clamp(0.0, 255.0) as u8;
                            c.extend_from_slice(&[r, g, b, 255]);
                        }
                    }

                    // Sides (Z+, Z-, X+, X-)
                    let neighbors = [
                        (
                            if z < CHUNK_DEPTH - 1 {
                                self.blocks[x][y][z + 1]
                            } else {
                                snap.get_block(wx, wy, wz + 1)
                            },
                            [0.0, 0.0, 1.0],
                            0.6,
                        ),
                        (
                            if z > 0 {
                                self.blocks[x][y][z - 1]
                            } else {
                                snap.get_block(wx, wy, wz - 1)
                            },
                            [0.0, 0.0, -1.0],
                            0.6,
                        ),
                        (
                            if x < CHUNK_WIDTH - 1 {
                                self.blocks[x + 1][y][z]
                            } else {
                                snap.get_block(wx + 1, wy, wz)
                            },
                            [1.0, 0.0, 0.0],
                            0.8,
                        ),
                        (
                            if x > 0 {
                                self.blocks[x - 1][y][z]
                            } else {
                                snap.get_block(wx - 1, wy, wz)
                            },
                            [-1.0, 0.0, 0.0],
                            0.8,
                        ),
                    ];
                    for (i, (neighbor, norm, s_mul)) in neighbors.iter().enumerate() {
                        if should_draw_face(block, *neighbor) {
                            if i == 0 {
                                // Z+
                                v.extend_from_slice(&[
                                    min_x,
                                    fy,
                                    max_z,
                                    max_x,
                                    fy,
                                    max_z,
                                    max_x,
                                    fy + block_top,
                                    max_z,
                                    min_x,
                                    fy,
                                    max_z,
                                    max_x,
                                    fy + block_top,
                                    max_z,
                                    min_x,
                                    fy + block_top,
                                    max_z,
                                ]);
                            } else if i == 1 {
                                // Z-
                                v.extend_from_slice(&[
                                    max_x,
                                    fy,
                                    min_z,
                                    min_x,
                                    fy,
                                    min_z,
                                    min_x,
                                    fy + block_top,
                                    min_z,
                                    max_x,
                                    fy,
                                    min_z,
                                    min_x,
                                    fy + block_top,
                                    min_z,
                                    max_x,
                                    fy + block_top,
                                    min_z,
                                ]);
                            } else if i == 2 {
                                // X+
                                v.extend_from_slice(&[
                                    max_x,
                                    fy,
                                    max_z,
                                    max_x,
                                    fy,
                                    min_z,
                                    max_x,
                                    fy + block_top,
                                    min_z,
                                    max_x,
                                    fy,
                                    max_z,
                                    max_x,
                                    fy + block_top,
                                    min_z,
                                    max_x,
                                    fy + block_top,
                                    max_z,
                                ]);
                            } else {
                                // X-
                                v.extend_from_slice(&[
                                    min_x,
                                    fy,
                                    min_z,
                                    min_x,
                                    fy,
                                    max_z,
                                    min_x,
                                    fy + block_top,
                                    max_z,
                                    min_x,
                                    fy,
                                    min_z,
                                    min_x,
                                    fy + block_top,
                                    max_z,
                                    min_x,
                                    fy + block_top,
                                    min_z,
                                ]);
                            }
                            t.extend_from_slice(&[u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0]);
                            for _ in 0..6 {
                                n.extend_from_slice(norm);
                            }
                            let base = (wx + norm[0] as i32, wy, wz + norm[2] as i32);
                            // Z+/Z- faces are in the X,Y plane; X+/X- faces
                            // are in the Z,Y plane. Winding matches each
                            // face's vertex order above.
                            let u_axis = if i < 2 { (1, 0, 0) } else { (0, 0, 1) };
                            let winding: [usize; 6] = if i == 0 || i == 3 {
                                [0, 1, 2, 0, 2, 3]
                            } else {
                                [1, 0, 3, 1, 3, 2]
                            };
                            let s = face_shading(base, u_axis, (0, 1, 0));
                            for idx in winding {
                                let (sky_f, block_f, ao_val) = s[idx];
                                let r = (255.0 * sky_f * s_mul).clamp(0.0, 255.0) as u8;
                                let g = (255.0 * block_f * s_mul).clamp(0.0, 255.0) as u8;
                                let b = (255.0 * ao_val).clamp(0.0, 255.0) as u8;
                                c.extend_from_slice(&[r, g, b, 255]);
                            }
                        }
                    }
                }
            }
        }

        (
            MeshData {
                v: v_op,
                t: t_op,
                n: n_op,
                c: c_op,
            },
            MeshData {
                v: v_tr,
                t: t_tr,
                n: n_tr,
                c: c_tr,
            },
            MeshData {
                v: v_wa,
                t: t_wa,
                n: n_wa,
                c: c_wa,
            },
        )
    }
}

impl Chunk {
    pub fn upload_mesh(&mut self, opaque: MeshData, transparent: MeshData, water: MeshData) {
        record_vertex_count_sample(
            (opaque.v.len() / 3 + transparent.v.len() / 3 + water.v.len() / 3) as u32,
        );
        self.mesh_opaque = if opaque.v.is_empty() {
            None
        } else {
            Some(Mesh::new(
                &opaque.v,
                Some(&opaque.t),
                Some(&opaque.n),
                Some(&opaque.c),
            ))
        };
        self.mesh_transparent = if transparent.v.is_empty() {
            None
        } else {
            Some(Mesh::new(
                &transparent.v,
                Some(&transparent.t),
                Some(&transparent.n),
                Some(&transparent.c),
            ))
        };
        self.mesh_water = if water.v.is_empty() {
            None
        } else {
            Some(Mesh::new(
                &water.v,
                Some(&water.t),
                Some(&water.n),
                Some(&water.c),
            ))
        };
        self.meshing_in_progress = false;
    }
}

static VERTEX_COUNT_SAMPLES: std::sync::LazyLock<std::sync::Mutex<Vec<u32>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

// Temporary instrumentation for hashed-splashing-haven M1: gathers the real
// per-chunk vertex-count distribution across gameplay so the GPU mesher's
// vertex-arena size (M4) can be sized from data instead of guessed.
fn record_vertex_count_sample(count: u32) {
    let mut samples = VERTEX_COUNT_SAMPLES.lock().unwrap();
    samples.push(count);
    if samples.len() % 64 == 0 {
        let mut sorted = samples.clone();
        sorted.sort_unstable();
        let p50 = sorted[sorted.len() / 2];
        let p99 = sorted[(sorted.len() * 99 / 100).min(sorted.len() - 1)];
        let max = *sorted.last().unwrap();
        let avg = sorted.iter().map(|&v| v as u64).sum::<u64>() / sorted.len() as u64;
        println!(
            "Chunk mesh vertex counts ({} samples): avg={} p50={} p99={} max={} (36 bytes/vertex)",
            sorted.len(),
            avg,
            p50,
            p99,
            max
        );
    }
}

pub fn generate_minable_vein(
    chunk: &mut Chunk,
    block_type: BlockType,
    number_of_blocks: usize,
    local_x: i32,
    local_y: i32,
    local_z: i32,
    rng: &mut ChunkRng,
) {
    let angle = rng.range_f32(0.0, std::f32::consts::PI);
    let sin_a = angle.sin();
    let cos_a = angle.cos();

    let d_blocks = number_of_blocks as f32;
    let x0 = local_x as f32;
    let z0 = local_z as f32;

    let d0 = (x0 + sin_a * d_blocks / 8.0) as f64;
    let d1 = (x0 - sin_a * d_blocks / 8.0) as f64;
    let d2 = (z0 + cos_a * d_blocks / 8.0) as f64;
    let d3 = (z0 - cos_a * d_blocks / 8.0) as f64;

    let d4 = (local_y + rng.range_i32(-2, 1)) as f64;
    let d5 = (local_y + rng.range_i32(-2, 1)) as f64;

    for i in 0..=number_of_blocks {
        let t = i as f64 / d_blocks as f64;
        let d6 = d0 + (d1 - d0) * t;
        let d7 = d4 + (d5 - d4) * t;
        let d8 = d2 + (d3 - d2) * t;

        let d9 = (rng.next_f64() * d_blocks as f64) / 16.0;
        let radius = (((t as f32 * std::f32::consts::PI).sin() + 1.0) as f64 * d9 + 1.0) / 2.0;

        let min_x = (d6 - radius).floor() as i32;
        let max_x = (d6 + radius).floor() as i32;
        let min_y = (d7 - radius).floor() as i32;
        let max_y = (d7 + radius).floor() as i32;
        let min_z = (d8 - radius).floor() as i32;
        let max_z = (d8 + radius).floor() as i32;

        for bx in min_x..=max_x {
            let x_dist = ((bx as f64 + 0.5) - d6) / radius;
            if x_dist * x_dist >= 1.0 {
                continue;
            }
            for by in min_y..=max_y {
                let y_dist = ((by as f64 + 0.5) - d7) / radius;
                if x_dist * x_dist + y_dist * y_dist >= 1.0 {
                    continue;
                }
                for bz in min_z..=max_z {
                    let z_dist = ((bz as f64 + 0.5) - d8) / radius;
                    if x_dist * x_dist + y_dist * y_dist + z_dist * z_dist < 1.0 {
                        if (0..CHUNK_WIDTH as i32).contains(&bx)
                            && (0..CHUNK_HEIGHT as i32).contains(&by)
                            && (0..CHUNK_DEPTH as i32).contains(&bz)
                        {
                            let cx = bx as usize;
                            let cy = by as usize;
                            let cz = bz as usize;
                            if chunk.blocks[cx][cy][cz] == BlockType::Stone {
                                chunk.blocks[cx][cy][cz] = block_type;
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Ground truth for a single snapshot cell, computed directly from the
    /// source `Chunk` neighbor grid and the fallback function -- entirely
    /// independent of `MeshSnapshot`'s internal storage/indexing, so it
    /// can't share a bug with whatever representation `build` uses.
    fn naive_lookup(
        cx: i32,
        cz: i32,
        neighbors: [[Option<&Chunk>; 3]; 3],
        fallback_block: impl Fn(i32, i32, i32) -> BlockType,
        wx: i32,
        wy: i32,
        wz: i32,
    ) -> (BlockType, u8, u8) {
        let lx = wx - cx * CHUNK_WIDTH as i32;
        let lz = wz - cz * CHUNK_DEPTH as i32;
        let dx = lx.div_euclid(CHUNK_WIDTH as i32);
        let dz = lz.div_euclid(CHUNK_DEPTH as i32);
        let bx = lx.rem_euclid(CHUNK_WIDTH as i32) as usize;
        let bz = lz.rem_euclid(CHUNK_DEPTH as i32) as usize;
        match neighbors[(dx + 1) as usize][(dz + 1) as usize] {
            Some(c) => (
                c.blocks[bx][wy as usize][bz],
                c.light[bx][wy as usize][bz],
                c.liquid_levels[bx][wy as usize][bz],
            ),
            None => {
                let block = fallback_block(wx, wy, wz);
                let liquid = if block == BlockType::Water || block == BlockType::Lava {
                    WATER_SOURCE
                } else {
                    0
                };
                (block, pack_light(15, 0), liquid)
            }
        }
    }

    /// Checks `snap`'s public accessors against `naive_lookup` across the
    /// entire 18x18xCHUNK_HEIGHT region the snapshot covers.
    fn assert_snapshot_matches_ground_truth(
        snap: &MeshSnapshot,
        cx: i32,
        cz: i32,
        neighbors: [[Option<&Chunk>; 3]; 3],
        fallback_block: impl Fn(i32, i32, i32) -> BlockType,
    ) {
        for sx in 0..SNAP_W {
            let wx = cx * CHUNK_WIDTH as i32 + (sx as i32 - 1);
            for sz in 0..SNAP_D {
                let wz = cz * CHUNK_DEPTH as i32 + (sz as i32 - 1);
                for y in 0..CHUNK_HEIGHT {
                    let (eb, el, eliq) =
                        naive_lookup(cx, cz, neighbors, &fallback_block, wx, y as i32, wz);
                    assert_eq!(
                        snap.get_block(wx, y as i32, wz),
                        eb,
                        "block mismatch at ({wx},{y},{wz})"
                    );
                    assert_eq!(
                        snap.get_light(wx, y as i32, wz),
                        el,
                        "light mismatch at ({wx},{y},{wz})"
                    );
                    assert_eq!(
                        snap.get_liquid_level(wx, y as i32, wz),
                        eliq,
                        "liquid mismatch at ({wx},{y},{wz})"
                    );
                }
            }
        }
    }

    fn distinct_chunk(cx: i32, cz: i32, seed: u64) -> Chunk {
        let mut c = Chunk::new(cx, cz, seed);
        for x in 0..CHUNK_WIDTH {
            for y in 0..CHUNK_HEIGHT {
                for z in 0..CHUNK_DEPTH {
                    let mix = cx
                        .wrapping_mul(131)
                        .wrapping_add(cz.wrapping_mul(577))
                        .wrapping_add((x * 13 + y * 3 + z * 41) as i32);
                    c.blocks[x][y][z] =
                        BlockType::from_u8(mix.rem_euclid(BlockType::COUNT as i32) as u8);
                    c.light[x][y][z] = mix.rem_euclid(16) as u8;
                    c.liquid_levels[x][y][z] = mix.rem_euclid(17) as u8;
                }
            }
        }
        c
    }

    fn fallback_air(_wx: i32, _wy: i32, _wz: i32) -> BlockType {
        BlockType::Air
    }

    fn fallback_pattern(wx: i32, wy: i32, wz: i32) -> BlockType {
        let mix = wx.wrapping_mul(3) ^ wy.wrapping_mul(5) ^ wz.wrapping_mul(7);
        BlockType::from_u8(mix.rem_euclid(BlockType::COUNT as i32) as u8)
    }

    #[test]
    fn mesh_snapshot_build_matches_naive_with_full_neighbor_grid() {
        let seed = 42;
        let grid: Vec<Vec<Chunk>> = (0..3)
            .map(|dx| {
                (0..3)
                    .map(|dz| distinct_chunk(dx - 1, dz - 1, seed))
                    .collect()
            })
            .collect();
        let mut neighbors: [[Option<&Chunk>; 3]; 3] = [[None; 3]; 3];
        for dx in 0..3usize {
            for dz in 0..3usize {
                neighbors[dx][dz] = Some(&grid[dx][dz]);
            }
        }

        let actual = MeshSnapshot::build(0, 0, neighbors, fallback_air);
        assert_snapshot_matches_ground_truth(&actual, 0, 0, neighbors, fallback_air);
    }

    #[test]
    fn mesh_snapshot_build_matches_naive_with_missing_neighbors() {
        let seed = 7;
        let center = distinct_chunk(0, 0, seed);
        let west = distinct_chunk(-1, 0, seed);
        let north = distinct_chunk(0, -1, seed);
        // East, south, and both diagonals stay unloaded so the test
        // exercises the fallback path on both the fast-path neighbor row
        // (west/center/east) and the z-border pass (north/south/corners).
        let mut neighbors: [[Option<&Chunk>; 3]; 3] = [[None; 3]; 3];
        neighbors[0][1] = Some(&west);
        neighbors[1][1] = Some(&center);
        neighbors[1][0] = Some(&north);

        let actual = MeshSnapshot::build(0, 0, neighbors, fallback_pattern);
        assert_snapshot_matches_ground_truth(&actual, 0, 0, neighbors, fallback_pattern);
        assert_eq!(unpack_light(actual.get_light(16, 64, 0)), (15, 0));
    }

    #[test]
    fn chunk_generation_is_repeatable_for_same_seed() {
        let mut first = Chunk::new(0, 0, 12_345);
        let mut second = Chunk::new(0, 0, 12_345);

        first.generate();
        second.generate();

        assert_eq!(first.blocks, second.blocks);
        assert_eq!(first.liquid_levels, second.liquid_levels);
    }

    #[test]
    fn chunk_generation_differs_across_seeds() {
        let mut first = Chunk::new(0, 0, 12_345);
        let mut second = Chunk::new(0, 0, 54_321);

        first.generate();
        second.generate();

        assert_ne!(
            first.blocks, second.blocks,
            "different seeds should produce different terrain"
        );
    }

    #[test]
    fn terrain_height_stays_below_world_ceiling() {
        for seed in [12_345_u64, 4_259_633_870_796_407_859_u64] {
            for x in (-256..=256).step_by(7) {
                for z in (-256..=256).step_by(7) {
                    let (height, _) = terrain_height_and_biome(x as f32, z as f32, seed);
                    assert!(
                        height <= MAX_TERRAIN_HEIGHT as usize,
                        "terrain height {height} reached ceiling near ({x}, {z}) for seed {seed}"
                    );
                }
            }
        }
    }

    #[test]
    fn terrain_height_changes_smoothly_across_columns() {
        let seed = 4_259_633_870_796_407_859_u64;
        for x in (-192..=192).step_by(3) {
            for z in (-192..=192).step_by(3) {
                let (height, _) = terrain_height_and_biome(x as f32, z as f32, seed);
                let (east, _) = terrain_height_and_biome((x + 1) as f32, z as f32, seed);
                let (south, _) = terrain_height_and_biome(x as f32, (z + 1) as f32, seed);
                assert!(
                    height.abs_diff(east).max(height.abs_diff(south)) <= 28,
                    "terrain height jumped near ({x}, {z}) for seed {seed}"
                );
            }
        }
    }

    #[test]
    fn hydrate_fluids_fills_missing_water_and_lava_levels() {
        let mut chunk = Chunk::new(0, 0, 1);
        chunk.blocks[3][10][4] = BlockType::Water;
        chunk.blocks[5][12][6] = BlockType::Lava;
        chunk.blocks[7][8][9] = BlockType::Stone;
        chunk.hydrate_fluids();
        assert_eq!(chunk.liquid_levels[3][10][4], WATER_SOURCE);
        assert_eq!(chunk.liquid_levels[5][12][6], WATER_SOURCE);
        assert_eq!(chunk.liquid_levels[7][8][9], 0);
    }

    #[test]
    fn minable_veins_cover_both_chunk_halves_deterministically() {
        let mut first = Chunk::new(0, 0, 73);
        let mut second = Chunk::new(0, 0, 73);
        for x in 0..CHUNK_WIDTH {
            for y in 56..=72 {
                for z in 0..CHUNK_DEPTH {
                    first.blocks[x][y][z] = BlockType::Stone;
                    second.blocks[x][y][z] = BlockType::Stone;
                }
            }
        }

        let mut first_rng = ChunkRng::new(73, 0, 0, 991);
        let mut second_rng = ChunkRng::new(73, 0, 0, 991);
        for local_x in 0..CHUNK_WIDTH as i32 {
            for local_z in 0..CHUNK_DEPTH as i32 {
                generate_minable_vein(
                    &mut first,
                    BlockType::CoalOre,
                    4,
                    local_x,
                    64,
                    local_z,
                    &mut first_rng,
                );
                generate_minable_vein(
                    &mut second,
                    BlockType::CoalOre,
                    4,
                    local_x,
                    64,
                    local_z,
                    &mut second_rng,
                );
            }
        }

        let ore_positions = |chunk: &Chunk| {
            let mut positions = Vec::new();
            for x in 0..CHUNK_WIDTH {
                for y in 0..CHUNK_HEIGHT {
                    for z in 0..CHUNK_DEPTH {
                        if chunk.blocks[x][y][z] == BlockType::CoalOre {
                            positions.push((x, y, z));
                        }
                    }
                }
            }
            positions
        };
        let first_positions = ore_positions(&first);
        assert_eq!(first_positions, ore_positions(&second));

        let (mut west, mut east, mut north, mut south) = (0, 0, 0, 0);
        for &(x, _, z) in &first_positions {
            if x < CHUNK_WIDTH / 2 {
                west += 1;
            } else {
                east += 1;
            }
            if z < CHUNK_DEPTH / 2 {
                north += 1;
            } else {
                south += 1;
            }
        }

        assert!(
            west * 2 >= east && east * 2 >= west,
            "west={west}, east={east}"
        );
        assert!(
            north * 2 >= south && south * 2 >= north,
            "north={north}, south={south}"
        );
    }

    #[test]
    fn test_light_packing_and_torch_emission() {
        let (sky, block) = unpack_light(pack_light(15, 14));
        assert_eq!(sky, 15);
        assert_eq!(block, 14);

        let mut chunk = Chunk::new(0, 0, 1);
        // Put solid roof above to block skylight
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_DEPTH {
                chunk.blocks[x][50][z] = BlockType::Stone;
            }
        }
        chunk.blocks[8][40][8] = BlockType::Torch;
        chunk.calculate_lighting();

        let (sky_t, block_t) = unpack_light(chunk.light[8][40][8]);
        assert_eq!(sky_t, 0, "torch under stone roof should have 0 skylight");
        assert_eq!(block_t, 14, "torch should emit level 14 block light");

        let (_sky_adj, block_adj) = unpack_light(chunk.light[8][40][9]);
        assert_eq!(
            block_adj, 13,
            "adjacent air cell should receive level 13 block light"
        );
    }

    #[test]
    fn redstone_torch_changes_recalculate_block_light() {
        let mut chunk = Chunk::new(0, 0, 1);
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_DEPTH {
                chunk.blocks[x][50][z] = BlockType::Stone;
            }
        }
        chunk.calculate_lighting();

        chunk.set_block(8, 40, 8, BlockType::RedstoneTorch);
        assert_eq!(unpack_light(chunk.light[8][40][8]).1, 7);

        chunk.set_block(8, 40, 8, BlockType::Air);
        assert_eq!(unpack_light(chunk.light[8][40][8]).1, 0);
    }

    #[test]
    fn light_factor_is_full_at_15_and_dark_at_zero() {
        assert_eq!(light_factor(0), 0.0);
        assert!((light_factor(15) - 1.0).abs() < 1e-6);
        assert!(light_factor(7) > light_factor(0));
        assert!(light_factor(7) < light_factor(15));
    }
}
