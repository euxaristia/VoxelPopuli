use crate::block::BlockType;
use glam::Vec3;

pub use crate::mob_catalog::MobKind;
use crate::mob_catalog::{Motion, Temper};

/// Aim with this game's arrow gravity and drag, keeping horizontal launch
/// speed at 18 blocks/s. Continuous drag approximates the frame-based solver.
pub fn aimed_arrow_velocity(delta: Vec3) -> Vec3 {
    let horizontal = delta.with_y(0.0);
    let distance = horizontal.length();
    if distance < 0.001 {
        return delta.normalize_or_zero() * 18.0;
    }
    let drag_rate = -20.0 * 0.99_f32.ln();
    let time = -(1.0 - drag_rate * distance / 18.0).max(0.01).ln() / drag_rate;
    let travel = (1.0 - (-drag_rate * time).exp()) / drag_rate;
    let vertical = (delta.y + 25.0 / drag_rate * (time - travel)) / travel;
    horizontal / distance * 18.0 + Vec3::Y * (vertical + 25.0 / 120.0)
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
    pub id: u32,
    pub skeleton: crate::skeleton_ai::SkeletonState,
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
    pub walk_phase: f32,
    pub walk_blend: f32,
    pub anger_time: f32,
    pub swim_pitch: f32,
    pub animation: crate::combat_animation::MobAnimation,
}

impl Mob {
    pub fn new(kind: MobKind, position: Vec3, home: Vec3, variant: u8) -> Self {
        static NEXT_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        let health = Self::max_health_for(kind);
        Self {
            id: NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            skeleton: Default::default(),
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
            walk_phase: 0.0,
            walk_blend: 0.0,
            anger_time: 0.0,
            swim_pitch: 0.0,
            animation: Default::default(),
        }
    }

    fn max_health_for(kind: MobKind) -> f32 {
        kind.species().health
    }

    pub fn take_damage(&mut self, damage: f32) -> bool {
        if damage.is_finite() && damage > 0.0 && self.health > 0.0 {
            self.animation.hurt_remaining = crate::combat_animation::HURT_SECONDS;
        }
        self.health = (self.health - damage.max(0.0)).max(0.0);
        if damage > 0.0 && self.kind.species().temper == Temper::Neutral {
            self.anger_time = 15.0;
        }
        if self.is_animal() && self.health > 0.0 {
            // PanicGoal lasts 100 ticks after the last hit.
            self.animal.panic_time = 5.0;
            self.animal.dest = None;
            self.animal.eat_time = 0.0;
        }
        self.health <= 0.0
    }

    pub fn take_combat_damage(&mut self, damage: f32) -> bool {
        if !self.uses_skeleton_ai() || !damage.is_finite() || damage <= 0.0 {
            return self.take_damage(damage);
        }
        let defense: i32 = self
            .skeleton
            .armor
            .iter()
            .flatten()
            .filter_map(|stack| crate::item::armor_properties(stack.block))
            .map(|p| p.defense)
            .sum();
        for slot in &mut self.skeleton.armor {
            if let Some(stack) = slot
                && let Some(properties) = crate::item::armor_properties(stack.block)
            {
                let wear = (damage / 4.0).floor().max(1.0) as u16;
                let remaining = stack
                    .durability
                    .unwrap_or(properties.durability)
                    .saturating_sub(wear);
                stack.durability = Some(remaining);
                if remaining == 0 {
                    *slot = None;
                }
            }
        }
        self.take_damage(damage * (1.0 - (defense as f32 * 0.04).min(0.8)))
    }

