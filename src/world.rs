use crate::block::BlockType;
use crate::chunk::{
    CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, Chunk, water_decay, water_is_falling, water_is_source,
    water_level_from_decay,
};
use crate::renderer;
use crate::renderer::{Mesh, Shader, Texture2D};
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use std::sync::RwLock;

pub const VIEW_DISTANCE: i32 = 35;
pub const POOL_WIDTH: i32 = VIEW_DISTANCE * 2 + 1;
pub const CHUNK_POOL_SIZE: usize = (POOL_WIDTH * POOL_WIDTH) as usize;

// WorldEdit removed in favor of HashMap edits

pub struct RaycastResult {
    pub hit: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub nx: i32,
    pub ny: i32,
    pub nz: i32,
    #[allow(dead_code)]
    pub block: BlockType,
}

pub struct Frustum {
    pub planes: [[f32; 4]; 6],
}

impl Frustum {
    pub fn from_matrix(m: &Mat4) -> Self {
        let mut planes = [[0.0; 4]; 6];
        let mat = m.to_cols_array_2d();

        // Right
        planes[0] = [
            mat[0][3] - mat[0][0],
            mat[1][3] - mat[1][0],
            mat[2][3] - mat[2][0],
            mat[3][3] - mat[3][0],
        ];
        // Left
        planes[1] = [
            mat[0][3] + mat[0][0],
            mat[1][3] + mat[1][0],
            mat[2][3] + mat[2][0],
            mat[3][3] + mat[3][0],
        ];
        // Bottom
        planes[2] = [
            mat[0][3] + mat[0][1],
            mat[1][3] + mat[1][1],
            mat[2][3] + mat[2][1],
            mat[3][3] + mat[3][1],
        ];
        // Top
        planes[3] = [
            mat[0][3] - mat[0][1],
            mat[1][3] - mat[1][1],
            mat[2][3] - mat[2][1],
            mat[3][3] - mat[3][1],
        ];
        // Far
        planes[4] = [
            mat[0][3] - mat[0][2],
            mat[1][3] - mat[1][2],
            mat[2][3] - mat[2][2],
            mat[3][3] - mat[3][2],
        ];
        // Near
        planes[5] = [
            mat[0][3] + mat[0][2],
            mat[1][3] + mat[1][2],
            mat[2][3] + mat[2][2],
            mat[3][3] + mat[3][2],
        ];

        for plane in planes.iter_mut() {
            let length = (plane[0] * plane[0] + plane[1] * plane[1] + plane[2] * plane[2]).sqrt();
            if length > 0.0 {
                for val in plane.iter_mut() {
                    *val /= length;
                }
            }
        }

        Self { planes }
    }

    pub fn is_box_visible(&self, min: Vec3, max: Vec3) -> bool {
        for plane in &self.planes {
            let px = if plane[0] > 0.0 { max.x } else { min.x };
            let py = if plane[1] > 0.0 { max.y } else { min.y };
            let pz = if plane[2] > 0.0 { max.z } else { min.z };
            if plane[0] * px + plane[1] * py + plane[2] * pz + plane[3] <= 0.0 {
                return false;
            }
        }
        true
    }
}

pub struct World {
    pub chunks: Vec<Option<Box<Chunk>>>,
    pub atlas: Option<Texture2D>,
    pub cloud_model: Option<Mesh>,
    pub star_model: Option<Mesh>,
    pub last_cloud_pos: Vec3,
    pub last_cloud_update_time: f32,
    pub edits: RwLock<HashMap<(i32, i32, i32), BlockType>>,
    pub last_pcx: i32,
    pub last_pcz: i32,
    pub dirty_count: i32,
    pub explosives: Vec<crate::block::ActiveExplosive>,
    pub particles: Vec<crate::block::Particle>,
    pub detonations: Vec<(glam::Vec3, crate::block::BlockType)>,
    pub cube_mesh: renderer::Mesh,
    pub is_loading: bool,
    pub loading_radius: i32,
    pub chunks_generated_count: i32,
    pub active_water: std::collections::HashSet<(i32, i32, i32)>,
    pub active_falling: std::collections::HashSet<(i32, i32, i32)>,
    pub water_tick_timer: f32,
    pub visible_chunks: Vec<usize>,
    pub meshing_in_flight: i32,
}

impl World {
    pub fn new() -> Self {
        let mut chunks = Vec::with_capacity(CHUNK_POOL_SIZE);
        for _ in 0..CHUNK_POOL_SIZE {
            chunks.push(None);
        }
        Self {
            chunks,
            atlas: None,
            cloud_model: None,
            star_model: None,
            last_cloud_pos: Vec3::new(-999999.0, -999999.0, -999999.0),
            last_cloud_update_time: -999.0,
            edits: RwLock::new(HashMap::new()),
            last_pcx: -999999,
            last_pcz: -999999,
            dirty_count: 0,
            explosives: Vec::new(),
            particles: Vec::new(),
            detonations: Vec::new(),
            cube_mesh: Self::create_cube_mesh(),
            is_loading: true,
            loading_radius: 0,
            chunks_generated_count: 0,
            active_water: std::collections::HashSet::new(),
            active_falling: std::collections::HashSet::new(),
            water_tick_timer: 0.0,
            visible_chunks: Vec::new(),
            meshing_in_flight: 0,
        }
    }

    fn create_cube_mesh() -> renderer::Mesh {
        let v: Vec<f32> = vec![
            0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0,
            0.0, // bottom
            0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0,
            0.0, // top
            0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 0.0,
            0.0, // front
            0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0,
            1.0, // back
            0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, 0.0,
            1.0, // left
            1.0, 0.0, 0.0, 1.0, 1.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 0.0,
            1.0, // right
        ];
        let t: Vec<f32> = vec![0.0; v.len() / 3 * 2];
        let n: Vec<f32> = vec![0.0; v.len()];
        let c: Vec<u8> = vec![255; v.len() / 3 * 4];
        renderer::Mesh::new(&v, Some(&t), Some(&n), Some(&c))
    }

    pub fn render_explosives(
        &self,
        shader: &crate::renderer::Shader,
        _mvp: &glam::Mat4,
        current_time: f32,
    ) {
        let _loc_mvp = shader.get_uniform_location("uMVP");
        let loc_model = shader.get_uniform_location("uModel");
        let loc_diff = shader.get_uniform_location("colDiffuse");

        for e in &self.explosives {
            let flash = (current_time * 4.0) as i32 % 2 == 0;
            let diffuse = if flash {
                glam::Vec4::new(4.0, 4.0, 4.0, 1.0)
            } else {
                glam::Vec4::ONE
            };
            shader.set_vec4(loc_diff, diffuse);

            let model = glam::Mat4::from_translation(e.position);
            shader.set_mat4(loc_model, &model);
            self.cube_mesh.draw();
        }
        shader.set_vec4(loc_diff, glam::Vec4::ONE);
    }

    pub fn render_particles(&self, shader: &crate::renderer::Shader, _mvp: &glam::Mat4) {
        let loc_model = shader.get_uniform_location("uModel");
        let loc_diff = shader.get_uniform_location("colDiffuse");

        for p in &self.particles {
            let alpha = (p.life / p.max_life).clamp(0.0, 1.0);
            shader.set_vec4(loc_diff, glam::Vec4::new(1.0, 1.0, 1.0, alpha));

            let model = glam::Mat4::from_translation(p.position)
                * glam::Mat4::from_scale(glam::Vec3::splat(p.scale * alpha));
            shader.set_mat4(loc_model, &model);
            self.cube_mesh.draw();
        }
    }

    fn get_pool_index(&self, cx: i32, cz: i32) -> usize {
        let ix = cx.rem_euclid(POOL_WIDTH);
        let iz = cz.rem_euclid(POOL_WIDTH);
        (ix + iz * POOL_WIDTH) as usize
    }

    pub fn get_chunk(&self, cx: i32, cz: i32) -> Option<&Chunk> {
        let index = self.get_pool_index(cx, cz);
        if let Some(chunk) = &self.chunks[index]
            && chunk.x == cx
            && chunk.z == cz
        {
            return Some(chunk.as_ref());
        }
        None
    }

    pub fn get_chunk_mut(&mut self, cx: i32, cz: i32) -> Option<&mut Chunk> {
        let index = self.get_pool_index(cx, cz);
        if let Some(chunk) = &mut self.chunks[index]
            && chunk.x == cx
            && chunk.z == cz
        {
            return Some(chunk.as_mut());
        }
        None
    }

    pub fn get_chunk_ptr(&self, cx: i32, cz: i32) -> Option<*const Chunk> {
        let index = self.get_pool_index(cx, cz);
        if let Some(chunk) = &self.chunks[index]
            && chunk.x == cx
            && chunk.z == cz
        {
            return Some(chunk.as_ref() as *const Chunk);
        }
        None
    }

