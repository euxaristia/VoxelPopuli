use crate::block::BlockType;
use crate::noise::perlin_2d;
use crate::renderer::Mesh;
use rand::RngExt;
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

pub fn get_biome(world_x: f32, world_z: f32) -> Biome {
    let temperature = perlin_2d(world_x + 5000.0, world_z + 5000.0, 0.002, 2);
    let humidity = perlin_2d(world_x + 10000.0, world_z + 10000.0, 0.002, 2);
    let mountain_noise = perlin_2d(world_x + 20000.0, world_z + 20000.0, 0.001, 3);
    let hills_noise = perlin_2d(world_x + 30000.0, world_z + 30000.0, 0.005, 2);

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

pub const CHUNK_WIDTH: usize = 16;
pub const CHUNK_HEIGHT: usize = 256;
pub const CHUNK_DEPTH: usize = 16;
pub const WATER_VERTEX_ALPHA: u8 = 180;

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

pub struct Chunk {
    pub blocks: Box<[[[BlockType; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]>,
    pub light: Box<[[[u8; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]>,
    pub liquid_levels: Box<[[[u8; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]>,
    pub mesh_opaque: Option<Mesh>,
    pub mesh_transparent: Option<Mesh>,
    pub mesh_water: Option<Mesh>,
    pub pending_mesh_opaque: Option<MeshData>,
    pub pending_mesh_transparent: Option<MeshData>,
    pub pending_mesh_water: Option<MeshData>,
    pub dirty: bool,
    pub meshing_in_progress: bool,
    // True while a rayon worker is running `generate()` against this chunk's blocks.
    // Used to guard slot recycling and to skip meshing dispatch until generation finishes.
    pub generation_in_progress: bool,
    // False after a worker has just finished generating; flipped to true by the main thread
    // after it applies pending edits and dirties this chunk + its neighbors.
    pub post_processed: bool,
    pub x: i32,
    pub z: i32,
}

impl Chunk {
    pub fn new(x: i32, z: i32) -> Self {
        Self {
            blocks: Box::new([[[BlockType::Air; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]),
            light: Box::new([[[0; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]),
            liquid_levels: Box::new([[[0; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]),
            mesh_opaque: None,
            mesh_transparent: None,
            mesh_water: None,
            pending_mesh_opaque: None,
            pending_mesh_transparent: None,
            pending_mesh_water: None,
            // A freshly-allocated chunk has no content yet. The post-process step
            // flips `dirty` after generation finishes and increments `World::dirty_count`.
            dirty: false,
            meshing_in_progress: false,
            generation_in_progress: false,
            post_processed: true,
            x,
            z,
        }
    }

    pub fn generate(&mut self) {
        let mut rng = rand::rng();

        // PASS 1: Terrain (biome-aware)
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_DEPTH {
                let world_x = (self.x * CHUNK_WIDTH as i32 + x as i32) as f32;
                let world_z = (self.z * CHUNK_DEPTH as i32 + z as i32) as f32;
                let biome = get_biome(world_x, world_z);

                let continental = perlin_2d(world_x, world_z, 0.005, 3);
                let detail = perlin_2d(world_x, world_z, 0.03, 4);

                let (base_h, height_scale) = match biome {
                    Biome::Mountains => {
                        let ridged = (-perlin_2d(world_x, world_z, 0.01, 3).abs() + 1.0).powi(2);
                        (continental * 50.0 + 140.0 + ridged * 60.0, 30.0)
                    }
                    Biome::HighHills => (continental * 40.0 + 135.0, 25.0),
                    _ => (continental * 40.0 + 128.0, 15.0),
                };

                let mut height = (base_h + detail * height_scale) as i32;

                // Fjord carving: if mountainous/hilly and near ocean (low continental)
                if (biome == Biome::Mountains || biome == Biome::HighHills) && continental < 0.1 {
                    let fjord_noise = perlin_2d(world_x, world_z, 0.02, 2).abs();
                    if fjord_noise < 0.15 {
                        let depth_factor = (0.15 - fjord_noise) / 0.15;
                        height =
                            (height as f32 * (1.0 - depth_factor) + 110.0 * depth_factor) as i32;
                    }
                }

                let height = height.clamp(0, CHUNK_HEIGHT as i32 - 1) as usize;

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
                    } else if y < 124 {
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

        // PASS 3: Coal Ores
        for _ in 0..150 {
            let x = rng.random_range(0..CHUNK_WIDTH) as i32;
            let y = rng.random_range(0..CHUNK_HEIGHT) as i32;
            let z = rng.random_range(0..CHUNK_DEPTH) as i32;
            let vein_size = rng.random_range(1..=17);
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
                    if self.blocks[cx][cy][cz] == BlockType::Stone {
                        self.blocks[cx][cy][cz] = BlockType::CoalOre;
                    }
                }
                let dir = rng.random_range(0..6);
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

        // PASS 3b: Iron Ores (Slightly rarer, deeper)
        for _ in 0..80 {
            let x = rng.random_range(0..CHUNK_WIDTH) as i32;
            let y = rng.random_range(0..64) as i32;
            let z = rng.random_range(0..CHUNK_DEPTH) as i32;
            let vein_size = rng.random_range(1..=9);
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
                    if self.blocks[cx][cy][cz] == BlockType::Stone {
                        self.blocks[cx][cy][cz] = BlockType::IronOre;
                    }
                }
                let dir = rng.random_range(0..6);
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

        // PASS 4: Gravel
        for _ in 0..8 {
            let x = rng.random_range(0..CHUNK_WIDTH) as i32;
            let y = rng.random_range(64..128) as i32;
            let z = rng.random_range(0..CHUNK_DEPTH) as i32;

            let vein_size = rng.random_range(16..=47);
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

                let dir = rng.random_range(0..6);
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

        // PASS 5: Caves — Authentic Minecraft 1.0 "Perlin Worm" carvers
        // A simple deterministic PRNG based on chunk coords and an arbitrary seed
        let world_seed: i32 = 42; // Ideally would come from World struct, but hardcoded for now

        let cave_rng = |mut seed: u64| -> f32 {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            ((seed % 10000) as f32) / 10000.0
        };

        // Carve caves coming from neighboring chunks to ensure seamless tunnels
        for cx in -1i32..=1 {
            for cz in -1i32..=1 {
                let neighbor_x = self.x + cx;
                let neighbor_z = self.z + cz;

                // Deterministic seed for this chunk
                let base_seed = (world_seed as i64)
                    .wrapping_mul(341873128712)
                    .wrapping_add((neighbor_x as i64).wrapping_mul(132897987541))
                    .wrapping_add((neighbor_z as i64).wrapping_mul(341873128712))
                    as u64;

                let num_caves = ((cave_rng(base_seed) * 15.0) as i32).max(0);

                for i in 0..num_caves {
                    let cave_seed = base_seed.wrapping_add((i * 9283711) as u64);

                    let start_x =
                        (neighbor_x * CHUNK_WIDTH as i32) as f32 + cave_rng(cave_seed) * 16.0;
                    let start_y = cave_rng(cave_seed.wrapping_add(1)) * 120.0 + 8.0;
                    let start_z = (neighbor_z * CHUNK_DEPTH as i32) as f32
                        + cave_rng(cave_seed.wrapping_add(2)) * 16.0;

                    let mut length = cave_rng(cave_seed.wrapping_add(3)) * 100.0 + 10.0; // 10 to 110 steps
                    let mut radius = cave_rng(cave_seed.wrapping_add(4)) * 2.0 + 1.5; // 1.5 to 3.5 base radius

                    // 25% chance of a spherical room
                    if cave_rng(cave_seed.wrapping_add(5)) < 0.25 {
                        radius *= 2.0;
                        length = 0.0; // Rooms don't immediately worm
                    }

                    // 10% chance of a massive tunnel starting
                    if cave_rng(cave_seed.wrapping_add(6)) < 0.10 {
                        radius *= cave_rng(cave_seed.wrapping_add(7)) * 2.0 + 1.0;
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
                        let rad = radius * (1.0 + (step / length.max(1.0)).sin());

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

                                    let dist_sq = (global_x - current_x).powi(2)
                                        + ((global_y - current_y) * 1.5).powi(2)
                                        + (global_z - current_z).powi(2);

                                    if dist_sq <= rad * rad {
                                        let current_block =
                                            self.blocks[bx as usize][by as usize][bz as usize];
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

        // PASS 5: Trees (Moved after carvers to avoid floating)
        let mut tree_positions: Vec<(usize, usize)> = Vec::new();

        let (tree_attempts, min_dist) = match get_biome(
            (self.x * CHUNK_WIDTH as i32 + 8) as f32,
            (self.z * CHUNK_DEPTH as i32 + 8) as f32,
        ) {
            Biome::Plains | Biome::HighHills => (2, 4),
            Biome::SnowyTaiga => (8, 5), // Reduced attempts and increased distance to prevent bunching
            _ => (0, 0),
        };

        for i in 0..tree_attempts {
            let x = rng.random_range(5..CHUNK_WIDTH - 5); // Increased padding
            let z = rng.random_range(5..CHUNK_DEPTH - 5);

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
            let biome = get_biome(world_x, world_z);
            let seed = (world_x * 73.1 + world_z * 91.7 + i as f32 * 111.1) as i32;

            // Find surface
            let mut surface_y = 0;
            for y in (60..CHUNK_HEIGHT - 32).rev() {
                let b = self.blocks[x][y][z];
                if b == BlockType::Grass || b == BlockType::SnowyGrass {
                    surface_y = y;
                    break;
                } else if b != BlockType::Air && b != BlockType::Water && b != BlockType::SnowLayer
                {
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

        // PASS 6: Snow layer for snowy biomes
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_DEPTH {
                let world_x = (self.x * CHUNK_WIDTH as i32 + x as i32) as f32;
                let world_z = (self.z * CHUNK_DEPTH as i32 + z as i32) as f32;
                let biome = get_biome(world_x, world_z);
                if biome == Biome::SnowyTundra
                    || biome == Biome::SnowyTaiga
                    || biome == Biome::Mountains
                {
                    // Find topmost solid block and place snow on top
                    for y in (1..CHUNK_HEIGHT - 1).rev() {
                        let b = self.blocks[x][y][z];
                        if b != BlockType::Air && b != BlockType::Water && b != BlockType::SnowLayer
                        {
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

        self.calculate_lighting();
    }

    pub fn calculate_lighting(&mut self) {
        // PASS 7: Simple skylight (top-down sunlight)
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_DEPTH {
                let mut sunlight: u8 = 15;
                for y in (0..CHUNK_HEIGHT).rev() {
                    let b = self.blocks[x][y][z];
                    match b {
                        b if b == BlockType::Air || b == BlockType::Water || b.is_transparent() => {
                            self.light[x][y][z] = sunlight;
                        }
                        _ => {
                            sunlight = 0;
                        }
                    }
                }
            }
        }

        // Horizontal Light Propagation (BFS queue — no cloning, visits only lit blocks)
        let mut queue = VecDeque::new();
        for x in 0..CHUNK_WIDTH {
            for y in 0..CHUNK_HEIGHT {
                for z in 0..CHUNK_DEPTH {
                    if self.light[x][y][z] > 1 {
                        queue.push_back((x, y, z));
                    }
                }
            }
        }

        while let Some((x, y, z)) = queue.pop_front() {
            let current_light = self.light[x][y][z];
            let drop_light = current_light.saturating_sub(1);
            if drop_light == 0 {
                continue;
            }

            let neighbors: [(i32, i32, i32); 6] = [
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
                    let nb = self.blocks[ux][uy][uz];
                    let occluding =
                        nb != BlockType::Air && nb != BlockType::Water && !nb.is_transparent();

                    if !occluding && self.light[ux][uy][uz] < drop_light {
                        self.light[ux][uy][uz] = drop_light;
                        queue.push_back((ux, uy, uz));
                    }
                }
            }
        }
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block: BlockType) {
        if x < CHUNK_WIDTH && y < CHUNK_HEIGHT && z < CHUNK_DEPTH {
            self.blocks[x][y][z] = block;
            if block == BlockType::Water {
                self.liquid_levels[x][y][z] = WATER_SOURCE;
            } else {
                self.liquid_levels[x][y][z] = 0;
            }
            self.dirty = true;
        }
    }

    pub fn get_block(&self, x: usize, y: usize, z: usize) -> BlockType {
        if x < CHUNK_WIDTH && y < CHUNK_HEIGHT && z < CHUNK_DEPTH {
            self.blocks[x][y][z]
        } else {
            BlockType::Air
        }
    }

    pub fn calculate_mesh_data(
        &self,
        world: &crate::world::World,
    ) -> (MeshData, MeshData, MeshData) {
        let mut v_op = Vec::new();
        let mut t_op = Vec::new();
        let mut n_op = Vec::new();
        let mut c_op = Vec::new();

        let v_tr = Vec::new();
        let t_tr = Vec::new();
        let n_tr = Vec::new();
        let c_tr = Vec::new();

        let mut v_wa = Vec::new();
        let mut t_wa = Vec::new();
        let mut n_wa = Vec::new();
        let mut c_wa = Vec::new();
        // Cache neighbor chunks
        let n_px = world.get_chunk_ptr(self.x + 1, self.z);
        let n_nx = world.get_chunk_ptr(self.x - 1, self.z);
        let n_pz = world.get_chunk_ptr(self.x, self.z + 1);
        let n_nz = world.get_chunk_ptr(self.x, self.z - 1);

        let should_draw_face = |current: BlockType, neighbor: BlockType| -> bool {
            if neighbor == BlockType::Air {
                return true;
            }
            if current == BlockType::Water {
                // Liquid shell: Only draw face if neighbor is NOT water
                return neighbor != BlockType::Water;
            }
            if neighbor == BlockType::Water && current != BlockType::Water {
                return true;
            }
            if neighbor.is_transparent() && current != neighbor {
                return true;
            }
            false
        };

        let get_block_safe = |wx: i32, wy: i32, wz: i32| -> BlockType {
            // Use the world pointer for 100% accurate neighbor data (including diagonals)
            world.get_block(wx, wy, wz)
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
                return 15;
            }

            // Local chunk coordinates
            let cx = wx - (self.x * CHUNK_WIDTH as i32);
            let cz = wz - (self.z * CHUNK_DEPTH as i32);

            if cx >= 0 && cx < CHUNK_WIDTH as i32 && cz >= 0 && cz < CHUNK_DEPTH as i32 {
                return self.light[cx as usize][wy as usize][cz as usize];
            }

            // Check neighbor chunks if out of bounds
            if cx < 0 {
                return unsafe {
                    n_nx.map_or(15, |c| {
                        (*c).light[(CHUNK_WIDTH as i32 + cx) as usize][wy as usize][if cz >= 0
                            && cz < CHUNK_DEPTH as i32
                        {
                            cz as usize
                        } else {
                            0
                        }]
                    })
                };
            } else if cx >= CHUNK_WIDTH as i32 {
                return unsafe {
                    n_px.map_or(15, |c| {
                        (*c).light[(cx - CHUNK_WIDTH as i32) as usize][wy as usize][if cz >= 0
                            && cz < CHUNK_DEPTH as i32
                        {
                            cz as usize
                        } else {
                            0
                        }]
                    })
                };
            } else if cz < 0 {
                return unsafe {
                    n_nz.map_or(15, |c| {
                        (*c).light[cx as usize][wy as usize][(CHUNK_DEPTH as i32 + cz) as usize]
                    })
                };
            } else if cz >= CHUNK_DEPTH as i32 {
                return unsafe {
                    n_pz.map_or(15, |c| {
                        (*c).light[cx as usize][wy as usize][(cz - CHUNK_DEPTH as i32) as usize]
                    })
                };
            }
            15
        };

        let calc_light_f = |light_val: u8| -> f32 {
            let l = light_val as f32;
            let mut f = 0.85f32.powf(15.0 - l);
            if f < 0.1 {
                f = 0.1;
            }
            f
        };

        let calc_vertex_light = |l0: u8, l1: u8, l2: u8, l3: u8| -> f32 {
            let avg =
                (calc_light_f(l0) + calc_light_f(l1) + calc_light_f(l2) + calc_light_f(l3)) / 4.0;
            if avg < 0.1 { 0.1 } else { avg }
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

                    if block == BlockType::Water {
                        let level = self.liquid_levels[x][y][z];
                        if level == 0 {
                            continue;
                        }

                        let wx = x as i32 + self.x * CHUNK_WIDTH as i32;
                        let wy = y as i32;
                        let wz = z as i32 + self.z * CHUNK_DEPTH as i32;
                        let (tx, ty) = (13, 12);
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
                                    if world.get_block(bx, wy + 1, bz) == BlockType::Water {
                                        return 1.0;
                                    }
                                    let b = world.get_block(bx, wy, bz);
                                    if b == BlockType::Water {
                                        let l = world.get_liquid_level(bx, wy, bz);
                                        total += water_render_height(l);
                                        count += 1;
                                    } else if !b.is_solid() {
                                        count += 1;
                                    }
                                }
                            }
                            if count == 0 {
                                0.0
                            } else {
                                total / count as f32
                            }
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
                        if neighbor_top != BlockType::Water {
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
                            let c_val =
                                (255.0 * calc_light_f(get_light_safe(wx, wy + 1, wz))) as u8;
                            for _ in 0..6 {
                                c_wa.extend_from_slice(&[c_val, c_val, c_val, WATER_VERTEX_ALPHA]);
                            }
                        }
                        // Bottom face
                        let neighbor_bottom = if y > 0 {
                            self.blocks[x][y - 1][z]
                        } else {
                            BlockType::Bedrock
                        };
                        if neighbor_bottom != BlockType::Water {
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
                            let shade =
                                (255.0 * 0.5 * calc_light_f(get_light_safe(wx, wy - 1, wz))) as u8;
                            for _ in 0..6 {
                                c_wa.extend_from_slice(&[shade, shade, shade, WATER_VERTEX_ALPHA]);
                            }
                        }
                        // Side faces with MC-style corner heights
                        // Z+ face
                        let n_zpb = if z < CHUNK_DEPTH - 1 {
                            self.blocks[x][y][z + 1]
                        } else {
                            unsafe { n_pz.map_or(BlockType::Air, |c| (*c).blocks[x][y][0]) }
                        };
                        if n_zpb != BlockType::Water {
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
                            let shade =
                                (255.0 * 0.6 * calc_light_f(get_light_safe(wx, wy, wz + 1))) as u8;
                            for _ in 0..6 {
                                c_wa.extend_from_slice(&[shade, shade, shade, WATER_VERTEX_ALPHA]);
                            }
                        }
                        // Z- face
                        let n_znb = if z > 0 {
                            self.blocks[x][y][z - 1]
                        } else {
                            unsafe {
                                n_nz.map_or(BlockType::Air, |c| (*c).blocks[x][y][CHUNK_DEPTH - 1])
                            }
                        };
                        if n_znb != BlockType::Water {
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
                            let shade =
                                (255.0 * 0.6 * calc_light_f(get_light_safe(wx, wy, wz - 1))) as u8;
                            for _ in 0..6 {
                                c_wa.extend_from_slice(&[shade, shade, shade, WATER_VERTEX_ALPHA]);
                            }
                        }
                        // X+ face
                        let n_xpb = if x < CHUNK_WIDTH - 1 {
                            self.blocks[x + 1][y][z]
                        } else {
                            unsafe { n_px.map_or(BlockType::Air, |c| (*c).blocks[0][y][z]) }
                        };
                        if n_xpb != BlockType::Water {
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
                            let shade =
                                (255.0 * 0.8 * calc_light_f(get_light_safe(wx + 1, wy, wz))) as u8;
                            for _ in 0..6 {
                                c_wa.extend_from_slice(&[shade, shade, shade, WATER_VERTEX_ALPHA]);
                            }
                        }
                        // X- face
                        let n_xnb = if x > 0 {
                            self.blocks[x - 1][y][z]
                        } else {
                            unsafe {
                                n_nx.map_or(BlockType::Air, |c| (*c).blocks[CHUNK_WIDTH - 1][y][z])
                            }
                        };
                        if n_xnb != BlockType::Water {
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
                            let shade =
                                (255.0 * 0.8 * calc_light_f(get_light_safe(wx - 1, wy, wz))) as u8;
                            for _ in 0..6 {
                                c_wa.extend_from_slice(&[shade, shade, shade, WATER_VERTEX_ALPHA]);
                            }
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
                    let block_top = if block == BlockType::SnowLayer {
                        0.125
                    } else if block == BlockType::Torch {
                        0.625
                    } else {
                        1.0
                    };
                    let wx = x as i32 + self.x * CHUNK_WIDTH as i32;
                    let wy = y as i32;
                    let wz = z as i32 + self.z * CHUNK_DEPTH as i32;

                    let is_solid = |b: BlockType| {
                        b != BlockType::Air && b != BlockType::Water && !b.is_transparent()
                    };

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
                            fx,
                            fy + block_top,
                            fz,
                            fx,
                            fy + block_top,
                            fz + 1.0,
                            fx + 1.0,
                            fy + block_top,
                            fz + 1.0,
                            fx,
                            fy + block_top,
                            fz,
                            fx + 1.0,
                            fy + block_top,
                            fz + 1.0,
                            fx + 1.0,
                            fy + block_top,
                            fz,
                        ]);
                        t.extend_from_slice(&[
                            tu0, tv0, tu0, tv1, tu1, tv1, tu0, tv0, tu1, tv1, tu1, tv0,
                        ]);

                        let l_b = get_light_safe(wx, wy + 1, wz);
                        let ll_01 = get_light_safe(wx - 1, wy + 1, wz);
                        let ll_10 = get_light_safe(wx, wy + 1, wz - 1);
                        let ll_21 = get_light_safe(wx + 1, wy + 1, wz);
                        let ll_12 = get_light_safe(wx, wy + 1, wz + 1);
                        let light00 = calc_vertex_light(
                            l_b,
                            ll_01,
                            ll_10,
                            get_light_safe(wx - 1, wy + 1, wz - 1),
                        );
                        let light10 = calc_vertex_light(
                            l_b,
                            ll_21,
                            ll_10,
                            get_light_safe(wx + 1, wy + 1, wz - 1),
                        );
                        let light01 = calc_vertex_light(
                            l_b,
                            ll_01,
                            ll_12,
                            get_light_safe(wx - 1, wy + 1, wz + 1),
                        );
                        let light11 = calc_vertex_light(
                            l_b,
                            ll_21,
                            ll_12,
                            get_light_safe(wx + 1, wy + 1, wz + 1),
                        );

                        for _ in 0..6 {
                            n.extend_from_slice(&[0.0, 1.0, 0.0]);
                        }
                        let c00 = (255.0 * light00 * ao00).max(35.0) as u8;
                        let c10 = (255.0 * light10 * ao10).max(35.0) as u8;
                        let c01 = (255.0 * light01 * ao01).max(35.0) as u8;
                        let c11 = (255.0 * light11 * ao11).max(35.0) as u8;
                        c.extend_from_slice(&[
                            c00, c00, c00, 255, c01, c01, c01, 255, c11, c11, c11, 255, c00, c00,
                            c00, 255, c11, c11, c11, 255, c10, c10, c10, 255,
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
                        t.extend_from_slice(&[u0, v0, u1, v1, u0, v1, u0, v0, u1, v0, u1, v1]);
                        for _ in 0..6 {
                            n.extend_from_slice(&[0.0, -1.0, 0.0]);
                        }
                        let light = calc_light_f(get_light_safe(wx, wy - 1, wz));
                        let shade = (255.0 * light * 0.5).max(35.0) as u8;
                        for _ in 0..6 {
                            c.extend_from_slice(&[shade, shade, shade, 255]);
                        }
                    }

                    // Sides (Z+, Z-, X+, X-)
                    let neighbors = [
                        (
                            if z < CHUNK_DEPTH - 1 {
                                self.blocks[x][y][z + 1]
                            } else {
                                unsafe { n_pz.map_or(BlockType::Air, |c| (*c).blocks[x][y][0]) }
                            },
                            [0.0, 0.0, 1.0],
                            0.6,
                        ),
                        (
                            if z > 0 {
                                self.blocks[x][y][z - 1]
                            } else {
                                unsafe {
                                    n_nz.map_or(BlockType::Air, |c| {
                                        (*c).blocks[x][y][CHUNK_DEPTH - 1]
                                    })
                                }
                            },
                            [0.0, 0.0, -1.0],
                            0.6,
                        ),
                        (
                            if x < CHUNK_WIDTH - 1 {
                                self.blocks[x + 1][y][z]
                            } else {
                                unsafe { n_px.map_or(BlockType::Air, |c| (*c).blocks[0][y][z]) }
                            },
                            [1.0, 0.0, 0.0],
                            0.8,
                        ),
                        (
                            if x > 0 {
                                self.blocks[x - 1][y][z]
                            } else {
                                unsafe {
                                    n_nx.map_or(BlockType::Air, |c| {
                                        (*c).blocks[CHUNK_WIDTH - 1][y][z]
                                    })
                                }
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
                                    fx,
                                    fy,
                                    fz + 1.0,
                                    fx + 1.0,
                                    fy,
                                    fz + 1.0,
                                    fx + 1.0,
                                    fy + block_top,
                                    fz + 1.0,
                                    fx,
                                    fy,
                                    fz + 1.0,
                                    fx + 1.0,
                                    fy + block_top,
                                    fz + 1.0,
                                    fx,
                                    fy + block_top,
                                    fz + 1.0,
                                ]);
                            } else if i == 1 {
                                // Z-
                                v.extend_from_slice(&[
                                    fx + 1.0,
                                    fy,
                                    fz,
                                    fx,
                                    fy,
                                    fz,
                                    fx,
                                    fy + block_top,
                                    fz,
                                    fx + 1.0,
                                    fy,
                                    fz,
                                    fx,
                                    fy + block_top,
                                    fz,
                                    fx + 1.0,
                                    fy + block_top,
                                    fz,
                                ]);
                            } else if i == 2 {
                                // X+
                                v.extend_from_slice(&[
                                    fx + 1.0,
                                    fy,
                                    fz + 1.0,
                                    fx + 1.0,
                                    fy,
                                    fz,
                                    fx + 1.0,
                                    fy + block_top,
                                    fz,
                                    fx + 1.0,
                                    fy,
                                    fz + 1.0,
                                    fx + 1.0,
                                    fy + block_top,
                                    fz,
                                    fx + 1.0,
                                    fy + block_top,
                                    fz + 1.0,
                                ]);
                            } else {
                                // X-
                                v.extend_from_slice(&[
                                    fx,
                                    fy,
                                    fz,
                                    fx,
                                    fy,
                                    fz + 1.0,
                                    fx,
                                    fy + block_top,
                                    fz + 1.0,
                                    fx,
                                    fy,
                                    fz,
                                    fx,
                                    fy + block_top,
                                    fz + 1.0,
                                    fx,
                                    fy + block_top,
                                    fz,
                                ]);
                            }
                            t.extend_from_slice(&[u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0]);
                            for _ in 0..6 {
                                n.extend_from_slice(norm);
                            }
                            let l1 = get_light_safe(wx + norm[0] as i32, wy, wz + norm[2] as i32);
                            let l2 =
                                get_light_safe(wx + norm[0] as i32, wy + 1, wz + norm[2] as i32);
                            let light = calc_light_f(l1.max(l2));
                            let shade = (255.0 * light * s_mul).max(35.0) as u8;
                            for _ in 0..6 {
                                c.extend_from_slice(&[shade, shade, shade, 255]);
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

    pub fn upload_mesh(&mut self, opaque: MeshData, transparent: MeshData, water: MeshData) {
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
