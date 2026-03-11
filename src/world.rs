use crate::chunk::{Chunk, CHUNK_WIDTH, CHUNK_DEPTH, CHUNK_HEIGHT};
use crate::block::BlockType;
use crate::renderer::{Mesh, Texture2D, Shader};
use glam::{Vec3, Mat4};

pub const VIEW_DISTANCE: i32 = 12;
pub const POOL_WIDTH: i32 = VIEW_DISTANCE * 2 + 1;
pub const CHUNK_POOL_SIZE: usize = (POOL_WIDTH * POOL_WIDTH) as usize;

#[derive(Clone, Copy)]
pub struct WorldEdit {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub block: BlockType,
}

pub struct RaycastResult {
    pub hit: bool,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub nx: i32,
    pub ny: i32,
    pub nz: i32,
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
        planes[0] = [mat[0][3] - mat[0][0], mat[1][3] - mat[1][0], mat[2][3] - mat[2][0], mat[3][3] - mat[3][0]];
        // Left
        planes[1] = [mat[0][3] + mat[0][0], mat[1][3] + mat[1][0], mat[2][3] + mat[2][0], mat[3][3] + mat[3][0]];
        // Bottom
        planes[2] = [mat[0][3] + mat[0][1], mat[1][3] + mat[1][1], mat[2][3] + mat[2][1], mat[3][3] + mat[3][1]];
        // Top
        planes[3] = [mat[0][3] - mat[0][1], mat[1][3] - mat[1][1], mat[2][3] - mat[2][1], mat[3][3] - mat[3][1]];
        // Far
        planes[4] = [mat[0][3] - mat[0][2], mat[1][3] - mat[1][2], mat[2][3] - mat[2][2], mat[3][3] - mat[3][2]];
        // Near
        planes[5] = [mat[0][3] + mat[0][2], mat[1][3] + mat[1][2], mat[2][3] + mat[2][2], mat[3][3] + mat[3][2]];

        for plane in planes.iter_mut() {
            let length = (plane[0] * plane[0] + plane[1] * plane[1] + plane[2] * plane[2]).sqrt();
            if length > 0.0 {
                for val in plane.iter_mut() { *val /= length; }
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
    pub edits: Vec<WorldEdit>,
    pub suppress_edit_recording: bool,
    pub last_pcx: i32,
    pub last_pcz: i32,
    pub dirty_count: i32,
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
            edits: Vec::new(),
            suppress_edit_recording: false,
            last_pcx: -999999,
            last_pcz: -999999,
            dirty_count: 0,
        }
    }

    fn get_pool_index(&self, cx: i32, cz: i32) -> usize {
        let ix = cx.rem_euclid(POOL_WIDTH);
        let iz = cz.rem_euclid(POOL_WIDTH);
        (ix + iz * POOL_WIDTH) as usize
    }

    pub fn get_chunk(&self, cx: i32, cz: i32) -> Option<&Chunk> {
        let index = self.get_pool_index(cx, cz);
        if let Some(chunk) = &self.chunks[index] {
            if chunk.x == cx && chunk.z == cz {
                return Some(chunk.as_ref());
            }
        }
        None
    }

    pub fn get_chunk_mut(&mut self, cx: i32, cz: i32) -> Option<&mut Chunk> {
        let index = self.get_pool_index(cx, cz);
        if let Some(chunk) = &mut self.chunks[index] {
            if chunk.x == cx && chunk.z == cz {
                return Some(chunk.as_mut());
            }
        }
        None
    }

    pub fn get_block(&self, x: i32, y: i32, z: i32) -> BlockType {
        if y < 0 || y >= CHUNK_HEIGHT as i32 {
            return BlockType::Air;
        }
        let cx = x.div_euclid(CHUNK_WIDTH as i32);
        let cz = z.div_euclid(CHUNK_DEPTH as i32);
        
        if let Some(chunk) = self.get_chunk(cx, cz) {
            let bx = x.rem_euclid(CHUNK_WIDTH as i32) as usize;
            let bz = z.rem_euclid(CHUNK_DEPTH as i32) as usize;
            chunk.get_block(bx, y as usize, bz)
        } else {
            BlockType::Air
        }
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, block: BlockType) {
        if y < 0 || y >= CHUNK_HEIGHT as i32 { return; }
        
        if !self.suppress_edit_recording {
            if let Some(pos) = self.edits.iter().position(|e| e.x == x && e.y == y && e.z == z) {
                self.edits[pos].block = block;
            } else {
                self.edits.push(WorldEdit { x, y, z, block });
            }
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
            
            let mut neighbors = Vec::new();
            if bx == 0 { neighbors.push((cx - 1, cz)); }
            if bx == CHUNK_WIDTH - 1 { neighbors.push((cx + 1, cz)); }
            if bz == 0 { neighbors.push((cx, cz - 1)); }
            if bz == CHUNK_DEPTH - 1 { neighbors.push((cx, cz + 1)); }
            
            for (nx, nz) in neighbors {
                if let Some(nc) = self.get_chunk_mut(nx, nz) {
                    if !nc.dirty {
                        nc.dirty = true;
                        self.dirty_count += 1;
                    }
                }
            }
        }
    }