    pub fn is_animal(&self) -> bool {
        self.kind.species().temper != Temper::Hostile
            && matches!(self.kind.species().motion, Motion::Walk | Motion::Hop)
            && !matches!(
                self.kind,
                MobKind::Villager
                    | MobKind::Golem
                    | MobKind::CopperGolem
                    | MobKind::SnowGolem
                    | MobKind::WanderingTrader
                    | MobKind::SulfurCube
                    | MobKind::ZombieHorse
                    | MobKind::SkeletonHorse
                    | MobKind::CamelHusk
                    | MobKind::Enderman
                    | MobKind::ZombifiedPiglin
            )
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
            _ => self.kind.species().speed / 10.75,
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
            MobKind::Cow | MobKind::Sheep | MobKind::Mooshroom | MobKind::Goat => {
                Some(BlockType::Wheat)
            }
            _ => None,
        }
    }

    pub fn is_hostile(&self) -> bool {
        self.kind.species().temper == Temper::Hostile || self.anger_time > 0.0
    }

    pub fn height(&self) -> f32 {
        self.kind.species().height * if self.is_baby() { 0.5 } else { 1.0 }
    }

    pub fn half_width(&self) -> f32 {
        self.kind.species().width * if self.is_baby() { 0.25 } else { 0.5 }
    }

    pub fn base_speed(&self) -> f32 {
        self.kind.species().speed
    }

    /// How far from home the mob is willing to wander
    pub fn leash_range(&self) -> f32 {
        match self.kind {
            MobKind::Villager => 22.0,
            MobKind::Golem => 30.0,
            _ if self.is_hostile() => 25.0,
            _ => 20.0,
        }
    }

    /// Drive strides from distance traveled, so blocked mobs do not moonwalk.
    pub fn animate_movement(&mut self, previous: Vec3, dt: f32) {
        if dt <= 0.0 {
            return;
        }
        let distance = (self.position - previous).with_y(0.0).length();
        self.walk_phase = (self.walk_phase
            + distance * 4.0 / if self.is_baby() { 0.5 } else { 1.0 })
            % std::f32::consts::TAU;
        let amount = (distance / dt / self.base_speed()).clamp(0.0, 1.0);
        if matches!(
            self.kind.species().motion,
            Motion::Swim | Motion::Amphibious
        ) {
            let movement = self.position - previous;
            let target = -movement.y.atan2(movement.with_y(0.0).length().max(0.01));
            self.swim_pitch += (target.clamp(-0.6, 0.6) - self.swim_pitch) * (dt * 5.0).min(1.0);
        }
        self.walk_blend += (amount - self.walk_blend) * (dt * 12.0).min(1.0);
    }

    pub fn is_ranged(&self) -> bool {
        if self.uses_skeleton_ai() {
            return !self.skeleton.melee;
        }
        matches!(
            self.kind,
            MobKind::Skeleton
                | MobKind::Stray
                | MobKind::Bogged
                | MobKind::Parched
                | MobKind::Pillager
        )
    }

    pub fn uses_skeleton_ai(&self) -> bool {
        self.kind == MobKind::Skeleton || (self.kind == MobKind::Stray && self.skeleton.converted)
    }

    /// Combat locomotion overrides wandering, including during reloads.
    pub fn track_combat_target(&mut self, target: Vec3, visible: bool) -> bool {
        let delta = target - self.position;
        let distance = delta.length();
        self.yaw = delta.z.atan2(delta.x);
        // Bedrock's skeleton uses a 15-block ranged goal. Other ranged mobs
        // retain their existing ranges until their own definitions are ported.
        let radius = if self.kind == MobKind::Skeleton {
            15.0
        } else {
            12.0
        };
        let firing_position = self.is_ranged() && distance <= radius && visible;
        self.walk_speed = if firing_position {
            0.0
        } else {
            self.base_speed()
        };
        firing_position && self.attack_cooldown <= 0.0
    }

    pub fn ranged_attack_interval(&self) -> f32 {
        // Normal difficulty in Mojang's skeleton behavior definition.
        if self.kind == MobKind::Skeleton {
            3.0
        } else {
            2.0
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
            MobKind::Mooshroom => Some((BlockType::RawBeef, 2)),
            MobKind::Stray | MobKind::Bogged | MobKind::Parched => Some((BlockType::Stick, 1)),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skeleton_holds_firing_position_even_while_reloading() {
        let mut mob = Mob::new(MobKind::Skeleton, Vec3::ZERO, Vec3::ZERO, 0);
        for distance in [0.1, 1.0, 8.0, 14.0, 15.0] {
            for cooldown in [0.0, 1.5] {
                mob.walk_speed = mob.base_speed();
                mob.attack_cooldown = cooldown;
                let fire = mob.track_combat_target(Vec3::Z * distance, true);
                assert_eq!(
                    mob.walk_speed, 0.0,
                    "distance {distance}, cooldown {cooldown}"
                );
                assert_eq!(fire, cooldown == 0.0);
                assert!((mob.yaw - std::f32::consts::FRAC_PI_2).abs() < 0.001);
            }
        }
    }

    #[test]
    fn skeleton_pursues_outside_range_or_without_a_clear_shot() {
        let mut mob = Mob::new(MobKind::Skeleton, Vec3::ZERO, Vec3::ZERO, 0);
        for (distance, visible) in [(15.1, true), (8.0, false)] {
            assert!(!mob.track_combat_target(Vec3::X * distance, visible));
            assert_eq!(mob.walk_speed, mob.base_speed());
        }
        assert!(mob.track_combat_target(Vec3::X * 8.0, true));
        assert_eq!(mob.walk_speed, 0.0);
    }

    #[test]
    fn melee_mobs_continue_closing_distance() {
        let mut mob = Mob::new(MobKind::Zombie, Vec3::ZERO, Vec3::ZERO, 0);
        assert!(!mob.track_combat_target(Vec3::X * 8.0, true));
        assert_eq!(mob.walk_speed, mob.base_speed());
    }

    #[test]
    fn skeleton_reload_uses_normal_bedrock_interval() {
        let mut mob = Mob::new(MobKind::Skeleton, Vec3::ZERO, Vec3::ZERO, 0);
        assert_eq!(mob.ranged_attack_interval(), 3.0);
        assert!(mob.track_combat_target(Vec3::X * 10.0, true));
        mob.attack_cooldown = mob.ranged_attack_interval();
        for _ in 0..29 {
            mob.attack_cooldown = (mob.attack_cooldown - 0.1).max(0.0);
            assert!(!mob.track_combat_target(Vec3::X * 10.0, true));
            assert_eq!(mob.walk_speed, 0.0);
        }
        mob.attack_cooldown = (mob.attack_cooldown - 0.101).max(0.0);
        assert!(mob.track_combat_target(Vec3::X * 10.0, true));
    }

    #[test]
    fn ranged_arrows_reach_player_height_at_firing_distances() {
        for fps in [30.0, 60.0, 144.0] {
            for distance in [3.0, 8.0, 15.0] {
                for elevation in [-2.0, 0.0, 2.0] {
                    let target = Vec3::new(distance, elevation, 0.0);
                    let mut velocity = aimed_arrow_velocity(target);
                    let mut position = Vec3::ZERO;
                    let dt = 1.0 / fps;
                    for _ in 0..300 {
                        let previous = position;
                        velocity.y -= 25.0 * dt;
                        velocity *= 0.99_f32.powf(dt * 20.0);
                        position += velocity * dt;
                        if position.x >= distance {
                            let crossing = previous.lerp(
                                position,
                                (distance - previous.x) / (position.x - previous.x),
                            );
                            assert!(
                                (crossing.y - elevation).abs() < 0.4,
                                "{fps} fps, {distance} blocks, elevation {elevation}: {crossing:?}"
                            );
                            break;
                        }
                    }
                    assert!(position.x >= distance);
                }
            }
        }
    }

    #[test]
    fn neutral_mobs_retaliate_but_passive_animals_flee() {
        let mut wolf = Mob::new(MobKind::Wolf, Vec3::ZERO, Vec3::ZERO, 0);
        assert!(!wolf.is_hostile());
        wolf.take_damage(2.0);
        assert!(wolf.is_hostile());
        let mut chicken = Mob::new(MobKind::Chicken, Vec3::ZERO, Vec3::ZERO, 0);
        chicken.take_damage(1.0);
        assert!(!chicken.is_hostile());
        assert!(chicken.animal.panic_time > 0.0);
    }

    #[test]
    fn strides_follow_distance_and_stop_when_blocked() {
        let mut mob = Mob::new(MobKind::Cow, Vec3::ZERO, Vec3::ZERO, 0);
        mob.walk_speed = 2.0;
        mob.animate_movement(Vec3::ZERO, 0.1);
        assert_eq!(mob.walk_phase, 0.0);
        assert_eq!(mob.walk_blend, 0.0);
        mob.position.x += 0.2;
        mob.animate_movement(Vec3::ZERO, 0.1);
        assert!(mob.walk_phase > 0.0 && mob.walk_blend > 0.8);
        let phase = mob.walk_phase;
        mob.animate_movement(mob.position, 0.1);
        assert_eq!(phase, mob.walk_phase);
        assert_eq!(mob.walk_blend, 0.0);
    }

    #[test]
    fn test_mob_dimensions_and_properties() {
        let pos = Vec3::ZERO;
        let home = Vec3::ZERO;

        let z = Mob::new(MobKind::Zombie, pos, home, 0);
        assert_eq!(z.height(), 1.95);
        assert_eq!(z.drop_item(), Some((BlockType::RawIron, 1)));

        let sk = Mob::new(MobKind::Skeleton, pos, home, 0);
        assert_eq!(sk.height(), 1.9);
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
