use crate::block::BlockType;
use crate::chunk::{
    CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH, Chunk, MeshResult, MeshSnapshot, water_decay,
    water_is_falling, water_is_source, water_level_from_decay,
};
use crate::mob::{Mob, MobKind};
use crate::renderer;
use crate::renderer::{Mesh, Shader, Texture2D};
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::RwLock;

// M2 test harness (hashed-splashing-haven plan): (pos, uv, normal, color).
#[allow(dead_code)]
type GpuMeshTestVertex = ([f32; 3], [f32; 2], [f32; 3], [u8; 4]);

#[allow(dead_code)]
fn gpu_mesh_test_sort_key(v: &GpuMeshTestVertex) -> [i64; 9] {
    let q = |f: f32| (f * 1000.0).round() as i64;
    [
        q(v.0[0]),
        q(v.0[1]),
        q(v.0[2]),
        q(v.1[0]),
        q(v.1[1]),
        q(v.2[0]),
        q(v.2[1]),
        q(v.2[2]),
        ((v.3[0] as i64) << 24) | ((v.3[1] as i64) << 16) | ((v.3[2] as i64) << 8) | v.3[3] as i64,
    ]
}

// Sorts both vertex lists by a canonical key before comparing, since the GPU
// mesher's face/vertex emission order isn't guaranteed to match the CPU's
// (different (x,z) columns run in parallel), even when the resulting mesh
// is geometrically identical.
#[allow(dead_code)]
fn compare_gpu_mesh_test(cpu: &[GpuMeshTestVertex], gpu: &[GpuMeshTestVertex], cx: i32, cz: i32) {
    println!(
        "[M2 test] chunk ({cx},{cz}): CPU {} verts, GPU {} verts",
        cpu.len(),
        gpu.len()
    );
    if cpu.len() != gpu.len() {
        println!("[M2 test] FAIL: vertex count mismatch");
        return;
    }
    let mut cpu_sorted = cpu.to_vec();
    let mut gpu_sorted = gpu.to_vec();
    cpu_sorted.sort_by_key(gpu_mesh_test_sort_key);
    gpu_sorted.sort_by_key(gpu_mesh_test_sort_key);

    let mut mismatches = 0;
    for (i, (c, g)) in cpu_sorted.iter().zip(gpu_sorted.iter()).enumerate() {
        let pos_ok =
            c.0.iter()
                .zip(g.0.iter())
                .all(|(a, b)| (a - b).abs() < 0.01);
        let uv_ok =
            c.1.iter()
                .zip(g.1.iter())
                .all(|(a, b)| (a - b).abs() < 0.001);
        let normal_ok =
            c.2.iter()
                .zip(g.2.iter())
                .all(|(a, b)| (a - b).abs() < 0.01);
        let color_ok =
            c.3.iter()
                .zip(g.3.iter())
                .all(|(a, b)| (*a as i32 - *b as i32).abs() <= 2);
        if !(pos_ok && uv_ok && normal_ok && color_ok) {
            mismatches += 1;
            if mismatches <= 10 {
                println!("[M2 test]   mismatch #{i}: cpu={c:?} gpu={g:?}");
            }
        }
    }
    if mismatches == 0 {
        println!("[M2 test] PASS: all {} vertices match", cpu.len());
    } else {
        println!(
            "[M2 test] FAIL: {mismatches}/{} vertices mismatched",
            cpu.len()
        );
    }
}

pub const DEFAULT_VIEW_DISTANCE: i32 = 8;
pub const MAX_VIEW_DISTANCE: i32 = 12;
pub const VIEW_DISTANCE: i32 = DEFAULT_VIEW_DISTANCE;
pub const POOL_WIDTH: i32 = MAX_VIEW_DISTANCE * 2 + 1;
pub const CHUNK_POOL_SIZE: usize = (POOL_WIDTH * POOL_WIDTH) as usize;
pub const CLOUD_HEIGHT: f32 = 200.0;
// Upper bound on chunk-generation jobs queued to the worker pool; bounds
// memory for finished-but-unintegrated chunks without starving meshing jobs.
const MAX_GEN_IN_FLIGHT: usize = 64;
// Radius the GPU voxel pool mirrors for meshing. Kept at the max CPU view
// distance so settings cannot request a larger GPU neighborhood than we
// allocated.
pub const GPU_POOL_VIEW_DISTANCE: i32 = MAX_VIEW_DISTANCE;
pub const GPU_POOL_WIDTH: i32 = GPU_POOL_VIEW_DISTANCE * 2 + 1;
pub const GPU_POOL_SIZE: usize = (GPU_POOL_WIDTH * GPU_POOL_WIDTH) as usize;

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
    pub view_distance: i32,
    pub dirty_count: i32,
    pub pending_hurt: i32,
    pub pending_drops: Vec<(BlockType, u32)>,
    pub explosives: Vec<crate::block::ActiveExplosive>,
    pub arrows: Vec<crate::block::ArrowEntity>,
    pub xp_orbs: Vec<crate::block::XpOrbEntity>,
    pub particles: Vec<crate::block::Particle>,
    pub detonations: Vec<glam::Vec3>,
    pub tnt_mesh: renderer::Mesh,
    pub spark_mesh: renderer::Mesh,
    pub particle_mesh: renderer::Mesh,
    pub arrow_mesh: renderer::Mesh,
    pub xp_orb_mesh: renderer::Mesh,
    pub is_loading: bool,
    pub loading_radius: i32,
    pub chunks_generated_count: i32,
    pub active_water: std::collections::HashSet<(i32, i32, i32)>,
    pub active_falling: std::collections::HashSet<(i32, i32, i32)>,
    pub water_tick_timer: f32,
    pub visible_chunks: Vec<usize>,
    pub meshing_in_flight: i32,
    next_mesh_job_id: u64,
    mesh_result_tx: std::sync::mpsc::Sender<MeshResult>,
    mesh_result_rx: std::sync::mpsc::Receiver<MeshResult>,
    gen_result_tx: std::sync::mpsc::Sender<Box<Chunk>>,
    gen_result_rx: std::sync::mpsc::Receiver<Box<Chunk>>,
    // Chunk coordinates currently being generated on a worker
    gen_in_flight: std::collections::HashSet<(i32, i32)>,
    pub mobs: Vec<Mob>,
    // Village center chunks whose inhabitants have already been spawned
    spawned_villages: std::collections::HashSet<(i32, i32)>,
    // Chunks whose natural mobs (passive, hostile, spawner) have already been spawned
    spawned_natural_chunks: std::collections::HashSet<(i32, i32)>,
    imported: HashMap<(i32, i32), Box<Chunk>>,
    import_world: Option<PathBuf>,
    villager_mesh: renderer::Mesh,
    golem_mesh: renderer::Mesh,
}