    pub fn apply_edits_to_chunk(&mut self, cx: i32, cz: i32) {
        let chunk_start_x = cx * CHUNK_WIDTH as i32;
        let chunk_start_z = cz * CHUNK_DEPTH as i32;
        let chunk_end_x = chunk_start_x + CHUNK_WIDTH as i32;
        let chunk_end_z = chunk_start_z + CHUNK_DEPTH as i32;

        let edits = self.edits.clone();
        for edit in edits {
            if edit.x >= chunk_start_x && edit.x < chunk_end_x && 
               edit.z >= chunk_start_z && edit.z < chunk_end_z {
                self.suppress_edit_recording = true;
                self.set_block(edit.x, edit.y, edit.z, edit.block);
                self.suppress_edit_recording = false;
            }
        }
    }

    pub fn generate_atlas(&mut self) {
        let mut data = vec![0u8; 256 * 256 * 4];
        let mut draw_pixel = |x: i32, y: i32, r: u8, g: u8, b: u8, a: u8| {
            if x >= 0 && x < 256 && y >= 0 && y < 256 {
                let idx = ((y * 256 + x) * 4) as usize;
                data[idx] = r; data[idx+1] = g; data[idx+2] = b; data[idx+3] = a;
            }
        };
        let mut draw_block = |tx: i32, ty: i32, r: u8, g: u8, b: u8| {
            for x in 0..16 {
                for y in 0..16 {
                    let noise = (crate::noise::noise_2d(x as f32 * 0.5, y as f32 * 0.5) * 20.0) as i32;
                    let nr = (r as i32 + noise).clamp(0, 255) as u8;
                    let ng = (g as i32 + noise).clamp(0, 255) as u8;
                    let nb = (b as i32 + noise).clamp(0, 255) as u8;
                    draw_pixel(tx * 16 + x, ty * 16 + y, nr, ng, nb, 255);
                }
            }
        };
        draw_block(1, 0, 140, 140, 140); draw_block(2, 0, 130, 90, 60);
        draw_block(3, 0, 80, 160, 40); draw_block(0, 0, 100, 180, 50);
        draw_block(4, 0, 100, 70, 40); draw_block(5, 1, 110, 80, 50);
        draw_block(5, 0, 40, 120, 30); draw_block(6, 0, 220, 210, 160);
        draw_block(7, 0, 120, 120, 130); draw_block(1, 1, 60, 60, 60);
        draw_block(2, 1, 140, 140, 140); draw_block(13, 12, 40, 80, 200);
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
        let mut t_max_x = if dir.x != 0.0 { ((ix as f32 + (step_x.max(0) as f32)) - origin.x) / dir.x } else { f32::INFINITY };
        let mut t_max_y = if dir.y != 0.0 { ((iy as f32 + (step_y.max(0) as f32)) - origin.y) / dir.y } else { f32::INFINITY };
        let mut t_max_z = if dir.z != 0.0 { ((iz as f32 + (step_z.max(0) as f32)) - origin.z) / dir.z } else { f32::INFINITY };
        let t_delta_x = if dir.x != 0.0 { (1.0 / dir.x).abs() } else { f32::INFINITY };
        let t_delta_y = if dir.y != 0.0 { (1.0 / dir.y).abs() } else { f32::INFINITY };
        let t_delta_z = if dir.z != 0.0 { (1.0 / dir.z).abs() } else { f32::INFINITY };
        let mut nx = 0; let mut ny = 0; let mut nz = 0;
        while t < max_distance {
            let block = self.get_block(ix, iy, iz);
            if block != BlockType::Air && block != BlockType::Water {
                return RaycastResult { hit: true, x: ix, y: iy, z: iz, nx, ny, nz, block };
            }
            if t_max_x < t_max_y {
                if t_max_x < t_max_z { ix += step_x; t = t_max_x; t_max_x += t_delta_x; nx = -step_x; ny = 0; nz = 0; }
                else { iz += step_z; t = t_max_z; t_max_z += t_delta_z; nx = 0; ny = 0; nz = -step_z; }
            } else {
                if t_max_y < t_max_z { iy += step_y; t = t_max_y; t_max_y += t_delta_y; nx = 0; ny = -step_y; nz = 0; }
                else { iz += step_z; t = t_max_z; t_max_z += t_delta_z; nx = 0; ny = 0; nz = -step_z; }
            }
        }
        RaycastResult { hit: false, x: 0, y: 0, z: 0, nx: 0, ny: 0, nz: 0, block: BlockType::Air }
    }

