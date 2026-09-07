use crate::block::BlockType;
use crate::item;
use crate::world::World;
use glam::Vec3;

const CRACK_NORMALS: [Vec3; 6] = [
    Vec3::NEG_X,
    Vec3::X,
    Vec3::NEG_Y,
    Vec3::Y,
    Vec3::NEG_Z,
    Vec3::Z,
];

fn crack_faces(origin: Vec3, eye: Vec3, exposed: [bool; 6]) -> u8 {
    CRACK_NORMALS
        .iter()
        .enumerate()
        .fold(0, |mask, (i, normal)| {
            mask | if exposed[i] && (eye - origin - Vec3::splat(0.5)).dot(*normal) > 0.5 {
                1 << i
            } else {
                0
            }
        })
}

fn crack_vertices(origin: Vec3, stage: u8, faces: u8) -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let mut vertices = Vec::new();
    let mut uv = Vec::new();
    let mut normals = Vec::new();
    // Inset sampling prevents adjacent atlas tiles bleeding into the decal.
    let u0 = (stage.min(9) as f32 * 16.0 + 0.5) / 256.0;
    let u1 = u0 + 15.0 / 256.0;
    let v0 = 48.5 / 256.0;
    let v1 = 63.5 / 256.0;
    for (i, &normal) in CRACK_NORMALS.iter().enumerate() {
        if faces & (1 << i) == 0 {
            continue;
        }
        let u = if normal.y.abs() > 0.5 {
            Vec3::X
        } else {
            Vec3::Y.cross(normal)
        };
        let v = normal.cross(u);
        let center = origin + Vec3::splat(0.5) + normal * 0.502;
        let corners = [
            center - u * 0.5 - v * 0.5,
            center + u * 0.5 - v * 0.5,
            center + u * 0.5 + v * 0.5,
            center - u * 0.5 + v * 0.5,
        ];
        let texels = [[u0, v1], [u1, v1], [u1, v0], [u0, v0]];
        for corner in [0, 1, 2, 0, 2, 3] {
            vertices.extend_from_slice(&corners[corner].to_array());
            normals.extend_from_slice(&normal.to_array());
            uv.extend_from_slice(&texels[corner]);
        }
    }
    (vertices, uv, normals)
}

#[derive(Default)]
pub struct CrackOverlay {
    key: Option<(i32, i32, i32, u8, u8)>,
    mesh: Option<crate::renderer::Mesh>,
}

impl CrackOverlay {
    pub fn draw(&mut self, world: &World, eye: Vec3, target: Option<(i32, i32, i32, u8)>) {
        let Some((x, y, z, stage)) = target else {
            return;
        };
        let origin = Vec3::new(x as f32, y as f32, z as f32);
        let exposed = CRACK_NORMALS.map(|n| {
            let block = world.get_block(x + n.x as i32, y + n.y as i32, z + n.z as i32);
            !block.is_solid() || block.is_transparent()
        });
        let faces = crack_faces(origin, eye, exposed);
        if faces == 0 {
            return;
        }
        let key = Some((x, y, z, stage, faces));
        if self.key != key {
            let (vertices, uv, normals) = crack_vertices(origin, stage, faces);
            let colors = vec![255; vertices.len() / 3 * 4];
            self.mesh = Some(crate::renderer::Mesh::new(
                &vertices,
                Some(&uv),
                Some(&normals),
                Some(&colors),
            ));
            self.key = key;
        }
        let Some(atlas) = world.atlas.as_ref() else {
            return;
        };
        atlas.bind(0);
        crate::renderer::set_blend(true);
        crate::renderer::set_depth_test(true);
        crate::renderer::set_depth_write(false);
        crate::renderer::set_polygon_offset(true);
        self.mesh.as_ref().unwrap().draw();
        crate::renderer::set_polygon_offset(false);
        crate::renderer::set_depth_write(true);
        crate::renderer::set_blend(false);
    }
}

/// Result of a completed block break.
pub struct MinedBlock {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    #[allow(dead_code)]
    pub block: BlockType,
    pub drop: BlockType,
    pub drop_count: u8,
}

/// Tracks progressive mining state.
pub struct MiningState {
    /// Block currently being mined.
    pub target: Option<(i32, i32, i32)>,
    /// Accumulated progress in seconds.
    pub progress: f32,
    /// Total time needed to break the target block.
    pub total_time: f32,
}

impl MiningState {
    pub fn new() -> Self {
        Self {
            target: None,
            progress: 0.0,
            total_time: 0.0,
        }
    }

    /// Reset mining progress (e.g., when player looks away or releases button).
    pub fn reset(&mut self) {
        self.target = None;
        self.progress = 0.0;
        self.total_time = 0.0;
    }