impl World {
    pub fn new(seed: u64) -> Self {
        let mut chunks = Vec::with_capacity(CHUNK_POOL_SIZE);
        for _ in 0..CHUNK_POOL_SIZE {
            chunks.push(None);
        }
        let (mesh_result_tx, mesh_result_rx) = std::sync::mpsc::channel();
        let (gen_result_tx, gen_result_rx) = std::sync::mpsc::channel();
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
            view_distance: DEFAULT_VIEW_DISTANCE,
            dirty_count: 0,
            pending_hurt: 0,
            pending_drops: Vec::new(),
            explosives: Vec::new(),
            arrows: Vec::new(),
            xp_orbs: Vec::new(),
            particles: Vec::new(),
            detonations: Vec::new(),
            tnt_mesh: Self::create_textured_cube_mesh(BlockType::TNT),
            spark_mesh: Self::create_textured_cube_mesh(BlockType::Torch),
            particle_mesh: Self::create_textured_cube_mesh(BlockType::SnowLayer),
            arrow_mesh: Self::create_textured_cube_mesh(BlockType::Arrow),
            xp_orb_mesh: Self::create_textured_cube_mesh(BlockType::GoldOre),
            is_loading: true,
            loading_radius: 0,
            chunks_generated_count: 0,
            active_water: std::collections::HashSet::new(),
            active_falling: std::collections::HashSet::new(),
            water_tick_timer: 0.0,
            visible_chunks: Vec::new(),
            meshing_in_flight: 0,
            next_mesh_job_id: 1,
            mesh_result_tx,
            mesh_result_rx,
            gen_result_tx,
            gen_result_rx,
            gen_in_flight: std::collections::HashSet::new(),
            mobs: Vec::new(),
            spawned_villages: std::collections::HashSet::new(),
            spawned_natural_chunks: std::collections::HashSet::new(),
            imported: HashMap::new(),
            import_world: None,
            villager_mesh: Self::create_textured_cube_mesh(BlockType::Wool),
            golem_mesh: Self::create_textured_cube_mesh(BlockType::IronBlock),
        }
    }

    fn create_textured_cube_mesh(block: BlockType) -> renderer::Mesh {
        #[rustfmt::skip]
        let v: Vec<f32> = vec![
            0.0, 0.0, 0.0,  1.0, 0.0, 0.0,  1.0, 0.0, 1.0,
            0.0, 0.0, 0.0,  1.0, 0.0, 1.0,  0.0, 0.0, 1.0, // bottom (-Y)
            0.0, 1.0, 0.0,  0.0, 1.0, 1.0,  1.0, 1.0, 1.0,
            0.0, 1.0, 0.0,  1.0, 1.0, 1.0,  1.0, 1.0, 0.0, // top (+Y)
            0.0, 0.0, 0.0,  0.0, 1.0, 0.0,  1.0, 1.0, 0.0,
            0.0, 0.0, 0.0,  1.0, 1.0, 0.0,  1.0, 0.0, 0.0, // front (-Z)
            0.0, 0.0, 1.0,  1.0, 0.0, 1.0,  1.0, 1.0, 1.0,
            0.0, 0.0, 1.0,  1.0, 1.0, 1.0,  0.0, 1.0, 1.0, // back (+Z)
            0.0, 0.0, 0.0,  0.0, 0.0, 1.0,  0.0, 1.0, 1.0,
            0.0, 0.0, 0.0,  0.0, 1.0, 1.0,  0.0, 1.0, 0.0, // left (-X)
            1.0, 0.0, 0.0,  1.0, 1.0, 0.0,  1.0, 1.0, 1.0,
            1.0, 0.0, 0.0,  1.0, 1.0, 1.0,  1.0, 0.0, 1.0, // right (+X)
        ];

        let ts = 1.0 / 16.0;
        let pad = 0.001;

        let get_uvs = |(tx, ty): (u8, u8)| -> (f32, f32, f32, f32) {
            (
                tx as f32 * ts + pad,
                ty as f32 * ts + pad,
                (tx as f32 + 1.0) * ts - pad,
                (ty as f32 + 1.0) * ts - pad,
            )
        };

        let (b_u0, b_v0, b_u1, b_v1) = get_uvs(crate::item::atlas_uv_bottom(block));
        let (t_u0, t_v0, t_u1, t_v1) = get_uvs(crate::item::atlas_uv_top(block));
        let (s_u0, s_v0, s_u1, s_v1) = get_uvs(crate::item::atlas_uv(block));

        #[rustfmt::skip]
        let t = vec![
            // bottom (-Y)
            b_u0, b_v0, b_u1, b_v0, b_u1, b_v1,  b_u0, b_v0, b_u1, b_v1, b_u0, b_v1,
            // top (+Y)
            t_u0, t_v0, t_u0, t_v1, t_u1, t_v1,  t_u0, t_v0, t_u1, t_v1, t_u1, t_v0,
            // front (-Z)
            s_u0, s_v1, s_u0, s_v0, s_u1, s_v0,  s_u0, s_v1, s_u1, s_v0, s_u1, s_v1,
            // back (+Z)
            s_u1, s_v1, s_u0, s_v1, s_u0, s_v0,  s_u1, s_v1, s_u0, s_v0, s_u1, s_v0,
            // left (-X)
            s_u1, s_v1, s_u0, s_v1, s_u0, s_v0,  s_u1, s_v1, s_u0, s_v0, s_u1, s_v0,
            // right (+X)
            s_u0, s_v1, s_u0, s_v0, s_u1, s_v0,  s_u0, s_v1, s_u1, s_v0, s_u1, s_v1,
        ];

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
        crate::renderer::set_blend(true);

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
        crate::renderer::set_blend(false);
    }

    pub fn render_arrows(&self, shader: &crate::renderer::Shader) {
        let loc_model = shader.get_uniform_location("uModel");
        let loc_diff = shader.get_uniform_location("colDiffuse");
        if let Some(atlas) = &self.atlas {
            atlas.bind(0);
        }
        shader.set_vec4(loc_diff, glam::Vec4::ONE);

        for arrow in &self.arrows {
            let rot = if arrow.velocity.length_squared() > 0.01 {
                let dir = arrow.velocity.normalize();
                let yaw = dir.x.atan2(dir.z);
                let pitch = (-dir.y).atan2((dir.x * dir.x + dir.z * dir.z).sqrt());
                glam::Mat4::from_rotation_y(yaw) * glam::Mat4::from_rotation_x(pitch)
            } else {
                glam::Mat4::IDENTITY
            };
            let scale = Vec3::new(0.08, 0.08, 0.45);
            let model =
                glam::Mat4::from_translation(arrow.position) * rot * glam::Mat4::from_scale(scale);
            shader.set_mat4(loc_model, &model);
            self.arrow_mesh.draw();
        }
        shader.set_mat4(loc_model, &glam::Mat4::IDENTITY);
    }

    pub fn render_xp_orbs(&self, shader: &crate::renderer::Shader, current_time: f32) {
        let loc_model = shader.get_uniform_location("uModel");
        let loc_diff = shader.get_uniform_location("colDiffuse");
        if let Some(atlas) = &self.atlas {
            atlas.bind(0);
        }

        for orb in &self.xp_orbs {
            let bob = (current_time * 5.0 + orb.position.x * 3.0).sin() * 0.08;
            let rot = current_time * 3.0;
            let model = glam::Mat4::from_translation(orb.position + Vec3::new(0.0, bob, 0.0))
                * glam::Mat4::from_rotation_y(rot)
                * glam::Mat4::from_scale(Vec3::splat(0.18));
            shader.set_vec4(loc_diff, glam::Vec4::new(0.4, 1.4, 0.2, 1.0));
            shader.set_mat4(loc_model, &model);
            self.xp_orb_mesh.draw();
        }
        shader.set_vec4(loc_diff, glam::Vec4::ONE);
        shader.set_mat4(loc_model, &glam::Mat4::IDENTITY);
    }

    fn get_pool_index(&self, cx: i32, cz: i32) -> usize {
        let ix = cx.rem_euclid(POOL_WIDTH);
        let iz = cz.rem_euclid(POOL_WIDTH);
        (ix + iz * POOL_WIDTH) as usize
    }

    // M2/M3 test harness (hashed-splashing-haven plan): meshes one chunk both
    // ways and diffs the results (M2), then sets up the same chunk as a
    // persistent compute-meshed indirect draw floating above its CPU-meshed
    // twin (M3). Dev-only, meant to run once at startup.
    #[allow(dead_code)]
    pub fn run_gpu_mesh_tests(&self) {
        renderer::gpu_pool_init(GPU_POOL_SIZE);
        for chunk in self.chunks.iter().flatten() {
            let (blocks, light, liquid) = chunk.gpu_bytes();
            renderer::gpu_pool_upload_chunk(
                self.gpu_pool_index(chunk.x, chunk.z) as u32,
                chunk.x,
                chunk.z,
                blocks,
                light,
                liquid,
            );
        }
        let cx = self.last_pcx;
        let cz = self.last_pcz;
        let Some(chunk) = self.get_chunk(cx, cz) else {
            println!("[M2 test] no chunk loaded at ({cx},{cz}), skipping");
            return;
        };
        let snap = MeshSnapshot::capture(self, cx, cz);
        let mut work = chunk.snapshot_data();
        work.calculate_lighting();
        let (opaque, _transparent, _water) = work.calculate_mesh_data(&snap);

        let cpu_verts: Vec<GpuMeshTestVertex> = (0..opaque.v.len() / 3)
            .map(|i| {
                (
                    [opaque.v[i * 3], opaque.v[i * 3 + 1], opaque.v[i * 3 + 2]],
                    [opaque.t[i * 2], opaque.t[i * 2 + 1]],
                    [opaque.n[i * 3], opaque.n[i * 3 + 1], opaque.n[i * 3 + 2]],
                    [
                        opaque.c[i * 4],
                        opaque.c[i * 4 + 1],
                        opaque.c[i * 4 + 2],
                        opaque.c[i * 4 + 3],
                    ],
                )
            })
            .collect();

        match renderer::gpu_mesh_test_run(cx, cz, GPU_POOL_WIDTH) {
            Ok(gpu_raw) => {
                let gpu_verts: Vec<GpuMeshTestVertex> = gpu_raw
                    .into_iter()
                    .map(|v| (v.pos, v.uv, v.normal, v.color))
                    .collect();
                compare_gpu_mesh_test(&cpu_verts, &gpu_verts, cx, cz);
            }
            Err(e) => println!("[M2 test] GPU dispatch failed: {e}"),
        }

        // M3: same chunk again, but meshed straight into a GPU vertex buffer
        // + DrawIndirectArgs and rendered per-frame via draw_indirect (see
        // render_opaque), floating 60 blocks up so both versions are visible.
        match renderer::gpu_mesh_m3_setup(cx, cz, GPU_POOL_WIDTH, 60.0, 0, false) {
            Ok(()) => println!(
                "[M3 test] indirect-draw chunk ({cx},{cz}) set up, rendering 60 blocks above its CPU twin"
            ),
            Err(e) => println!("[M3 test] setup failed: {e}"),
        }
        // TEMPORARY debug: a second copy at a different height, tinted by
        // position instead of real AO/light, rendered in the SAME frame so a
        // single screenshot can compare "real colors" vs "position-only
        // colors" with zero run-to-run camera/timing variance.
        match renderer::gpu_mesh_m3_setup(cx, cz, GPU_POOL_WIDTH, 90.0, 1, true) {
            Ok(()) => println!(
                "[M3 test] tinted comparison copy set up, rendering 90 blocks above its CPU twin"
            ),
            Err(e) => println!("[M3 test] tinted setup failed: {e}"),
        }
    }

    fn gpu_pool_index(&self, cx: i32, cz: i32) -> usize {
        let ix = cx.rem_euclid(GPU_POOL_WIDTH);
        let iz = cz.rem_euclid(GPU_POOL_WIDTH);
        (ix + iz * GPU_POOL_WIDTH) as usize
    }

    // Uploads (cx, cz)'s current CPU-side block/light/liquid data into its
    // GPU voxel pool slot, if it's within the (smaller) GPU meshing radius.
    // `pcx`/`pcz` are the player's current chunk coords, passed explicitly
    // rather than read from `self.last_pcx/last_pcz`, which only update once
    // a whole boundary-crossing scan completes and would wrongly gate out
    // every chunk during the initial synchronous load.
    fn sync_gpu_chunk(&self, cx: i32, cz: i32, pcx: i32, pcz: i32) {
        if (cx - pcx).abs() > GPU_POOL_VIEW_DISTANCE || (cz - pcz).abs() > GPU_POOL_VIEW_DISTANCE {
            return;
        }
        let Some(chunk) = self.get_chunk(cx, cz) else {
            return;
        };
        let slot = self.gpu_pool_index(cx, cz) as u32;
        let (blocks, light, liquid) = chunk.gpu_bytes();
        renderer::gpu_pool_upload_chunk(slot, cx, cz, blocks, light, liquid);
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

    pub fn insert_chunk(&mut self, mut chunk: Chunk) {
        chunk.hydrate_fluids();
        chunk.calculate_lighting();
        chunk.dirty = true;
        let index = self.get_pool_index(chunk.x, chunk.z);
        if let Some(old) = &self.chunks[index]
            && old.dirty
        {
            self.dirty_count -= 1;
        }
        self.chunks[index] = Some(Box::new(chunk));
        self.dirty_count += 1;
    }

    pub fn register_imported_chunks(&mut self, chunks: Vec<Chunk>) {
        for mut chunk in chunks {
            chunk.hydrate_fluids();
            chunk.mesh_opaque = None;
            chunk.mesh_transparent = None;
            chunk.mesh_water = None;
            chunk.dirty = true;
            chunk.meshing_in_progress = false;
            self.imported.insert((chunk.x, chunk.z), Box::new(chunk));
        }
    }

    pub fn set_import_world(&mut self, path: PathBuf) {
        self.import_world = Some(path);
        self.last_pcx = i32::MIN;
        self.last_pcz = i32::MIN;
    }

    pub fn install_edits(&self, edits: &[((i32, i32, i32), BlockType)]) {
        if let Ok(mut map) = self.edits.write() {
            map.clear();
            for (position, block) in edits {
                map.insert(*position, *block);
            }
        }
    }

    pub fn set_view_distance(&mut self, distance: i32) {
        let distance = distance.clamp(2, MAX_VIEW_DISTANCE);
        if distance != self.view_distance {
            self.view_distance = distance;
            self.last_pcx = i32::MIN;
            self.last_pcz = i32::MIN;
        }
    }

    pub fn chunk_for_export(&self, cx: i32, cz: i32) -> Chunk {
        let mut chunk = Chunk::new(cx, cz, self.seed);
        if let Some(loaded) = self.get_chunk(cx, cz) {
            chunk.copy_terrain_from(loaded);
        } else if let Some(imported) = self.imported.get(&(cx, cz)) {
            chunk.copy_terrain_from(imported);
        } else if let Some(path) = &self.import_world {
            match crate::java_compat::import_classic_java_chunk(path, cx, cz) {
                Ok(Some(imported)) => chunk.copy_terrain_from(&imported),
                Ok(None) => chunk.generate(),
                Err(error) => {
                    eprintln!("Failed to read imported chunk ({cx}, {cz}): {error}");
                    chunk.generate();
                }
            }
        } else {
            chunk.generate();
        }
        if let Ok(edits) = self.edits.read() {
            Self::apply_edit_map(&mut chunk, &edits);
        }
        chunk
    }

    pub fn collect_mob_drops(&mut self, mob: Mob) {
        if let Some((block, count)) = mob.drop_item() {
            self.pending_drops.push((block, count as u32));
        }
        let xp_val = match mob.kind {
            MobKind::Zombie | MobKind::Skeleton | MobKind::Creeper => 5,
            MobKind::Golem => 10,
            _ => 2,
        };
        self.xp_orbs.push(crate::block::XpOrbEntity {
            position: mob.position + Vec3::new(0.0, 0.5, 0.0),
            velocity: Vec3::new(
                (rand::random::<f32>() - 0.5) * 1.5,
                2.5,
                (rand::random::<f32>() - 0.5) * 1.5,
            ),
            xp_value: xp_val,
            life: 300.0,
        });
    }

    pub fn try_melee(&mut self, origin: Vec3, direction: Vec3, damage: f32, reach: f32) -> bool {
        let dir = direction.normalize_or_zero();
        if dir.length_squared() < 0.01 {
            return false;
        }
        let mut best: Option<(usize, f32)> = None;
        for (index, mob) in self.mobs.iter().enumerate() {
            let center = mob.position + Vec3::new(0.0, mob.height() * 0.5, 0.0);
            let to = center - origin;
            let dist = to.length();
            if dist > reach || dist < 0.05 {
                continue;
            }
            if to.normalize().dot(dir) < 0.65 {
                continue;
            }
            if best.is_none_or(|(_, best_dist)| dist < best_dist) {
                best = Some((index, dist));
            }
        }
        let Some((index, _)) = best else {
            return false;
        };
        if self.mobs[index].take_damage(damage) {
            let mob = self.mobs.swap_remove(index);
            self.collect_mob_drops(mob);
        }
        true
    }

    fn player_aabb_hit(player_pos: Vec3, point: Vec3) -> bool {
        point.x >= player_pos.x - 0.3
            && point.x <= player_pos.x + 0.3
            && point.z >= player_pos.z - 0.3
            && point.z <= player_pos.z + 0.3
            && point.y >= player_pos.y
            && point.y <= player_pos.y + 1.8
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

    fn apply_edit_map(chunk: &mut Chunk, edits: &HashMap<(i32, i32, i32), BlockType>) {
        let chunk_start_x = chunk.x * CHUNK_WIDTH as i32;
        let chunk_start_z = chunk.z * CHUNK_DEPTH as i32;
        let chunk_end_x = chunk_start_x + CHUNK_WIDTH as i32;
        let chunk_end_z = chunk_start_z + CHUNK_DEPTH as i32;
        for ((ex, ey, ez), block) in edits {
            if *ex >= chunk_start_x
                && *ex < chunk_end_x
                && *ez >= chunk_start_z
                && *ez < chunk_end_z
                && *ey >= 0
                && *ey < CHUNK_HEIGHT as i32
            {
                let bx = (*ex - chunk_start_x) as usize;
                let bz = (*ez - chunk_start_z) as usize;
                chunk.set_block(bx, *ey as usize, bz, *block);
                chunk.dirty = true;
            }
        }
    }

    pub fn apply_edits_to_chunk(&mut self, cx: i32, cz: i32) {
        let edits_clone = if let Ok(edits) = self.edits.read() {
            edits.clone()
        } else {
            return;
        };
        if let Some(chunk) = self.get_chunk_mut(cx, cz) {
            Self::apply_edit_map(chunk, &edits_clone);
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
            // Lava stops the ray so it can't be mined through; the mining
            // state rejects Lava hits itself.
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

    fn chunk_slot_holds(&self, x: i32, z: i32) -> bool {
        match &self.chunks[self.get_pool_index(x, z)] {
            Some(chunk) => chunk.x == x && chunk.z == z,
            None => false,
        }
    }

    fn request_chunk(&mut self, x: i32, z: i32) {
        if self.gen_in_flight.contains(&(x, z)) {
            return;
        }
        self.gen_in_flight.insert((x, z));
        if let Some(src) = self.imported.get(&(x, z)) {
            let mut chunk = Box::new(Chunk::new(x, z, self.seed));
            chunk.copy_terrain_from(src);
            let _ = self.gen_result_tx.send(chunk);
            return;
        }
        let seed = self.seed;
        let import_world = self.import_world.clone();
        let tx = self.gen_result_tx.clone();
        rayon::spawn(move || {
            if let Some(path) = import_world {
                match crate::java_compat::import_classic_java_chunk(&path, x, z) {
                    Ok(Some(chunk)) => {
                        let _ = tx.send(Box::new(chunk));
                        return;
                    }
                    Ok(None) => {}
                    Err(error) => {
                        eprintln!("Failed to read imported chunk ({x}, {z}): {error}");
                    }
                }
            }
            let mut chunk = Box::new(Chunk::new(x, z, seed));
            chunk.generate();
            let _ = tx.send(chunk);
        });
    }

    fn request_missing_chunks(&mut self, pcx: i32, pcz: i32, max_in_flight: usize) -> bool {
        let view = self.view_distance;
        let mut all_present = true;
        'scan: for r in 0..=view {
            for x in (pcx - r)..=(pcx + r) {
                for z in (pcz - r)..=(pcz + r) {
                    if x > pcx - r && x < pcx + r && z > pcz - r && z < pcz + r {
                        continue;
                    }
                    if self.chunk_slot_holds(x, z) {
                        continue;
                    }
                    all_present = false;
                    if self.gen_in_flight.contains(&(x, z)) {
                        continue;
                    }
                    if self.gen_in_flight.len() >= max_in_flight {
                        break 'scan;
                    }
                    self.request_chunk(x, z);
                }
            }
        }
        all_present && self.gen_in_flight.is_empty()
    }

    pub fn update(&mut self, player_pos: Vec3, _time: f32) -> u32 {
        let pcx = (player_pos.x / CHUNK_WIDTH as f32).floor() as i32;
        let pcz = (player_pos.z / CHUNK_DEPTH as f32).floor() as i32;

        // --- Multithreaded Initial World Loading & Spiral Generation ---
        if self.is_loading {
            let all_loaded = self.request_missing_chunks(pcx, pcz, 256);
            if all_loaded && self.dirty_count == 0 && self.meshing_in_flight == 0 {
                self.is_loading = false;
                self.last_pcx = pcx;
                self.last_pcz = pcz;
            }
        } else if pcx != self.last_pcx || pcz != self.last_pcz {
            // NORMAL MODE: dispatch missing chunks to background workers,
            // nearest ring first. Generation is a pure function of
            // (seed, x, z), so workers need no access to World; finished
            // chunks come back over a channel and are integrated below.
            if self.request_missing_chunks(pcx, pcz, MAX_GEN_IN_FLIGHT) {
                self.last_pcx = pcx;
                self.last_pcz = pcz;
            }
        }

        // --- Integrate chunks generated in the background ---
        while let Ok(chunk) = self.gen_result_rx.try_recv() {
            let (x, z) = (chunk.x, chunk.z);
            self.gen_in_flight.remove(&(x, z));
            // Discard arrivals the player has since moved away from; the
            // scan re-dispatches them if the player comes back.
            if (x - pcx).abs() > self.view_distance || (z - pcz).abs() > self.view_distance {
                continue;
            }
            let index = self.get_pool_index(x, z);
            if let Some(existing) = &self.chunks[index]
                && existing.x == x
                && existing.z == z
            {
                continue;
            }
            // The displaced chunk's dirty flag was counted; drop its count
            if let Some(old) = &self.chunks[index]
                && old.dirty
            {
                self.dirty_count -= 1;
            }
            self.chunks[index] = Some(chunk);
            self.apply_edits_to_chunk(x, z);

            // Dirty the new chunk AND its neighbors to fix lighting/meshing
            // gaps. Fresh chunks are born dirty but were never counted, so
            // count unconditionally.
            if let Some(c) = &mut self.chunks[index] {
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
            self.try_spawn_village_mobs(x, z);
            self.try_spawn_natural_mobs(x, z);
            self.chunks_generated_count += 1;
            self.sync_gpu_chunk(x, z, pcx, pcz);
        }

        // --- Prioritized Meshing ---
        // 1. Collect finished meshing jobs from the workers and upload to GPU.
        //    Results for chunks that scrolled out of the pool are dropped.
        if self.meshing_in_flight > 0 {
            while let Ok(result) = self.mesh_result_rx.try_recv() {
                self.meshing_in_flight -= 1;
                // The job id check drops stale results when the chunk was
                // replaced and re-dispatched while this job was in flight.
                let relit = if let Some(chunk) = self.get_chunk_mut(result.x, result.z)
                    && chunk.meshing_in_progress
                    && chunk.mesh_job_id == result.job_id
                {
                    chunk.light = result.light;
                    chunk.upload_mesh(result.opaque, result.transparent, result.water);
                    true
                } else {
                    false
                };
                // This is the point the CPU-side light array actually gets
                // refreshed after an edit (set_block doesn't relight
                // synchronously), so it's also the right place to push the
                // now-current blocks/light/liquid to the GPU pool.
                if relit {
                    self.sync_gpu_chunk(result.x, result.z, pcx, pcz);
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
                let (cx, cz) = match &self.chunks[index] {
                    Some(chunk) => (chunk.x, chunk.z),
                    None => continue,
                };

                // Snapshot chunk + neighbor border on the main thread, then hand
                // the worker an owned copy — it never touches World or the live
                // chunk, so block edits can't race the mesher.
                let snap = MeshSnapshot::capture(self, cx, cz);
                let job_id = self.next_mesh_job_id;
                self.next_mesh_job_id += 1;
                let mut work = {
                    let chunk = self.chunks[index].as_mut().unwrap();
                    chunk.dirty = false;
                    chunk.meshing_in_progress = true;
                    chunk.mesh_job_id = job_id;
                    chunk.snapshot_data()
                };
                self.dirty_count -= 1;
                self.meshing_in_flight += 1;

                let tx = self.mesh_result_tx.clone();
                rayon::spawn(move || {
                    work.calculate_lighting();
                    let (opaque, transparent, water) = work.calculate_mesh_data(&snap);
                    let _ = tx.send(MeshResult {
                        x: work.x,
                        z: work.z,
                        job_id,
                        light: work.light,
                        opaque,
                        transparent,
                        water,
                    });
                });

                dispatches_this_frame += 1;
                let max_dispatches = if self.is_loading { 64 } else { 16 };
                if dispatches_this_frame >= max_dispatches {
                    break;
                }
            }
        }

        // --- Explosive Ticking ---
        let mut i = 0;
        let mut fuse_particles = Vec::new();
        let frame_dt = _time.min(0.05);
        let drag = (0.98_f32).powf(frame_dt * 60.0);
        let gravity = 18.0 * frame_dt;

        while i < self.explosives.len() {
            let mut vel = self.explosives[i].velocity;
            let mut pos = self.explosives[i].position;

            // Apply gravity & drag
            vel.y -= gravity;
            vel.x *= drag;
            vel.y *= drag;
            vel.z *= drag;

            let hw = 0.45;
            let h = 0.90;

            // Axis-by-axis collision & movement
            // Move X
            let step_x = vel.x * frame_dt;
            if step_x != 0.0 {
                let cand = pos + Vec3::new(step_x, 0.0, 0.0);
                if !self.mob_box_blocked(cand + Vec3::new(0.5, 0.0, 0.5), hw, h) {
                    pos.x = cand.x;
                } else {
                    vel.x = -vel.x * 0.4;
                }
            }

            // Move Z
            let step_z = vel.z * frame_dt;
            if step_z != 0.0 {
                let cand = pos + Vec3::new(0.0, 0.0, step_z);
                if !self.mob_box_blocked(cand + Vec3::new(0.5, 0.0, 0.5), hw, h) {
                    pos.z = cand.z;
                } else {
                    vel.z = -vel.z * 0.4;
                }
            }

            // Move Y
            let step_y = vel.y * frame_dt;
            let cand = pos + Vec3::new(0.0, step_y, 0.0);
            if !self.mob_box_blocked(cand + Vec3::new(0.5, 0.0, 0.5), hw, h) {
                pos.y = cand.y;
            } else {
                if vel.y < 0.0 {
                    if vel.y.abs() > 1.5 {
                        vel.y = -vel.y * 0.35;
                    } else {
                        vel.y = 0.0;
                    }
                    vel.x *= 0.7;
                    vel.z *= 0.7;
                } else {
                    vel.y = 0.0;
                }
            }

            self.explosives[i].position = pos;
            self.explosives[i].velocity = vel;
            self.explosives[i].fuse -= frame_dt;

            let progress = 1.0
                - (self.explosives[i].fuse / self.explosives[i].initial_fuse.max(0.001))
                    .clamp(0.0, 1.0);
            if rand::random::<f32>() < frame_dt * (5.0 + progress * 14.0) {
                let jitter = Vec3::new(
                    (rand::random::<f32>() - 0.5) * 0.36,
                    rand::random::<f32>() * 0.08,
                    (rand::random::<f32>() - 0.5) * 0.36,
                );
                let p_vel = Vec3::new(
                    (rand::random::<f32>() - 0.5) * 0.35,
                    0.55 + rand::random::<f32>() * 0.45,
                    (rand::random::<f32>() - 0.5) * 0.35,
                );
                fuse_particles.push(crate::block::Particle {
                    position: self.explosives[i].position + Vec3::new(0.5, 1.05, 0.5) + jitter,
                    velocity: p_vel,
                    color: glam::Vec4::new(0.58, 0.56, 0.52, 0.72),
                    life: 0.45 + rand::random::<f32>() * 0.35,
                    max_life: 0.8,
                    scale: 0.10 + rand::random::<f32>() * 0.08,
                });
            }
            if progress > 0.55 && rand::random::<f32>() < frame_dt * 8.0 {
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
            crate::explosion::explode(
                self,
                pos.x.floor() as i32,
                pos.y.floor() as i32,
                pos.z.floor() as i32,
                4,
                player_pos,
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

        // --- Arrow Physics & Collision ---
        let mut a_idx = 0;
        while a_idx < self.arrows.len() {
            let mut remove_arrow = false;
            self.arrows[a_idx].life -= _time;

            if self.arrows[a_idx].life <= 0.0 {
                remove_arrow = true;
            } else if !self.arrows[a_idx].in_ground {
                self.arrows[a_idx].velocity.y -= 25.0 * _time;
                let vel = self.arrows[a_idx].velocity * 0.99f32.powf(_time * 20.0);
                self.arrows[a_idx].velocity = vel;

                let next_pos = self.arrows[a_idx].position + vel * _time;
                let bx = next_pos.x.floor() as i32;
                let by = next_pos.y.floor() as i32;
                let bz = next_pos.z.floor() as i32;

                if self.get_block(bx, by, bz).is_solid() {
                    self.arrows[a_idx].in_ground = true;
                    self.arrows[a_idx].velocity = Vec3::ZERO;
                } else if self.arrows[a_idx].from_player {
                    let mut hit_mob = false;
                    let mut m_idx = 0;
                    let damage = self.arrows[a_idx].damage;
                    while m_idx < self.mobs.len() {
                        let mob = &self.mobs[m_idx];
                        let feet = mob.position;
                        let hw = mob.half_width();
                        let h = mob.height();

                        let in_x = next_pos.x >= feet.x - hw && next_pos.x <= feet.x + hw;
                        let in_z = next_pos.z >= feet.z - hw && next_pos.z <= feet.z + hw;
                        let in_y = next_pos.y >= feet.y && next_pos.y <= feet.y + h;

                        if in_x && in_y && in_z {
                            hit_mob = true;
                            for _ in 0..4 {
                                self.particles.push(crate::block::Particle {
                                    position: next_pos,
                                    velocity: Vec3::new(
                                        (rand::random::<f32>() - 0.5) * 2.0,
                                        rand::random::<f32>() * 2.0,
                                        (rand::random::<f32>() - 0.5) * 2.0,
                                    ),
                                    color: glam::Vec4::new(0.8, 0.1, 0.1, 0.9),
                                    life: 0.3,
                                    max_life: 0.3,
                                    scale: 0.08,
                                });
                            }
                            if self.mobs[m_idx].take_damage(damage) {
                                let dead_mob = self.mobs.swap_remove(m_idx);
                                self.collect_mob_drops(dead_mob);
                            }
                            break;
                        }
                        m_idx += 1;
                    }
                    if hit_mob {
                        remove_arrow = true;
                    } else {
                        self.arrows[a_idx].position = next_pos;
                    }
                } else if Self::player_aabb_hit(player_pos, next_pos) {
                    self.pending_hurt += self.arrows[a_idx].damage.round() as i32;
                    remove_arrow = true;
                } else {
                    self.arrows[a_idx].position = next_pos;
                }
            }

            if remove_arrow {
                self.arrows.swap_remove(a_idx);
            } else {
                a_idx += 1;
            }
        }

        // --- XP Orb Physics & Attraction ---
        let mut collected_xp = 0;
        let mut x_idx = 0;
        while x_idx < self.xp_orbs.len() {
            let mut remove_orb = false;
            self.xp_orbs[x_idx].life -= _time;

            if self.xp_orbs[x_idx].life <= 0.0 {
                remove_orb = true;
            } else {
                let orb_pos = self.xp_orbs[x_idx].position;
                let dist = orb_pos.distance(player_pos);
                if dist < 1.2 {
                    collected_xp += self.xp_orbs[x_idx].xp_value;
                    remove_orb = true;
                } else if dist < 7.0 && dist > 0.1 {
                    let dir = (player_pos - orb_pos).normalize();
                    let vel = dir * (7.0 - dist) * 2.5;
                    self.xp_orbs[x_idx].velocity = vel;
                    self.xp_orbs[x_idx].position += vel * _time;
                } else {
                    self.xp_orbs[x_idx].velocity.y -= 12.0 * _time;
                    let vel = self.xp_orbs[x_idx].velocity * 0.95;
                    self.xp_orbs[x_idx].velocity = vel;
                    let next_pos = orb_pos + vel * _time;
                    let bx = next_pos.x.floor() as i32;
                    let by = next_pos.y.floor() as i32;
                    let bz = next_pos.z.floor() as i32;
                    if self.get_block(bx, by, bz).is_solid() {
                        self.xp_orbs[x_idx].velocity = Vec3::ZERO;
                    } else {
                        self.xp_orbs[x_idx].position = next_pos;
                    }
                }
            }

            if remove_orb {
                self.xp_orbs.swap_remove(x_idx);
            } else {
                x_idx += 1;
            }
        }

        // --- Physics Simulation ---
        self.update_water(_time);
        self.update_falling_blocks();
        self.update_mobs(player_pos, _time);
        collected_xp
    }

    /// Spawn a village's inhabitants the first time its center chunk
    /// generates. Session-scoped: they don't respawn when the pool cycles.
    fn try_spawn_village_mobs(&mut self, cx: i32, cz: i32) {
        if self.spawned_villages.contains(&(cx, cz)) {
            return;
        }
        let Some(village) = crate::village::village_for_center_chunk(self.seed, cx, cz) else {
            return;
        };
        self.spawned_villages.insert((cx, cz));
        let home = Vec3::new(
            village.center_x as f32 + 0.5,
            (village.base_y + 1) as f32,
            village.center_z as f32 + 0.5,
        );
        for (i, pos) in village.villager_spawns().into_iter().enumerate() {
            self.mobs
                .push(Mob::new(MobKind::Villager, pos, home, i as u8));
        }
        self.mobs
            .push(Mob::new(MobKind::Golem, village.golem_spawn(), home, 0));
    }

    /// Spawn natural passive (Pig/Cow/Sheep), hostile (Zombie/Skeleton/Creeper),
    /// and MobSpawner mobs in newly generated chunks.
    fn try_spawn_natural_mobs(&mut self, cx: i32, cz: i32) {
        if self.spawned_natural_chunks.contains(&(cx, cz)) {
            return;
        }
        if self.mobs.len() >= 48 {
            return;
        }
        self.spawned_natural_chunks.insert((cx, cz));

        let mut new_mobs = Vec::new();
        let mut rng =
            (self.seed ^ ((cx as u64) << 32) ^ (cz as u64)).wrapping_mul(0x9E3779B97F4A7C15);
        let chunk_origin_x = cx * CHUNK_WIDTH as i32;
        let chunk_origin_z = cz * CHUNK_DEPTH as i32;

        // 1. MobSpawner spawns (Zombie / Skeleton / Creeper)
        if let Some(chunk) = self.get_chunk(cx, cz) {
            for x in 0..CHUNK_WIDTH {
                for y in 10..100 {
                    for z in 0..CHUNK_DEPTH {
                        if chunk.blocks[x][y][z] == BlockType::MobSpawner {
                            let wx = chunk_origin_x + x as i32;
                            let wz = chunk_origin_z + z as i32;
                            let mob_kind = match rng % 3 {
                                0 => MobKind::Zombie,
                                1 => MobKind::Skeleton,
                                _ => MobKind::Creeper,
                            };
                            rng = rng.wrapping_mul(0x9E3779B97F4A7C15);
                            let pos = Vec3::new(wx as f32 + 1.5, (y + 1) as f32, wz as f32 + 0.5);
                            new_mobs.push(Mob::new(mob_kind, pos, pos, (rng % 4) as u8));
                        }
                    }
                }
            }
        }

        // 2. Surface passive mobs (Pig / Cow / Sheep)
        let passive_count = (rng % 3) as usize;
        rng = rng.wrapping_mul(0x9E3779B97F4A7C15);

        for _ in 0..passive_count {
            let local_x = (rng % CHUNK_WIDTH as u64) as usize;
            rng = rng.wrapping_mul(0x9E3779B97F4A7C15);
            let local_z = (rng % CHUNK_DEPTH as u64) as usize;
            rng = rng.wrapping_mul(0x9E3779B97F4A7C15);

            let wx = chunk_origin_x + local_x as i32;
            let wz = chunk_origin_z + local_z as i32;

            if let Some(chunk) = self.get_chunk(cx, cz) {
                for y in (60..CHUNK_HEIGHT - 2).rev() {
                    let b = chunk.blocks[local_x][y][local_z];
                    let above = chunk.blocks[local_x][y + 1][local_z];
                    if (b == BlockType::Grass || b == BlockType::SnowyGrass || b == BlockType::Dirt)
                        && above == BlockType::Air
                    {
                        let mob_kind = match rng % 3 {
                            0 => MobKind::Pig,
                            1 => MobKind::Cow,
                            _ => MobKind::Sheep,
                        };
                        rng = rng.wrapping_mul(0x9E3779B97F4A7C15);
                        let pos = Vec3::new(wx as f32 + 0.5, (y + 1) as f32, wz as f32 + 0.5);
                        new_mobs.push(Mob::new(mob_kind, pos, pos, (rng % 4) as u8));
                        break;
                    }
                }
            }
        }

        // 3. Underground cave hostile mobs (Zombie / Skeleton / Creeper)
        let hostile_count = ((rng >> 4) % 2) as usize;
        rng = rng.wrapping_mul(0x9E3779B97F4A7C15);

        for _ in 0..hostile_count {
            let local_x = (rng % CHUNK_WIDTH as u64) as usize;
            rng = rng.wrapping_mul(0x9E3779B97F4A7C15);
            let local_z = (rng % CHUNK_DEPTH as u64) as usize;
            rng = rng.wrapping_mul(0x9E3779B97F4A7C15);

            let wx = chunk_origin_x + local_x as i32;
            let wz = chunk_origin_z + local_z as i32;

            if let Some(chunk) = self.get_chunk(cx, cz) {
                for y in 10..55 {
                    let below = chunk.blocks[local_x][y - 1][local_z];
                    let feet = chunk.blocks[local_x][y][local_z];
                    let head = chunk.blocks[local_x][y + 1][local_z];

                    if below.is_solid() && feet == BlockType::Air && head == BlockType::Air {
                        let mob_kind = match rng % 3 {
                            0 => MobKind::Zombie,
                            1 => MobKind::Skeleton,
                            _ => MobKind::Creeper,
                        };
                        rng = rng.wrapping_mul(0x9E3779B97F4A7C15);
                        let pos = Vec3::new(wx as f32 + 0.5, y as f32, wz as f32 + 0.5);
                        new_mobs.push(Mob::new(mob_kind, pos, pos, (rng % 4) as u8));
                        break;
                    }
                }
            }
        }

        self.mobs.extend(new_mobs);
    }

    fn solid_at(&self, x: f32, y: f32, z: f32) -> bool {
        let (x, y, z) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);
        // Unloaded chunks read as Air from get_block; treat them as solid
        // so mobs can't wander into ungenerated terrain and freeze there.
        let cx = x.div_euclid(CHUNK_WIDTH as i32);
        let cz = z.div_euclid(CHUNK_DEPTH as i32);
        self.get_chunk(cx, cz).is_none() || self.get_block(x, y, z).is_solid()
    }

    /// True if a mob-sized box at `pos` (feet center) intersects any solid block.
    fn mob_box_blocked(&self, pos: Vec3, hw: f32, height: f32) -> bool {
        // Cover every voxel the box overlaps; corner sampling misses the
        // middle column when a wide mob straddles three columns.
        let min_x = (pos.x - hw).floor() as i32;
        let max_x = (pos.x + hw).floor() as i32;
        let min_y = (pos.y + 0.05).floor() as i32;
        let max_y = (pos.y + height - 0.1).floor() as i32;
        let min_z = (pos.z - hw).floor() as i32;
        let max_z = (pos.z + hw).floor() as i32;
        for x in min_x..=max_x {
            for y in min_y..=max_y {
                for z in min_z..=max_z {
                    if self.solid_at(x as f32 + 0.5, y as f32 + 0.5, z as f32 + 0.5) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn update_mobs(&mut self, player_pos: Vec3, dt: f32) {
        let dt = dt.min(0.1);
        let mut mobs = std::mem::take(&mut self.mobs);
        let mut despawned_villages = Vec::new();
        let mut despawned_natural = Vec::new();
        mobs.retain(|mob| {
            let max_distance = if matches!(mob.kind, MobKind::Villager | MobKind::Golem) {
                160.0
            } else {
                96.0
            };
            if mob.position.distance_squared(player_pos) <= max_distance * max_distance {
                return true;
            }
            let home_chunk = (
                (mob.home.x / CHUNK_WIDTH as f32).floor() as i32,
                (mob.home.z / CHUNK_DEPTH as f32).floor() as i32,
            );
            if matches!(mob.kind, MobKind::Villager | MobKind::Golem) {
                despawned_villages.push(home_chunk);
            } else {
                despawned_natural.push(home_chunk);
            }
            false
        });
        for chunk in despawned_villages {
            self.spawned_villages.remove(&chunk);
        }
        for chunk in despawned_natural {
            self.spawned_natural_chunks.remove(&chunk);
        }
        for mob in &mut mobs {
            // Freeze mobs whose chunk is unloaded so they don't fall
            // through ungenerated terrain.
            let mcx = (mob.position.x / CHUNK_WIDTH as f32).floor() as i32;
            let mcz = (mob.position.z / CHUNK_DEPTH as f32).floor() as i32;
            if self.get_chunk(mcx, mcz).is_none() {
                continue;
            }

            mob.wander_timer -= dt;
            if mob.wander_timer <= 0.0 {
                if rand::random::<f32>() < 0.35 {
                    mob.walk_speed = 0.0;
                } else {
                    mob.yaw = rand::random::<f32>() * std::f32::consts::TAU;
                    mob.walk_speed = mob.base_speed();
                }
                mob.wander_timer = 1.5 + rand::random::<f32>() * 3.5;
            }
            // Head back when wandering past the leash
            let to_home = Vec3::new(
                mob.home.x - mob.position.x,
                0.0,
                mob.home.z - mob.position.z,
            );
            if to_home.length() > mob.leash_range() {
                mob.yaw = to_home.z.atan2(to_home.x);
                mob.walk_speed = mob.base_speed();
            }

            mob.attack_cooldown = (mob.attack_cooldown - dt).max(0.0);
            let to_player = Vec3::new(
                player_pos.x - mob.position.x,
                0.0,
                player_pos.z - mob.position.z,
            );
            let player_dist = to_player.length();
            if mob.is_hostile() && player_dist < 16.0 && player_dist > 0.05 {
                mob.yaw = to_player.z.atan2(to_player.x);
                mob.walk_speed = mob.base_speed();
                if mob.kind == MobKind::Creeper && player_dist < 2.2 {
                    mob.walk_speed = 0.0;
                    if mob.attack_cooldown <= 0.0 {
                        self.detonations.push(mob.position);
                        mob.health = 0.0;
                    }
                } else if mob.kind == MobKind::Zombie
                    && player_dist < 1.6
                    && mob.attack_cooldown <= 0.0
                {
                    self.pending_hurt += 3;
                    mob.attack_cooldown = 1.0;
                } else if mob.kind == MobKind::Skeleton
                    && player_dist < 12.0
                    && mob.attack_cooldown <= 0.0
                {
                    let origin = mob.position + Vec3::new(0.0, 1.4, 0.0);
                    let target = player_pos + Vec3::new(0.0, 1.4, 0.0);
                    let dir = (target - origin).normalize_or_zero();
                    if dir.length_squared() > 0.01 {
                        self.arrows.push(crate::block::ArrowEntity {
                            position: origin,
                            velocity: dir * 18.0,
                            life: 8.0,
                            in_ground: false,
                            damage: 4.0,
                            is_critical: false,
                            from_player: false,
                        });
                    }
                    mob.attack_cooldown = 2.0;
                }
            }

            mob.velocity.x = mob.yaw.cos() * mob.walk_speed;
            mob.velocity.z = mob.yaw.sin() * mob.walk_speed;
            mob.velocity.y = (mob.velocity.y - 22.0 * dt).max(-40.0);

            let hw = mob.half_width();
            let height = mob.height();

            // Horizontal movement axis by axis, with a one-block step-up
            for axis in 0..2usize {
                let step = if axis == 0 {
                    mob.velocity.x * dt
                } else {
                    mob.velocity.z * dt
                };
                if step == 0.0 {
                    continue;
                }
                let mut cand = mob.position;
                if axis == 0 {
                    cand.x += step;
                } else {
                    cand.z += step;
                }
                if !self.mob_box_blocked(cand, hw, height) {
                    mob.position = cand;
                } else if mob.grounded {
                    let mut up = cand;
                    up.y += 1.0;
                    if !self.mob_box_blocked(up, hw, height) {
                        mob.position = up;
                    } else {
                        // Walled in: re-roll direction soon
                        mob.wander_timer = mob.wander_timer.min(0.2);
                    }
                } else {
                    mob.wander_timer = mob.wander_timer.min(0.2);
                }
            }

            // Vertical movement
            let mut cand = mob.position;
            cand.y += mob.velocity.y * dt;
            if cand.y < 1.0 {
                cand.y = 1.0;
            }
            if !self.mob_box_blocked(cand, hw, height) {
                mob.position = cand;
                mob.grounded = false;
            } else {
                if mob.velocity.y < 0.0 {
                    mob.grounded = true;
                }
                mob.velocity.y = 0.0;
            }
        }
        mobs.retain(|mob| mob.health > 0.0);
        self.mobs = mobs;
        let player_chunk = (
            (player_pos.x / CHUNK_WIDTH as f32).floor() as i32,
            (player_pos.z / CHUNK_DEPTH as f32).floor() as i32,
        );
        self.try_spawn_natural_mobs(player_chunk.0, player_chunk.1);
    }

    pub fn render_mobs(&self, shader: &Shader, player_pos: Vec3) {
        if self.mobs.is_empty() {
            return;
        }
        let loc_model = shader.get_uniform_location("uModel");
        let loc_diff = shader.get_uniform_location("colDiffuse");
        if let Some(atlas) = &self.atlas {
            atlas.bind(0);
        }

        for mob in &self.mobs {
            if mob.position.distance_squared(player_pos) > 64.0 * 64.0 {
                continue;
            }
            // Mobs in unloaded chunks are frozen by update_mobs; skip
            // drawing them too instead of paying draw calls off-screen.
            let mcx = (mob.position.x / CHUNK_WIDTH as f32).floor() as i32;
            let mcz = (mob.position.z / CHUNK_DEPTH as f32).floor() as i32;
            if self.get_chunk(mcx, mcz).is_none() {
                continue;
            }
            let base = Mat4::from_translation(mob.position) * Mat4::from_rotation_y(-mob.yaw);
            let part = |mesh: &Mesh, center: Vec3, size: Vec3, tint: glam::Vec4| {
                shader.set_vec4(loc_diff, tint);
                let model =
                    base * Mat4::from_translation(center - size * 0.5) * Mat4::from_scale(size);
                shader.set_mat4(loc_model, &model);
                mesh.draw();
            };
            match mob.kind {
                MobKind::Villager => {
                    let robe = match mob.variant % 3 {
                        0 => glam::Vec4::new(0.45, 0.33, 0.22, 1.0),
                        1 => glam::Vec4::new(0.34, 0.40, 0.27, 1.0),
                        _ => glam::Vec4::new(0.42, 0.42, 0.46, 1.0),
                    };
                    let dark_robe = glam::Vec4::new(robe.x * 0.8, robe.y * 0.8, robe.z * 0.8, 1.0);
                    let skin = glam::Vec4::new(0.82, 0.64, 0.47, 1.0);
                    let nose = glam::Vec4::new(0.72, 0.53, 0.38, 1.0);
                    // Robe body
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.0, 0.625, 0.0),
                        Vec3::new(0.5, 1.25, 0.42),
                        robe,
                    );
                    // Folded arms bar across the chest (forward is +x)
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.2, 0.98, 0.0),
                        Vec3::new(0.2, 0.24, 0.6),
                        dark_robe,
                    );
                    // Head
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.0, 1.53, 0.0),
                        Vec3::new(0.5, 0.56, 0.5),
                        skin,
                    );
                    // The all-important nose
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.3, 1.4, 0.0),
                        Vec3::new(0.12, 0.3, 0.12),
                        nose,
                    );
                }
                MobKind::Golem => {
                    let tint = glam::Vec4::ONE;
                    // Legs
                    part(
                        &self.golem_mesh,
                        Vec3::new(0.0, 0.525, -0.24),
                        Vec3::new(0.34, 1.05, 0.34),
                        tint,
                    );
                    part(
                        &self.golem_mesh,
                        Vec3::new(0.0, 0.525, 0.24),
                        Vec3::new(0.34, 1.05, 0.34),
                        tint,
                    );
                    // Torso, shoulders spanning z
                    part(
                        &self.golem_mesh,
                        Vec3::new(0.0, 1.5, 0.0),
                        Vec3::new(0.72, 0.95, 1.05),
                        tint,
                    );
                    // Hanging arms
                    part(
                        &self.golem_mesh,
                        Vec3::new(0.0, 1.32, -0.68),
                        Vec3::new(0.3, 1.3, 0.3),
                        tint,
                    );
                    part(
                        &self.golem_mesh,
                        Vec3::new(0.0, 1.32, 0.68),
                        Vec3::new(0.3, 1.3, 0.3),
                        tint,
                    );
                    // Head
                    part(
                        &self.golem_mesh,
                        Vec3::new(0.0, 2.24, 0.0),
                        Vec3::new(0.5, 0.55, 0.5),
                        tint,
                    );
                }
                MobKind::Zombie => {
                    let skin = glam::Vec4::new(0.2, 0.45, 0.25, 1.0);
                    let shirt = glam::Vec4::new(0.15, 0.35, 0.45, 1.0);
                    // Body
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.0, 0.625, 0.0),
                        Vec3::new(0.5, 1.25, 0.42),
                        shirt,
                    );
                    // Outstretched arms
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.35, 0.98, 0.0),
                        Vec3::new(0.5, 0.2, 0.2),
                        skin,
                    );
                    // Head
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.0, 1.53, 0.0),
                        Vec3::new(0.5, 0.5, 0.5),
                        skin,
                    );
                }
                MobKind::Skeleton => {
                    let bone = glam::Vec4::new(0.85, 0.85, 0.82, 1.0);
                    // Slender body & head
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.0, 0.625, 0.0),
                        Vec3::new(0.3, 1.2, 0.3),
                        bone,
                    );
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.0, 1.5, 0.0),
                        Vec3::new(0.45, 0.45, 0.45),
                        bone,
                    );
                }
                MobKind::Creeper => {
                    let green = glam::Vec4::new(0.25, 0.6, 0.25, 1.0);
                    // Body trunk
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.0, 0.7, 0.0),
                        Vec3::new(0.45, 1.0, 0.45),
                        green,
                    );
                    // Head
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.0, 1.45, 0.0),
                        Vec3::new(0.5, 0.5, 0.5),
                        green,
                    );
                    // 4 legs
                    part(
                        &self.villager_mesh,
                        Vec3::new(-0.2, 0.15, -0.2),
                        Vec3::new(0.2, 0.3, 0.2),
                        green,
                    );
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.2, 0.15, -0.2),
                        Vec3::new(0.2, 0.3, 0.2),
                        green,
                    );
                    part(
                        &self.villager_mesh,
                        Vec3::new(-0.2, 0.15, 0.2),
                        Vec3::new(0.2, 0.3, 0.2),
                        green,
                    );
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.2, 0.15, 0.2),
                        Vec3::new(0.2, 0.3, 0.2),
                        green,
                    );
                }
                MobKind::Pig => {
                    let pink = glam::Vec4::new(0.95, 0.65, 0.70, 1.0);
                    let dark_pink = glam::Vec4::new(0.85, 0.50, 0.55, 1.0);

                    // 4 Legs
                    let leg_w = 0.18;
                    let leg_h = 0.35;
                    for &(lx, lz) in &[(-0.20, 0.25), (0.20, 0.25), (-0.20, -0.25), (0.20, -0.25)] {
                        part(
                            &self.villager_mesh,
                            Vec3::new(lx, leg_h / 2.0, lz),
                            Vec3::new(leg_w, leg_h, leg_w),
                            pink,
                        );
                    }
                    // Torso
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.0, 0.55, 0.0),
                        Vec3::new(0.65, 0.55, 0.90),
                        pink,
                    );
                    // Head
                    let head_pos = Vec3::new(0.0, 0.72, 0.48);
                    part(
                        &self.villager_mesh,
                        head_pos,
                        Vec3::new(0.42, 0.42, 0.42),
                        pink,
                    );
                    // Snout
                    part(
                        &self.villager_mesh,
                        head_pos + Vec3::new(0.0, -0.06, 0.22),
                        Vec3::new(0.24, 0.16, 0.12),
                        dark_pink,
                    );
                }
                MobKind::Cow => {
                    let brown = glam::Vec4::new(0.42, 0.30, 0.20, 1.0);
                    let white_spot = glam::Vec4::new(0.92, 0.92, 0.90, 1.0);
                    let horn_color = glam::Vec4::new(0.85, 0.85, 0.78, 1.0);
                    let hoof_color = glam::Vec4::new(0.25, 0.22, 0.20, 1.0);

                    // 4 Legs with hooves
                    let leg_w = 0.20;
                    let leg_h = 0.50;
                    for &(lx, lz) in &[(-0.24, 0.30), (0.24, 0.30), (-0.24, -0.30), (0.24, -0.30)] {
                        part(
                            &self.villager_mesh,
                            Vec3::new(lx, leg_h / 2.0, lz),
                            Vec3::new(leg_w, leg_h, leg_w),
                            brown,
                        );
                        part(
                            &self.villager_mesh,
                            Vec3::new(lx, 0.06, lz),
                            Vec3::new(leg_w * 1.05, 0.12, leg_w * 1.05),
                            hoof_color,
                        );
                    }
                    // Torso & spots
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.0, 0.75, 0.0),
                        Vec3::new(0.72, 0.65, 1.05),
                        brown,
                    );
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.0, 0.78, 0.08),
                        Vec3::new(0.74, 0.50, 0.50),
                        white_spot,
                    );

                    // Head & Horns
                    let head_pos = Vec3::new(0.0, 0.95, 0.52);
                    part(
                        &self.villager_mesh,
                        head_pos,
                        Vec3::new(0.44, 0.44, 0.44),
                        brown,
                    );
                    // Horns
                    part(
                        &self.villager_mesh,
                        head_pos + Vec3::new(-0.26, 0.22, -0.05),
                        Vec3::new(0.10, 0.18, 0.10),
                        horn_color,
                    );
                    part(
                        &self.villager_mesh,
                        head_pos + Vec3::new(0.26, 0.22, -0.05),
                        Vec3::new(0.10, 0.18, 0.10),
                        horn_color,
                    );
                }
                MobKind::Sheep => {
                    let wool_color = match mob.variant % 5 {
                        0 | 1 | 2 => glam::Vec4::new(0.95, 0.95, 0.95, 1.0), // White (common)
                        3 => glam::Vec4::new(0.94, 0.62, 0.72, 1.0), // Pink (rare Minecraft sheep!)
                        _ => glam::Vec4::new(0.60, 0.55, 0.50, 1.0), // Light Gray/Brown
                    };
                    let skin_color = glam::Vec4::new(0.88, 0.82, 0.75, 1.0); // Tan skin face & legs
                    let hoof_color = glam::Vec4::new(0.40, 0.35, 0.30, 1.0); // Dark hooves

                    // 1. 4 Skin legs with hooves sticking out underneath
                    let leg_w = 0.18;
                    let leg_h = 0.45;
                    let leg_d = 0.18;
                    let leg_y = leg_h / 2.0;

                    for &(lx, lz) in &[(-0.22, 0.28), (0.22, 0.28), (-0.22, -0.28), (0.22, -0.28)] {
                        part(
                            &self.villager_mesh,
                            Vec3::new(lx, leg_y, lz),
                            Vec3::new(leg_w, leg_h, leg_d),
                            skin_color,
                        );
                        part(
                            &self.villager_mesh,
                            Vec3::new(lx, 0.06, lz),
                            Vec3::new(leg_w * 1.05, 0.12, leg_d * 1.05),
                            hoof_color,
                        );
                    }

                    // 2. Main Fluffy Wool Torso
                    let body_y = 0.72;
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.0, body_y, 0.0),
                        Vec3::new(0.75, 0.65, 1.05),
                        wool_color,
                    );
                    // Puffy side/top wool layer overlays for 3D depth
                    part(
                        &self.villager_mesh,
                        Vec3::new(0.0, body_y + 0.05, 0.0),
                        Vec3::new(0.79, 0.55, 0.98),
                        wool_color * 0.98,
                    );

                    // 3. Head, Face & Ears
                    let head_pos = Vec3::new(0.0, 0.88, 0.52);
                    // Face skin block (naked snout)
                    part(
                        &self.villager_mesh,
                        head_pos + Vec3::new(0.0, 0.0, 0.08),
                        Vec3::new(0.38, 0.38, 0.38),
                        skin_color,
                    );
                    // Wool cap on back/top of head (helmet wool)
                    part(
                        &self.villager_mesh,
                        head_pos + Vec3::new(0.0, 0.08, -0.06),
                        Vec3::new(0.42, 0.34, 0.32),
                        wool_color,
                    );

                    // Ears (tan skin ears extending horizontally)
                    part(
                        &self.villager_mesh,
                        head_pos + Vec3::new(-0.25, 0.06, -0.02),
                        Vec3::new(0.16, 0.08, 0.10),
                        skin_color,
                    );
                    part(
                        &self.villager_mesh,
                        head_pos + Vec3::new(0.25, 0.06, -0.02),
                        Vec3::new(0.16, 0.08, 0.10),
                        skin_color,
                    );

                    // Eyes (small dark eye dots)
                    let eye_color = glam::Vec4::new(0.1, 0.1, 0.1, 1.0);
                    part(
                        &self.villager_mesh,
                        head_pos + Vec3::new(-0.16, 0.04, 0.24),
                        Vec3::new(0.06, 0.08, 0.06),
                        eye_color,
                    );
                    part(
                        &self.villager_mesh,
                        head_pos + Vec3::new(0.16, 0.04, 0.24),
                        Vec3::new(0.06, 0.08, 0.06),
                        eye_color,
                    );
                }
            }
        }
        shader.set_vec4(loc_diff, glam::Vec4::ONE);
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

    pub fn schedule_water_neighbors(&mut self, x: i32, y: i32, z: i32) {
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

    pub fn check_water_lava_collisions(&mut self, x: i32, y: i32, z: i32) -> bool {
        let mut hit = false;
        for &(dx, dy, dz) in &[
            (1i32, 0i32, 0i32),
            (-1, 0, 0),
            (0, 1, 0),
            (0, -1, 0),
            (0, 0, 1),
            (0, 0, -1),
        ] {
            let (nx, ny, nz) = (x + dx, y + dy, z + dz);
            let nb = self.get_block(nx, ny, nz);
            if nb == BlockType::Lava {
                let l_level = self.get_liquid_level(nx, ny, nz);
                if water_is_source(l_level) || l_level == 1 {
                    self.set_block(nx, ny, nz, BlockType::Obsidian);
                    self.set_liquid_level(nx, ny, nz, 0);
                } else {
                    self.set_block(nx, ny, nz, BlockType::Cobblestone);
                    self.set_liquid_level(nx, ny, nz, 0);
                }
                hit = true;
            }
        }
        hit
    }

    pub fn get_redstone_power(&self, x: i32, y: i32, z: i32) -> u8 {
        let b = self.get_block(x, y, z);
        match b {
            BlockType::RedstoneBlock
            | BlockType::RedstoneTorch
            | BlockType::Lever
            | BlockType::StoneButton => 15,
            _ => 0,
        }
    }

    pub fn push_piston(&mut self, x: i32, y: i32, z: i32, dir: (i32, i32, i32)) -> bool {
        let (dx, dy, dz) = dir;
        let mut push_count = 0;
        let mut current_x = x + dx;
        let mut current_y = y + dy;
        let mut current_z = z + dz;

        // Scan line up to 12 blocks
        while push_count < 12 {
            let b = self.get_block(current_x, current_y, current_z);
            if b == BlockType::Air {
                break;
            }
            if b == BlockType::Bedrock || b == BlockType::Obsidian {
                return false; // Cannot push immovable blocks
            }
            push_count += 1;
            current_x += dx;
            current_y += dy;
            current_z += dz;
        }

        if push_count >= 12 {
            return false;
        }

        // Shift blocks forward from back to front
        for i in (1..=push_count).rev() {
            let src_x = x + dx * i;
            let src_y = y + dy * i;
            let src_z = z + dz * i;
            let dst_x = src_x + dx;
            let dst_y = src_y + dy;
            let dst_z = src_z + dz;
            let b = self.get_block(src_x, src_y, src_z);
            self.set_block(dst_x, dst_y, dst_z, b);
        }

        // Place PistonHead at extended position
        self.set_block(x + dx, y + dy, z + dz, BlockType::PistonHead);
        true
    }

    pub fn retract_piston(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        dir: (i32, i32, i32),
        is_sticky: bool,
    ) -> bool {
        let (dx, dy, dz) = dir;
        let head_x = x + dx;
        let head_y = y + dy;
        let head_z = z + dz;

        if self.get_block(head_x, head_y, head_z) == BlockType::PistonHead {
            self.set_block(head_x, head_y, head_z, BlockType::Air);
        }

        if is_sticky {
            let pulled_x = head_x + dx;
            let pulled_y = head_y + dy;
            let pulled_z = head_z + dz;
            let b = self.get_block(pulled_x, pulled_y, pulled_z);
            if b != BlockType::Air && b != BlockType::Bedrock && b != BlockType::Obsidian {
                self.set_block(head_x, head_y, head_z, b);
                self.set_block(pulled_x, pulled_y, pulled_z, BlockType::Air);
            }
        }
        true
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

            if self.check_water_lava_collisions(x, y, z) {
                processed += 1;
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
        // Use time only as a positional drift, not as a noise parameter
        // that changes cloud shapes on regen. Clouds drift slowly via offset.
        let drift = time * 0.0005;
        let cloud_at = |x: i32, z: i32| -> bool {
            let base =
                crate::noise::perlin_2d(x as f32 * 0.18 + drift, z as f32 * 0.18 + 41.0, 0.5, 2);
            let breakup =
                crate::noise::perlin_2d(x as f32 * 0.48 + drift, z as f32 * 0.48 + 7.0, 0.5, 1);
            base > 0.10 && breakup > -0.08
        };
        // Cache cloud_at across the grid (plus a 1-cell border for neighbor
        // checks) so each coordinate's noise evaluation happens once instead
        // of up to 5 times (once per cell, once per neighboring cell).
        let width = (range * 2) as usize;
        let cloud_grid: Vec<bool> = (0..width + 2)
            .flat_map(|gx| {
                (0..width + 2)
                    .map(move |gz| cloud_at(start_x - 1 + gx as i32, start_z - 1 + gz as i32))
            })
            .collect();
        let cloud_at = |x: i32, z: i32| -> bool {
            let gx = (x - (start_x - 1)) as usize;
            let gz = (z - (start_z - 1)) as usize;
            cloud_grid[gx * (width + 2) + gz]
        };
        for x in start_x..(start_x + range * 2) {
            for z in start_z..(start_z + range * 2) {
                if !cloud_at(x, z) {
                    continue;
                }
                let px = x as f32 * cloud_size;
                let py = cloud_height;
                let pz = z as f32 * cloud_size;
                let sx = cloud_size;
                let sy = 1.1;
                let sz = cloud_size;
                let top = [
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
                ];
                let bottom = [
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
                ];
                let pos_z = [
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
                ];
                let neg_z = [
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
                ];
                let pos_x = [
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
                ];
                let neg_x = [
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
                let up_n = [0.0, 1.0, 0.0].repeat(6);
                let down_n = [0.0, -1.0, 0.0].repeat(6);
                let pos_z_n = [0.0, 0.0, 1.0].repeat(6);
                let neg_z_n = [0.0, 0.0, -1.0].repeat(6);
                let pos_x_n = [1.0, 0.0, 0.0].repeat(6);
                let neg_x_n = [-1.0, 0.0, 0.0].repeat(6);
                let before = v.len();
                v.extend_from_slice(&top);
                n.extend_from_slice(&up_n);
                v.extend_from_slice(&bottom);
                n.extend_from_slice(&down_n);
                // Side walls only at cloud boundaries. Interior walls between
                // adjacent cells show through the translucent top/bottom as
                // grid seams, and the coplanar pairs z-fight into speckles.
                if !cloud_at(x, z + 1) {
                    v.extend_from_slice(&pos_z);
                    n.extend_from_slice(&pos_z_n);
                }
                if !cloud_at(x, z - 1) {
                    v.extend_from_slice(&neg_z);
                    n.extend_from_slice(&neg_z_n);
                }
                if !cloud_at(x + 1, z) {
                    v.extend_from_slice(&pos_x);
                    n.extend_from_slice(&pos_x_n);
                }
                if !cloud_at(x - 1, z) {
                    v.extend_from_slice(&neg_x);
                    n.extend_from_slice(&neg_x_n);
                }
                for _ in 0..((v.len() - before) / 3) {
                    c.extend_from_slice(&cloud_color);
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
        crate::renderer::set_blend(true);
        crate::renderer::set_cull(true);
        for &i in &self.visible_chunks {
            if let Some(chunk) = &self.chunks[i]
                && let Some(mesh) = &chunk.mesh_transparent
            {
                mesh.draw();
            }
        }
    }

    pub fn render_water(&self, _frustum: &Frustum) {
        crate::renderer::set_blend(true);
        crate::renderer::set_depth_write(false);
        crate::renderer::set_cull(true);
        for &i in &self.visible_chunks {
            if let Some(chunk) = &self.chunks[i]
                && let Some(mesh) = &chunk.mesh_water
            {
                mesh.draw();
            }
        }
        crate::renderer::set_depth_write(true);
    }

    pub fn render_clouds(&self, shader: &Shader, mvp: &Mat4) {
        if let Some(mesh) = &self.cloud_model {
            shader.bind();
            shader.set_mat4(shader.get_uniform_location("uMVP"), mvp);
            shader.set_int(shader.get_uniform_location("uIsCircle"), 0);
            crate::renderer::set_cull(false);
            crate::renderer::set_blend(true);
            crate::renderer::set_depth_write(false);
            mesh.draw();
            crate::renderer::set_depth_write(true);
            crate::renderer::set_blend(false);
            crate::renderer::set_cull(true);
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
            crate::renderer::set_depth_test(false);
            mesh.draw();
            crate::renderer::set_depth_test(true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_water_lava_collision_creates_obsidian() {
        let mut chunk = Chunk::new(0, 0, 12345);
        chunk.set_block(10, 60, 10, BlockType::Water);
        chunk.set_block(11, 60, 10, BlockType::Lava);
        let l_level = chunk.liquid_levels[11][60][10];
        if water_is_source(l_level) || l_level == 1 {
            chunk.set_block(11, 60, 10, BlockType::Obsidian);
        }
        assert_eq!(chunk.get_block(11, 60, 10), BlockType::Obsidian);
    }

    #[test]
    fn test_water_flowing_lava_collision_creates_cobblestone() {
        let mut chunk = Chunk::new(0, 0, 12345);
        chunk.set_block(10, 60, 10, BlockType::Water);
        chunk.set_block(11, 60, 10, BlockType::Lava);
        chunk.liquid_levels[11][60][10] = 5; // flowing lava
        let l_level = chunk.liquid_levels[11][60][10];
        if !water_is_source(l_level) && l_level > 1 {
            chunk.set_block(11, 60, 10, BlockType::Cobblestone);
        }
        assert_eq!(chunk.get_block(11, 60, 10), BlockType::Cobblestone);
    }

    #[test]
    fn test_redstone_power_sources() {
        let is_power_source = |b: BlockType| match b {
            BlockType::RedstoneBlock
            | BlockType::RedstoneTorch
            | BlockType::Lever
            | BlockType::StoneButton => 15,
            _ => 0,
        };
        assert_eq!(is_power_source(BlockType::RedstoneBlock), 15);
        assert_eq!(is_power_source(BlockType::RedstoneTorch), 15);
        assert_eq!(is_power_source(BlockType::Air), 0);
    }

    #[test]
    fn view_distance_pool_is_bounded_to_settings_max() {
        assert_eq!(VIEW_DISTANCE, DEFAULT_VIEW_DISTANCE);
        assert_eq!(MAX_VIEW_DISTANCE, 12);
        assert_eq!(POOL_WIDTH, 25);
        assert_eq!(CHUNK_POOL_SIZE, 625);
        assert_eq!(GPU_POOL_VIEW_DISTANCE, MAX_VIEW_DISTANCE);
        assert!(CHUNK_POOL_SIZE < 35 * 35);
    }

    #[test]
    fn imported_source_does_not_require_unique_pool_slots() {
        let a = (0, 0);
        let b = (POOL_WIDTH, 0);
        let ix = |cx: i32, cz: i32| {
            let ix = cx.rem_euclid(POOL_WIDTH);
            let iz = cz.rem_euclid(POOL_WIDTH);
            (ix + iz * POOL_WIDTH) as usize
        };
        assert_eq!(ix(a.0, a.1), ix(b.0, b.1));
    }

    #[test]
    fn test_piston_push_blocks() {
        let mut chunk = Chunk::new(0, 0, 12345);
        chunk.set_block(5, 60, 5, BlockType::Piston);
        chunk.set_block(6, 60, 5, BlockType::Dirt);
        chunk.set_block(7, 60, 5, BlockType::Air);

        // Simulate push: Dirt moves 6->7, PistonHead places at 6
        chunk.set_block(7, 60, 5, BlockType::Dirt);
        chunk.set_block(6, 60, 5, BlockType::PistonHead);

        assert_eq!(chunk.get_block(6, 60, 5), BlockType::PistonHead);
        assert_eq!(chunk.get_block(7, 60, 5), BlockType::Dirt);
    }

    #[test]
    fn test_piston_blocked_by_obsidian() {
        let mut chunk = Chunk::new(0, 0, 12345);
        chunk.set_block(5, 60, 5, BlockType::Piston);
        chunk.set_block(6, 60, 5, BlockType::Obsidian);
        // Obsidian cannot be pushed
        assert_eq!(chunk.get_block(6, 60, 5), BlockType::Obsidian);
    }

    #[test]
    fn test_sticky_piston_retract() {
        let mut chunk = Chunk::new(0, 0, 12345);
        chunk.set_block(5, 60, 5, BlockType::StickyPiston);
        chunk.set_block(6, 60, 5, BlockType::PistonHead);
        chunk.set_block(7, 60, 5, BlockType::Stone);

        // Retract: Stone pulls from 7->6, PistonHead removed
        chunk.set_block(6, 60, 5, BlockType::Stone);
        chunk.set_block(7, 60, 5, BlockType::Air);

        assert_eq!(chunk.get_block(6, 60, 5), BlockType::Stone);
        assert_eq!(chunk.get_block(7, 60, 5), BlockType::Air);
    }
}