    pub fn update(&mut self, player_pos: Vec3, _time: f32) {
        let pcx = (player_pos.x / CHUNK_WIDTH as f32).floor() as i32;
        let pcz = (player_pos.z / CHUNK_DEPTH as f32).floor() as i32;
        if pcx != self.last_pcx || pcz != self.last_pcz {
            for x in (pcx - VIEW_DISTANCE)..=(pcx + VIEW_DISTANCE) {
                for z in (pcz - VIEW_DISTANCE)..=(pcz + VIEW_DISTANCE) {
                    let index = self.get_pool_index(x, z);
                    let should_replace = match &self.chunks[index] { None => true, Some(c) => c.x != x || c.z != z };
                    if should_replace {
                        let mut chunk = Box::new(Chunk::new(x, z));
                        chunk.generate();
                        self.chunks[index] = Some(chunk);
                        self.apply_edits_to_chunk(x, z);
                        if let Some(c) = &mut self.chunks[index] { c.dirty = true; self.dirty_count += 1; }
                    }
                }
            }
            self.last_pcx = pcx; self.last_pcz = pcz;
        }
        let mut builds_this_frame = 0;
        let mut indices_to_build = Vec::new();
        for i in 0..CHUNK_POOL_SIZE {
            if let Some(chunk) = &mut self.chunks[i] {
                if chunk.dirty { indices_to_build.push(i); chunk.dirty = false; self.dirty_count -= 1; builds_this_frame += 1; if builds_this_frame >= 4 { break; } }
            }
        }
        for index in indices_to_build {
            let mut chunk = self.chunks[index].take().unwrap();
            chunk.build_mesh(self);
            self.chunks[index] = Some(chunk);
        }
    }

    pub fn init_celestial(&mut self) {
        let mut v = Vec::new(); let mut c = Vec::new();
        for _ in 0..300 {
            let x = (rand::random::<f32>() * 2.0 - 1.0) * 400.0;
            let y = rand::random::<f32>() * 200.0 + 50.0;
            let z = (rand::random::<f32>() * 2.0 - 1.0) * 400.0;
            let q = 0.5;
            v.extend_from_slice(&[x-q, y, z, x+q, y, z, x+q, y+q, z]);
            v.extend_from_slice(&[x-q, y, z, x+q, y+q, z, x-q, y+q, z]);
            for _ in 0..6 { c.extend_from_slice(&[255, 255, 255, 255]); }
        }
        self.star_model = Some(Mesh::new(&v, None, None, Some(&c)));
    }

