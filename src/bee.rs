//! Bee goal state. Timings and search bounds follow Mojang's Bedrock bee example.
use glam::Vec3;

pub type BlockPos = (i32, i32, i32);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BeeState {
    pub hive: Option<BlockPos>,
    pub inside: bool,
    pub nectar: bool,
    pub pollination: f32,
    pub residence: f32,
    pub search_time: f32,
    pub retry: f32,
    pub sting_death: f32,
    pub crop_charges: u8,
    pub flower: Option<BlockPos>,
    pub goal: Option<Vec3>,
    pub goal_age: f32,
    pub path: Vec<Vec3>,
    pub random: u64,
    pub alert_pending: bool,
}

impl BeeState {
    pub fn random(&mut self, id: u32) -> f32 {
        if self.random == 0 {
            self.random = u64::from(id) + 1;
        }
        self.random = self
            .random
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.random >> 40) as f32 / (1u32 << 24) as f32
    }
}

pub fn center(p: BlockPos) -> Vec3 {
    Vec3::new(p.0 as f32 + 0.5, p.1 as f32, p.2 as f32 + 0.5)
}

pub fn is_flower(b: crate::block::BlockType) -> bool {
    matches!(
        b,
        crate::block::BlockType::Poppy
            | crate::block::BlockType::Dandelion
            | crate::block::BlockType::Sunflower
            | crate::block::BlockType::SunflowerTop
            | crate::block::BlockType::PinkPetals
            | crate::block::BlockType::Cornflower
            | crate::block::BlockType::Allium
            | crate::block::BlockType::OxeyeDaisy
    )
}

pub fn is_hive(b: crate::block::BlockType) -> bool {
    matches!(
        b,
        crate::block::BlockType::BeeNest | crate::block::BlockType::Beehive
    )
}
