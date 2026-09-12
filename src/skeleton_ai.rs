//! Bedrock skeleton goals. Published settings and engine approximations are
//! distinguished in docs/skeleton-combat.md. No renderer or world allocation.
use crate::{block::BlockType, inventory::ItemStack, mob::MobKind};
use glam::{IVec3, Vec3};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum Difficulty {
    Peaceful,
    Easy,
    #[default]
    Normal,
    Hard,
}

impl Difficulty {
    pub const ALL: [Self; 4] = [Self::Peaceful, Self::Easy, Self::Normal, Self::Hard];
    pub fn from_byte(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Peaceful),
            1 => Some(Self::Easy),
            2 => Some(Self::Normal),
            3 => Some(Self::Hard),
            _ => None,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Peaceful => "Peaceful",
            Self::Easy => "Easy",
            Self::Normal => "Normal",
            Self::Hard => "Hard",
        }
    }
    pub fn shot_interval(self) -> f32 {
        if self == Self::Hard { 2.0 } else { 3.0 }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TargetId {
    Player,
    Mob(u32),
}

#[derive(Clone, Copy, Debug)]
pub struct Target {
    pub id: TargetId,
    pub position: Vec3,
    pub height: f32,
    pub half_width: f32,
    pub visible: bool,
    pub eligible: bool,
}

#[derive(Clone, Debug)]
pub struct SkeletonState {
    pub target: Option<TargetId>,
    pub attacker: Option<TargetId>,
    pub unseen: f32,
    pub seen: f32,
    pub last_seen: Vec3,
    pub melee: bool,
    pub weapon: Option<ItemStack>,
    pub armor: [Option<ItemStack>; 4],
    pub picked_up: [bool; 5],
    pub persistent: bool,
    pub powder_time: f32,
    pub fire_time: f32,
    pub damage_timer: f32,
    pub inactive_time: f32,
    pub path: Vec<Vec3>,
    pub path_goal: Option<Vec3>,
    pub path_timer: f32,
    pub wolf: Option<u32>,
    pub goal: Goal,
    pub wander_goal: Option<Vec3>,
    pub look_time: f32,
    pub sight_ready: bool,
    pub converted: bool,
    pub looking_at_player: bool,
    pub shelter_retry: f32,
}

impl Default for SkeletonState {
    fn default() -> Self {
        Self {
            target: None,
            attacker: None,
            unseen: 0.0,
            seen: 0.0,
            last_seen: Vec3::ZERO,
            melee: false,
            weapon: Some(ItemStack::new(BlockType::Bow, 1)),
            armor: [None; 4],
            picked_up: [false; 5],
            persistent: false,
            powder_time: 0.0,
            fire_time: 0.0,
            damage_timer: 0.0,
            inactive_time: 0.0,
            path: Vec::new(),
            path_goal: None,
            path_timer: 0.0,
            wolf: None,
            goal: Goal::Idle,
            wander_goal: None,
            look_time: 0.0,
            sight_ready: false,
            converted: false,
            looking_at_player: false,
            shelter_retry: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Goal {
    Ranged,
    FleeSun,
    AvoidWolf,
    Melee,
    Pickup,
    Wander,
    #[default]
    Idle,
}

impl SkeletonState {
    pub fn hurt_by(&mut self, id: TargetId, kind: Option<MobKind>) {
        if kind != Some(MobKind::Breeze) {
            self.attacker = Some(id);
        }
    }

    pub fn environment(&mut self, underwater: bool, in_water: bool, powder: bool, dt: f32) -> bool {
        let bow = self.weapon.is_some_and(|s| s.block == BlockType::Bow);
        if underwater || !bow {
            self.melee = true;
        } else if !in_water {
            self.melee = false;
        }
        self.powder_time = if powder {
            (self.powder_time + dt).min(20.0)
        } else {
            0.0
        };
        self.powder_time >= 20.0
    }

    pub fn select_target(&mut self, position: Vec3, targets: &[Target], dt: f32) -> Option<Target> {
        let in_range = |t: &&Target| position.distance_squared(t.position) <= 256.0;
        let previous = self.target;
        let retaliation = self
            .attacker
            .and_then(|id| targets.iter().filter(in_range).find(|t| t.id == id));
        if retaliation.is_none() {
            self.attacker = None;
        }
        let nearest = targets
            .iter()
            .filter(in_range)
            .filter(|t| t.visible && t.eligible)
            .min_by(|a, b| {
                position
                    .distance_squared(a.position)
                    .total_cmp(&position.distance_squared(b.position))
            });
        let current = self
            .target
            .and_then(|id| targets.iter().filter(in_range).find(|t| t.id == id));
        let selected = retaliation.or(nearest).or(current).copied();
        let Some(target) = selected else {
            self.clear_target();
            return None;
        };
        if previous != Some(target.id) {
            self.unseen = 0.0;
            self.seen = 0.0;
            self.path_timer = 0.0;
            self.sight_ready = false;
            self.last_seen = target.position;
        }
        if target.visible {
            self.unseen = 0.0;
            self.seen += dt;
            self.sight_ready |= self.seen >= 1.0;
            self.last_seen = target.position;
        } else {
            self.unseen += dt;
            self.seen = 0.0;
            if self.unseen >= 3.0 {
                self.clear_target();
                return None;
            }
        }
        self.target = Some(target.id);
        Some(target)
    }

    fn clear_target(&mut self) {
        self.target = None;
        self.attacker = None;
        self.seen = 0.0;
        self.unseen = 0.0;
        self.path.clear();
        self.path_goal = None;
        self.sight_ready = false;
    }

    pub fn choose_goal(
        &self,
        target: bool,
        sun_shelter: bool,
        wolf: bool,
        pickup: bool,
        wander: bool,
    ) -> Goal {
        if target && !self.melee {
            Goal::Ranged
        } else if sun_shelter {
            Goal::FleeSun
        } else if wolf {
            Goal::AvoidWolf
        } else if target {
            Goal::Melee
        } else if pickup {
            Goal::Pickup
        } else if wander {
            Goal::Wander
        } else {
            Goal::Idle
        }
    }

    pub fn equipment_slot(&self, stack: ItemStack) -> Option<usize> {
        let slot = crate::item::armor_properties(stack.block)
            .map(|p| p.slot as usize + 1)
            .or_else(|| weapon_rank(stack.block).map(|_| 0))?;
        let current = if slot == 0 {
            self.weapon
        } else {
            self.armor[slot - 1]
        };
        let better = current.is_none_or(|old| {
            (if slot == 0 {
                weapon_rank(stack.block) < weapon_rank(old.block)
            } else {
                armor_rank(stack.block) < armor_rank(old.block)
            }) || (old.block == stack.block
                && stack.durability.unwrap_or(u16::MAX) > old.durability.unwrap_or(u16::MAX))
        });
        better.then_some(slot)
    }

    pub fn equip(&mut self, stack: ItemStack) -> Option<Option<ItemStack>> {
        let slot = self.equipment_slot(stack)?;
        let destination = if slot == 0 {
            &mut self.weapon
        } else {
            &mut self.armor[slot - 1]
        };
        let old = destination.replace(stack.with_count(1));
        self.picked_up[slot] = true;
        self.persistent = true;
        Some(old)
    }

    pub fn should_despawn(&mut self, distance: f32, dt: f32, random: f32) -> bool {
        if self.persistent {
            return false;
        }
        if distance > 128.0 {
            return true;
        }
        if distance <= 32.0 {
            self.inactive_time = 0.0;
            return false;
        }
        self.inactive_time += dt;
        self.inactive_time >= 30.0 && random < 1.0 - (799.0_f32 / 800.0).powf(dt * 20.0)
    }
}

fn armor_rank(block: BlockType) -> u8 {
    use BlockType::*;
    match block {
        DiamondHelmet | DiamondChestplate | DiamondLeggings | DiamondBoots => 1,
        IronHelmet | IronChestplate | IronLeggings | IronBoots => 2,
        GoldHelmet | GoldChestplate | GoldLeggings | GoldBoots => 4,
        _ => 6,
    }
}

fn weapon_rank(block: BlockType) -> Option<u8> {
    use BlockType::*;
    Some(match block {
        DiamondSword => 1,
        IronSword => 2,
        GoldSword => 3,
        StoneSword => 5,
        WoodSword => 6,
        Bow => 7,
        _ if block != Air => 8,
        _ => return None,
    })
}

pub fn melee_reaches(position: Vec3, target: Target) -> bool {
    let delta = target.position - position;
    delta.x.abs() <= 0.3 + 0.8 + target.half_width
        && delta.z.abs() <= 0.3 + 0.8 + target.half_width
        && delta.y < 1.9
        && delta.y + target.height > 0.0
}

/// Bounded walk navigation with one-block ascents and safe drops. The public
/// navigation component does not expose Bedrock's path-search implementation.
pub fn find_path(
    start: IVec3,
    goal: impl Fn(IVec3) -> bool,
    standable: impl Fn(IVec3) -> bool,
) -> Vec<Vec3> {
    let mut queue = VecDeque::from([start]);
    let mut parents = HashMap::from([(start, start)]);
    while let Some(node) = queue.pop_front() {
        if node != start && goal(node) {
            let mut route = Vec::new();
            let mut cursor = node;
            while cursor != start {
                route.push(cursor.as_vec3() + Vec3::new(0.5, 0.0, 0.5));
                cursor = parents[&cursor];
            }
            route.reverse();
            return route;
        }
        if parents.len() >= 768 {
            break;
        }
        for direction in [IVec3::X, -IVec3::X, IVec3::Z, -IVec3::Z] {
            for dy in [0, 1, -1, -2, -3] {
                let next = node + direction + IVec3::Y * dy;
                if (next - start).abs().max_element() > 16 || !standable(next) {
                    continue;
                }
                if let std::collections::hash_map::Entry::Vacant(entry) = parents.entry(next) {
                    entry.insert(node);
                    queue.push_back(next);
                }
                break;
            }
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn target(id: TargetId, distance: f32, visible: bool) -> Target {
        Target {
            id,
            position: Vec3::X * distance,
            height: 1.8,
            half_width: 0.3,
            visible,
            eligible: true,
        }
    }

    #[test]
    fn skeleton_requires_sight_and_forgets_after_three_seconds() {
        let mut state = SkeletonState::default();
        let mut player = target(TargetId::Player, 8.0, false);
        assert!(state.select_target(Vec3::ZERO, &[player], 0.1).is_none());
        player.visible = true;
        assert_eq!(
            state.select_target(Vec3::ZERO, &[player], 0.1).unwrap().id,
            TargetId::Player
        );
        player.visible = false;
        player.position.x = 12.0;
        assert!(state.select_target(Vec3::ZERO, &[player], 2.9).is_some());
        assert_eq!(state.last_seen.x, 8.0);
        assert!(state.select_target(Vec3::ZERO, &[player], 0.11).is_none());
    }

    #[test]
    fn skeleton_reselects_nearest_but_retaliation_has_priority() {
        let mut state = SkeletonState::default();
        let player = target(TargetId::Player, 8.0, true);
        let mut golem = target(TargetId::Mob(10), 4.0, true);
        assert_eq!(
            state
                .select_target(Vec3::ZERO, &[player, golem], 0.1)
                .unwrap()
                .id,
            golem.id
        );
        golem.position.x = 12.0;
        assert_eq!(
            state
                .select_target(Vec3::ZERO, &[player, golem], 0.1)
                .unwrap()
                .id,
            player.id
        );
        state.hurt_by(golem.id, Some(MobKind::Golem));
        assert_eq!(
            state
                .select_target(Vec3::ZERO, &[player, golem], 0.1)
                .unwrap()
                .id,
            golem.id
        );
        state.hurt_by(player.id, Some(MobKind::Breeze));
        assert_eq!(state.attacker, Some(golem.id));
        assert_eq!(
            state.select_target(Vec3::ZERO, &[player], 0.1).unwrap().id,
            player.id
        );
    }

    #[test]
    fn skeleton_loses_targets_outside_detection_range() {
        let mut state = SkeletonState::default();
        assert!(
            state
                .select_target(Vec3::ZERO, &[target(TargetId::Player, 16.0, true)], 0.1)
                .is_some()
        );
        assert!(
            state
                .select_target(Vec3::ZERO, &[target(TargetId::Player, 16.1, true)], 0.1)
                .is_none()
        );
    }

    #[test]
    fn skeleton_water_mode_has_exit_hysteresis_and_requires_a_bow() {
        let mut state = SkeletonState::default();
        state.environment(false, true, false, 0.1);
        assert!(!state.melee);
        state.environment(true, true, false, 0.1);
        assert!(state.melee);
        state.environment(false, true, false, 0.1);
        assert!(state.melee);
        state.environment(false, false, false, 0.1);
        assert!(!state.melee);
        state.weapon = None;
        state.environment(false, false, false, 0.1);
        assert!(state.melee);
    }

    #[test]
    fn skeleton_powder_conversion_requires_twenty_uninterrupted_seconds() {
        let mut state = SkeletonState::default();
        assert!(!state.environment(false, false, true, 19.9));
        assert!(!state.environment(false, false, false, 0.1));
        assert!(!state.environment(false, false, true, 19.9));
        assert!(state.environment(false, false, true, 0.11));
    }

    #[test]
    fn skeleton_goal_priorities_do_not_let_wandering_override_combat() {
        let mut state = SkeletonState::default();
        assert_eq!(
            state.choose_goal(true, true, true, true, true),
            Goal::Ranged
        );
        state.melee = true;
        assert_eq!(
            state.choose_goal(true, true, true, true, true),
            Goal::FleeSun
        );
        assert_eq!(
            state.choose_goal(true, false, true, true, true),
            Goal::AvoidWolf
        );
        assert_eq!(
            state.choose_goal(true, false, false, true, true),
            Goal::Melee
        );
        assert_eq!(
            state.choose_goal(false, false, false, true, true),
            Goal::Pickup
        );
    }

    #[test]
    fn skeleton_equips_one_item_and_keeps_better_gear() {
        let mut state = SkeletonState::default();
        let old = state
            .equip(ItemStack::new(BlockType::IronSword, 3))
            .unwrap()
            .unwrap();
        assert_eq!(old.block, BlockType::Bow);
        assert_eq!(state.weapon.unwrap().count, 1);
        assert!(state.persistent && state.picked_up[0]);
        assert!(
            state
                .equip(ItemStack::new(BlockType::WoodSword, 1))
                .is_none()
        );
        state
            .equip(ItemStack::new(BlockType::GoldHelmet, 1))
            .unwrap();
        assert!(
            state
                .equip(ItemStack::new(BlockType::IronHelmet, 1))
                .is_some()
        );
    }

    #[test]
    fn skeleton_melee_uses_boxes_including_vertical_overlap() {
        let mut player = target(TargetId::Player, 1.39, true);
        assert!(melee_reaches(Vec3::ZERO, player));
        player.position.y = 2.0;
        assert!(!melee_reaches(Vec3::ZERO, player));
        player.position = Vec3::X * 1.41;
        assert!(!melee_reaches(Vec3::ZERO, player));
    }

    #[test]
    fn skeleton_navigation_detours_around_walls_and_avoids_cliffs() {
        let goal = IVec3::new(4, 1, 0);
        let path = find_path(
            IVec3::Y,
            |p| p == goal,
            |p| p.y == 1 && !(p.x == 2 && p.z.abs() < 2),
        );
        assert_eq!(path.last().unwrap().floor().as_ivec3(), goal);
        assert!(path.iter().any(|p| p.z.abs() >= 2.0));
        assert!(find_path(IVec3::Y, |p| p == goal, |_| false).is_empty());
    }

    #[test]
    fn skeleton_uses_documented_dimensions() {
        let mob = crate::mob::Mob::new(MobKind::Skeleton, Vec3::ZERO, Vec3::ZERO, 0);
        assert_eq!(mob.height(), 1.9);
        assert_eq!(mob.half_width(), 0.3);
        assert_eq!(mob.health, 20.0);
    }

    #[test]
    fn skeleton_despawn_respects_distance_inactivity_and_picked_up_gear() {
        let mut state = SkeletonState::default();
        assert!(!state.should_despawn(40.0, 29.0, 0.0));
        assert!(state.should_despawn(40.0, 1.1, 0.0));
        assert!(!state.should_despawn(32.0, 0.1, 0.0));
        assert_eq!(state.inactive_time, 0.0);
        assert!(state.should_despawn(128.1, 0.1, 1.0));
        state.persistent = true;
        assert!(!state.should_despawn(150.0, 60.0, 0.0));
    }

    #[test]
    fn skeleton_armor_reduces_hits_and_breaks_after_use() {
        let mut mob = crate::mob::Mob::new(MobKind::Skeleton, Vec3::ZERO, Vec3::ZERO, 0);
        mob.skeleton.armor[1] = Some(ItemStack {
            block: BlockType::DiamondChestplate,
            count: 1,
            durability: Some(1),
        });
        mob.take_combat_damage(5.0);
        assert!(mob.health > 15.0);
        assert!(mob.skeleton.armor[1].is_none());
    }
}