    pub fn update_clouds(&mut self, player_pos: Vec3, time: f32) {
        let dist = self.last_cloud_pos.distance(player_pos);
        let time_passed = (time - self.last_cloud_update_time) > 0.5;
        if dist < 32.0 && !time_passed && self.cloud_model.is_some() { return; }
        self.cloud_model = None; self.last_cloud_pos = player_pos; self.last_cloud_update_time = time;
        let cloud_height = 200.0; let cloud_size = 16.0; let range = 64;
        let start_x = (player_pos.x / cloud_size).floor() as i32 - range;
        let start_z = (player_pos.z / cloud_size).floor() as i32 - range;
        let mut v = Vec::new(); let mut c = Vec::new(); let mut n = Vec::new();
        let cloud_color = [225, 233, 240, 150];
        for x in start_x..(start_x + range * 2) {
            for z in start_z..(start_z + range * 2) {
                let base = crate::noise::perlin_2d(x as f32 * 0.18 + time * 0.003, z as f32 * 0.18 + 41.0, 0.5, 2);
                let breakup = crate::noise::perlin_2d(x as f32 * 0.48 - time * 0.005, z as f32 * 0.48 + 7.0, 0.5, 1);
                if base > 0.10 && breakup > -0.08 {
                    let px = x as f32 * cloud_size; let py = cloud_height; let pz = z as f32 * cloud_size;
                    let sx = cloud_size; let sy = 1.1; let sz = cloud_size;
                    let cube = [
                        px, py + sy, pz, px, py + sy, pz + sz, px + sx, py + sy, pz + sz,
                        px, py + sy, pz, px + sx, py + sy, pz + sz, px + sx, py + sy, pz,
                        px, py, pz, px + sx, py, pz + sz, px, py, pz + sz, px, py, pz,
                        px + sx, py, pz, px + sx, py, pz + sz,
                        px, py, pz + sz, px + sx, py, pz + sz, px + sx, py + sy, pz + sz,
                        px, py, pz + sz, px + sx, py + sy, pz + sz, px, py + sy, pz + sz,
                        px + sx, py, pz, px, py, pz, px, py + sy, pz, px + sx, py, pz, px, py + sy, pz, px + sx, py + sy, pz,
                        px + sx, py, pz + sz, px + sx, py, pz, px + sx, py + sy, pz, px + sx, py, pz + sz, px + sx, py + sy, pz, px + sx, py + sy, pz + sz,
                        px, py, pz, px, py, pz + sz, px, py + sy, pz + sz, px, py, pz, px, py + sy, pz + sz, px, py + sy, pz,
                    ];
                    v.extend_from_slice(&cube); for _ in 0..36 { c.extend_from_slice(&cloud_color); n.extend_from_slice(&[0.0, 1.0, 0.0]); }
                }
            }
        }
        if !v.is_empty() { self.cloud_model = Some(Mesh::new(&v, None, Some(&n), Some(&c))); }
    }

    pub fn render_opaque(&self, frustum: &Frustum) {
        if let Some(atlas) = &self.atlas { atlas.bind(0); }
        for chunk_opt in &self.chunks {
            if let Some(chunk) = chunk_opt {
                let min = Vec3::new(chunk.x as f32 * CHUNK_WIDTH as f32, 0.0, chunk.z as f32 * CHUNK_DEPTH as f32);
                let max = Vec3::new(min.x + CHUNK_WIDTH as f32, CHUNK_HEIGHT as f32, min.z + CHUNK_DEPTH as f32);
                if frustum.is_box_visible(min, max) { if let Some(mesh) = &chunk.mesh_opaque { mesh.draw(); } }
            }
        }
    }

    pub fn render_transparent(&self, frustum: &Frustum) {
        for chunk_opt in &self.chunks {
            if let Some(chunk) = chunk_opt {
                let min = Vec3::new(chunk.x as f32 * CHUNK_WIDTH as f32, 0.0, chunk.z as f32 * CHUNK_DEPTH as f32);
                let max = Vec3::new(min.x + CHUNK_WIDTH as f32, CHUNK_HEIGHT as f32, min.z + CHUNK_DEPTH as f32);
                if frustum.is_box_visible(min, max) { if let Some(mesh) = &chunk.mesh_transparent { mesh.draw(); } }
            }
        }
    }

    pub fn render_clouds(&self, shader: &Shader, mvp: &Mat4) {
        if let Some(mesh) = &self.cloud_model {
            shader.bind(); shader.set_mat4(shader.get_uniform_location("uMVP"), mvp);
            shader.set_int(shader.get_uniform_location("uIsCircle"), 0);
            unsafe {
                gl::Disable(gl::CULL_FACE); gl::Enable(gl::BLEND); gl::DepthMask(gl::FALSE);
                mesh.draw(); gl::DepthMask(gl::TRUE); gl::Enable(gl::CULL_FACE);
            }
        }
    }

    pub fn render_stars(&self, player_pos: Vec3, time: f32, shader: &Shader, mvp: &Mat4) {
        let angle = (time / 1200.0) * 2.0 * std::f32::consts::PI;
        let sun_y = angle.sin(); if sun_y > -0.3 { return; }
        let star_intensity = (1.0 - ((sun_y + 0.3) / 0.3)).clamp(0.0, 1.0);
        let star_alpha = (200.0 * star_intensity) as u8; if star_alpha < 30 { return; }
        if let Some(mesh) = &self.star_model {
            shader.bind(); shader.set_int(shader.get_uniform_location("uIsCircle"), 0);
            let star_mvp = *mvp * Mat4::from_translation(player_pos);
            shader.set_mat4(shader.get_uniform_location("uMVP"), &star_mvp);
            unsafe { gl::Disable(gl::DEPTH_TEST); mesh.draw(); gl::Enable(gl::DEPTH_TEST); }
        }
    }
}
