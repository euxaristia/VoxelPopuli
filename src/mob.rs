use crate::block::BlockType;
use glam::Vec3;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MobKind {
    Villager,
    Golem,
    Zombie,
    Skeleton,
    Creeper,
    Pig,
    Cow,
    Sheep,
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
            MobKind::Zombie => 1.95,
            MobKind::Skeleton => 1.99,
            MobKind::Creeper => 1.7,
            MobKind::Pig => 0.9,
            MobKind::Cow => 1.4,
            MobKind::Sheep => 1.3,
        }
    }

    pub fn half_width(&self) -> f32 {
        match self.kind {
            MobKind::Villager => 0.28,
            MobKind::Golem => 0.55,
            MobKind::Zombie | MobKind::Skeleton | MobKind::Creeper => 0.3,
            MobKind::Pig | MobKind::Cow | MobKind::Sheep => 0.4,
        }
    }

    pub fn base_speed(&self) -> f32 {
        match self.kind {
            MobKind::Villager => 1.7,
            MobKind::Golem => 1.15,
            MobKind::Zombie => 1.5,
            MobKind::Skeleton => 1.6,
            MobKind::Creeper => 1.4,
            MobKind::Pig | MobKind::Cow | MobKind::Sheep => 1.2,
        }
    }

    /// How far from home the mob is willing to wander
    pub fn leash_range(&self) -> f32 {
        match self.kind {
            MobKind::Villager => 22.0,
            MobKind::Golem => 30.0,
            MobKind::Zombie | MobKind::Skeleton | MobKind::Creeper => 25.0,
            MobKind::Pig | MobKind::Cow | MobKind::Sheep => 20.0,
        }
    }

    /// Returns the loot drop when this mob is defeated
    pub fn drop_item(&self) -> Option<(BlockType, u8)> {
        match self.kind {
            MobKind::Villager => None,
            MobKind::Golem => Some((BlockType::IronIngot, 3)),
            MobKind::Zombie => Some((BlockType::RawIron, 1)),
            MobKind::Skeleton => Some((BlockType::Stick, 1)),
            MobKind::Creeper => Some((BlockType::Gunpowder, 1)),
            MobKind::Pig => Some((BlockType::RawPorkchop, 1)),
            MobKind::Cow => Some((BlockType::RawBeef, 1)),
            MobKind::Sheep => Some((BlockType::Wool, 1)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mob_dimensions_and_properties() {
        let pos = Vec3::ZERO;
        let home = Vec3::ZERO;

        let z = Mob::new(MobKind::Zombie, pos, home, 0);
        assert_eq!(z.height(), 1.95);
        assert_eq!(z.drop_item(), Some((BlockType::RawIron, 1)));

        let sk = Mob::new(MobKind::Skeleton, pos, home, 0);
        assert_eq!(sk.height(), 1.99);
        assert_eq!(sk.drop_item(), Some((BlockType::Stick, 1)));

        let c = Mob::new(MobKind::Creeper, pos, home, 0);
        assert_eq!(c.drop_item(), Some((BlockType::Gunpowder, 1)));

        let p = Mob::new(MobKind::Pig, pos, home, 0);
        assert_eq!(p.height(), 0.9);
        assert_eq!(p.drop_item(), Some((BlockType::RawPorkchop, 1)));

        let cow = Mob::new(MobKind::Cow, pos, home, 0);
        assert_eq!(cow.height(), 1.4);
        assert_eq!(cow.drop_item(), Some((BlockType::RawBeef, 1)));

        let sh = Mob::new(MobKind::Sheep, pos, home, 0);
        assert_eq!(sh.height(), 1.3);
        assert_eq!(sh.drop_item(), Some((BlockType::Wool, 1)));

        let g = Mob::new(MobKind::Golem, pos, home, 0);
        assert_eq!(g.drop_item(), Some((BlockType::IronIngot, 3)));

        let v = Mob::new(MobKind::Villager, pos, home, 0);
        assert_eq!(v.drop_item(), None);
    }
}