    /// Returns the crack stage (0-9) if currently mining, plus the target position.
    pub fn crack_stage(&self) -> Option<(i32, i32, i32, u8)> {
        if let Some((x, y, z)) = self.target
            && self.total_time > 0.0
        {
            let frac = (self.progress / self.total_time).clamp(0.0, 0.999);
            let stage = (frac * 10.0) as u8;
            return Some((x, y, z, stage));
        }
        None
    }

    /// Update mining state. Called every frame while left mouse is held.
    /// Returns Some(MinedBlock) when a block is fully broken.
    pub fn update(
        &mut self,
        world: &World,
        eye_pos: Vec3,
        look_dir: Vec3,
        held_item: BlockType,
        dt: f32,
    ) -> Option<MinedBlock> {
        // Raycast to find targeted block
        let res = world.raycast(eye_pos, look_dir, 8.0);
        if !res.hit {
            self.reset();
            return None;
        }

        let block = world.get_block(res.x, res.y, res.z);

        // Can't mine air, liquids, or bedrock
        if block == BlockType::Air
            || block == BlockType::Water
            || block == BlockType::Lava
            || block == BlockType::Bedrock
        {
            self.reset();
            return None;
        }

        let target_pos = (res.x, res.y, res.z);

        // Check if target changed
        if self.target != Some(target_pos) {
            self.target = Some(target_pos);
            self.progress = 0.0;
            self.total_time = item::breaking_time(block, held_item);
        }

        // Accumulate progress
        self.progress += dt;

        // Check if block is broken
        if self.progress >= self.total_time {
            let (drop, drop_count) = item::get_drop(block, held_item);
            let result = MinedBlock {
                x: res.x,
                y: res.y,
                z: res.z,
                block,
                drop,
                drop_count,
            };
            self.reset();
            return Some(result);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cracks_only_build_exposed_camera_facing_sides() {
        let origin = Vec3::ZERO;
        assert_eq!(
            crack_faces(origin, Vec3::splat(3.0), [true; 6]).count_ones(),
            3
        );
        assert_eq!(crack_faces(origin, Vec3::splat(3.0), [false; 6]), 0);
        for (face, normal) in CRACK_NORMALS.iter().enumerate() {
            let mask = crack_faces(origin, Vec3::splat(0.5) + normal * 3.0, [true; 6]);
            assert_eq!(mask, 1 << face);
            let (vertices, uv, normals) = crack_vertices(origin, 9, mask);
            assert_eq!(vertices.len(), 18);
            assert_eq!(uv.len(), 12);
            for (triangle, n) in vertices
                .as_chunks::<9>()
                .0
                .iter()
                .zip(normals.as_chunks::<9>().0.iter())
            {
                let a = Vec3::from_slice(&triangle[0..3]);
                let b = Vec3::from_slice(&triangle[3..6]);
                let c = Vec3::from_slice(&triangle[6..9]);
                assert!(
                    (b - a).cross(c - a).dot(Vec3::from_slice(n)) > 0.99,
                    "crack face wound inward"
                );
                assert!((a - Vec3::splat(0.5)).dot(*normal) > 0.5);
            }
        }
    }

    #[test]
    fn test_new_mining_state() {
        let state = MiningState::new();
        assert!(state.target.is_none());
        assert_eq!(state.progress, 0.0);
        assert_eq!(state.total_time, 0.0);
    }

    #[test]
    fn test_reset() {
        let mut state = MiningState::new();
        state.target = Some((1, 2, 3));
        state.progress = 0.5;
        state.total_time = 1.0;
        state.reset();
        assert!(state.target.is_none());
        assert_eq!(state.progress, 0.0);
    }

    #[test]
    fn test_crack_stage_none_when_no_target() {
        let state = MiningState::new();
        assert!(state.crack_stage().is_none());
    }

    #[test]
    fn test_crack_stage_progression() {
        let mut state = MiningState::new();
        state.target = Some((0, 0, 0));
        state.total_time = 1.0;

        state.progress = 0.0;
        assert_eq!(state.crack_stage().unwrap().3, 0);

        state.progress = 0.25;
        assert_eq!(state.crack_stage().unwrap().3, 2);

        state.progress = 0.5;
        assert_eq!(state.crack_stage().unwrap().3, 5);

        state.progress = 0.9;
        assert_eq!(state.crack_stage().unwrap().3, 9);
    }

    #[test]
    fn test_crack_stage_clamp() {
        let mut state = MiningState::new();
        state.target = Some((0, 0, 0));
        state.total_time = 1.0;
        state.progress = 5.0; // Way past done
        // Should clamp to 9
        assert_eq!(state.crack_stage().unwrap().3, 9);
    }

    // ── crack_stage returns correct position coordinates ────────────────────

    #[test]
    fn test_crack_stage_returns_correct_coordinates() {
        let mut state = MiningState::new();
        state.target = Some((10, 64, -30));
        state.total_time = 2.0;
        state.progress = 1.0;
        let (x, y, z, stage) = state.crack_stage().unwrap();
        assert_eq!(x, 10);
        assert_eq!(y, 64);
        assert_eq!(z, -30);
        assert_eq!(stage, 5); // 1.0/2.0 * 10 = 5
    }

    #[test]
    fn test_crack_stage_negative_coordinates() {
        let mut state = MiningState::new();
        state.target = Some((-100, 0, -200));
        state.total_time = 4.0;
        state.progress = 0.8;
        let (x, y, z, stage) = state.crack_stage().unwrap();
        assert_eq!(x, -100);
        assert_eq!(y, 0);
        assert_eq!(z, -200);
        assert_eq!(stage, 2); // 0.8/4.0 = 0.2, 0.2*10 = 2
    }

    // ── crack_stage with zero total_time returns None ───────────────────────

    #[test]
    fn test_crack_stage_zero_total_time_returns_none() {
        let mut state = MiningState::new();
        state.target = Some((5, 10, 15));
        state.total_time = 0.0;
        state.progress = 0.5;
        assert!(state.crack_stage().is_none());
    }

    // ── Progress accumulation ───────────────────────────────────────────────

    #[test]
    fn test_progress_accumulation_manual() {
        let mut state = MiningState::new();
        state.target = Some((0, 0, 0));
        state.total_time = 2.0;
        state.progress = 0.0;

        // Simulate accumulating progress
        state.progress += 0.5;
        assert!((state.progress - 0.5).abs() < f32::EPSILON);

        state.progress += 0.3;
        assert!((state.progress - 0.8).abs() < f32::EPSILON);

        // Check crack stage at this point: 0.8/2.0 = 0.4, stage = 4
        assert_eq!(state.crack_stage().unwrap().3, 4);

        state.progress += 1.2;
        assert!((state.progress - 2.0).abs() < f32::EPSILON);
        // At exactly total_time, clamped to 0.999 * 10 = 9
        assert_eq!(state.crack_stage().unwrap().3, 9);
    }

    #[test]
    fn test_reset_clears_all_state() {
        let mut state = MiningState::new();
        state.target = Some((42, 100, -7));
        state.progress = 1.5;
        state.total_time = 3.0;

        state.reset();

        assert!(state.target.is_none());
        assert_eq!(state.progress, 0.0);
        assert_eq!(state.total_time, 0.0);
        assert!(state.crack_stage().is_none());
    }

    #[test]
    fn test_crack_stage_boundary_values() {
        let mut state = MiningState::new();
        state.target = Some((0, 0, 0));
        state.total_time = 10.0;

        // At exactly 0 progress -> stage 0
        state.progress = 0.0;
        assert_eq!(state.crack_stage().unwrap().3, 0);

        // At 0.99 progress -> stage 0 (0.099 * 10 = 0)
        state.progress = 0.99;
        assert_eq!(state.crack_stage().unwrap().3, 0);

        // At 1.0 progress -> stage 1
        state.progress = 1.0;
        assert_eq!(state.crack_stage().unwrap().3, 1);

        // At 9.99 progress -> stage 9
        state.progress = 9.99;
        assert_eq!(state.crack_stage().unwrap().3, 9);
    }

    #[test]
    fn test_crack_stage_returns_correct_position() {
        let mut state = MiningState::new();
        state.target = Some((10, 20, 30));
        state.total_time = 5.0;
        state.progress = 2.5;
        let (x, y, z, stage) = state.crack_stage().unwrap();
        assert_eq!(x, 10);
        assert_eq!(y, 20);
        assert_eq!(z, 30);
        assert_eq!(stage, 5); // 2.5/5.0 = 0.5 * 10 = 5
    }

    #[test]
    fn test_crack_stage_zero_total_time() {
        let mut state = MiningState::new();
        state.target = Some((0, 0, 0));
        state.total_time = 0.0;
        state.progress = 0.0;
        // zero total_time should return None
        assert!(state.crack_stage().is_none());
    }

    #[test]
    fn test_reset_clears_everything() {
        let mut state = MiningState::new();
        state.target = Some((5, 10, 15));
        state.progress = 3.0;
        state.total_time = 5.0;
        state.reset();
        assert!(state.target.is_none());
        assert_eq!(state.progress, 0.0);
        assert_eq!(state.total_time, 0.0);
        assert!(state.crack_stage().is_none());
    }
}
