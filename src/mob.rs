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

/// Minecraft animal goal timers. Hostiles leave this at default.
#[derive(Clone, Debug, Default)]
pub struct AnimalState {
    /// PanicGoal: seconds left fleeing after a hit (100 ticks).
    pub panic_time: f32,
    /// LookAtPlayerGoal / RandomLookAroundGoal duration.
    pub look_time: f32,
    /// EatBlockGoal: sheep grazing, 40 ticks.
    pub eat_time: f32,
    /// WaterAvoidingRandomStrollGoal destination, XZ.
    pub dest: Option<(f32, f32)>,
    /// BreedGoal love mode, 30 seconds.
    pub love_time: f32,
    /// 5 minutes after a successful breed.
    pub breed_cooldown: f32,
    /// Seconds until adulthood. Zero means adult.
    pub growth: f32,
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
    pub health: f32,
    pub attack_cooldown: f32,
    /// Cosmetic variation (robe tint for villagers)
    pub variant: u8,
    pub animal: AnimalState,
}

impl Mob {
    pub fn new(kind: MobKind, position: Vec3, home: Vec3, variant: u8) -> Self {
        let health = Self::max_health_for(kind);
        Self {
            kind,
            position,
            velocity: Vec3::ZERO,
            yaw: (variant as f32) * 1.3,
            walk_speed: 0.0,
            wander_timer: 0.5 + (variant as f32) * 0.37,
            home,
            grounded: false,
            health,
            attack_cooldown: 0.0,
            variant,
            animal: AnimalState::default(),
        }
    }

    fn max_health_for(kind: MobKind) -> f32 {
        match kind {
            MobKind::Golem => 100.0,
            // Java Cow/Pig createAttributes MAX_HEALTH 10, Sheep 8.
            MobKind::Cow | MobKind::Pig => 10.0,
            MobKind::Sheep => 8.0,
            MobKind::Villager | MobKind::Zombie | MobKind::Skeleton | MobKind::Creeper => 20.0,
        }
    }

    pub fn take_damage(&mut self, damage: f32) -> bool {
        self.health = (self.health - damage.max(0.0)).max(0.0);
        if self.is_animal() && self.health > 0.0 {
            // PanicGoal lasts 100 ticks after the last hit.
            self.animal.panic_time = 5.0;
            self.animal.dest = None;
            self.animal.eat_time = 0.0;
        }
        self.health <= 0.0
    }

    pub fn is_animal(&self) -> bool {
        matches!(self.kind, MobKind::Pig | MobKind::Cow | MobKind::Sheep)
    }

    pub fn is_baby(&self) -> bool {
        self.animal.growth > 0.0
    }

    /// Java `Attributes.MOVEMENT_SPEED` for animals.
    pub fn movement_attribute(&self) -> f32 {
        match self.kind {
            MobKind::Cow => 0.2,
            MobKind::Sheep => 0.23,
            MobKind::Pig => 0.25,
            _ => 0.25,
        }
    }

    /// Ground travel in blocks/s. 0.2 attribute is ~2.15 m/s on land.
    pub fn walk_speed_mps(&self) -> f32 {
        self.movement_attribute() * 10.75
    }

    pub fn panic_multiplier(&self) -> f32 {
        match self.kind {
            MobKind::Cow => 2.0,
            _ => 1.25,
        }
    }

    pub fn tempt_multiplier(&self) -> f32 {
        match self.kind {
            MobKind::Cow => 1.25,
            MobKind::Pig => 1.2,
            MobKind::Sheep => 1.1,
            _ => 1.0,
        }
    }

    /// TemptGoal / BreedGoal food. Pigs want carrots, which this build
    /// does not have, so they are not tempted.
    pub fn food_item(&self) -> Option<BlockType> {
        match self.kind {
            MobKind::Cow | MobKind::Sheep => Some(BlockType::Wheat),
            _ => None,
        }
    }

    pub fn is_hostile(&self) -> bool {
        matches!(
            self.kind,
            MobKind::Zombie | MobKind::Skeleton | MobKind::Creeper
        )
    }

    pub fn height(&self) -> f32 {
        let h = match self.kind {
            MobKind::Villager => 1.8,
            MobKind::Golem => 2.5,
            MobKind::Zombie => 1.95,
            MobKind::Skeleton => 1.99,
            MobKind::Creeper => 1.7,
            MobKind::Pig => 0.9,
            MobKind::Cow => 1.4,
            MobKind::Sheep => 1.3,
        };
        if self.is_baby() { h * 0.5 } else { h }
    }

