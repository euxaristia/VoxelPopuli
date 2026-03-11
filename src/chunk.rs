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

        let ts = 1.0 / 16.0;
        let pad = 0.0001;

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
                    let w_off = 0.0;
                    let bleed = 0.005;

                    // Top
                    let neighbor_top = if y < CHUNK_HEIGHT - 1 { self.blocks[x][y + 1][z] } else { BlockType::Air };
                    if should_draw_face(block, neighbor_top) {
                        let (mut tu0, mut tv0, mut tu1, mut tv1) = (u0, v0, u1, v1);
                        if block == BlockType::Grass {
                            tu0 = 0.0; tv0 = 0.0; tu1 = ts - pad; tv1 = ts - pad;
                        } else if block == BlockType::OakLog {
                            tu0 = 5.0 * ts + pad; tv0 = ts + pad; tu1 = 6.0 * ts - pad; tv1 = 2.0 * ts - pad;
                        }
                        v.extend_from_slice(&[
                            fx - bleed, fy + 1.0 - w_off, fz - bleed, 
                            fx - bleed, fy + 1.0 - w_off, fz + 1.0 + bleed, 
                            fx + 1.0 + bleed, fy + 1.0 - w_off, fz + 1.0 + bleed, 
                            fx - bleed, fy + 1.0 - w_off, fz - bleed, 
                            fx + 1.0 + bleed, fy + 1.0 - w_off, fz + 1.0 + bleed, 
                            fx + 1.0 + bleed, fy + 1.0 - w_off, fz - bleed
                        ]);
                        t.extend_from_slice(&[tu0, tv0, tu0, tv1, tu1, tv1, tu0, tv0, tu1, tv1, tu1, tv0]);
                        let light = self.light[x][y.min(CHUNK_HEIGHT - 1)][z];
                        let mut light_f = light as f32 / 15.0;
                        if light_f < 0.7 { light_f = 0.7; }
                        for _ in 0..6 {
                            n.extend_from_slice(&[0.0, 1.0, 0.0]);
                            let val = (255.0 * light_f) as u8;
                            c.extend_from_slice(&[val, val, val, if block == BlockType::Water { WATER_VERTEX_ALPHA } else { 255 }]);
                        }
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
                        v.extend_from_slice(&[
                            fx - bleed, fy, fz - bleed, 
                            fx + 1.0 + bleed, fy, fz + 1.0 + bleed, 
                            fx - bleed, fy, fz + 1.0 + bleed, 
                            fx - bleed, fy, fz - bleed, 
                            fx + 1.0 + bleed, fy, fz - bleed, 
                            fx + 1.0 + bleed, fy, fz + 1.0 + bleed
                        ]);
                        t.extend_from_slice(&[tu0, tv0, tu1, tv1, tu0, tv1, tu0, tv0, tu1, tv0, tu1, tv1]);
                        let light = if y > 0 { self.light[x][y - 1][z] } else { 0 };
                        let mut light_f = light as f32 / 15.0;
                        if light_f < 0.05 { light_f = 0.05; }
                        for _ in 0..6 {
                            n.extend_from_slice(&[0.0, -1.0, 0.0]);
                            let val = (120.0 * light_f) as u8;
                            c.extend_from_slice(&[val, val, val, 255]);
                        }
                    }

                    // Sides
                    // Z+
                    let neighbor_zp = if z < CHUNK_DEPTH - 1 { self.blocks[x][y][z + 1] } else { n_pz.map_or(BlockType::Air, |c| c.blocks[x][y][0]) };
                    if should_draw_face(block, neighbor_zp) {
                        v.extend_from_slice(&[
                            fx - bleed, fy - bleed, fz + 1.0, 
                            fx + 1.0 + bleed, fy - bleed, fz + 1.0, 
                            fx + 1.0 + bleed, fy + 1.0 + bleed - w_off, fz + 1.0, 
                            fx - bleed, fy - bleed, fz + 1.0, 
                            fx + 1.0 + bleed, fy + 1.0 + bleed - w_off, fz + 1.0, 
                            fx - bleed, fy + 1.0 + bleed - w_off, fz + 1.0
                        ]);
                        t.extend_from_slice(&[u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0]);
                        let light = if z < CHUNK_DEPTH - 1 { self.light[x][y][z + 1] } else { 15 };
                        let mut light_f = light as f32 / 15.0;
                        if block == BlockType::Water && light_f < 0.7 { light_f = 0.7; }
                        if light_f < 0.05 { light_f = 0.05; }
                        for _ in 0..6 {
                            n.extend_from_slice(&[0.0, 0.0, 1.0]);
                            let val = (255.0 * 0.6 * light_f) as u8;
                            c.extend_from_slice(&[val, val, val, if block == BlockType::Water { WATER_VERTEX_ALPHA } else { 255 }]);
                        }
                    }
                    // Z-
                    let neighbor_zn = if z > 0 { self.blocks[x][y][z - 1] } else { n_nz.map_or(BlockType::Air, |c| c.blocks[x][y][CHUNK_DEPTH - 1]) };
                    if should_draw_face(block, neighbor_zn) {
                        v.extend_from_slice(&[
                            fx + 1.0 + bleed, fy - bleed, fz, 
                            fx - bleed, fy - bleed, fz, 
                            fx - bleed, fy + 1.0 + bleed - w_off, fz, 
                            fx + 1.0 + bleed, fy - bleed, fz, 
                            fx - bleed, fy + 1.0 + bleed - w_off, fz, 
                            fx + 1.0 + bleed, fy + 1.0 + bleed - w_off, fz
                        ]);
                        t.extend_from_slice(&[u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0]);
                        let light = if z > 0 { self.light[x][y][z - 1] } else { 15 };
                        let mut light_f = light as f32 / 15.0;
                        if block == BlockType::Water && light_f < 0.7 { light_f = 0.7; }
                        if light_f < 0.05 { light_f = 0.05; }
                        for _ in 0..6 {
                            n.extend_from_slice(&[0.0, 0.0, -1.0]);
                            let val = (255.0 * 0.6 * light_f) as u8;
                            c.extend_from_slice(&[val, val, val, if block == BlockType::Water { WATER_VERTEX_ALPHA } else { 255 }]);
                        }
                    }
                    // X+
                    let neighbor_xp = if x < CHUNK_WIDTH - 1 { self.blocks[x + 1][y][z] } else { n_px.map_or(BlockType::Air, |c| c.blocks[0][y][z]) };
                    if should_draw_face(block, neighbor_xp) {
                        v.extend_from_slice(&[
                            fx + 1.0, fy - bleed, fz + 1.0 + bleed, 
                            fx + 1.0, fy - bleed, fz - bleed, 
                            fx + 1.0, fy + 1.0 + bleed - w_off, fz - bleed, 
                            fx + 1.0, fy - bleed, fz + 1.0 + bleed, 
                            fx + 1.0, fy + 1.0 + bleed - w_off, fz - bleed, 
                            fx + 1.0, fy + 1.0 + bleed - w_off, fz + 1.0 + bleed
                        ]);
                        t.extend_from_slice(&[u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0]);
                        let light = if x < CHUNK_WIDTH - 1 { self.light[x + 1][y][z] } else { 15 };
                        let mut light_f = light as f32 / 15.0;
                        if block == BlockType::Water && light_f < 0.7 { light_f = 0.7; }
                        if light_f < 0.05 { light_f = 0.05; }
                        for _ in 0..6 {
                            n.extend_from_slice(&[1.0, 0.0, 0.0]);
                            let val = (255.0 * 0.8 * light_f) as u8;
                            c.extend_from_slice(&[val, val, val, if block == BlockType::Water { WATER_VERTEX_ALPHA } else { 255 }]);
                        }
                    }
                    // X-
                    let neighbor_xn = if x > 0 { self.blocks[x - 1][y][z] } else { n_nx.map_or(BlockType::Air, |c| c.blocks[CHUNK_WIDTH - 1][y][z]) };
                    if should_draw_face(block, neighbor_xn) {
                        v.extend_from_slice(&[
                            fx, fy - bleed, fz - bleed, 
                            fx, fy - bleed, fz + 1.0 + bleed, 
                            fx, fy + 1.0 + bleed - w_off, fz + 1.0 + bleed, 
                            fx, fy - bleed, fz - bleed, 
                            fx, fy + 1.0 + bleed - w_off, fz + 1.0 + bleed, 
                            fx, fy + 1.0 + bleed - w_off, fz - bleed
                        ]);
                        t.extend_from_slice(&[u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0]);
                        let light = if x > 0 { self.light[x - 1][y][z] } else { 15 };
                        let mut light_f = light as f32 / 15.0;
                        if block == BlockType::Water && light_f < 0.7 { light_f = 0.7; }
                        if light_f < 0.05 { light_f = 0.05; }
                        for _ in 0..6 {
                            n.extend_from_slice(&[-1.0, 0.0, 0.0]);
                            let val = (255.0 * 0.8 * light_f) as u8;
                            c.extend_from_slice(&[val, val, val, if block == BlockType::Water { WATER_VERTEX_ALPHA } else { 255 }]);
                        }
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
