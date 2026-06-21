use crate::block::BlockType;
use crate::item;
use crate::world::World;
use glam::Vec3;

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

        // Can't mine air, water, or bedrock
        if block == BlockType::Air || block == BlockType::Water || block == BlockType::Bedrock {
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
