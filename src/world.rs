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
pub const CLOUD_HEIGHT: f32 = 200.0;

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
    pub seed: u64,
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
    pub detonations: Vec<glam::Vec3>,
    pub tnt_mesh: renderer::Mesh,
    pub spark_mesh: renderer::Mesh,
    pub particle_mesh: renderer::Mesh,
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
    pub fn new(seed: u64) -> Self {
        let mut chunks = Vec::with_capacity(CHUNK_POOL_SIZE);
        for _ in 0..CHUNK_POOL_SIZE {
            chunks.push(None);
        }
        Self {
            seed,
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
            tnt_mesh: Self::create_textured_cube_mesh(BlockType::TNT),
            spark_mesh: Self::create_textured_cube_mesh(BlockType::Torch),
            particle_mesh: Self::create_textured_cube_mesh(BlockType::SnowLayer),
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

    fn create_textured_cube_mesh(block: BlockType) -> renderer::Mesh {
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
        let (tx, ty) = crate::item::atlas_uv(block);
        let ts = 1.0 / 16.0;
        let pad = 0.001;
        let u0 = tx as f32 * ts + pad;
        let v0 = ty as f32 * ts + pad;
        let u1 = (tx as f32 + 1.0) * ts - pad;
        let v1 = (ty as f32 + 1.0) * ts - pad;
        let face_uv = [u0, v1, u1, v1, u1, v0, u0, v1, u1, v0, u0, v0];
        let mut t = Vec::with_capacity(72);
        for _ in 0..6 {
            t.extend_from_slice(&face_uv);
        }
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
        if let Some(atlas) = &self.atlas {
            atlas.bind(0);
        }

        for e in &self.explosives {
            let progress = 1.0 - (e.fuse / e.initial_fuse.max(0.001)).clamp(0.0, 1.0);
            let flash_rate = 4.0 + progress * 12.0;
            let flash = (current_time * flash_rate) as i32 % 2 == 0;
            let diffuse = if flash {
                glam::Vec4::new(1.8, 1.8, 1.8, 1.0)
            } else {
                glam::Vec4::ONE
            };
            shader.set_vec4(loc_diff, diffuse);

            let pulse = 1.0 + progress * 0.12;
            let model = glam::Mat4::from_translation(e.position + Vec3::splat(0.5))
                * glam::Mat4::from_scale(Vec3::splat(pulse))
                * glam::Mat4::from_translation(Vec3::splat(-0.5));
            shader.set_mat4(loc_model, &model);
            self.tnt_mesh.draw();

            let spark_phase = current_time * (18.0 + progress * 24.0);
            let spark_scale = 0.11 + spark_phase.sin().abs() * 0.05;
            let spark_offset = Vec3::new(
                spark_phase.sin() * 0.16,
                1.06 + (spark_phase * 1.7).sin().abs() * 0.08,
                spark_phase.cos() * 0.16,
            );
            shader.set_vec4(loc_diff, glam::Vec4::new(2.4, 1.35, 0.35, 1.0));
            let spark_model =
                glam::Mat4::from_translation(e.position + Vec3::new(0.5, 0.0, 0.5) + spark_offset)
                    * glam::Mat4::from_scale(Vec3::splat(spark_scale));
            shader.set_mat4(loc_model, &spark_model);
            self.spark_mesh.draw();
        }
        shader.set_vec4(loc_diff, glam::Vec4::ONE);
        shader.set_mat4(loc_model, &glam::Mat4::IDENTITY);
    }

    pub fn render_particles(&self, shader: &crate::renderer::Shader, _mvp: &glam::Mat4) {
        let loc_model = shader.get_uniform_location("uModel");
        let loc_diff = shader.get_uniform_location("colDiffuse");
        if let Some(atlas) = &self.atlas {
            atlas.bind(0);
        }
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        }

        for p in &self.particles {
            let alpha = (p.life / p.max_life).clamp(0.0, 1.0);
            shader.set_vec4(
                loc_diff,
                glam::Vec4::new(p.color.x, p.color.y, p.color.z, alpha * p.color.w),
            );

            let model = glam::Mat4::from_translation(p.position)
                * glam::Mat4::from_scale(glam::Vec3::splat(p.scale * alpha));
            shader.set_mat4(loc_model, &model);
            self.particle_mesh.draw();
        }
        shader.set_vec4(loc_diff, glam::Vec4::ONE);
        shader.set_mat4(loc_model, &glam::Mat4::IDENTITY);
        unsafe {
            gl::Disable(gl::BLEND);
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
            let was_dirty = chunk.dirty;
            chunk.set_block(bx, y as usize, bz, block);
            if !was_dirty {
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
        let data = crate::atlas::generate_atlas_data();
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
            if block != BlockType::Air && block != BlockType::Water && block != BlockType::Lava {
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
                                let mut chunk = Box::new(Chunk::new(x, z, self.seed));
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
                                let mut chunk = Box::new(Chunk::new(x, z, self.seed));
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
                    if chunk.pending_mesh_revision == chunk.edit_revision {
                        chunk.upload_mesh(opaque, trans, water);
                    } else {
                        chunk.meshing_in_progress = false;
                        if !chunk.dirty {
                            chunk.dirty = true;
                            self.dirty_count += 1;
                        }
                    }
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
                    chunk.meshing_revision = chunk.edit_revision;
                    self.dirty_count -= 1;
                    self.meshing_in_flight += 1;

                    let chunk_ptr = chunk.as_mut() as *mut Chunk as usize;
                    rayon::spawn(move || {
                        let w = unsafe { &*(world_ptr as *const World) };
                        let c = unsafe { &mut *(chunk_ptr as *mut Chunk) };

                        c.calculate_lighting();
                        let (op, tr, wa) = c.calculate_mesh_data(w);

                        c.pending_mesh_revision = c.meshing_revision;
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
        let mut fuse_particles = Vec::new();
        while i < self.explosives.len() {
            let velocity = self.explosives[i].velocity;
            let mut next_pos = self.explosives[i].position + velocity * _time;
            let center_x = (next_pos.x + 0.5).floor() as i32;
            let center_z = (next_pos.z + 0.5).floor() as i32;
            let below_y = next_pos.y.floor() as i32 - 1;

            self.explosives[i].velocity.y -= 9.8 * _time;
            if velocity.y < 0.0 && self.get_block(center_x, below_y, center_z).is_solid() {
                next_pos.y = (below_y + 1) as f32;
                self.explosives[i].velocity.y = -velocity.y * 0.35;
                self.explosives[i].velocity.x *= 0.75;
                self.explosives[i].velocity.z *= 0.75;
                if self.explosives[i].velocity.y < 0.2 {
                    self.explosives[i].velocity.y = 0.0;
                }
            }

            self.explosives[i].position = next_pos;
            self.explosives[i].fuse -= _time;
            let progress = 1.0
                - (self.explosives[i].fuse / self.explosives[i].initial_fuse.max(0.001))
                    .clamp(0.0, 1.0);
            if rand::random::<f32>() < _time * (5.0 + progress * 14.0) {
                let jitter = Vec3::new(
                    (rand::random::<f32>() - 0.5) * 0.36,
                    rand::random::<f32>() * 0.08,
                    (rand::random::<f32>() - 0.5) * 0.36,
                );
                let velocity = Vec3::new(
                    (rand::random::<f32>() - 0.5) * 0.35,
                    0.55 + rand::random::<f32>() * 0.45,
                    (rand::random::<f32>() - 0.5) * 0.35,
                );
                fuse_particles.push(crate::block::Particle {
                    position: self.explosives[i].position + Vec3::new(0.5, 1.05, 0.5) + jitter,
                    velocity,
                    color: glam::Vec4::new(0.58, 0.56, 0.52, 0.72),
                    life: 0.45 + rand::random::<f32>() * 0.35,
                    max_life: 0.8,
                    scale: 0.10 + rand::random::<f32>() * 0.08,
                });
            }
            if progress > 0.55 && rand::random::<f32>() < _time * 8.0 {
                fuse_particles.push(crate::block::Particle {
                    position: self.explosives[i].position + Vec3::new(0.5, 1.12, 0.5),
                    velocity: Vec3::new(
                        (rand::random::<f32>() - 0.5) * 0.8,
                        0.7,
                        (rand::random::<f32>() - 0.5) * 0.8,
                    ),
                    color: glam::Vec4::new(1.0, 0.42, 0.08, 0.95),
                    life: 0.18,
                    max_life: 0.18,
                    scale: 0.07,
                });
            }
            if self.explosives[i].fuse <= 0.0 {
                let e = self.explosives.remove(i);
                self.detonations.push(e.position);
            } else {
                i += 1;
            }
        }
        self.particles.extend(fuse_particles);

        // --- Process Detonations ---
        let dets: Vec<glam::Vec3> = std::mem::take(&mut self.detonations);
        for pos in dets {
            crate::explosion::explode(self, pos.x as i32, pos.y as i32, pos.z as i32, 4);
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
                if target == BlockType::Air
                    || target == BlockType::Water
                    || target == BlockType::Lava
                {
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
        let cloud_height = CLOUD_HEIGHT;
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