    pub fn half_width(&self) -> f32 {
        let w = match self.kind {
            MobKind::Villager => 0.28,
            MobKind::Golem => 0.55,
            MobKind::Zombie | MobKind::Skeleton | MobKind::Creeper => 0.3,
            // Java collision boxes are 0.9 wide for the three animals.
            MobKind::Pig | MobKind::Cow | MobKind::Sheep => 0.45,
        };
        if self.is_baby() { w * 0.5 } else { w }
    }

    pub fn base_speed(&self) -> f32 {
        match self.kind {
            MobKind::Villager => 1.7,
            MobKind::Golem => 1.15,
            MobKind::Zombie => 1.5,
            MobKind::Skeleton => 1.6,
            MobKind::Creeper => 1.4,
            MobKind::Pig | MobKind::Cow | MobKind::Sheep => self.walk_speed_mps(),
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
            MobKind::Pig => Some((BlockType::RawPorkchop, 1 + (rand::random::<u8>() % 3))),
            MobKind::Cow => Some((BlockType::RawBeef, 1 + (rand::random::<u8>() % 3))),
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
        assert_eq!(p.health, 10.0);
        let pig_drop = p.drop_item().unwrap();
        assert_eq!(pig_drop.0, BlockType::RawPorkchop);
        assert!((1..=3).contains(&pig_drop.1));

        let cow = Mob::new(MobKind::Cow, pos, home, 0);
        assert_eq!(cow.height(), 1.4);
        assert_eq!(cow.health, 10.0);
        let cow_drop = cow.drop_item().unwrap();
        assert_eq!(cow_drop.0, BlockType::RawBeef);
        assert!((1..=3).contains(&cow_drop.1));

        let sh = Mob::new(MobKind::Sheep, pos, home, 0);
        assert_eq!(sh.height(), 1.3);
        assert_eq!(sh.drop_item(), Some((BlockType::Wool, 1)));

        let g = Mob::new(MobKind::Golem, pos, home, 0);
        assert_eq!(g.drop_item(), Some((BlockType::IronIngot, 3)));
        assert_eq!(g.health, 100.0);

        let v = Mob::new(MobKind::Villager, pos, home, 0);
        assert_eq!(v.drop_item(), None);
    }

    #[test]
    fn animals_use_minecraft_movement_attributes() {
        let cow = Mob::new(MobKind::Cow, Vec3::ZERO, Vec3::ZERO, 0);
        let pig = Mob::new(MobKind::Pig, Vec3::ZERO, Vec3::ZERO, 0);
        let sheep = Mob::new(MobKind::Sheep, Vec3::ZERO, Vec3::ZERO, 0);
        assert!((cow.movement_attribute() - 0.2).abs() < 1e-6);
        assert!((pig.movement_attribute() - 0.25).abs() < 1e-6);
        assert!((sheep.movement_attribute() - 0.23).abs() < 1e-6);
        assert!((cow.panic_multiplier() - 2.0).abs() < 1e-6);
        assert!((pig.panic_multiplier() - 1.25).abs() < 1e-6);
        assert_eq!(cow.food_item(), Some(BlockType::Wheat));
        assert_eq!(sheep.food_item(), Some(BlockType::Wheat));
        assert_eq!(pig.food_item(), None);
    }

    #[test]
    fn a_hit_starts_a_five_second_panic() {
        let mut cow = Mob::new(MobKind::Cow, Vec3::ZERO, Vec3::ZERO, 0);
        assert!(!cow.take_damage(1.0));
        assert!((cow.animal.panic_time - 5.0).abs() < 1e-6);
        let mut zombie = Mob::new(MobKind::Zombie, Vec3::ZERO, Vec3::ZERO, 0);
        zombie.take_damage(1.0);
        assert_eq!(zombie.animal.panic_time, 0.0);
    }

    #[test]
    fn babies_are_half_size_until_they_grow_up() {
        let mut calf = Mob::new(MobKind::Cow, Vec3::ZERO, Vec3::ZERO, 0);
        calf.animal.growth = 1200.0;
        assert!(calf.is_baby());
        assert!((calf.height() - 0.7).abs() < 1e-6);
        calf.animal.growth = 0.0;
        assert!(!calf.is_baby());
        assert!((calf.height() - 1.4).abs() < 1e-6);
    }

    #[test]
    fn mobs_survive_partial_damage_and_die_at_zero() {
        let mut zombie = Mob::new(MobKind::Zombie, Vec3::ZERO, Vec3::ZERO, 0);
        assert!(!zombie.take_damage(5.0));
        assert_eq!(zombie.health, 15.0);
        assert!(zombie.take_damage(15.0));
        assert_eq!(zombie.health, 0.0);
    }
}