    pub fn get_block(&self, x: i32, y: i32, z: i32) -> BlockType {
        if y < 0 || y >= CHUNK_HEIGHT as i32 {
            return BlockType::Air;
        }

        // 1. Prioritize loaded chunk data (Actual World State)
        let cx = x.div_euclid(CHUNK_WIDTH as i32);
        let cz = z.div_euclid(CHUNK_DEPTH as i32);
        if let Some(chunk) = self.get_chunk(cx, cz) {
            let bx = x.rem_euclid(CHUNK_WIDTH as i32) as usize;
            let bz = z.rem_euclid(CHUNK_DEPTH as i32) as usize;
            return chunk.get_block(bx, y as usize, bz);
        }

        // 2. Fallback to edits (Unloaded Changes)
        if let Ok(edits) = self.edits.read()
            && let Some(b) = edits.get(&(x, y, z))
        {
            return *b;
        }

        BlockType::Air
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block: BlockType) {
        if y < 0 || y >= CHUNK_HEIGHT as i32 {
            return;
        }

        if let Ok(mut edits) = self.edits.write() {
            edits.insert((x, y, z), block);
        }

        let cx = x.div_euclid(CHUNK_WIDTH as i32);
        let cz = z.div_euclid(CHUNK_DEPTH as i32);
        let bx = x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        let bz = z.rem_euclid(CHUNK_DEPTH as i32) as usize;

        if let Some(chunk) = self.get_chunk_mut(cx, cz) {
            chunk.set_block(bx, y as usize, bz, block);
            if !chunk.dirty {
                chunk.dirty = true;
                self.dirty_count += 1;
            }

            if block == BlockType::Water {
                self.active_water.insert((x, y, z));
            } else if block == BlockType::Sand || block == BlockType::Gravel {
                self.active_falling.insert((x, y, z));
            }
            // Neighbors might now be able to flow or fall
            let neighbors = [
                (x, y + 1, z),
                (x, y - 1, z),
                (x + 1, y, z),
                (x - 1, y, z),
                (x, y, z + 1),
                (x, y, z - 1),
            ];
            for (nx, ny, nz) in neighbors {
                let nb = self.get_block(nx, ny, nz);
                if nb == BlockType::Water {
                    self.active_water.insert((nx, ny, nz));
                } else if nb == BlockType::Sand || nb == BlockType::Gravel {
                    self.active_falling.insert((nx, ny, nz));
                }
            }

            let mut neighbors = Vec::new();
            if bx == 0 {
                neighbors.push((cx - 1, cz));
            }
            if bx == CHUNK_WIDTH - 1 {
                neighbors.push((cx + 1, cz));
            }
            if bz == 0 {
                neighbors.push((cx, cz - 1));
            }
            if bz == CHUNK_DEPTH - 1 {
                neighbors.push((cx, cz + 1));
            }

            for (nx, nz) in neighbors {
                if let Some(nc) = self.get_chunk_mut(nx, nz)
                    && !nc.dirty
                {
                    nc.dirty = true;
                    self.dirty_count += 1;
                }
            }
        }
    }

    pub fn apply_edits_to_chunk(&mut self, cx: i32, cz: i32) {
        let chunk_start_x = cx * CHUNK_WIDTH as i32;
        let chunk_start_z = cz * CHUNK_DEPTH as i32;
        let chunk_end_x = chunk_start_x + CHUNK_WIDTH as i32;
        let chunk_end_z = chunk_start_z + CHUNK_DEPTH as i32;

        let edits_clone = if let Ok(edits) = self.edits.read() {
            edits.clone()
        } else {
            return;
        };

        if let Some(chunk) = self.get_chunk_mut(cx, cz) {
            for ((ex, ey, ez), block) in edits_clone {
                if ex >= chunk_start_x
                    && ex < chunk_end_x
                    && ez >= chunk_start_z
                    && ez < chunk_end_z
                {
                    let bx = (ex - chunk_start_x) as usize;
                    let bz = (ez - chunk_start_z) as usize;
                    chunk.set_block(bx, ey as usize, bz, block);
                    chunk.dirty = true;
                }
            }
        }
    }

