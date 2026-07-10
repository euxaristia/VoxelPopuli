// Village inhabitants: passive mobs with a simple wander-near-home AI.

use glam::Vec3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MobKind {
    Villager,
    Golem,
}

pub struct Mob {
    pub kind: MobKind,
    /// Feet-center position in world space
    pub position: Vec3,
    pub velocity: Vec3,
    /// Heading in radians; movement direction is (cos, 0, sin)
    pub yaw: f32,
    /// Current walk speed; zero while idling
    pub walk_speed: f32,
    /// Seconds until the mob re-rolls its wander state
    pub wander_timer: f32,
    /// Anchor the mob stays near (the village plaza)
    pub home: Vec3,
    pub grounded: bool,
    /// Cosmetic variation (robe tint for villagers)
    pub variant: u8,
}

impl Mob {
    pub fn new(kind: MobKind, position: Vec3, home: Vec3, variant: u8) -> Self {
        Self {
            kind,
            position,
            velocity: Vec3::ZERO,
            yaw: (variant as f32) * 1.3,
            walk_speed: 0.0,
            wander_timer: 0.5 + (variant as f32) * 0.37,
            home,
            grounded: false,
            variant,
        }
    }

    pub fn height(&self) -> f32 {
        match self.kind {
            MobKind::Villager => 1.8,
            MobKind::Golem => 2.5,
        }
    }

    pub fn half_width(&self) -> f32 {
        match self.kind {
            MobKind::Villager => 0.28,
            MobKind::Golem => 0.55,
        }
    }

    pub fn base_speed(&self) -> f32 {
        match self.kind {
            MobKind::Villager => 1.7,
            MobKind::Golem => 1.15,
        }
    }

    /// How far from home the mob is willing to wander
    pub fn leash_range(&self) -> f32 {
        match self.kind {
            MobKind::Villager => 22.0,
            MobKind::Golem => 30.0,
        }
    }
}
