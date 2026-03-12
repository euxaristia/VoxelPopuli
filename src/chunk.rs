use crate::block::BlockType;
use crate::noise::perlin_2d;
use crate::renderer::Mesh;
use rand::Rng;

pub const CHUNK_WIDTH: usize = 16;
pub const CHUNK_HEIGHT: usize = 256;
pub const CHUNK_DEPTH: usize = 16;
pub const WATER_VERTEX_ALPHA: u8 = 240;

pub struct Chunk {
    pub blocks: Box<[[[BlockType; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]>,
    pub light: Box<[[[u8; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]>,
    pub mesh_opaque: Option<Mesh>,
    pub mesh_transparent: Option<Mesh>,
    pub dirty: bool,
    pub x: i32,
    pub z: i32,
    pub min_y: usize,
    pub max_y: usize,
}

impl Chunk {
    pub fn new(x: i32, z: i32) -> Self {
        Self {
            blocks: Box::new([[[BlockType::Air; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]),
            light: Box::new([[[0; CHUNK_DEPTH]; CHUNK_HEIGHT]; CHUNK_WIDTH]),
            mesh_opaque: None,
            mesh_transparent: None,
            dirty: true,
            x,
            z,
            min_y: CHUNK_HEIGHT,
            max_y: 0,
        }
    }

    pub fn generate(&mut self) {
        let mut rng = rand::thread_rng();

        // PASS 1: Terrain
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_DEPTH {
                let world_x = (self.x * CHUNK_WIDTH as i32 + x as i32) as f32;
                let world_z = (self.z * CHUNK_DEPTH as i32 + z as i32) as f32;
                let continental = perlin_2d(world_x, world_z, 0.005, 3);
                let base_h = continental * 40.0 + 128.0;
                let detail = perlin_2d(world_x, world_z, 0.03, 4);
                let hill_f = if continental > 0.2 { (continental - 0.2) * 2.0 } else { 0.0 };
                let height = (base_h + detail * 15.0 * (1.0 + hill_f)) as usize;
                let height = height.clamp(0, CHUNK_HEIGHT - 1);

                for y in 0..CHUNK_HEIGHT {
                    let b = if y == 0 {
                        BlockType::Bedrock
                    } else if y < height.saturating_sub(4) {
                        BlockType::Stone
                    } else if y < height {
                        if height < 126 { BlockType::Sand } else { BlockType::Dirt }
                    } else if y == height {
                        if height < 126 { BlockType::Sand } else { BlockType::Grass }
                    } else if y < 124 {
                        BlockType::Water
                    } else {
                        BlockType::Air
                    };
                    self.blocks[x][y][z] = b;
                }
            }
        }

        // PASS 2: Trees
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_DEPTH {
                let world_x = (self.x * CHUNK_WIDTH as i32 + x as i32) as f32;
                let world_z = (self.z * CHUNK_DEPTH as i32 + z as i32) as f32;

                let continental = perlin_2d(world_x, world_z, 0.005, 3);
                let base_h = continental * 40.0 + 128.0;
                let detail = perlin_2d(world_x, world_z, 0.03, 4);
                let hill_f = if continental > 0.2 { (continental - 0.2) * 2.0 } else { 0.0 };
                let height = (base_h + detail * 15.0 * (1.0 + hill_f)) as usize;

                if height >= 126 && x > 2 && x < CHUNK_WIDTH - 3 && z > 2 && z < CHUNK_DEPTH - 3 {
                    let seed = (world_x * 73.1 + world_z * 91.7) as i32;
                    if seed % 85 == 0 {
                        let tree_h = 4 + seed.rem_euclid(3) as usize;

                        if self.blocks[x][height][z] == BlockType::Grass {
                            for h in 1..=tree_h {
                                if height + h < CHUNK_HEIGHT {
                                    self.blocks[x][height + h][z] = BlockType::OakLog;
                                }
                            }
                            for ly in -2_i32..=1 {
                                let rad = if ly < 0 { 2 } else { 1 };
                                for lx in (-rad..=rad).map(|x| x as i32) {
                                    for lz in (-rad..=rad).map(|x| x as i32) {
                                        if ly < 0 && lx.abs() == rad && lz.abs() == rad && (seed + lx + lz) % 3 == 0 {
                                            continue;
                                        }
                                        if ly == 1 && (lx.abs() + lz.abs() > 1) {
                                            continue;
                                        }

                                        let ly_idx = height as i32 + tree_h as i32 + ly + 1;
                                        let bx = (x as i32 + lx) as usize;
                                        let bz = (z as i32 + lz) as usize;

                                        if ly_idx >= 0 && ly_idx < CHUNK_HEIGHT as i32 && bx < CHUNK_WIDTH && bz < CHUNK_DEPTH {
                                            if self.blocks[bx][ly_idx as usize][bz] == BlockType::Air {
                                                self.blocks[bx][ly_idx as usize][bz] = BlockType::OakLeaves;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // PASS 3: Ores
        for _ in 0..150 {
            let x = rng.gen_range(0..CHUNK_WIDTH) as i32;
            let y = rng.gen_range(0..CHUNK_HEIGHT) as i32;
            let z = rng.gen_range(0..CHUNK_DEPTH) as i32;

            let vein_size = rng.gen_range(1..=17);
            let mut current_x = x;
            let mut current_y = y;
            let mut current_z = z;

            for _ in 0..vein_size {
                if current_x >= 0 && current_x < CHUNK_WIDTH as i32 &&
                   current_z >= 0 && current_z < CHUNK_DEPTH as i32 &&
                   current_y >= 0 && current_y < CHUNK_HEIGHT as i32 {
                    let cx = current_x as usize;
                    let cy = current_y as usize;
                    let cz = current_z as usize;
                    if self.blocks[cx][cy][cz] == BlockType::Stone {
                        self.blocks[cx][cy][cz] = BlockType::CoalOre;
                    }
                }

                let dir = rng.gen_range(0..6);
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
            let x = rng.gen_range(0..CHUNK_WIDTH) as i32;
            let y = rng.gen_range(64..128) as i32;
            let z = rng.gen_range(0..CHUNK_DEPTH) as i32;

            let vein_size = rng.gen_range(16..=47);
            let mut current_x = x;
            let mut current_y = y;
            let mut current_z = z;

            for _ in 0..vein_size {
                if current_x >= 0 && current_x < CHUNK_WIDTH as i32 &&
                   current_z >= 0 && current_z < CHUNK_DEPTH as i32 &&
                   current_y >= 0 && current_y < CHUNK_HEIGHT as i32 {
                    let cx = current_x as usize;
                    let cy = current_y as usize;
                    let cz = current_z as usize;
                    let b = self.blocks[cx][cy][cz];
                    if b == BlockType::Stone || b == BlockType::Dirt {
                        self.blocks[cx][cy][cz] = BlockType::Gravel;
                    }
                }

                let dir = rng.gen_range(0..6);
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
        
        let mut cave_rng = |mut seed: u64| -> f32 {
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
                                .wrapping_add((neighbor_z as i64).wrapping_mul(341873128712)) as u64;

                let num_caves = ((cave_rng(base_seed) * 15.0) as i32).max(0);
                
                for i in 0..num_caves {
                    let cave_seed = base_seed.wrapping_add((i * 9283711) as u64);
                    
                    let start_x = (neighbor_x * CHUNK_WIDTH as i32) as f32 + cave_rng(cave_seed) * 16.0;
                    let start_y = cave_rng(cave_seed.wrapping_add(1)) * 120.0 + 8.0;
                    let start_z = (neighbor_z * CHUNK_DEPTH as i32) as f32 + cave_rng(cave_seed.wrapping_add(2)) * 16.0;
                    
                    let mut length = cave_rng(cave_seed.wrapping_add(3)) * 100.0 + 10.0; // 10 to 110 steps
                    let mut radius = cave_rng(cave_seed.wrapping_add(4)) * 2.0 + 1.5;    // 1.5 to 3.5 base radius
                    
                    // 25% chance of a spherical room
                    if cave_rng(cave_seed.wrapping_add(5)) < 0.25 {
                        radius *= 2.0; 
                        length = 0.0; // Rooms don't immediately worm
                    }

                    // 10% chance of a massive tunnel starting
                    if cave_rng(cave_seed.wrapping_add(6)) < 0.10 {
                        radius *= (cave_rng(cave_seed.wrapping_add(7)) * 2.0 + 1.0); 
                    }

                    let mut current_x = start_x;
                    let mut current_y = start_y;
                    let mut current_z = start_z;
                    
                    let mut yaw = cave_rng(cave_seed.wrapping_add(8)) * std::f32::consts::PI * 2.0;
                    let mut pitch = (cave_rng(cave_seed.wrapping_add(9)) - 0.5) * std::f32::consts::PI * 0.5;
                    
                    let mut step = 0.0;
                    let mut s = cave_seed.wrapping_add(10);
                    
                    while step < length || length == 0.0 {
                        let rad = radius * (1.0 + (step / length.max(1.0)).sin());

                        // Carve the sphere at the current point
                        let min_cx = ((current_x - rad).floor() as i32) - (self.x * CHUNK_WIDTH as i32);
                        let max_cx = ((current_x + rad).ceil() as i32) - (self.x * CHUNK_WIDTH as i32);
                        let min_cy = (current_y - rad).floor() as i32;
                        let max_cy = (current_y + rad).ceil() as i32;
                        let min_cz = ((current_z - rad).floor() as i32) - (self.z * CHUNK_DEPTH as i32);
                        let max_cz = ((current_z + rad).ceil() as i32) - (self.z * CHUNK_DEPTH as i32);

                        for bx in min_cx.max(0)..=max_cx.min(CHUNK_WIDTH as i32 - 1) {
                            for by in min_cy.max(0)..=max_cy.min(CHUNK_HEIGHT as i32 - 1) {
                                for bz in min_cz.max(0)..=max_cz.min(CHUNK_DEPTH as i32 - 1) {
                                    let global_x = (self.x * CHUNK_WIDTH as i32 + bx) as f32;
                                    let global_y = by as f32;
                                    let global_z = (self.z * CHUNK_DEPTH as i32 + bz) as f32;
                                    
                                    let dist_sq = (global_x - current_x).powi(2) + 
                                                  ((global_y - current_y) * 1.5).powi(2) + 
                                                  (global_z - current_z).powi(2);
                                                  
                                    if dist_sq <= rad * rad {
                                        let current_block = self.blocks[bx as usize][by as usize][bz as usize];
                                        if matches!(current_block, BlockType::Stone | BlockType::Dirt | BlockType::Gravel | BlockType::Grass) {
                                            self.blocks[bx as usize][by as usize][bz as usize] = BlockType::Air;
                                        }
                                    }
                                }
                            }
                        }

                        if length == 0.0 { break; } // It was just a room

                        // Step the worm forward
                        current_x += yaw.cos() * pitch.cos();
                        current_y += pitch.sin();
                        current_z += yaw.sin() * pitch.cos();
                        
                        s = s.wrapping_add(1);
                        yaw += (cave_rng(s) - 0.5) * 0.4;
                        s = s.wrapping_add(1);
                        pitch += (cave_rng(s) - 0.5) * 0.4;
                        pitch = pitch.clamp(-std::f32::consts::PI * 0.4, std::f32::consts::PI * 0.4);

                        step += 1.0;
                    }
                }
            }
        }

        // PASS 6: Simple skylight (top-down sunlight)
        for x in 0..CHUNK_WIDTH {
            for z in 0..CHUNK_DEPTH {
                let mut sunlight: u8 = 15;
                for y in (0..CHUNK_HEIGHT).rev() {
                    let b = self.blocks[x][y][z];
                    match b {
                        BlockType::Air | BlockType::Water | BlockType::OakLeaves => {
                            self.light[x][y][z] = sunlight;
                        }
                        _ => {
                            sunlight = 0;
                        }
                    }
                }
            }
        }

        // Horizontal Light Propagation (Flood Fill)
        let mut light_changed = true;
        let mut iterations = 0;
        let max_iterations = 14;

        while light_changed && iterations < max_iterations {
            light_changed = false;
            iterations += 1;

            let old_light = self.light.clone();

            for x in 0..CHUNK_WIDTH {
                for y in 0..CHUNK_HEIGHT {
                    for z in 0..CHUNK_DEPTH {
                        let current_light = old_light[x][y][z];
                        if current_light == 0 { continue; }

                        let drop_light = current_light.saturating_sub(1);
                        if drop_light == 0 { continue; }

                        let neighbors = [
                            (x as i32 + 1, y as i32, z as i32),
                            (x as i32 - 1, y as i32, z as i32),
                            (x as i32, y as i32 + 1, z as i32),
                            (x as i32, y as i32 - 1, z as i32),
                            (x as i32, y as i32, z as i32 + 1),
                            (x as i32, y as i32, z as i32 - 1),
                        ];

                        for (nx, ny, nz) in neighbors {
                            if nx >= 0 && nx < CHUNK_WIDTH as i32 && ny >= 0 && ny < CHUNK_HEIGHT as i32 && nz >= 0 && nz < CHUNK_DEPTH as i32 {
                                let occluding = match self.blocks[nx as usize][ny as usize][nz as usize] {
                                    BlockType::Air | BlockType::Water | BlockType::OakLeaves => false,
                                    _ => true,
                                };

                                if !occluding && old_light[nx as usize][ny as usize][nz as usize] < drop_light {
                                    if self.light[nx as usize][ny as usize][nz as usize] < drop_light {
                                        self.light[nx as usize][ny as usize][nz as usize] = drop_light;
                                        light_changed = true;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, block: BlockType) {
        if x < CHUNK_WIDTH && y < CHUNK_HEIGHT && z < CHUNK_DEPTH {
            self.blocks[x][y][z] = block;
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

    pub fn build_mesh(&mut self, world: &crate::world::World) {
        let mut v_op = Vec::new();
        let mut t_op = Vec::new();
        let mut n_op = Vec::new();
        let mut c_op = Vec::new();

        let mut v_tr = Vec::new();
        let mut t_tr = Vec::new();
        let mut n_tr = Vec::new();
        let mut c_tr = Vec::new();




        let mut current_min_y = CHUNK_HEIGHT;
        let mut current_max_y = 0;

        // Cache neighbor chunks
        let n_px = world.get_chunk(self.x + 1, self.z);
        let n_nx = world.get_chunk(self.x - 1, self.z);
        let n_pz = world.get_chunk(self.x, self.z + 1);
        let n_nz = world.get_chunk(self.x, self.z - 1);

        let should_draw_face = |current: BlockType, neighbor: BlockType| -> bool {
            if neighbor == BlockType::Air { return true; }
            if neighbor == BlockType::Water && current != BlockType::Water { return true; }
            if neighbor == BlockType::OakLeaves && current != BlockType::OakLeaves { return true; }
            false
        };

        let get_block_safe = |wx: i32, wy: i32, wz: i32| -> BlockType {
            if wy < 0 || wy >= CHUNK_HEIGHT as i32 { return BlockType::Air; }
            
            // Local chunk coordinates
            let mut cx = wx - (self.x * CHUNK_WIDTH as i32);
            let mut cz = wz - (self.z * CHUNK_DEPTH as i32);
            
            if cx >= 0 && cx < CHUNK_WIDTH as i32 && cz >= 0 && cz < CHUNK_DEPTH as i32 {
                return self.blocks[cx as usize][wy as usize][cz as usize];
            }
            
            // Check neighbor chunks if out of bounds
            if cx < 0 {
                return n_nx.map_or(BlockType::Air, |c| c.blocks[(CHUNK_WIDTH as i32 + cx) as usize][wy as usize][if cz >= 0 && cz < CHUNK_DEPTH as i32 {cz as usize} else {0}]);
            } else if cx >= CHUNK_WIDTH as i32 {
                return n_px.map_or(BlockType::Air, |c| c.blocks[(cx - CHUNK_WIDTH as i32) as usize][wy as usize][if cz >= 0 && cz < CHUNK_DEPTH as i32 {cz as usize} else {0}]);
            } else if cz < 0 {
                return n_nz.map_or(BlockType::Air, |c| c.blocks[cx as usize][wy as usize][(CHUNK_DEPTH as i32 + cz) as usize]);
            } else if cz >= CHUNK_DEPTH as i32 {
                return n_pz.map_or(BlockType::Air, |c| c.blocks[cx as usize][wy as usize][(cz - CHUNK_DEPTH as i32) as usize]);
            }
            BlockType::Air
        };

        // Standard Voxel AO calculation 
        // Side1, Side2, Corner are the three blocks adjacent to the vertex
        let calc_ao = |s1: bool, s2: bool, c: bool| -> f32 {
            if s1 && s2 { return 0.4; }
            let mut occ = 0;
            if s1 { occ += 1; }
            if s2 { occ += 1; }
            if c { occ += 1; }
            match occ {
                0 => 1.0,
                1 => 0.8,
                2 => 0.6,
                _ => 0.4,
            }
        };

        let get_light_safe = |wx: i32, wy: i32, wz: i32| -> u8 {
            if wy < 0 || wy >= CHUNK_HEIGHT as i32 { return 15; }
            
            // Local chunk coordinates
            let mut cx = wx - (self.x * CHUNK_WIDTH as i32);
            let mut cz = wz - (self.z * CHUNK_DEPTH as i32);
            
            if cx >= 0 && cx < CHUNK_WIDTH as i32 && cz >= 0 && cz < CHUNK_DEPTH as i32 {
                return self.light[cx as usize][wy as usize][cz as usize];
            }
            
            // Check neighbor chunks if out of bounds
            if cx < 0 {
                return n_nx.map_or(15, |c| c.light[(CHUNK_WIDTH as i32 + cx) as usize][wy as usize][if cz >= 0 && cz < CHUNK_DEPTH as i32 {cz as usize} else {0}]);
            } else if cx >= CHUNK_WIDTH as i32 {
                return n_px.map_or(15, |c| c.light[(cx - CHUNK_WIDTH as i32) as usize][wy as usize][if cz >= 0 && cz < CHUNK_DEPTH as i32 {cz as usize} else {0}]);
            } else if cz < 0 {
                return n_nz.map_or(15, |c| c.light[cx as usize][wy as usize][(CHUNK_DEPTH as i32 + cz) as usize]);
            } else if cz >= CHUNK_DEPTH as i32 {
                return n_pz.map_or(15, |c| c.light[cx as usize][wy as usize][(cz - CHUNK_DEPTH as i32) as usize]);
            }
            15
        };

        let calc_light_f = |light_val: u8| -> f32 {
            let l = light_val as f32;
            let mut f = 0.82f32.powf(15.0 - l);
            if f < 0.05 { f = 0.05; }
            f
        };

        let calc_vertex_light = |l0: u8, l1: u8, l2: u8, l3: u8| -> f32 {
            let avg = (calc_light_f(l0) + calc_light_f(l1) + calc_light_f(l2) + calc_light_f(l3)) / 4.0;
            if avg < 0.05 { 0.05 } else { avg }
        };

        for x in 0..CHUNK_WIDTH {
            for y in 0..CHUNK_HEIGHT {
                for z in 0..CHUNK_DEPTH {
                    let block = self.blocks[x][y][z];
                    if block == BlockType::Air { continue; }

                    if y < current_min_y { current_min_y = y; }
                    if y > current_max_y { current_max_y = y; }

                    let is_t = block == BlockType::Water;
                    let (v, t, n, c) = if is_t {
                        (&mut v_tr, &mut t_tr, &mut n_tr, &mut c_tr)
                    } else {
                        (&mut v_op, &mut t_op, &mut n_op, &mut c_op)
                    };

                    let fx = (x as i32 + self.x * CHUNK_WIDTH as i32) as f32;
                    let fy = y as f32;
                    let fz = (z as i32 + self.z * CHUNK_DEPTH as i32) as f32;
                    let ts = 1.0 / 16.0;
                    let pad = 0.5 / 256.0; // Half a pixel padding to prevent mipmap/texture bleeding
                                let (tx, ty) = match block {
                                    BlockType::Stone => (1, 0),
                                    BlockType::Dirt => (2, 0),
                                    BlockType::Grass => (3, 0),
                                    BlockType::Bedrock => (1, 1),
                                    BlockType::Water => (13, 12),
                                    BlockType::OakLog => (4, 0),
                                    BlockType::OakLeaves => (5, 0),
                                    BlockType::Sand => (6, 0),
                                    BlockType::Gravel => (7, 0),
                                    BlockType::CoalOre => (2, 1),
                                    _ => (0, 0),
                                };

                                let u0 = tx as f32 * ts + pad;
                                let v0 = ty as f32 * ts + pad;
                                let u1 = (tx + 1) as f32 * ts - pad;
                                let v1 = (ty + 1) as f32 * ts - pad;

                    let w_off = -0.005;
                    let wx = x as i32 + self.x * CHUNK_WIDTH as i32;
                    let wy = y as i32;
                    let wz = z as i32 + self.z * CHUNK_DEPTH as i32;

                    let is_solid = |b: BlockType| match b {
                        BlockType::Air | BlockType::Water | BlockType::OakLeaves => false,
                        _ => true,
                    };

                    // Top
                    let neighbor_top = if y < CHUNK_HEIGHT - 1 { self.blocks[x][y + 1][z] } else { BlockType::Air };
                    if should_draw_face(block, neighbor_top) {
                        let (mut tu0, mut tv0, mut tu1, mut tv1) = (u0, v0, u1, v1);
                        if block == BlockType::Grass {
                            tu0 = 0.0; tv0 = 0.0; tu1 = ts - pad; tv1 = ts - pad;
                        } else if block == BlockType::OakLog {
                            tu0 = 5.0 * ts + pad; tv0 = ts + pad; tu1 = 6.0 * ts - pad; tv1 = 2.0 * ts - pad;
                        }

                        let l_00 = is_solid(get_block_safe(wx - 1, wy + 1, wz - 1));
                        let l_10 = is_solid(get_block_safe(wx,     wy + 1, wz - 1));
                        let l_20 = is_solid(get_block_safe(wx + 1, wy + 1, wz - 1));
                        let l_01 = is_solid(get_block_safe(wx - 1, wy + 1, wz    ));
                        let l_21 = is_solid(get_block_safe(wx + 1, wy + 1, wz    ));
                        let l_02 = is_solid(get_block_safe(wx - 1, wy + 1, wz + 1));
                        let l_12 = is_solid(get_block_safe(wx,     wy + 1, wz + 1));
                        let l_22 = is_solid(get_block_safe(wx + 1, wy + 1, wz + 1));

                        let ao00 = calc_ao(l_01, l_10, l_00); // 0,0 (-x, -z)
                        let ao10 = calc_ao(l_21, l_10, l_20); // 1,0 (+x, -z)
                        let ao01 = calc_ao(l_01, l_12, l_02); // 0,1 (-x, +z)
                        let ao11 = calc_ao(l_21, l_12, l_22); // 1,1 (+x, +z)

                        v.extend_from_slice(&[fx, fy + 1.0 - w_off, fz, fx, fy + 1.0 - w_off, fz + 1.0, fx + 1.0, fy + 1.0 - w_off, fz + 1.0, fx, fy + 1.0 - w_off, fz, fx + 1.0, fy + 1.0 - w_off, fz + 1.0, fx + 1.0, fy + 1.0 - w_off, fz]);
                        t.extend_from_slice(&[tu0, tv0, tu0, tv1, tu1, tv1, tu0, tv0, tu1, tv1, tu1, tv0]);
                        
                        let l_b = get_light_safe(wx, wy + 1, wz);
                        let ll_01 = get_light_safe(wx - 1, wy + 1, wz);
                        let ll_10 = get_light_safe(wx, wy + 1, wz - 1);
                        let ll_21 = get_light_safe(wx + 1, wy + 1, wz);
                        let ll_12 = get_light_safe(wx, wy + 1, wz + 1);
                        let ll_00 = get_light_safe(wx - 1, wy + 1, wz - 1);
                        let ll_20 = get_light_safe(wx + 1, wy + 1, wz - 1);
                        let ll_02 = get_light_safe(wx - 1, wy + 1, wz + 1);
                        let ll_22 = get_light_safe(wx + 1, wy + 1, wz + 1);

                        let light00 = calc_vertex_light(l_b, ll_01, ll_10, ll_00);
                        let light10 = calc_vertex_light(l_b, ll_21, ll_10, ll_20);
                        let light01 = calc_vertex_light(l_b, ll_01, ll_12, ll_02);
                        let light11 = calc_vertex_light(l_b, ll_21, ll_12, ll_22);

                        let base_shade = 1.0;
                        for _ in 0..6 { n.extend_from_slice(&[0.0, 1.0, 0.0]); }
                        
                        let c00 = (255.0 * light00 * base_shade * ao00) as u8;
                        let c10 = (255.0 * light10 * base_shade * ao10) as u8;
                        let c01 = (255.0 * light01 * base_shade * ao01) as u8;
                        let c11 = (255.0 * light11 * base_shade * ao11) as u8;
                        let a = if block == BlockType::Water { WATER_VERTEX_ALPHA } else { 255 };
                        c.extend_from_slice(&[c00, c00, c00, a, c01, c01, c01, a, c11, c11, c11, a, c00, c00, c00, a, c11, c11, c11, a, c10, c10, c10, a]);
                    }

                    // Bottom
                    let neighbor_bottom = if y > 0 { self.blocks[x][y - 1][z] } else { BlockType::Bedrock };
                    if should_draw_face(block, neighbor_bottom) {
                        let (mut tu0, mut tv0, mut tu1, mut tv1) = (u0, v0, u1, v1);
                        if block == BlockType::Grass {
                            tu0 = 2.0 * ts + pad; tv0 = 0.0; tu1 = 3.0 * ts - pad; tv1 = ts - pad;
                        } else if block == BlockType::OakLog {
                            tu0 = 5.0 * ts + pad; tv0 = ts + pad; tu1 = 6.0 * ts - pad; tv1 = 2.0 * ts - pad;
                        }

                        let l_00 = is_solid(get_block_safe(wx - 1, wy - 1, wz - 1));
                        let l_10 = is_solid(get_block_safe(wx,     wy - 1, wz - 1));
                        let l_20 = is_solid(get_block_safe(wx + 1, wy - 1, wz - 1));
                        let l_01 = is_solid(get_block_safe(wx - 1, wy - 1, wz    ));
                        let l_21 = is_solid(get_block_safe(wx + 1, wy - 1, wz    ));
                        let l_02 = is_solid(get_block_safe(wx - 1, wy - 1, wz + 1));
                        let l_12 = is_solid(get_block_safe(wx,     wy - 1, wz + 1));
                        let l_22 = is_solid(get_block_safe(wx + 1, wy - 1, wz + 1));

                        let ao00 = calc_ao(l_01, l_10, l_00);
                        let ao10 = calc_ao(l_21, l_10, l_20);
                        let ao01 = calc_ao(l_01, l_12, l_02);
                        let ao11 = calc_ao(l_21, l_12, l_22);

                        v.extend_from_slice(&[fx, fy, fz, fx + 1.0, fy, fz + 1.0, fx, fy, fz + 1.0, fx, fy, fz, fx + 1.0, fy, fz, fx + 1.0, fy, fz + 1.0]);
                        t.extend_from_slice(&[tu0, tv0, tu1, tv1, tu0, tv1, tu0, tv0, tu1, tv0, tu1, tv1]);
                        
                        let l_b = get_light_safe(wx, wy - 1, wz);
                        let ll_01 = get_light_safe(wx - 1, wy - 1, wz);
                        let ll_10 = get_light_safe(wx, wy - 1, wz - 1);
                        let ll_21 = get_light_safe(wx + 1, wy - 1, wz);
                        let ll_12 = get_light_safe(wx, wy - 1, wz + 1);
                        let ll_00 = get_light_safe(wx - 1, wy - 1, wz - 1);
                        let ll_20 = get_light_safe(wx + 1, wy - 1, wz - 1);
                        let ll_02 = get_light_safe(wx - 1, wy - 1, wz + 1);
                        let ll_22 = get_light_safe(wx + 1, wy - 1, wz + 1);

                        let light00 = calc_vertex_light(l_b, ll_01, ll_10, ll_00);
                        let light10 = calc_vertex_light(l_b, ll_21, ll_10, ll_20);
                        let light01 = calc_vertex_light(l_b, ll_01, ll_12, ll_02);
                        let light11 = calc_vertex_light(l_b, ll_21, ll_12, ll_22);

                        let base_shade = 0.5;
                        for _ in 0..6 { n.extend_from_slice(&[0.0, -1.0, 0.0]); }

                        let c00 = (255.0 * light00 * base_shade * ao00) as u8;
                        let c10 = (255.0 * light10 * base_shade * ao10) as u8;
                        let c01 = (255.0 * light01 * base_shade * ao01) as u8;
                        let c11 = (255.0 * light11 * base_shade * ao11) as u8;
                        let a = 255;
                        c.extend_from_slice(&[c00, c00, c00, a, c11, c11, c11, a, c01, c01, c01, a, c00, c00, c00, a, c10, c10, c10, a, c11, c11, c11, a]);
                    }

                    // Sides
                    // Z+
                    let neighbor_zp = if z < CHUNK_DEPTH - 1 { self.blocks[x][y][z + 1] } else { n_pz.map_or(BlockType::Air, |c| c.blocks[x][y][0]) };
                    if should_draw_face(block, neighbor_zp) {
                        let l_00 = is_solid(get_block_safe(wx - 1, wy - 1, wz + 1));
                        let l_10 = is_solid(get_block_safe(wx,     wy - 1, wz + 1));
                        let l_20 = is_solid(get_block_safe(wx + 1, wy - 1, wz + 1));
                        let l_01 = is_solid(get_block_safe(wx - 1, wy,     wz + 1));
                        let l_21 = is_solid(get_block_safe(wx + 1, wy,     wz + 1));
                        let l_02 = is_solid(get_block_safe(wx - 1, wy + 1, wz + 1));
                        let l_12 = is_solid(get_block_safe(wx,     wy + 1, wz + 1));
                        let l_22 = is_solid(get_block_safe(wx + 1, wy + 1, wz + 1));

                        let ao00 = calc_ao(l_01, l_10, l_00);
                        let ao10 = calc_ao(l_21, l_10, l_20);
                        let ao01 = calc_ao(l_01, l_12, l_02);
                        let ao11 = calc_ao(l_21, l_12, l_22);

                        v.extend_from_slice(&[fx, fy, fz + 1.0, fx + 1.0, fy, fz + 1.0, fx + 1.0, fy + 1.0 - w_off, fz + 1.0, fx, fy, fz + 1.0, fx + 1.0, fy + 1.0 - w_off, fz + 1.0, fx, fy + 1.0 - w_off, fz + 1.0]);
                        t.extend_from_slice(&[u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0]);
                        
                        let l_b = get_light_safe(wx, wy, wz + 1);
                        let ll_01 = get_light_safe(wx - 1, wy, wz + 1);
                        let ll_10 = get_light_safe(wx, wy - 1, wz + 1);
                        let ll_21 = get_light_safe(wx + 1, wy, wz + 1);
                        let ll_12 = get_light_safe(wx, wy + 1, wz + 1);
                        let ll_00 = get_light_safe(wx - 1, wy - 1, wz + 1);
                        let ll_20 = get_light_safe(wx + 1, wy - 1, wz + 1);
                        let ll_02 = get_light_safe(wx - 1, wy + 1, wz + 1);
                        let ll_22 = get_light_safe(wx + 1, wy + 1, wz + 1);

                        let light00 = calc_vertex_light(l_b, ll_01, ll_10, ll_00);
                        let light10 = calc_vertex_light(l_b, ll_21, ll_10, ll_20);
                        let light01 = calc_vertex_light(l_b, ll_01, ll_12, ll_02);
                        let light11 = calc_vertex_light(l_b, ll_21, ll_12, ll_22);

                        let base_shade = 0.6;
                        for _ in 0..6 { n.extend_from_slice(&[0.0, 0.0, 1.0]); }

                        let c00 = (255.0 * light00 * base_shade * ao00) as u8;
                        let c10 = (255.0 * light10 * base_shade * ao10) as u8;
                        let c01 = (255.0 * light01 * base_shade * ao01) as u8;
                        let c11 = (255.0 * light11 * base_shade * ao11) as u8;
                        let a = if block == BlockType::Water { WATER_VERTEX_ALPHA } else { 255 };
                        c.extend_from_slice(&[c00, c00, c00, a, c10, c10, c10, a, c11, c11, c11, a, c00, c00, c00, a, c11, c11, c11, a, c01, c01, c01, a]);
                    }
                    // Z-
                    let neighbor_zn = if z > 0 { self.blocks[x][y][z - 1] } else { n_nz.map_or(BlockType::Air, |c| c.blocks[x][y][CHUNK_DEPTH - 1]) };
                    if should_draw_face(block, neighbor_zn) {
                        let l_00 = is_solid(get_block_safe(wx + 1, wy - 1, wz - 1));
                        let l_10 = is_solid(get_block_safe(wx,     wy - 1, wz - 1));
                        let l_20 = is_solid(get_block_safe(wx - 1, wy - 1, wz - 1));
                        let l_01 = is_solid(get_block_safe(wx + 1, wy,     wz - 1));
                        let l_21 = is_solid(get_block_safe(wx - 1, wy,     wz - 1));
                        let l_02 = is_solid(get_block_safe(wx + 1, wy + 1, wz - 1));
                        let l_12 = is_solid(get_block_safe(wx,     wy + 1, wz - 1));
                        let l_22 = is_solid(get_block_safe(wx - 1, wy + 1, wz - 1));

                        let ao00 = calc_ao(l_01, l_10, l_00);
                        let ao10 = calc_ao(l_21, l_10, l_20);
                        let ao01 = calc_ao(l_01, l_12, l_02);
                        let ao11 = calc_ao(l_21, l_12, l_22);

                        v.extend_from_slice(&[fx + 1.0, fy, fz, fx, fy, fz, fx, fy + 1.0 - w_off, fz, fx + 1.0, fy, fz, fx, fy + 1.0 - w_off, fz, fx + 1.0, fy + 1.0 - w_off, fz]);
                        t.extend_from_slice(&[u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0]);
                        
                        let l_b = get_light_safe(wx, wy, wz - 1);
                        let ll_01 = get_light_safe(wx + 1, wy, wz - 1);
                        let ll_10 = get_light_safe(wx, wy - 1, wz - 1);
                        let ll_21 = get_light_safe(wx - 1, wy, wz - 1);
                        let ll_12 = get_light_safe(wx, wy + 1, wz - 1);
                        let ll_00 = get_light_safe(wx + 1, wy - 1, wz - 1);
                        let ll_20 = get_light_safe(wx - 1, wy - 1, wz - 1);
                        let ll_02 = get_light_safe(wx + 1, wy + 1, wz - 1);
                        let ll_22 = get_light_safe(wx - 1, wy + 1, wz - 1);

                        let light00 = calc_vertex_light(l_b, ll_01, ll_10, ll_00);
                        let light10 = calc_vertex_light(l_b, ll_21, ll_10, ll_20);
                        let light01 = calc_vertex_light(l_b, ll_01, ll_12, ll_02);
                        let light11 = calc_vertex_light(l_b, ll_21, ll_12, ll_22);

                        let base_shade = 0.6;
                        for _ in 0..6 { n.extend_from_slice(&[0.0, 0.0, -1.0]); }

                        let c00 = (255.0 * light00 * base_shade * ao00) as u8;
                        let c10 = (255.0 * light10 * base_shade * ao10) as u8;
                        let c01 = (255.0 * light01 * base_shade * ao01) as u8;
                        let c11 = (255.0 * light11 * base_shade * ao11) as u8;
                        let a = if block == BlockType::Water { WATER_VERTEX_ALPHA } else { 255 };
                        c.extend_from_slice(&[c00, c00, c00, a, c10, c10, c10, a, c11, c11, c11, a, c00, c00, c00, a, c11, c11, c11, a, c01, c01, c01, a]);
                    }
                    // X+
                    let neighbor_xp = if x < CHUNK_WIDTH - 1 { self.blocks[x + 1][y][z] } else { n_px.map_or(BlockType::Air, |c| c.blocks[0][y][z]) };
                    if should_draw_face(block, neighbor_xp) {
                        let l_00 = is_solid(get_block_safe(wx + 1, wy - 1, wz + 1));
                        let l_10 = is_solid(get_block_safe(wx + 1, wy - 1, wz    ));
                        let l_20 = is_solid(get_block_safe(wx + 1, wy - 1, wz - 1));
                        let l_01 = is_solid(get_block_safe(wx + 1, wy,     wz + 1));
                        let l_21 = is_solid(get_block_safe(wx + 1, wy,     wz - 1));
                        let l_02 = is_solid(get_block_safe(wx + 1, wy + 1, wz + 1));
                        let l_12 = is_solid(get_block_safe(wx + 1, wy + 1, wz    ));
                        let l_22 = is_solid(get_block_safe(wx + 1, wy + 1, wz - 1));

                        let ao00 = calc_ao(l_01, l_10, l_00);
                        let ao10 = calc_ao(l_21, l_10, l_20);
                        let ao01 = calc_ao(l_01, l_12, l_02);
                        let ao11 = calc_ao(l_21, l_12, l_22);

                        v.extend_from_slice(&[fx + 1.0, fy, fz + 1.0, fx + 1.0, fy, fz, fx + 1.0, fy + 1.0 - w_off, fz, fx + 1.0, fy, fz + 1.0, fx + 1.0, fy + 1.0 - w_off, fz, fx + 1.0, fy + 1.0 - w_off, fz + 1.0]);
                        t.extend_from_slice(&[u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0]);
                        
                        let l_b = get_light_safe(wx + 1, wy, wz);
                        let ll_01 = get_light_safe(wx + 1, wy, wz + 1);
                        let ll_10 = get_light_safe(wx + 1, wy - 1, wz);
                        let ll_21 = get_light_safe(wx + 1, wy, wz - 1);
                        let ll_12 = get_light_safe(wx + 1, wy + 1, wz);
                        let ll_00 = get_light_safe(wx + 1, wy - 1, wz + 1);
                        let ll_20 = get_light_safe(wx + 1, wy - 1, wz - 1);
                        let ll_02 = get_light_safe(wx + 1, wy + 1, wz + 1);
                        let ll_22 = get_light_safe(wx + 1, wy + 1, wz - 1);

                        let light00 = calc_vertex_light(l_b, ll_01, ll_10, ll_00);
                        let light10 = calc_vertex_light(l_b, ll_21, ll_10, ll_20);
                        let light01 = calc_vertex_light(l_b, ll_01, ll_12, ll_02);
                        let light11 = calc_vertex_light(l_b, ll_21, ll_12, ll_22);

                        let base_shade = 0.8;
                        for _ in 0..6 { n.extend_from_slice(&[1.0, 0.0, 0.0]); }

                        let c00 = (255.0 * light00 * base_shade * ao00) as u8;
                        let c10 = (255.0 * light10 * base_shade * ao10) as u8;
                        let c01 = (255.0 * light01 * base_shade * ao01) as u8;
                        let c11 = (255.0 * light11 * base_shade * ao11) as u8;
                        let a = if block == BlockType::Water { WATER_VERTEX_ALPHA } else { 255 };
                        c.extend_from_slice(&[c00, c00, c00, a, c10, c10, c10, a, c11, c11, c11, a, c00, c00, c00, a, c11, c11, c11, a, c01, c01, c01, a]);
                    }
                    // X-
                    let neighbor_xn = if x > 0 { self.blocks[x - 1][y][z] } else { n_nx.map_or(BlockType::Air, |c| c.blocks[CHUNK_WIDTH - 1][y][z]) };
                    if should_draw_face(block, neighbor_xn) {
                        let l_00 = is_solid(get_block_safe(wx - 1, wy - 1, wz - 1));
                        let l_10 = is_solid(get_block_safe(wx - 1, wy - 1, wz    ));
                        let l_20 = is_solid(get_block_safe(wx - 1, wy - 1, wz + 1));
                        let l_01 = is_solid(get_block_safe(wx - 1, wy,     wz - 1));
                        let l_21 = is_solid(get_block_safe(wx - 1, wy,     wz + 1));
                        let l_02 = is_solid(get_block_safe(wx - 1, wy + 1, wz - 1));
                        let l_12 = is_solid(get_block_safe(wx - 1, wy + 1, wz    ));
                        let l_22 = is_solid(get_block_safe(wx - 1, wy + 1, wz + 1));

                        let ao00 = calc_ao(l_01, l_10, l_00);
                        let ao10 = calc_ao(l_21, l_10, l_20);
                        let ao01 = calc_ao(l_01, l_12, l_02);
                        let ao11 = calc_ao(l_21, l_12, l_22);

                        v.extend_from_slice(&[fx, fy, fz, fx, fy, fz + 1.0, fx, fy + 1.0 - w_off, fz + 1.0, fx, fy, fz, fx, fy + 1.0 - w_off, fz + 1.0, fx, fy + 1.0 - w_off, fz]);
                        t.extend_from_slice(&[u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0]);
                        
                        let l_b = get_light_safe(wx - 1, wy, wz);
                        let ll_01 = get_light_safe(wx - 1, wy, wz - 1);
                        let ll_10 = get_light_safe(wx - 1, wy - 1, wz);
                        let ll_21 = get_light_safe(wx - 1, wy, wz + 1);
                        let ll_12 = get_light_safe(wx - 1, wy + 1, wz);
                        let ll_00 = get_light_safe(wx - 1, wy - 1, wz - 1);
                        let ll_20 = get_light_safe(wx - 1, wy - 1, wz + 1);
                        let ll_02 = get_light_safe(wx - 1, wy + 1, wz - 1);
                        let ll_22 = get_light_safe(wx - 1, wy + 1, wz + 1);

                        let light00 = calc_vertex_light(l_b, ll_01, ll_10, ll_00);
                        let light10 = calc_vertex_light(l_b, ll_21, ll_10, ll_20);
                        let light01 = calc_vertex_light(l_b, ll_01, ll_12, ll_02);
                        let light11 = calc_vertex_light(l_b, ll_21, ll_12, ll_22);

                        let base_shade = 0.8;
                        for _ in 0..6 { n.extend_from_slice(&[-1.0, 0.0, 0.0]); }

                        let c00 = (255.0 * light00 * base_shade * ao00) as u8;
                        let c10 = (255.0 * light10 * base_shade * ao10) as u8;
                        let c01 = (255.0 * light01 * base_shade * ao01) as u8;
                        let c11 = (255.0 * light11 * base_shade * ao11) as u8;
                        let a = if block == BlockType::Water { WATER_VERTEX_ALPHA } else { 255 };
                        c.extend_from_slice(&[c00, c00, c00, a, c10, c10, c10, a, c11, c11, c11, a, c00, c00, c00, a, c11, c11, c11, a, c01, c01, c01, a]);
                    }
                }
            }
        }

        self.mesh_opaque = if v_op.is_empty() { None } else { Some(Mesh::new(&v_op, Some(&t_op), Some(&n_op), Some(&c_op))) };
        self.mesh_transparent = if v_tr.is_empty() { None } else { Some(Mesh::new(&v_tr, Some(&t_tr), Some(&n_tr), Some(&c_tr))) };
        self.min_y = current_min_y;
        self.max_y = current_max_y;
        self.dirty = false;
    }
}