    pub fn generate_atlas(&mut self) {
        let mut data = vec![0u8; 256 * 256 * 4];
        let mut draw_pixel = |x: i32, y: i32, r: u8, g: u8, b: u8, a: u8| {
            if (0..256).contains(&x) && (0..256).contains(&y) {
                let idx = ((y * 256 + x) * 4) as usize;
                data[idx] = r;
                data[idx + 1] = g;
                data[idx + 2] = b;
                data[idx + 3] = a;
            }
        };
        {
            let mut draw_block = |tx: i32, ty: i32, r: u8, g: u8, b: u8| {
                for x in 0..16 {
                    for y in 0..16 {
                        let noise =
                            (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 20.0) as i32;
                        let nr = (r as i32 + noise).clamp(0, 255) as u8;
                        let ng = (g as i32 + noise).clamp(0, 255) as u8;
                        let nb = (b as i32 + noise).clamp(0, 255) as u8;
                        draw_pixel(tx * 16 + x, ty * 16 + y, nr, ng, nb, 255);
                    }
                }
            };
            draw_block(1, 0, 140, 140, 140);
            draw_block(2, 0, 130, 90, 60);
            draw_block(3, 0, 80, 160, 40);
            draw_block(0, 0, 100, 180, 50);
            draw_block(4, 0, 100, 70, 40);
            draw_block(5, 1, 110, 80, 50);
            draw_block(5, 0, 40, 120, 30);
            draw_block(7, 0, 120, 120, 130);
            draw_block(1, 1, 60, 60, 60);
            draw_block(2, 1, 140, 140, 140);
            draw_block(13, 12, 40, 80, 200);
            draw_block(8, 0, 240, 245, 250); // Snow
            draw_block(9, 0, 240, 245, 250); // SnowyGrass top (same as snow)
            draw_block(11, 0, 60, 40, 25); // SpruceLog
            draw_block(12, 0, 30, 80, 30); // SpruceLeaves
            draw_block(6, 1, 230, 230, 230); // IronBlock
            draw_block(7, 1, 210, 160, 130); // RawIron item
            draw_block(8, 1, 245, 245, 245); // IronIngot item

            // Authentic Flint & Steel (hand-drawn pixels from image)
            let fs_p: [(u8, u8, u8, u8); 256] = [
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (153, 153, 153, 255),
                (180, 180, 180, 255),
                (153, 153, 153, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (153, 153, 153, 255),
                (180, 180, 180, 255),
                (180, 180, 180, 255),
                (153, 153, 153, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (120, 120, 120, 255),
                (153, 153, 153, 255),
                (180, 180, 180, 255),
                (153, 153, 153, 255),
                (120, 120, 120, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (120, 120, 120, 255),
                (153, 153, 153, 255),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (153, 153, 153, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (120, 120, 120, 255),
                (120, 120, 120, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (153, 153, 153, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (153, 153, 153, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (60, 60, 60, 255),
                (60, 60, 60, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (153, 153, 153, 255),
                (153, 153, 153, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (60, 60, 60, 255),
                (100, 100, 100, 255),
                (100, 100, 100, 255),
                (60, 60, 60, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (100, 100, 100, 255),
                (153, 153, 153, 255),
                (110, 110, 110, 255),
                (60, 60, 60, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (120, 120, 120, 255),
                (60, 60, 60, 255),
                (100, 100, 100, 255),
                (60, 60, 60, 255),
                (46, 46, 46, 255),
                (15, 15, 15, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (60, 60, 60, 255),
                (60, 60, 60, 255),
                (60, 60, 60, 255),
                (60, 60, 60, 255),
                (46, 46, 46, 255),
                (15, 15, 15, 255),
                (15, 15, 15, 255),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (60, 60, 60, 255),
                (60, 60, 60, 255),
                (60, 60, 60, 255),
                (60, 60, 60, 255),
                (46, 46, 46, 255),
                (15, 15, 15, 255),
                (15, 15, 15, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (60, 60, 60, 255),
                (60, 60, 60, 255),
                (46, 46, 46, 255),
                (15, 15, 15, 255),
                (15, 15, 15, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (46, 46, 46, 255),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
                (0, 0, 0, 0),
            ];
            for y in 0..16 {
                for x in 0..16 {
                    let (r, g, b, a) = fs_p[y * 16 + x];
                    if a > 0 {
                        draw_pixel(9 * 16 + x as i32, 16 + y as i32, r, g, b, a);
                    }
                }
            }
        }

        // IronOre: Stone with pinkish-tan spots
        {
            let mut draw_block = |tx: i32, ty: i32| {
                for x in 0..16 {
                    for y in 0..16 {
                        let noise =
                            (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 20.0) as i32;
                        let nr = (140i32 + noise).clamp(0, 255) as u8;
                        let ng = (140i32 + noise).clamp(0, 255) as u8;
                        let nb = (140i32 + noise).clamp(0, 255) as u8;
                        draw_pixel(tx * 16 + x, ty * 16 + y, nr, ng, nb, 255);
                    }
                }
            };
            draw_block(3, 1);
            for x in 0..16 {
                for y in 0..16 {
                    if (x * 7 + y * 3) % 11 == 0 {
                        draw_pixel(3 * 16 + x, 16 + y, 220, 180, 150, 255);
                    }
                }
            }
        }

        // TNT: Red with white stripe and "TNT" text
        let tnt_pixels: [(u8, u8, u8); 256] = [
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (124, 124, 124),
            (124, 124, 124),
            (124, 124, 124),
            (124, 124, 124),
            (124, 124, 124),
            (124, 124, 124),
            (124, 124, 124),
            (124, 124, 124),
            (124, 124, 124),
            (124, 124, 124),
            (124, 124, 124),
            (124, 124, 124),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (198, 198, 198),
            (32, 32, 32),
            (32, 32, 32),
            (32, 32, 32),
            (198, 198, 198),
            (32, 32, 32),
            (32, 32, 32),
            (32, 32, 32),
            (198, 198, 198),
            (32, 32, 32),
            (32, 32, 32),
            (32, 32, 32),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (198, 198, 198),
            (198, 198, 198),
            (32, 32, 32),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (32, 32, 32),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (32, 32, 32),
            (198, 198, 198),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (198, 198, 198),
            (198, 198, 198),
            (32, 32, 32),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (32, 32, 32),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (32, 32, 32),
            (198, 198, 198),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (198, 198, 198),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (58, 58, 58),
            (58, 58, 58),
            (58, 58, 58),
            (58, 58, 58),
            (58, 58, 58),
            (58, 58, 58),
            (58, 58, 58),
            (58, 58, 58),
            (58, 58, 58),
            (58, 58, 58),
            (58, 58, 58),
            (58, 58, 58),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (210, 51, 38),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (210, 51, 38),
            (186, 45, 27),
            (186, 45, 27),
            (210, 51, 38),
        ];
        for y in 0..16 {
            for x in 0..16 {
                let (r, g, b) = tnt_pixels[y * 16 + x];
                draw_pixel(4 * 16 + x as i32, 16 + y as i32, r, g, b, 255);
            }
        }

        // Nuke: High-fidelity Radiation Symbol
        let nuke_pixels: [(u8, u8, u8); 256] = [
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (0, 0, 0),
            (0, 0, 0),
            (0, 0, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
            (255, 255, 0),
        ];
        for y in 0..16 {
            for x in 0..16 {
                let (r, g, b) = nuke_pixels[y * 16 + x];
                draw_pixel(5 * 16 + x as i32, 2 * 16 + y as i32, r, g, b, 255);
            }
        }
        // SnowyGrass side: dirt with white snow strip on top rows
        for x in 0..16i32 {
            for y in 0..16i32 {
                let noise = (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 20.0) as i32;
                let (r, g, b) = if y < 3 {
                    (
                        (240i32 + noise).clamp(0, 255) as u8,
                        (245i32 + noise).clamp(0, 255) as u8,
                        (250i32 + noise).clamp(0, 255) as u8,
                    )
                } else {
                    (
                        (130i32 + noise).clamp(0, 255) as u8,
                        (90i32 + noise).clamp(0, 255) as u8,
                        (60i32 + noise).clamp(0, 255) as u8,
                    )
                };
                draw_pixel(10 * 16 + x, y, r, g, b, 255);
            }
        }

        let sand_pixels: [(u8, u8, u8); 256] = [
            (200, 190, 141),
            (227, 216, 170),
            (227, 217, 168),
            (221, 212, 158),
            (231, 220, 173),
            (221, 212, 156),
            (212, 203, 147),
            (221, 212, 156),
            (231, 219, 177),
            (221, 211, 160),
            (211, 205, 157),
            (222, 214, 162),
            (218, 211, 159),
            (252, 245, 204),
            (202, 196, 146),
            (218, 211, 157),
            (224, 214, 162),
            (218, 208, 157),
            (224, 212, 168),
            (231, 219, 175),
            (191, 180, 136),
            (212, 203, 147),
            (210, 200, 144),
            (213, 204, 148),
            (228, 222, 190),
            (229, 218, 174),
            (255, 253, 220),
            (216, 209, 154),
            (221, 213, 161),
            (216, 209, 154),
            (221, 213, 161),
            (234, 226, 174),
            (226, 215, 171),
            (237, 226, 182),
            (249, 238, 194),
            (215, 206, 152),
            (218, 208, 154),
            (215, 206, 152),
            (221, 212, 158),
            (221, 212, 158),
            (220, 211, 157),
            (228, 221, 166),
            (231, 217, 174),
            (233, 219, 181),
            (218, 207, 159),
            (208, 199, 145),
            (212, 202, 151),
            (247, 236, 190),
            (204, 199, 144),
            (212, 208, 152),
            (213, 209, 153),
            (222, 215, 160),
            (208, 201, 146),
            (223, 216, 161),
            (204, 199, 144),
            (214, 207, 152),
            (177, 169, 115),
            (214, 207, 152),
            (217, 210, 156),
            (228, 221, 166),
            (233, 223, 174),
            (233, 223, 174),
            (220, 211, 157),
            (193, 186, 131),
            (227, 223, 167),
            (214, 207, 152),
            (233, 219, 179),
            (214, 207, 152),
            (233, 220, 177),
            (233, 219, 179),
            (231, 217, 174),
            (233, 220, 177),
            (211, 198, 150),
            (207, 200, 145),
            (210, 203, 149),
            (231, 217, 176),
            (216, 202, 161),
            (197, 187, 134),
            (239, 228, 184),
            (201, 194, 139),
            (204, 199, 144),
            (223, 216, 161),
            (207, 200, 145),
            (254, 255, 242),
            (211, 204, 152),
            (235, 227, 175),
            (222, 215, 160),
            (221, 210, 168),
            (214, 207, 152),
            (242, 235, 180),
            (220, 213, 158),
            (197, 190, 136),
            (207, 200, 145),
            (209, 202, 148),
            (218, 211, 157),
            (193, 186, 131),
            (187, 182, 127),
            (222, 211, 169),
            (203, 191, 149),
            (221, 212, 156),
            (192, 185, 130),
            (227, 220, 165),
            (215, 208, 153),
            (210, 203, 149),
            (214, 207, 152),
            (214, 207, 152),
            (233, 219, 179),
            (254, 247, 204),
            (221, 214, 159),
            (231, 217, 178),
            (224, 214, 160),
            (218, 206, 165),
            (217, 212, 157),
            (214, 207, 152),
            (229, 218, 174),
            (218, 209, 152),
            (220, 213, 158),
            (209, 202, 148),
            (229, 222, 167),
            (251, 244, 189),
            (222, 215, 160),
            (210, 196, 155),
            (215, 206, 152),
            (213, 204, 150),
            (223, 216, 161),
            (225, 211, 172),
            (238, 234, 201),
            (232, 220, 179),
            (224, 217, 163),
            (222, 211, 169),
            (237, 226, 184),
            (231, 219, 175),
            (224, 212, 168),
            (211, 204, 150),
            (233, 220, 177),
            (224, 210, 167),
            (214, 204, 155),
            (255, 254, 209),
            (193, 186, 131),
            (221, 214, 159),
            (214, 207, 152),
            (220, 206, 168),
            (224, 211, 165),
            (220, 213, 158),
            (194, 187, 132),
            (216, 209, 154),
            (228, 217, 175),
            (219, 209, 156),
            (218, 208, 154),
            (221, 210, 166),
            (211, 198, 150),
            (210, 203, 149),
            (218, 211, 157),
            (221, 214, 159),
            (221, 214, 159),
            (239, 232, 178),
            (218, 211, 157),
            (222, 213, 168),
            (217, 206, 159),
            (232, 222, 170),
            (224, 217, 163),
            (225, 212, 168),
            (173, 164, 110),
            (229, 218, 176),
            (200, 191, 137),
            (237, 226, 182),
            (217, 207, 151),
            (220, 207, 166),
            (241, 230, 188),
            (241, 233, 179),
            (231, 224, 170),
            (214, 207, 152),
            (218, 211, 157),
            (252, 251, 231),
            (215, 208, 151),
            (220, 211, 157),
            (206, 199, 144),
            (231, 217, 176),
            (231, 221, 167),
            (200, 189, 147),
            (224, 214, 160),
            (215, 206, 152),
            (222, 213, 159),
            (218, 208, 154),
            (222, 215, 160),
            (221, 214, 159),
            (216, 209, 154),
            (255, 250, 195),
            (210, 203, 149),
            (210, 203, 149),
            (209, 202, 148),
            (186, 179, 124),
            (239, 226, 180),
            (222, 215, 160),
            (211, 204, 150),
            (238, 231, 177),
            (223, 216, 161),
            (210, 203, 149),
            (225, 218, 164),
            (190, 183, 129),
            (199, 192, 137),
            (221, 214, 159),
            (215, 208, 153),
            (209, 202, 148),
            (222, 213, 168),
            (231, 217, 174),
            (210, 200, 146),
            (213, 206, 151),
            (223, 215, 168),
            (232, 218, 177),
            (206, 199, 144),
            (180, 173, 118),
            (209, 202, 148),
            (210, 203, 149),
            (211, 204, 150),
            (248, 231, 191),
            (224, 212, 170),
            (244, 233, 189),
            (231, 219, 175),
            (224, 217, 163),
            (227, 220, 165),
            (214, 207, 152),
            (232, 221, 177),
            (215, 204, 160),
            (226, 215, 169),
            (217, 203, 162),
            (217, 207, 151),
            (207, 200, 145),
            (221, 212, 167),
            (215, 201, 158),
            (220, 213, 158),
            (225, 218, 164),
            (200, 193, 138),
            (223, 216, 161),
            (186, 179, 124),
            (214, 207, 152),
            (229, 217, 178),
            (203, 192, 147),
            (210, 200, 146),
            (222, 212, 165),
            (215, 205, 154),
            (229, 219, 168),
            (217, 207, 153),
            (194, 187, 132),
            (255, 255, 208),
            (223, 216, 163),
            (199, 192, 137),
            (239, 225, 184),
            (218, 211, 157),
            (239, 231, 186),
            (209, 202, 148),
            (221, 213, 163),
            (224, 213, 164),
            (222, 212, 161),
            (217, 207, 153),
            (203, 193, 139),
        ];

        for y in 0..16 {
            for x in 0..16 {
                let (r, g, b) = sand_pixels[y * 16 + x];
                draw_pixel(6 * 16 + x as i32, y as i32, r, g, b, 255);
            }
        }
        // === NEW BLOCK TEXTURES (Phase 2) ===
        {
            fn sp(d: &mut [u8], x: i32, y: i32, r: u8, g: u8, b: u8, a: u8) {
                if (0..256).contains(&x) && (0..256).contains(&y) {
                    let i = ((y * 256 + x) * 4) as usize;
                    d[i] = r;
                    d[i + 1] = g;
                    d[i + 2] = b;
                    d[i + 3] = a;
                }
            }
            fn nb(d: &mut [u8], tx: i32, ty: i32, r: u8, g: u8, b: u8) {
                for x in 0..16i32 {
                    for y in 0..16i32 {
                        let n =
                            (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 20.0) as i32;
                        sp(
                            d,
                            tx * 16 + x,
                            ty * 16 + y,
                            (r as i32 + n).clamp(0, 255) as u8,
                            (g as i32 + n).clamp(0, 255) as u8,
                            (b as i32 + n).clamp(0, 255) as u8,
                            255,
                        );
                    }
                }
            }

            // Cobblestone (0,2)
            nb(&mut data, 0, 2, 125, 125, 125);
            for x in 0..16i32 {
                for y in 0..16i32 {
                    if (x * 5 + y * 7) % 13 == 0 || (x * 3 + y * 11) % 17 == 0 {
                        let n =
                            (crate::noise::noise_2d(x as f32 * 0.3, y as f32 * 0.3) * 15.0) as i32;
                        sp(
                            &mut data,
                            x,
                            2 * 16 + y,
                            (100 + n).clamp(0, 255) as u8,
                            (100 + n).clamp(0, 255) as u8,
                            (100 + n).clamp(0, 255) as u8,
                            255,
                        );
                    }
                }
            }

            // Oak Planks (1,2)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    let n = (crate::noise::noise_2d(x as f32 * 0.8, y as f32 * 0.15) * 15.0) as i32;
                    let l = if y % 4 == 0 { -15i32 } else { 0 };
                    sp(
                        &mut data,
                        16 + x,
                        2 * 16 + y,
                        (180 + n + l).clamp(0, 255) as u8,
                        (140 + n + l).clamp(0, 255) as u8,
                        (90 + n + l).clamp(0, 255) as u8,
                        255,
                    );
                }
            }

            // Crafting Table top (2,2)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    let n = (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 10.0) as i32;
                    let g = x == 0 || y == 0 || x == 8 || y == 8;
                    let (r, gr, b) = if g {
                        (60 + n, 45 + n, 30 + n)
                    } else {
                        (170 + n, 130 + n, 80 + n)
                    };
                    sp(
                        &mut data,
                        2 * 16 + x,
                        2 * 16 + y,
                        r.clamp(0, 255) as u8,
                        gr.clamp(0, 255) as u8,
                        b.clamp(0, 255) as u8,
                        255,
                    );
                }
            }

            // Crafting Table side (3,2)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    let n = (crate::noise::noise_2d(x as f32 * 0.8, y as f32 * 0.15) * 15.0) as i32;
                    let s = if y < 2 {
                        -40i32
                    } else if y % 4 == 0 {
                        -15
                    } else {
                        0
                    };
                    sp(
                        &mut data,
                        3 * 16 + x,
                        2 * 16 + y,
                        (170 + n + s).clamp(0, 255) as u8,
                        (125 + n + s).clamp(0, 255) as u8,
                        (75 + n + s).clamp(0, 255) as u8,
                        255,
                    );
                }
            }

            // Furnace front (4,2)
            nb(&mut data, 4, 2, 130, 130, 130);
            for x in 5..11i32 {
                for y in 6..13i32 {
                    sp(&mut data, 4 * 16 + x, 2 * 16 + y, 30, 20, 15, 255);
                }
            }

            // Furnace side (5,2) - reuse cobblestone texture but brighter
            nb(&mut data, 5, 2, 135, 135, 135);

            // Glass (6,2)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    let edge = x == 0 || x == 15 || y == 0 || y == 15;
                    let inner = x == 1 || x == 14 || y == 1 || y == 14;
                    if edge {
                        sp(&mut data, 6 * 16 + x, 2 * 16 + y, 180, 210, 230, 200);
                    } else if inner {
                        sp(&mut data, 6 * 16 + x, 2 * 16 + y, 200, 225, 240, 100);
                    } else {
                        sp(&mut data, 6 * 16 + x, 2 * 16 + y, 220, 235, 248, 40);
                    }
                }
            }

            // Gold Ore (7,2)
            nb(&mut data, 7, 2, 140, 140, 140);
            for x in 0..16i32 {
                for y in 0..16i32 {
                    if (x * 7 + y * 5) % 11 == 0 {
                        sp(&mut data, 7 * 16 + x, 2 * 16 + y, 250, 220, 50, 255);
                    }
                }
            }

            // Diamond Ore (8,2)
            nb(&mut data, 8, 2, 140, 140, 140);
            for x in 0..16i32 {
                for y in 0..16i32 {
                    if (x * 7 + y * 3) % 11 == 0 {
                        sp(&mut data, 8 * 16 + x, 2 * 16 + y, 80, 220, 230, 255);
                    }
                }
            }

            // Obsidian (9,2)
            nb(&mut data, 9, 2, 20, 15, 30);

            // Brick (10,2)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    let n = (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 10.0) as i32;
                    let m = y % 4 == 0 || (x + (y / 4) * 8) % 8 == 0;
                    let (r, g, b) = if m {
                        (180 + n, 170 + n, 150 + n)
                    } else {
                        (170 + n, 85 + n, 60 + n)
                    };
                    sp(
                        &mut data,
                        10 * 16 + x,
                        2 * 16 + y,
                        r.clamp(0, 255) as u8,
                        g.clamp(0, 255) as u8,
                        b.clamp(0, 255) as u8,
                        255,
                    );
                }
            }

            // Stone Brick (11,2)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    let n = (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 10.0) as i32;
                    let m = y % 8 == 0 || (x + (y / 8) * 8) % 8 == 0;
                    let v = if m { 110 + n } else { 145 + n };
                    sp(
                        &mut data,
                        11 * 16 + x,
                        2 * 16 + y,
                        v.clamp(0, 255) as u8,
                        v.clamp(0, 255) as u8,
                        v.clamp(0, 255) as u8,
                        255,
                    );
                }
            }

            // Sandstone (12,2), Wool (13,2), Bookshelf (14,2), Sponge (15,2)
            nb(&mut data, 12, 2, 215, 205, 160);
            nb(&mut data, 13, 2, 235, 235, 235);

            // Bookshelf (14,2)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    let n = (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 8.0) as i32;
                    if !(2..=13).contains(&y) {
                        sp(
                            &mut data,
                            14 * 16 + x,
                            2 * 16 + y,
                            (180 + n).clamp(0, 255) as u8,
                            (140 + n).clamp(0, 255) as u8,
                            (90 + n).clamp(0, 255) as u8,
                            255,
                        );
                    } else {
                        let c = (x / 3) % 4;
                        let (r, g, b) = match c {
                            0 => (150 + n, 40 + n, 40 + n),
                            1 => (40 + n, 100 + n, 40 + n),
                            2 => (40 + n, 40 + n, 140 + n),
                            _ => (130 + n, 100 + n, 40 + n),
                        };
                        sp(
                            &mut data,
                            14 * 16 + x,
                            2 * 16 + y,
                            r.clamp(0, 255) as u8,
                            g.clamp(0, 255) as u8,
                            b.clamp(0, 255) as u8,
                            255,
                        );
                    }
                }
            }

            // Sponge (15,2)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    let n = (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 15.0) as i32;
                    let h = (x * 5 + y * 7) % 9 == 0;
                    let (r, g, b) = if h {
                        (160 + n, 150 + n, 50 + n)
                    } else {
                        (210 + n, 200 + n, 80 + n)
                    };
                    sp(
                        &mut data,
                        15 * 16 + x,
                        2 * 16 + y,
                        r.clamp(0, 255) as u8,
                        g.clamp(0, 255) as u8,
                        b.clamp(0, 255) as u8,
                        255,
                    );
                }
            }

            // Chest (0,7), Chest top (1,7), Mossy Cobblestone (2,7), Lapis Block (3,7), Lapis Ore (4,7), Torch (5,7)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    let n = (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 10.0) as i32;
                    let latch = (6..=9).contains(&x) && (5..=8).contains(&y);
                    let edge = x == 0 || x == 15 || y == 0 || y == 15;
                    let (r, g, b) = if latch {
                        (80, 80, 80)
                    } else if edge {
                        (100 + n, 70 + n, 40 + n)
                    } else {
                        (160 + n, 120 + n, 60 + n)
                    };
                    sp(
                        &mut data,
                        x,
                        7 * 16 + y,
                        r.clamp(0, 255) as u8,
                        g.clamp(0, 255) as u8,
                        b.clamp(0, 255) as u8,
                        255,
                    );
                }
            }
            nb(&mut data, 1, 7, 150, 110, 55);
            nb(&mut data, 2, 7, 125, 125, 125);
            for x in 0..16i32 {
                for y in 0..16i32 {
                    if (x * 3 + y * 7) % 5 == 0 {
                        let n =
                            (crate::noise::noise_2d(x as f32 * 0.8, y as f32 * 0.8) * 10.0) as i32;
                        sp(
                            &mut data,
                            2 * 16 + x,
                            7 * 16 + y,
                            (50 + n).clamp(0, 255) as u8,
                            (110 + n).clamp(0, 255) as u8,
                            (40 + n).clamp(0, 255) as u8,
                            255,
                        );
                    }
                }
            }
            nb(&mut data, 3, 7, 30, 50, 160);
            nb(&mut data, 4, 7, 140, 140, 140);
            for x in 0..16i32 {
                for y in 0..16i32 {
                    if (x * 7 + y * 3) % 11 == 0 {
                        sp(&mut data, 4 * 16 + x, 7 * 16 + y, 30, 60, 180, 255);
                    }
                }
            }
            // Torch (5,7)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    let flame = (6..=9).contains(&x) && (2..=5).contains(&y);
                    let stick = (7..=8).contains(&x) && (6..=14).contains(&y);
                    if flame {
                        sp(&mut data, 5 * 16 + x, 7 * 16 + y, 255, 200, 50, 255);
                    } else if stick {
                        sp(&mut data, 5 * 16 + x, 7 * 16 + y, 130, 90, 50, 255);
                    }
                }
            }

            // === MATERIAL ITEM SPRITES (Row 4) ===
            // Stick (0,4)
            for i in 0..12i32 {
                sp(&mut data, 4 + i, 4 * 16 + 3 + i, 130, 90, 50, 255);
                sp(&mut data, 5 + i, 4 * 16 + 3 + i, 110, 75, 40, 255);
            }
            // Coal (1,4)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    let d = (x - 8) * (x - 8) + (y - 8) * (y - 8);
                    if d < 30 {
                        let n =
                            (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 15.0) as i32;
                        sp(
                            &mut data,
                            16 + x,
                            4 * 16 + y,
                            (25 + n).clamp(0, 255) as u8,
                            (25 + n).clamp(0, 255) as u8,
                            (25 + n).clamp(0, 255) as u8,
                            255,
                        );
                    }
                }
            }
            // Diamond (2,4)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    if (x - 8).abs() + (y - 8).abs() < 7 {
                        let n =
                            (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 15.0) as i32;
                        sp(
                            &mut data,
                            2 * 16 + x,
                            4 * 16 + y,
                            (80 + n).clamp(0, 255) as u8,
                            (220 + n).clamp(0, 255) as u8,
                            (230 + n).clamp(0, 255) as u8,
                            255,
                        );
                    }
                }
            }
            // Gold Ingot (3,4)
            for x in 2..14i32 {
                for y in 5..11i32 {
                    let n = (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 10.0) as i32;
                    let (r, g, b) = if y < 7 {
                        (255i32, 230 + n, 50 + n)
                    } else {
                        (220 + n, 190 + n, 30 + n)
                    };
                    sp(
                        &mut data,
                        3 * 16 + x,
                        4 * 16 + y,
                        r.clamp(0, 255) as u8,
                        g.clamp(0, 255) as u8,
                        b.clamp(0, 255) as u8,
                        255,
                    );
                }
            }
            // Lapis Lazuli (4,4)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    if (x - 8) * (x - 8) + (y - 8) * (y - 8) < 28 {
                        let n =
                            (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 15.0) as i32;
                        sp(
                            &mut data,
                            4 * 16 + x,
                            4 * 16 + y,
                            (30 + n).clamp(0, 255) as u8,
                            (60 + n).clamp(0, 255) as u8,
                            (180 + n).clamp(0, 255) as u8,
                            255,
                        );
                    }
                }
            }
            // String (5,4)
            for y in 2..14i32 {
                sp(&mut data, 5 * 16 + 8, 4 * 16 + y, 230, 230, 230, 255);
                if y % 3 == 0 {
                    sp(&mut data, 5 * 16 + 7, 4 * 16 + y, 200, 200, 200, 255);
                }
            }
            // Gunpowder (6,4)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    if (x * 3 + y * 7 + 5) % 4 < 2 && (x - 8) * (x - 8) + (y - 8) * (y - 8) < 35 {
                        let n =
                            (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 10.0) as i32;
                        sp(
                            &mut data,
                            6 * 16 + x,
                            4 * 16 + y,
                            (60 + n).clamp(0, 255) as u8,
                            (60 + n).clamp(0, 255) as u8,
                            (60 + n).clamp(0, 255) as u8,
                            255,
                        );
                    }
                }
            }
            // Leather (7,4)
            for x in 0..16i32 {
                for y in 0..16i32 {
                    if (x - 8).abs() < 5 && (y - 8).abs() < 6 {
                        let n =
                            (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 12.0) as i32;
                        sp(
                            &mut data,
                            7 * 16 + x,
                            4 * 16 + y,
                            (160 + n).clamp(0, 255) as u8,
                            (100 + n).clamp(0, 255) as u8,
                            (50 + n).clamp(0, 255) as u8,
                            255,
                        );
                    }
                }
            }

            // === TOOL SPRITES ===
            let mat_colors: [(u8, u8, u8); 5] = [
                (160, 120, 60),
                (130, 130, 130),
                (210, 210, 210),
                (80, 220, 230),
                (250, 220, 50),
            ];
            let tool_layout: [(i32, i32, usize, u8); 25] = [
                (0, 5, 0, 0),
                (1, 5, 0, 1),
                (2, 5, 0, 2),
                (3, 5, 0, 3),
                (4, 5, 0, 4),
                (5, 5, 1, 0),
                (6, 5, 1, 1),
                (7, 5, 1, 2),
                (8, 5, 1, 3),
                (9, 5, 1, 4),
                (0, 6, 2, 0),
                (1, 6, 2, 1),
                (2, 6, 2, 2),
                (3, 6, 2, 3),
                (4, 6, 2, 4),
                (5, 6, 3, 0),
                (6, 6, 3, 1),
                (7, 6, 3, 2),
                (8, 6, 3, 3),
                (9, 6, 3, 4),
                (10, 6, 4, 0),
                (11, 6, 4, 1),
                (12, 6, 4, 2),
                (13, 6, 4, 3),
                (14, 6, 4, 4),
            ];
            for &(tx, ty, mat, shape) in &tool_layout {
                let (hr, hg, hb) = mat_colors[mat];
                let stk = |d: &mut [u8], tx: i32, ty: i32| {
                    for i in 0..7i32 {
                        sp(d, tx * 16 + 3 + i, ty * 16 + 15 - i, 130, 90, 50, 255);
                        sp(d, tx * 16 + 4 + i, ty * 16 + 15 - i, 110, 75, 40, 255);
                    }
                };
                match shape {
                    0 => {
                        // Pickaxe
                        stk(&mut data, tx, ty);
                        for x in 2..14i32 {
                            sp(&mut data, tx * 16 + x, ty * 16 + 2, hr, hg, hb, 255);
                            sp(&mut data, tx * 16 + x, ty * 16 + 3, hr, hg, hb, 255);
                        }
                        sp(&mut data, tx * 16 + 2, ty * 16 + 4, hr, hg, hb, 255);
                        sp(&mut data, tx * 16 + 3, ty * 16 + 4, hr, hg, hb, 255);
                        sp(&mut data, tx * 16 + 12, ty * 16 + 4, hr, hg, hb, 255);
                        sp(&mut data, tx * 16 + 13, ty * 16 + 4, hr, hg, hb, 255);
                    }
                    1 => {
                        // Axe
                        stk(&mut data, tx, ty);
                        for x in 3..9i32 {
                            sp(&mut data, tx * 16 + x, ty * 16 + 1, hr, hg, hb, 255);
                            sp(&mut data, tx * 16 + x, ty * 16 + 2, hr, hg, hb, 255);
                        }
                        for y in 3..7i32 {
                            sp(&mut data, tx * 16 + 3, ty * 16 + y, hr, hg, hb, 255);
                            sp(&mut data, tx * 16 + 4, ty * 16 + y, hr, hg, hb, 255);
                        }
                        sp(&mut data, tx * 16 + 5, ty * 16 + 3, hr, hg, hb, 255);
                        sp(&mut data, tx * 16 + 5, ty * 16 + 4, hr, hg, hb, 255);
                    }
                    2 => {
                        // Shovel
                        stk(&mut data, tx, ty);
                        for y in 1..6i32 {
                            sp(&mut data, tx * 16 + 7, ty * 16 + y, hr, hg, hb, 255);
                            sp(&mut data, tx * 16 + 8, ty * 16 + y, hr, hg, hb, 255);
                            sp(&mut data, tx * 16 + 9, ty * 16 + y, hr, hg, hb, 255);
                        }
                        sp(&mut data, tx * 16 + 8, ty * 16, hr, hg, hb, 255);
                    }
                    3 => {
                        // Sword
                        for x in 5..11i32 {
                            sp(&mut data, tx * 16 + x, ty * 16 + 8, 80, 80, 80, 255);
                        }
                        for y in 1..8i32 {
                            sp(&mut data, tx * 16 + 7, ty * 16 + y, hr, hg, hb, 255);
                            sp(&mut data, tx * 16 + 8, ty * 16 + y, hr, hg, hb, 255);
                        }
                        sp(&mut data, tx * 16 + 8, ty * 16, hr, hg, hb, 255);
                        for y in 9..14i32 {
                            sp(&mut data, tx * 16 + 7, ty * 16 + y, 130, 90, 50, 255);
                            sp(&mut data, tx * 16 + 8, ty * 16 + y, 110, 75, 40, 255);
                        }
                    }
                    4 => {
                        // Hoe
                        stk(&mut data, tx, ty);
                        for x in 8..14i32 {
                            sp(&mut data, tx * 16 + x, ty * 16 + 1, hr, hg, hb, 255);
                            sp(&mut data, tx * 16 + x, ty * 16 + 2, hr, hg, hb, 255);
                        }
                        sp(&mut data, tx * 16 + 8, ty * 16 + 3, hr, hg, hb, 255);
                        sp(&mut data, tx * 16 + 9, ty * 16 + 3, hr, hg, hb, 255);
                    }
                    _ => {}
                }
            }

            // === CRACK STAGE TEXTURES (row 3, columns 0-9) ===
            for stage in 0..10u32 {
                let density = (stage + 1) as f32 / 12.0;
                for x in 0..16i32 {
                    for y in 0..16i32 {
                        let hash = ((x * 374761393 + y * 668265263 + stage as i32 * 1274126177)
                            as u32)
                            .wrapping_mul(1103515245)
                            .wrapping_add(12345);
                        let frac = (hash % 1000) as f32 / 1000.0;
                        if frac < density {
                            let alpha = (80 + stage * 18).min(255) as u8;
                            sp(&mut data, stage as i32 * 16 + x, 3 * 16 + y, 0, 0, 0, alpha);
                        }
                    }
                }
            }
        }

        self.atlas = Some(Texture2D::from_data(&data, 256, 256));
    }

    pub fn raycast(&self, origin: Vec3, direction: Vec3, max_distance: f32) -> RaycastResult {
        let dir = direction.normalize();
        let mut t = 0.0;
        let mut ix = origin.x.floor() as i32;
        let mut iy = origin.y.floor() as i32;
        let mut iz = origin.z.floor() as i32;
        let step_x = dir.x.signum() as i32;
        let step_y = dir.y.signum() as i32;
        let step_z = dir.z.signum() as i32;
        let mut t_max_x = if dir.x != 0.0 {
            ((ix as f32 + (step_x.max(0) as f32)) - origin.x) / dir.x
        } else {
            f32::INFINITY
        };
        let mut t_max_y = if dir.y != 0.0 {
            ((iy as f32 + (step_y.max(0) as f32)) - origin.y) / dir.y
        } else {
            f32::INFINITY
        };
        let mut t_max_z = if dir.z != 0.0 {
            ((iz as f32 + (step_z.max(0) as f32)) - origin.z) / dir.z
        } else {
            f32::INFINITY
        };
        let t_delta_x = if dir.x != 0.0 {
            (1.0 / dir.x).abs()
        } else {
            f32::INFINITY
        };
        let t_delta_y = if dir.y != 0.0 {
            (1.0 / dir.y).abs()
        } else {
            f32::INFINITY
        };
        let t_delta_z = if dir.z != 0.0 {
            (1.0 / dir.z).abs()
        } else {
            f32::INFINITY
        };
        let mut nx = 0;
        let mut ny = 0;
        let mut nz = 0;
        while t < max_distance {
            let block = self.get_block(ix, iy, iz);
            if block != BlockType::Air && block != BlockType::Water {
                return RaycastResult {
                    hit: true,
                    x: ix,
                    y: iy,
                    z: iz,
                    nx,
                    ny,
                    nz,
                    block,
                };
            }
            if t_max_x < t_max_y {
                if t_max_x < t_max_z {
                    ix += step_x;
                    t = t_max_x;
                    t_max_x += t_delta_x;
                    nx = -step_x;
                    ny = 0;
                    nz = 0;
                } else {
                    iz += step_z;
                    t = t_max_z;
                    t_max_z += t_delta_z;
                    nx = 0;
                    ny = 0;
                    nz = -step_z;
                }
            } else {
                if t_max_y < t_max_z {
                    iy += step_y;
                    t = t_max_y;
                    t_max_y += t_delta_y;
                    nx = 0;
                    ny = -step_y;
                    nz = 0;
                } else {
                    iz += step_z;
                    t = t_max_z;
                    t_max_z += t_delta_z;
                    nx = 0;
                    ny = 0;
                    nz = -step_z;
                }
            }
        }
        RaycastResult {
            hit: false,
            x: 0,
            y: 0,
            z: 0,
            nx: 0,
            ny: 0,
            nz: 0,
            block: BlockType::Air,
        }
    }

    pub fn update(&mut self, player_pos: Vec3, _time: f32) {
        let pcx = (player_pos.x / CHUNK_WIDTH as f32).floor() as i32;
        let pcz = (player_pos.z / CHUNK_DEPTH as f32).floor() as i32;

        // --- Spiral Chunk Generation ---
        if pcx != self.last_pcx || pcz != self.last_pcz {
            if self.is_loading {
                // In loading state, we process the spiral radius-by-radius, a few chunks per frame
                let mut generated_this_frame = 0;
                while self.loading_radius <= VIEW_DISTANCE && generated_this_frame < 64 {
                    let r = self.loading_radius;
                    // We need to iterate over the ring r
                    // This is slightly complex to do incrementally inside the loop without more state,
                    // but we can just do the whole ring if it's small, or just a few.
                    // Let's simplify and just do the whole ring r then increment loading_radius.
                    for x in (pcx - r)..=(pcx + r) {
                        for z in (pcz - r)..=(pcz + r) {
                            if x > pcx - r && x < pcx + r && z > pcz - r && z < pcz + r {
                                continue;
                            }

                            let index = self.get_pool_index(x, z);
                            let should_replace = match &self.chunks[index] {
                                None => true,
                                Some(c) => c.x != x || c.z != z,
                            };
                            if should_replace {
                                let mut chunk = Box::new(Chunk::new(x, z));
                                chunk.generate();
                                self.chunks[index] = Some(chunk);
                                self.apply_edits_to_chunk(x, z);
                                if let Some(c) = &mut self.chunks[index] {
                                    c.dirty = true;
                                    self.dirty_count += 1;
                                }
                                self.chunks_generated_count += 1;
                                generated_this_frame += 1;
                                if generated_this_frame >= 64 {
                                    break;
                                }
                            }
                        }
                        if generated_this_frame >= 64 {
                            break;
                        }
                    }
                    if generated_this_frame < 64 {
                        self.loading_radius += 1;
                    }
                }
                if self.loading_radius > VIEW_DISTANCE {
                    self.is_loading = false;
                    self.last_pcx = pcx;
                    self.last_pcz = pcz;
                }
            } else {
                // NORMAL MODE: Non-blocking spiral (Discovery)
                // Throttle generation to prevent massive stutters when crossing boundaries
                let mut generated_this_frame = 0;
                for r in 0..=VIEW_DISTANCE {
                    for x in (pcx - r)..=(pcx + r) {
                        for z in (pcz - r)..=(pcz + r) {
                            if x > pcx - r && x < pcx + r && z > pcz - r && z < pcz + r {
                                continue;
                            }

                            let index = self.get_pool_index(x, z);
                            let should_replace = match &self.chunks[index] {
                                None => true,
                                Some(c) => c.x != x || c.z != z,
                            };
                            if should_replace {
                                let mut chunk = Box::new(Chunk::new(x, z));
                                chunk.generate();
                                self.chunks[index] = Some(chunk);
                                self.apply_edits_to_chunk(x, z);

                                // Dirty the new chunk AND its neighbors to fix lighting/meshing gaps
                                if let Some(c) = &mut self.chunks[index]
                                    && !c.dirty
                                {
                                    c.dirty = true;
                                    self.dirty_count += 1;
                                }
                                let neighbors = [(x - 1, z), (x + 1, z), (x, z - 1), (x, z + 1)];
                                for (nx, nz) in neighbors {
                                    if let Some(nc) = self.get_chunk_mut(nx, nz)
                                        && !nc.dirty
                                    {
                                        nc.dirty = true;
                                        self.dirty_count += 1;
                                    }
                                }

                                generated_this_frame += 1;
                                if generated_this_frame >= 8 {
                                    break;
                                }
                            }
                        }
                        if generated_this_frame >= 8 {
                            break;
                        }
                    }
                    if generated_this_frame >= 8 {
                        break;
                    }
                }

                // Only update last_pcx/last_pcz once we've at least tried to load the immediate ring
                // Actually, let's keep the boundary check simple.
                if generated_this_frame < 8 {
                    self.last_pcx = pcx;
                    self.last_pcz = pcz;
                }
            }
        }

        // --- Prioritized Meshing ---
        // 1. Collect and upload finished meshes to GPU (skip scan if nothing in flight)
        if self.meshing_in_flight > 0 {
            let mut ready_indices = Vec::new();
            for i in 0..CHUNK_POOL_SIZE {
                if let Some(chunk) = &self.chunks[i]
                    && chunk.meshing_in_progress
                    && chunk.pending_mesh_opaque.is_some()
                {
                    let dx = chunk.x as f32 * CHUNK_WIDTH as f32 + 8.0 - player_pos.x;
                    let dz = chunk.z as f32 * CHUNK_DEPTH as f32 + 8.0 - player_pos.z;
                    let dist_sq = dx * dx + dz * dz;
                    ready_indices.push((i, dist_sq));
                }
            }
            ready_indices.sort_unstable_by(|a, b| {
                a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
            });

            for (index, _) in ready_indices {
                if let Some(chunk) = &mut self.chunks[index]
                    && let (Some(opaque), Some(trans), Some(water)) = (
                        chunk.pending_mesh_opaque.take(),
                        chunk.pending_mesh_transparent.take(),
                        chunk.pending_mesh_water.take(),
                    )
                {
                    chunk.upload_mesh(opaque, trans, water);
                    self.meshing_in_flight -= 1;
                }
            }
        }

        // 2. Scan for dirty chunks and dispatch to background (skip if nothing dirty)
        if self.dirty_count > 0 && self.detonations.is_empty() {
            let mut dirty_indices = Vec::new();
            for i in 0..CHUNK_POOL_SIZE {
                if let Some(chunk) = &self.chunks[i]
                    && chunk.dirty
                    && !chunk.meshing_in_progress
                {
                    let dx = chunk.x as f32 * CHUNK_WIDTH as f32 + 8.0 - player_pos.x;
                    let dz = chunk.z as f32 * CHUNK_DEPTH as f32 + 8.0 - player_pos.z;
                    let dist_sq = dx * dx + dz * dz;
                    dirty_indices.push((i, dist_sq));
                }
            }
            dirty_indices.sort_unstable_by(|a, b| {
                a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
            });

            let mut dispatches_this_frame = 0;
            for (index, _) in dirty_indices {
                let world_ptr = self as *const World as usize;
                if let Some(chunk) = &mut self.chunks[index] {
                    chunk.dirty = false;
                    chunk.meshing_in_progress = true;
                    self.dirty_count -= 1;
                    self.meshing_in_flight += 1;

                    let chunk_ptr = chunk.as_mut() as *mut Chunk as usize;
                    rayon::spawn(move || {
                        let w = unsafe { &*(world_ptr as *const World) };
                        let c = unsafe { &mut *(chunk_ptr as *mut Chunk) };

                        c.calculate_lighting();
                        let (op, tr, wa) = c.calculate_mesh_data(w);

                        c.pending_mesh_opaque = Some(op);
                        c.pending_mesh_transparent = Some(tr);
                        c.pending_mesh_water = Some(wa);
                    });

                    dispatches_this_frame += 1;
                    if dispatches_this_frame >= 16 {
                        break;
                    }
                }
            }
        }

        // --- Explosive Ticking ---
        let mut i = 0;
        while i < self.explosives.len() {
            self.explosives[i].fuse -= _time;
            if self.explosives[i].fuse <= 0.0 {
                let e = self.explosives.remove(i);
                self.detonations.push((e.position, e.block_type));
            } else {
                i += 1;
            }
        }

        // --- Process Detonations ---
        let dets: Vec<(glam::Vec3, BlockType)> = std::mem::take(&mut self.detonations);
        for (pos, btype) in dets {
            let (radius, is_nuke) = match btype {
                BlockType::Nuke => (20, true),
                _ => (4, false),
            };
            crate::explosion::explode(
                self,
                pos.x as i32,
                pos.y as i32,
                pos.z as i32,
                radius,
                is_nuke,
            );
        }

        // --- Particle Ticking ---
        let mut i = 0;
        while i < self.particles.len() {
            let vel = self.particles[i].velocity;
            self.particles[i].position += vel * _time;
            self.particles[i].life -= _time;
            if self.particles[i].life <= 0.0 {
                self.particles.remove(i);
            } else {
                i += 1;
            }
        }

        // --- Physics Simulation ---
        self.update_water(_time);
        self.update_falling_blocks();
    }

    pub fn get_liquid_level(&self, x: i32, y: i32, z: i32) -> u8 {
        if y < 0 || y >= CHUNK_HEIGHT as i32 {
            return 0;
        }
        let cx = x.div_euclid(CHUNK_WIDTH as i32);
        let cz = z.div_euclid(CHUNK_DEPTH as i32);
        let bx = x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        let bz = z.rem_euclid(CHUNK_DEPTH as i32) as usize;

        if let Some(chunk) = self.get_chunk(cx, cz) {
            return chunk.liquid_levels[bx][y as usize][bz];
        }
        0
    }

    pub fn set_liquid_level(&mut self, x: i32, y: i32, z: i32, level: u8) {
        if y < 0 || y >= CHUNK_HEIGHT as i32 {
            return;
        }
        let cx = x.div_euclid(CHUNK_WIDTH as i32);
        let cz = z.div_euclid(CHUNK_DEPTH as i32);
        let bx = x.rem_euclid(CHUNK_WIDTH as i32) as usize;
        let bz = z.rem_euclid(CHUNK_DEPTH as i32) as usize;

        if let Some(chunk) = self.get_chunk_mut(cx, cz) {
            let old_level = chunk.liquid_levels[bx][y as usize][bz];
            if old_level == level {
                return;
            }
            chunk.liquid_levels[bx][y as usize][bz] = level;
            if level > 0 {
                if chunk.blocks[bx][y as usize][bz] != BlockType::Water {
                    chunk.blocks[bx][y as usize][bz] = BlockType::Water;
                }
            } else {
                if chunk.blocks[bx][y as usize][bz] == BlockType::Water {
                    chunk.blocks[bx][y as usize][bz] = BlockType::Air;
                }
            }
            if !chunk.dirty {
                chunk.dirty = true;
                self.dirty_count += 1;
            }
        }
    }

    fn schedule_water_neighbors(&mut self, x: i32, y: i32, z: i32) {
        for &(dx, dy, dz) in &[
            (1i32, 0i32, 0i32),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ] {
            let (nx, ny, nz) = (x + dx, y + dy, z + dz);
            if self.get_block(nx, ny, nz) == BlockType::Water {
                self.active_water.insert((nx, ny, nz));
            }
        }
    }

    // MC 1.0: Find the smallest flow decay among horizontal neighbors and above
    fn calc_flow_decay(&self, x: i32, y: i32, z: i32) -> i32 {
        let mut min_decay: i32 = 1000;
        for &(dx, dz) in &[(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            let nl = self.get_liquid_level(x + dx, y, z + dz);
            if nl == 0 {
                continue;
            }
            let nd = if water_is_source(nl) || water_is_falling(nl) {
                0
            } else {
                water_decay(nl) as i32
            };
            if nd < min_decay {
                min_decay = nd;
            }
        }
        // Water falling from above acts as source
        if self.get_liquid_level(x, y + 1, z) > 0 {
            min_decay = 0;
        }
        if min_decay >= 1000 {
            return -1;
        }
        let new_decay = min_decay + 1;
        if new_decay > 7 { -1 } else { new_decay }
    }

    // MC 1.0: Pathfind toward nearest drop-off within 4 blocks
    fn get_flow_directions(&self, x: i32, y: i32, z: i32) -> [bool; 4] {
        let dirs: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        let mut costs = [1000i32; 4];
        for (dir_idx, &(dx, dz)) in dirs.iter().enumerate() {
            let (nx, nz) = (x + dx, z + dz);
            let nb = self.get_block(nx, y, nz);
            if nb.is_solid() {
                continue;
            }
            if nb == BlockType::Water && water_is_source(self.get_liquid_level(nx, y, nz)) {
                continue;
            }

            // A genuine drop is Air or Flowing water with nothing solid below
            let below = self.get_block(nx, y - 1, nz);
            if y > 0
                && (below == BlockType::Air
                    || (below == BlockType::Water
                        && !water_is_source(self.get_liquid_level(nx, y - 1, nz))))
            {
                costs[dir_idx] = 0;
            } else {
                costs[dir_idx] = self.find_drop_cost(nx, y, nz, 1, dir_idx);
            }
        }
        let min_cost = *costs.iter().min().unwrap();
        if min_cost >= 1000 {
            // No drops found — flow in all non-blocked directions
            let mut result = [false; 4];
            for (i, &(dx, dz)) in dirs.iter().enumerate() {
                result[i] = !self.get_block(x + dx, y, z + dz).is_solid();
            }
            return result;
        }
        [
            costs[0] == min_cost,
            costs[1] == min_cost,
            costs[2] == min_cost,
            costs[3] == min_cost,
        ]
    }

    fn find_drop_cost(&self, x: i32, y: i32, z: i32, depth: i32, from_dir: usize) -> i32 {
        if depth > 4 {
            return 1000;
        }
        let opposite = [1usize, 0, 3, 2];
        let dirs: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        let mut min_cost = 1000;
        for (dir_idx, &(dx, dz)) in dirs.iter().enumerate() {
            if dir_idx == opposite[from_dir] {
                continue;
            }
            let (nx, nz) = (x + dx, z + dz);
            let nb = self.get_block(nx, y, nz);
            if nb.is_solid() {
                continue;
            }
            if nb == BlockType::Water && water_is_source(self.get_liquid_level(nx, y, nz)) {
                continue;
            }

            let below = self.get_block(nx, y - 1, nz);
            if y > 0
                && (below == BlockType::Air
                    || (below == BlockType::Water
                        && !water_is_source(self.get_liquid_level(nx, y - 1, nz))))
            {
                return depth;
            }
            let cost = self.find_drop_cost(nx, y, nz, depth + 1, dir_idx);
            if cost < min_cost {
                min_cost = cost;
            }
        }
        min_cost
    }

    // MC 1.0 water physics: tick-based, level decay, infinite source, pathfinding
    pub fn update_water(&mut self, dt: f32) {
        self.water_tick_timer += dt;
        if self.water_tick_timer < 0.25 {
            return;
        }
        self.water_tick_timer -= 0.25;

        if self.active_water.is_empty() {
            return;
        }
        let to_process: Vec<(i32, i32, i32)> = self.active_water.drain().collect();

        let mut processed = 0usize;
        for &(x, y, z) in &to_process {
            let level = self.get_liquid_level(x, y, z);
            if level == 0 {
                continue;
            }

            let is_src = water_is_source(level);

            // 1. Infinite water source: non-source with 2+ horizontal source neighbors → become source
            let mut source_count = 0;
            for &(dx, dz) in &[(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                if water_is_source(self.get_liquid_level(x + dx, y, z + dz)) {
                    source_count += 1;
                }
            }

            if !is_src && source_count >= 2 {
                let below = self.get_block(x, y - 1, z);
                if below.is_solid() || below == BlockType::Water {
                    self.set_liquid_level(x, y, z, 1); // become source
                    self.schedule_water_neighbors(x, y, z);
                    processed += 1;
                }
            }

            // 2. Recalculate flow decay for non-source blocks
            let level = self.get_liquid_level(x, y, z);
            let is_src = water_is_source(level);
            if !is_src {
                let new_decay = self.calc_flow_decay(x, y, z);
                if new_decay < 0 {
                    self.set_liquid_level(x, y, z, 0);
                    self.schedule_water_neighbors(x, y, z);
                    processed += 1;
                    continue;
                }
                let new_level = water_level_from_decay(new_decay as u8, false);
                if new_level != level {
                    self.set_liquid_level(x, y, z, new_level);
                }
            }

            // Re-read level after possible changes
            let level = self.get_liquid_level(x, y, z);
            if level == 0 {
                processed += 1;
                continue;
            }
            let decay = water_decay(level);
            let is_src = water_is_source(level);

            // 3. Flow downward
            if y > 0 {
                let below = self.get_block(x, y - 1, z);
                if !below.is_solid() {
                    let falling_level = water_level_from_decay(decay, true);
                    if below == BlockType::Air {
                        self.set_liquid_level(x, y - 1, z, falling_level);
                        if is_src {
                            self.set_liquid_level(x, y, z, 0);
                            self.schedule_water_neighbors(x, y, z);
                        }
                        self.active_water.insert((x, y - 1, z));
                    } else if below == BlockType::Water {
                        let bl = self.get_liquid_level(x, y - 1, z);
                        if !water_is_source(bl) && !water_is_falling(bl) {
                            self.set_liquid_level(x, y - 1, z, falling_level);
                            if is_src {
                                self.set_liquid_level(x, y, z, 0);
                                self.schedule_water_neighbors(x, y, z);
                            }
                            self.active_water.insert((x, y - 1, z));
                        }
                    }
                    if is_src {
                        processed += 1;
                        continue;
                    }
                }
            }

            // 4. Spread horizontally (Hybrid Finite/Infinite)
            if decay < 7 || is_src {
                let flow_dirs = self.get_flow_directions(x, y, z);
                let dirs: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

                if is_src && flow_dirs.iter().any(|&b| b) {
                    if source_count >= 2 {
                        // INFINITE SPREAD: Copy the source (standard MC behavior for oceans/large lakes)
                        for (dir_idx, &(dx, dz)) in dirs.iter().enumerate() {
                            if !flow_dirs[dir_idx] {
                                continue;
                            }
                            let (nx, nz) = (x + dx, z + dz);
                            let nb = self.get_block(nx, y, nz);
                            let nl = self.get_liquid_level(nx, y, nz);
                            if nb == BlockType::Air
                                || (nb == BlockType::Water && !water_is_source(nl))
                            {
                                self.set_liquid_level(nx, y, nz, level);
                                self.active_water.insert((nx, y, nz));
                            }
                        }
                    } else {
                        // FINITE MOVEMENT: Move the source block (conserves volume for puddles/small flows)
                        for (dir_idx, &(dx, dz)) in dirs.iter().enumerate() {
                            if !flow_dirs[dir_idx] {
                                continue;
                            }
                            let (nx, nz) = (x + dx, z + dz);
                            let nb = self.get_block(nx, y, nz);
                            let nl = self.get_liquid_level(nx, y, nz);
                            if nb == BlockType::Air
                                || (nb == BlockType::Water && !water_is_source(nl))
                            {
                                self.set_liquid_level(nx, y, nz, level);
                                self.set_liquid_level(x, y, z, 0);
                                self.schedule_water_neighbors(x, y, z);
                                self.active_water.insert((nx, y, nz));
                                break;
                            }
                        }
                    }
                } else {
                    // Traditional Flowing Water Spreading
                    let spread_decay = if is_src { 1u8 } else { decay + 1 };
                    let spread_level = water_level_from_decay(spread_decay, false);
                    for (dir_idx, &(dx, dz)) in dirs.iter().enumerate() {
                        if !flow_dirs[dir_idx] {
                            continue;
                        }
                        let (nx, nz) = (x + dx, z + dz);
                        let nb = self.get_block(nx, y, nz);
                        if nb.is_solid() {
                            continue;
                        }
                        if nb == BlockType::Air {
                            self.set_liquid_level(nx, y, nz, spread_level);
                            self.active_water.insert((nx, y, nz));
                        } else if nb == BlockType::Water {
                            let nl = self.get_liquid_level(nx, y, nz);
                            if !water_is_source(nl) && nl > spread_level {
                                self.set_liquid_level(nx, y, nz, spread_level);
                                self.active_water.insert((nx, y, nz));
                            }
                        }
                    }
                }
            }

            processed += 1;
            if processed > 4000 {
                for &pos in &to_process[processed..] {
                    self.active_water.insert(pos);
                }
                break;
            }
        }
    }

    pub fn update_falling_blocks(&mut self) {
        if self.active_falling.is_empty() {
            return;
        }
        let to_process: Vec<(i32, i32, i32)> = self.active_falling.drain().collect();

        let mut processed = 0;
        for (x, y, z) in to_process.iter().copied() {
            let b = self.get_block(x, y, z);
            if b != BlockType::Sand && b != BlockType::Gravel {
                continue;
            }

            if y > 0 {
                let target = self.get_block(x, y - 1, z);
                if target == BlockType::Air || target == BlockType::Water {
                    self.set_block(x, y, z, BlockType::Air);
                    self.set_block(x, y - 1, z, b);
                }
            }

            processed += 1;
            if processed > 50000 {
                self.active_falling
                    .extend(to_process.into_iter().skip(processed));
                break;
            }
        }
    }

    pub fn init_celestial(&mut self) {
        let mut v = Vec::new();
        let mut c = Vec::new();
        for _ in 0..300 {
            let x = (rand::random::<f32>() * 2.0 - 1.0) * 400.0;
            let y = rand::random::<f32>() * 200.0 + 50.0;
            let z = (rand::random::<f32>() * 2.0 - 1.0) * 400.0;
            let q = 0.5;
            v.extend_from_slice(&[x - q, y, z, x + q, y, z, x + q, y + q, z]);
            v.extend_from_slice(&[x - q, y, z, x + q, y + q, z, x - q, y + q, z]);
            for _ in 0..6 {
                c.extend_from_slice(&[255, 255, 255, 255]);
            }
        }
        self.star_model = Some(Mesh::new(&v, None, None, Some(&c)));
    }

    pub fn update_clouds(&mut self, player_pos: Vec3, time: f32) {
        let dist = self.last_cloud_pos.distance(player_pos);
        let time_passed = (time - self.last_cloud_update_time) > 30.0;
        if dist < 128.0 && !time_passed && self.cloud_model.is_some() {
            return;
        }
        self.cloud_model = None;
        self.last_cloud_pos = player_pos;
        self.last_cloud_update_time = time;
        let cloud_height = 200.0;
        let cloud_size = 16.0;
        let range = 64;
        let start_x = (player_pos.x / cloud_size).floor() as i32 - range;
        let start_z = (player_pos.z / cloud_size).floor() as i32 - range;
        let mut v = Vec::new();
        let mut c = Vec::new();
        let mut n = Vec::new();
        let cloud_color = [225, 233, 240, 150];
        for x in start_x..(start_x + range * 2) {
            for z in start_z..(start_z + range * 2) {
                // Use time only as a positional drift, not as a noise parameter
                // that changes cloud shapes on regen. Clouds drift slowly via offset.
                let drift = time * 0.0005;
                let base = crate::noise::perlin_2d(
                    x as f32 * 0.18 + drift,
                    z as f32 * 0.18 + 41.0,
                    0.5,
                    2,
                );
                let breakup =
                    crate::noise::perlin_2d(x as f32 * 0.48 + drift, z as f32 * 0.48 + 7.0, 0.5, 1);
                if base > 0.10 && breakup > -0.08 {
                    let px = x as f32 * cloud_size;
                    let py = cloud_height;
                    let pz = z as f32 * cloud_size;
                    let sx = cloud_size;
                    let sy = 1.1;
                    let sz = cloud_size;
                    let cube = [
                        px,
                        py + sy,
                        pz,
                        px,
                        py + sy,
                        pz + sz,
                        px + sx,
                        py + sy,
                        pz + sz,
                        px,
                        py + sy,
                        pz,
                        px + sx,
                        py + sy,
                        pz + sz,
                        px + sx,
                        py + sy,
                        pz,
                        px,
                        py,
                        pz,
                        px + sx,
                        py,
                        pz + sz,
                        px,
                        py,
                        pz + sz,
                        px,
                        py,
                        pz,
                        px + sx,
                        py,
                        pz,
                        px + sx,
                        py,
                        pz + sz,
                        px,
                        py,
                        pz + sz,
                        px + sx,
                        py,
                        pz + sz,
                        px + sx,
                        py + sy,
                        pz + sz,
                        px,
                        py,
                        pz + sz,
                        px + sx,
                        py + sy,
                        pz + sz,
                        px,
                        py + sy,
                        pz + sz,
                        px + sx,
                        py,
                        pz,
                        px,
                        py,
                        pz,
                        px,
                        py + sy,
                        pz,
                        px + sx,
                        py,
                        pz,
                        px,
                        py + sy,
                        pz,
                        px + sx,
                        py + sy,
                        pz,
                        px + sx,
                        py,
                        pz + sz,
                        px + sx,
                        py,
                        pz,
                        px + sx,
                        py + sy,
                        pz,
                        px + sx,
                        py,
                        pz + sz,
                        px + sx,
                        py + sy,
                        pz,
                        px + sx,
                        py + sy,
                        pz + sz,
                        px,
                        py,
                        pz,
                        px,
                        py,
                        pz + sz,
                        px,
                        py + sy,
                        pz + sz,
                        px,
                        py,
                        pz,
                        px,
                        py + sy,
                        pz + sz,
                        px,
                        py + sy,
                        pz,
                    ];
                    v.extend_from_slice(&cube);
                    for _ in 0..36 {
                        c.extend_from_slice(&cloud_color);
                        n.extend_from_slice(&[0.0, 1.0, 0.0]);
                    }
                }
            }
        }
        if !v.is_empty() {
            self.cloud_model = Some(Mesh::new(&v, None, Some(&n), Some(&c)));
        }
    }

    pub fn compute_visible_chunks(&mut self, frustum: &Frustum) {
        self.visible_chunks.clear();
        for (i, chunk_opt) in self.chunks.iter().enumerate() {
            if let Some(chunk) = chunk_opt {
                let min = Vec3::new(
                    chunk.x as f32 * CHUNK_WIDTH as f32,
                    0.0,
                    chunk.z as f32 * CHUNK_DEPTH as f32,
                );
                let max = Vec3::new(
                    min.x + CHUNK_WIDTH as f32,
                    CHUNK_HEIGHT as f32,
                    min.z + CHUNK_DEPTH as f32,
                );
                if frustum.is_box_visible(min, max) {
                    self.visible_chunks.push(i);
                }
            }
        }
    }

    pub fn render_opaque(&self, _frustum: &Frustum) {
        if let Some(atlas) = &self.atlas {
            atlas.bind(0);
        }
        for &i in &self.visible_chunks {
            if let Some(chunk) = &self.chunks[i]
                && let Some(mesh) = &chunk.mesh_opaque
            {
                mesh.draw();
            }
        }
    }

    pub fn render_transparent(&self, _frustum: &Frustum) {
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::Enable(gl::CULL_FACE);
        }
        for &i in &self.visible_chunks {
            if let Some(chunk) = &self.chunks[i]
                && let Some(mesh) = &chunk.mesh_transparent
            {
                mesh.draw();
            }
        }
    }

    pub fn render_water(&self, _frustum: &Frustum) {
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::DepthMask(gl::FALSE);
            gl::Enable(gl::CULL_FACE);
        }
        for &i in &self.visible_chunks {
            if let Some(chunk) = &self.chunks[i]
                && let Some(mesh) = &chunk.mesh_water
            {
                mesh.draw();
            }
        }
        unsafe {
            gl::DepthMask(gl::TRUE);
        }
    }

    pub fn render_clouds(&self, shader: &Shader, mvp: &Mat4) {
        if let Some(mesh) = &self.cloud_model {
            shader.bind();
            shader.set_mat4(shader.get_uniform_location("uMVP"), mvp);
            shader.set_int(shader.get_uniform_location("uIsCircle"), 0);
            unsafe {
                gl::Disable(gl::CULL_FACE);
                gl::Enable(gl::BLEND);
                gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
                gl::DepthMask(gl::FALSE);
                mesh.draw();
                gl::DepthMask(gl::TRUE);
                gl::Disable(gl::BLEND);
                gl::Enable(gl::CULL_FACE);
            }
        }
    }

    pub fn render_stars(&self, player_pos: Vec3, time: f32, shader: &Shader, mvp: &Mat4) {
        let angle = (time / 1200.0) * 2.0 * std::f32::consts::PI;
        let sun_y = angle.sin();
        if sun_y > -0.3 {
            return;
        }
        let star_intensity = (1.0 - ((sun_y + 0.3) / 0.3)).clamp(0.0, 1.0);
        let star_alpha = (200.0 * star_intensity) as u8;
        if star_alpha < 30 {
            return;
        }
        if let Some(mesh) = &self.star_model {
            shader.bind();
            shader.set_int(shader.get_uniform_location("uIsCircle"), 0);
            let star_mvp = *mvp * Mat4::from_translation(player_pos);
            shader.set_mat4(shader.get_uniform_location("uMVP"), &star_mvp);
            unsafe {
                gl::Disable(gl::DEPTH_TEST);
                mesh.draw();
                gl::Enable(gl::DEPTH_TEST);
            }
        }
    }
}
