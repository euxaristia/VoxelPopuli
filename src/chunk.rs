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

fn chunk_hash(seed: u64, x: i32, z: i32, salt: i32) -> u32 {
    let mixed = mix64(
        seed ^ (x as i64 as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ (z as i64 as u64).wrapping_mul(0xBF58_476D_1CE4_E5B9)
            ^ (salt as i64 as u64).wrapping_mul(0x94D0_49BB_1331_11EB),
    );
    (mixed ^ (mixed >> 32)) as u32
}

fn hash_unit(seed: u64, x: i32, z: i32, salt: i32) -> f32 {
    chunk_hash(seed, x, z, salt) as f32 / u32::MAX as f32
}

struct ChunkRng {
    state: u64,
}

impl ChunkRng {
    fn new(seed: u64, x: i32, z: i32, salt: u64) -> Self {
        let state = mix64(
            seed ^ (x as i64 as u64).wrapping_mul(0xD1B5_4A32_D192_ED03)
                ^ (z as i64 as u64).wrapping_mul(0xABC9_83DB_5F35_53B5)
                ^ salt,
        );
        Self { state }
    }

    fn next_u32(&mut self) -> u32 {
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
    pub edit_revision: u64,
    pub meshing_revision: u64,
    pub pending_mesh_revision: u64,
    pub dirty: bool,
    pub meshing_in_progress: bool,
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
            pending_mesh_opaque: None,
            pending_mesh_transparent: None,
            pending_mesh_water: None,
            edit_revision: 0,
            meshing_revision: 0,
            pending_mesh_revision: 0,
            dirty: true,
            meshing_in_progress: false,
            x,
            z,
            seed,
        }
    }

    fn surface_y(&self, x: usize, z: usize) -> Option<usize> {
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

    fn generate_village_outpost(&mut self) {
        if hash_unit(self.seed, self.x, self.z, 7201) > 0.07 {
            return;
        }

        let world_x = (self.x * CHUNK_WIDTH as i32 + 8) as f32;
        let world_z = (self.z * CHUNK_DEPTH as i32 + 8) as f32;
        let biome = get_biome_seeded(world_x, world_z, self.seed);
        if biome != Biome::Plains && biome != Biome::Desert {
            return;
        }

        let (x0, z0) = (4usize, 4usize);
        let (x1, z1) = (11usize, 11usize);
        let mut min_y = CHUNK_HEIGHT;
        let mut max_y = 0usize;
        for x in x0..=x1 {
            for z in z0..=z1 {
                let Some(y) = self.surface_y(x, z) else {
                    return;
                };
                if self.blocks[x][y + 1][z] == BlockType::Water {
                    return;
                }
                min_y = min_y.min(y);
                max_y = max_y.max(y);
            }
        }
        if max_y - min_y > 3 || max_y + 6 >= CHUNK_HEIGHT {
            return;
        }

        let base_y = max_y + 1;
        let wall = if biome == Biome::Desert {
            BlockType::Sandstone
        } else {
            BlockType::OakPlanks
        };
        let roof = if biome == Biome::Desert {
            BlockType::Sandstone
        } else {
            BlockType::OakLog
        };

        for x in x0..=x1 {
            for z in z0..=z1 {
                if let Some(surface) = self.surface_y(x, z) {
                    for y in surface + 1..base_y {
                        self.blocks[x][y][z] = wall;
                    }
                }
                self.blocks[x][base_y][z] = wall;
                for y in base_y + 1..=base_y + 3 {
                    let edge = x == x0 || x == x1 || z == z0 || z == z1;
                    self.blocks[x][y][z] = if edge { wall } else { BlockType::Air };
                }
                self.blocks[x][base_y + 4][z] = roof;
            }
        }

        for y in base_y + 1..=base_y + 2 {
            self.blocks[7][y][z0] = BlockType::Air;
            self.blocks[8][y][z0] = BlockType::Air;
        }
        self.blocks[5][base_y + 2][z0] = BlockType::Glass;
        self.blocks[10][base_y + 2][z0] = BlockType::Glass;
        self.blocks[5][base_y + 2][z1] = BlockType::Glass;
        self.blocks[10][base_y + 2][z1] = BlockType::Glass;
        self.blocks[6][base_y + 1][9] = BlockType::Chest;
        self.blocks[9][base_y + 1][9] = BlockType::CraftingTable;
        self.blocks[7][base_y + 2][10] = BlockType::Torch;
        self.blocks[8][base_y + 2][10] = BlockType::Torch;

        let farm_y = base_y;
        for x in 1usize..=3 {
            for z in 4usize..=12 {
                for y in 1..farm_y {
                    if self.blocks[x][y][z] == BlockType::Air
                        || self.blocks[x][y][z] == BlockType::Water
                    {
                        self.blocks[x][y][z] = BlockType::Dirt;
                    }
                }
                if z == 8 && x == 2 {
                    self.blocks[x][farm_y][z] = BlockType::Water;
                    self.liquid_levels[x][farm_y][z] = WATER_SOURCE;
                } else {
                    self.blocks[x][farm_y][z] = BlockType::Farmland;
                    if (x + z) % 2 == 0 && farm_y + 1 < CHUNK_HEIGHT {
                        self.blocks[x][farm_y + 1][z] = BlockType::Wheat;
                    }
                }
            }
        }
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
                let biome = get_biome_seeded(world_x, world_z, self.seed);

                let continental = seeded_perlin_2d(world_x, world_z, 0.005, 3, self.seed, 100);
                let detail = seeded_perlin_2d(world_x, world_z, 0.03, 4, self.seed, 110);

                let (base_h, height_scale) = match biome {
                    Biome::Mountains => {
                        let ridged = (-seeded_perlin_2d(world_x, world_z, 0.01, 3, self.seed, 120)
                            .abs()
                            + 1.0)
                            .powi(2);
                        (continental * 50.0 + 140.0 + ridged * 60.0, 30.0)
                    }
                    Biome::HighHills => (continental * 40.0 + 135.0, 25.0),
                    _ => (continental * 40.0 + 128.0, 15.0),
                };

                let mut height = (base_h + detail * height_scale) as i32;

                // Fjord carving: if mountainous/hilly and near ocean (low continental)
                if (biome == Biome::Mountains || biome == Biome::HighHills) && continental < 0.1 {
                    let fjord_noise =
                        seeded_perlin_2d(world_x, world_z, 0.02, 2, self.seed, 130).abs();
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
            let x = rng.range_usize(0, CHUNK_WIDTH) as i32;
            let y = rng.range_usize(0, CHUNK_HEIGHT) as i32;
            let z = rng.range_usize(0, CHUNK_DEPTH) as i32;
            let vein_size = rng.range_i32_inclusive(1, 17);
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

        // PASS 3b: Iron Ores (Slightly rarer, deeper)
        for _ in 0..80 {
            let x = rng.range_usize(0, CHUNK_WIDTH) as i32;
            let y = rng.range_i32(0, 64);
            let z = rng.range_usize(0, CHUNK_DEPTH) as i32;
            let vein_size = rng.range_i32_inclusive(1, 9);
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

        // PASS 3c: Rare progression ores
        for (block, attempts, max_y, max_vein) in [
            (BlockType::GoldOre, 18, 32, 9),
            (BlockType::DiamondOre, 7, 16, 7),
            (BlockType::LapisOre, 8, 32, 7),
            (BlockType::RedstoneOre, 12, 16, 8),
        ] {
            for _ in 0..attempts {
                let x = rng.range_usize(0, CHUNK_WIDTH) as i32;
                let y = rng.range_i32(1, max_y);
                let z = rng.range_usize(0, CHUNK_DEPTH) as i32;
                let vein_size = rng.range_i32_inclusive(1, max_vein);
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
                            self.blocks[cx][cy][cz] = block;
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

        self.generate_village_outpost();
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
                        b if b == BlockType::Air
                            || b == BlockType::Water
                            || b == BlockType::Lava
                            || b.is_transparent() =>
                        {
                            self.light[x][y][z] = sunlight;
                        }
                        _ => {
                            self.light[x][y][z] = 0;
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
                    let emitted = match self.blocks[x][y][z] {
                        BlockType::Torch => 14,
                        BlockType::Lava => 15,
                        _ => 0,
                    };
                    if emitted > self.light[x][y][z] {
                        self.light[x][y][z] = emitted;
                    }
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
                    let occluding = nb.is_solid() && !nb.is_transparent();

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
            if block == BlockType::Water || block == BlockType::Lava {
                self.liquid_levels[x][y][z] = WATER_SOURCE;
            } else {
                self.liquid_levels[x][y][z] = 0;
            }
            self.edit_revision = self.edit_revision.wrapping_add(1);
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

        let mut v_tr = Vec::new();
        let mut t_tr = Vec::new();
        let mut n_tr = Vec::new();
        let mut c_tr = Vec::new();

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
                                    if world.get_block(bx, wy + 1, bz) == block {
                                        return 1.0;
                                    }
                                    let b = world.get_block(bx, wy, bz);
                                    if b == block {
                                        let l = world.get_liquid_level(bx, wy, bz);
                                        total += if is_lava { 1.0 } else { water_render_height(l) };
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
                        if neighbor_top != block {
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
                                c_wa.extend_from_slice(&[c_val, c_val, c_val, liquid_alpha]);
                            }
                        }
                        // Bottom face
                        let neighbor_bottom = if y > 0 {
                            self.blocks[x][y - 1][z]
                        } else {
                            BlockType::Bedrock
                        };
                        if neighbor_bottom != block {
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
                                c_wa.extend_from_slice(&[shade, shade, shade, liquid_alpha]);
                            }
                        }
                        // Side faces with MC-style corner heights
                        // Z+ face
                        let n_zpb = if z < CHUNK_DEPTH - 1 {
                            self.blocks[x][y][z + 1]
                        } else {
                            unsafe { n_pz.map_or(BlockType::Air, |c| (*c).blocks[x][y][0]) }
                        };
                        if n_zpb != block {
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
                                c_wa.extend_from_slice(&[shade, shade, shade, liquid_alpha]);
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
                        if n_znb != block {
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
                                c_wa.extend_from_slice(&[shade, shade, shade, liquid_alpha]);
                            }
                        }
                        // X+ face
                        let n_xpb = if x < CHUNK_WIDTH - 1 {
                            self.blocks[x + 1][y][z]
                        } else {
                            unsafe { n_px.map_or(BlockType::Air, |c| (*c).blocks[0][y][z]) }
                        };
                        if n_xpb != block {
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
                                c_wa.extend_from_slice(&[shade, shade, shade, liquid_alpha]);
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
                        if n_xnb != block {
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
                                c_wa.extend_from_slice(&[shade, shade, shade, liquid_alpha]);
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
                        let light = calc_light_f(get_light_safe(wx, wy, wz).max(get_light_safe(
                            wx,
                            wy + 1,
                            wz,
                        )));
                        let shade = (255.0 * light).max(70.0) as u8;
                        let color = [shade, shade, shade, 255];
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

                    let is_solid = |b: BlockType| b.is_solid() && !b.is_transparent();

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_generation_is_repeatable_for_same_seed() {
        let mut first = Chunk::new(0, 0, 12_345);
        let mut second = Chunk::new(0, 0, 12_345);

        first.generate();
        second.generate();

        assert_eq!(first.blocks, second.blocks);
        assert_eq!(first.liquid_levels, second.liquid_levels);
    }
}
