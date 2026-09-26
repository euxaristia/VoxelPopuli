use super::*;
use crate::skeleton_ai::TargetId;

/// `minecraft:nearest_attackable_target` acquires the monster family within
/// `within_default`, 10 blocks. `minecraft:follow_range` is 64 and governs how
/// far an already-acquired or retaliated target is pursued.
const MONSTER_ACQUIRE_DISTANCE: f32 = 10.0;
const FOLLOW_RANGE: f32 = 64.0;

fn monster(kind: MobKind) -> bool {
    kind != MobKind::Creeper
        && (kind.species().temper == crate::mob_catalog::Temper::Hostile
            || matches!(kind, MobKind::Enderman | MobKind::ZombifiedPiglin))
}

impl World {
    pub(super) fn tick_golem(
        &mut self,
        mob: &mut Mob,
        peers: &[skeletons::CreatureSnapshot],
        player: Vec3,
    ) -> Option<(u32, f32, u32)> {
        if mob.health <= 0.0 {
            return None;
        }
        let within = |pos: Vec3, radius: f32| mob.position.distance_squared(pos) <= radius * radius;
        let origin = mob.position + Vec3::Y * mob.height() * 0.75;
        let visible = |pos: Vec3, height: f32| {
            let delta = pos + Vec3::Y * height * 0.5 - origin;
            delta.length_squared() < 0.001 || !self.raycast(origin, delta, delta.length()).hit
        };
        let retained = match mob.golem_target {
            Some(TargetId::Player)
                if self.player_targetable
                    && self.difficulty != crate::skeleton_ai::Difficulty::Peaceful
                    && within(player, FOLLOW_RANGE) =>
            {
                Some((TargetId::Player, player, 1.8, 0.3))
            }
            Some(TargetId::Mob(id)) => peers
                .iter()
                .find(|p| {
                    p.id == id
                        && p.alive
                        && p.kind != MobKind::Creeper
                        && within(p.position, FOLLOW_RANGE)
                })
                .map(|p| (TargetId::Mob(p.id), p.position, p.height, p.half_width)),
            _ => None,
        };
        let target = retained.or_else(|| {
            peers
                .iter()
                .filter(|p| {
                    p.id != mob.id
                        && p.alive
                        && monster(p.kind)
                        && within(p.position, MONSTER_ACQUIRE_DISTANCE)
                })
                .filter(|p| visible(p.position, p.height))
                .min_by(|a, b| {
                    mob.position
                        .distance_squared(a.position)
                        .total_cmp(&mob.position.distance_squared(b.position))
                })
                .map(|p| (TargetId::Mob(p.id), p.position, p.height, p.half_width))
        });
        mob.golem_target = target.map(|t| t.0);
        let Some((id, pos, height, half_width)) = target else {
            if mob.walk_speed > 0.0 {
                let returning_home =
                    (mob.position - mob.home).with_y(0.0).length() > mob.leash_range();
                mob.walk_speed = mob.base_speed() * if returning_home { 1.0 } else { 0.6 };
            }
            return None;
        };
        let can_see = visible(pos, height);
        let delta = pos - mob.position;
        mob.yaw = delta.z.atan2(delta.x);
        let reach = mob.half_width() + 0.8 + half_width;
        let in_reach = delta.x.abs() <= reach
            && delta.z.abs() <= reach
            && pos.y < mob.position.y + mob.height()
            && pos.y + height > mob.position.y;
        mob.walk_speed = if in_reach { 0.0 } else { mob.base_speed() };
        if !in_reach || !can_see || mob.attack_cooldown > 0.0 {
            return None;
        }
        let damage = rand::random_range(7..=21);
        mob.attack_cooldown = 1.0;
        mob.animation.attack();
        match id {
            TargetId::Player => {
                self.pending_hurt += damage;
                self.pending_hurt_origin = Some(mob.position);
                None
            }
            TargetId::Mob(id) => Some((id, damage as f32, mob.id)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;

    fn arena() -> World {
        let mut world = World::simulation(42);
        world.day_time = 900.0;
        world.night_spawn_timer = 10000.0;
        let mut chunk = Chunk::new(0, 0, 42);
        for column in chunk.blocks.iter_mut() {
            column[60].fill(BlockType::OakPlanks);
        }
        let index = world.get_pool_index(0, 0);
        world.chunks[index] = Some(Box::new(chunk));
        world
    }

    fn spawn(world: &mut World, kind: MobKind, x: f32) -> u32 {
        let pos = Vec3::new(x, 61.0, 8.0);
        let mut mob = Mob::new(kind, pos, pos, 0);
        mob.grounded = true;
        mob.wander_timer = 100.0;
        mob.attack_cooldown = if kind == MobKind::Golem { 0.0 } else { 100.0 };
        let id = mob.id;
        world.mobs.push(mob);
        id
    }

    #[test]
    fn golem_attacks_monsters_without_player_anger() {
        for kind in [
            MobKind::Zombie,
            MobKind::Skeleton,
            MobKind::Enderman,
            MobKind::ZombifiedPiglin,
        ] {
            let mut world = arena();
            spawn(&mut world, MobKind::Golem, 8.0);
            spawn(&mut world, kind, 9.6);
            world.mobs[1].health = 100.0;
            let health = world.mobs[1].health;
            world.update_mobs(Vec3::new(7.0, 61.0, 8.0), 0.05, BlockType::Air);
            assert!(world.mobs[1].health < health, "{kind:?}");
            assert_eq!(world.pending_hurt, 0);
            assert_eq!(world.mobs[0].attack_cooldown, 1.0);
            let health = world.mobs[1].health;
            world.update_mobs(Vec3::new(7.0, 61.0, 8.0), 0.05, BlockType::Air);
            assert_eq!(world.mobs[1].health, health);
        }
    }

    #[test]
    fn golem_environmental_damage_never_blames_player() {
        let mut world = arena();
        spawn(&mut world, MobKind::Golem, 8.0);
        world.mobs[0].take_damage(1.0);
        world.update_mobs(Vec3::new(9.5, 61.0, 8.0), 0.05, BlockType::Air);
        assert_eq!(world.pending_hurt, 0);
    }

    #[test]
    fn golem_resists_melee_knockback_but_retaliates_against_player() {
        let mut world = arena();
        spawn(&mut world, MobKind::Golem, 8.0);
        let player = Vec3::new(9.5, 61.0, 8.0);
        assert!(world.try_melee(player + Vec3::Y * 1.45, -Vec3::X, 1.0, 3.0));
        assert_eq!(world.mobs[0].velocity, Vec3::ZERO);
        world.update_mobs(player, 0.05, BlockType::Air);
        assert!((7..=21).contains(&world.pending_hurt));
    }

    #[test]
    fn golem_survives_distance_despawn_and_unloaded_chunks() {
        let mut world = arena();
        let id = spawn(&mut world, MobKind::Golem, 8.0);
        let pos = world.mobs[0].position;
        world.chunks.iter_mut().for_each(|c| *c = None);
        world.update_mobs(Vec3::new(500.0, 61.0, 8.0), 0.05, BlockType::Air);
        assert_eq!(world.mobs.len(), 1);
        assert_eq!(world.mobs[0].id, id);
        assert_eq!(world.mobs[0].position, pos);
    }

    #[test]
    fn golem_ingot_repair_uses_existing_consumption_contract() {
        let mut world = arena();
        spawn(&mut world, MobKind::Golem, 8.0);
        let eye = Vec3::new(10.0, 62.45, 8.0);
        assert!(!world.try_feed_animal(eye, -Vec3::X, BlockType::IronIngot));
        world.mobs[0].health = 60.0;
        assert!(!world.try_feed_animal(eye, -Vec3::X, BlockType::Wheat));
        assert!(world.try_feed_animal(eye, -Vec3::X, BlockType::IronIngot));
        assert_eq!(world.mobs[0].health, 85.0);
        assert!(world.try_feed_animal(eye, -Vec3::X, BlockType::IronIngot));
        assert_eq!(world.mobs[0].health, 100.0);
        assert_eq!(world.mobs[0].animal.love_time, 0.0);
        world.mobs[0].health = 0.0;
        assert!(!world.try_feed_animal(eye, -Vec3::X, BlockType::IronIngot));
    }

    #[test]
    fn golem_loot_has_variable_ingots_poppies_and_no_xp() {
        let mut world = arena();
        let mut ingots = [false; 3];
        let mut flowers = [false; 3];
        for _ in 0..256 {
            let mob = Mob::new(MobKind::Golem, Vec3::ZERO, Vec3::ZERO, 0);
            world.collect_mob_drops(&mob);
            let count = world
                .pending_drops
                .iter()
                .find(|d| d.0 == BlockType::IronIngot)
                .unwrap()
                .1;
            assert!((3..=5).contains(&count));
            ingots[(count - 3) as usize] = true;
            let poppies = world
                .pending_drops
                .iter()
                .find(|d| d.0 == BlockType::Poppy)
                .map_or(0, |d| d.1);
            assert!(poppies <= 2);
            flowers[poppies as usize] = true;
            world.pending_drops.clear();
        }
        assert_eq!(ingots, [true; 3]);
        assert_eq!(flowers, [true; 3]);
        assert!(world.xp_orbs.is_empty());
    }

    #[test]
    fn golem_ignores_creepers_animals_and_untargetable_players() {
        for kind in [
            MobKind::Creeper,
            MobKind::Villager,
            MobKind::Cow,
            MobKind::Wolf,
        ] {
            let mut world = arena();
            spawn(&mut world, MobKind::Golem, 8.0);
            spawn(&mut world, kind, 9.6);
            let health = world.mobs[1].health;
            world.update_mobs(Vec3::new(7.0, 61.0, 8.0), 0.05, BlockType::Air);
            assert_eq!(world.mobs[1].health, health);
            assert!(world.mobs[0].golem_target.is_none());
        }
        let mut world = arena();
        spawn(&mut world, MobKind::Golem, 8.0);
        world.mobs[0].record_attacker(crate::skeleton_ai::TargetId::Player, None, 1.0);
        world.player_targetable = false;
        world.update_mobs(Vec3::new(9.0, 61.0, 8.0), 0.05, BlockType::Air);
        assert_eq!(world.pending_hurt, 0);
        assert!(world.mobs[0].golem_target.is_none());
    }

    #[test]
    fn golem_retaliates_against_actual_melee_attacker_not_bystander() {
        let mut world = arena();
        spawn(&mut world, MobKind::Golem, 8.0);
        let zombie = spawn(&mut world, MobKind::Zombie, 9.6);
        world.mobs[0].attack_cooldown = 100.0;
        world.mobs[1].attack_cooldown = 0.0;
        let player = Vec3::new(7.0, 61.0, 8.0);
        world.update_mobs(player, 0.05, BlockType::Air);
        assert!(world.mobs[0].health < 100.0);
        assert_eq!(
            world.mobs[0].golem_target,
            Some(crate::skeleton_ai::TargetId::Mob(zombie))
        );
        world.mobs[0].attack_cooldown = 0.0;
        world.update_mobs(player, 0.05, BlockType::Air);
        assert_eq!(world.pending_hurt, 0);
        assert_eq!(world.mobs[0].attack_cooldown, 1.0);
    }

    #[test]
    fn golem_arrow_retaliation_tracks_owner_and_clears_missing_owner() {
        let mut world = arena();
        spawn(&mut world, MobKind::Golem, 8.0);
        let owner = spawn(&mut world, MobKind::Skeleton, 12.0);
        world.arrows.push(crate::block::ArrowEntity {
            position: Vec3::new(8.9, 62.0, 8.0),
            velocity: -Vec3::X * 18.0,
            life: 8.0,
            in_ground: false,
            damage: 2.0,
            is_critical: false,
            from_player: false,
            owner: Some(owner),
        });
        world.update_arrows(Vec3::new(7.0, 61.0, 8.0), 0.05);
        assert_eq!(world.mobs[0].health, 98.0);
        assert_eq!(
            world.mobs[0].golem_target,
            Some(crate::skeleton_ai::TargetId::Mob(owner))
        );
        world.mobs.pop();
        world.update_mobs(Vec3::new(7.0, 61.0, 8.0), 0.05, BlockType::Air);
        assert_eq!(world.pending_hurt, 0);
        assert!(world.mobs[0].golem_target.is_none());
    }

    #[test]
    fn golem_repair_and_attacks_cannot_pass_through_wall() {
        let mut world = arena();
        spawn(&mut world, MobKind::Golem, 8.0);
        spawn(&mut world, MobKind::Zombie, 10.5);
        world.mobs[0].health = 60.0;
        for y in 61..65 {
            world.get_chunk_mut(0, 0).unwrap().blocks[9][y][8] = BlockType::Stone;
        }
        assert!(!world.try_feed_animal(
            Vec3::new(10.0, 62.45, 8.0),
            -Vec3::X,
            BlockType::IronIngot
        ));
        world.update_mobs(Vec3::new(7.0, 61.0, 8.0), 0.05, BlockType::Air);
        assert!(world.mobs[0].golem_target.is_none());
        assert_eq!(world.mobs[1].health, 20.0);
    }

    #[test]
    fn golem_explosion_damage_does_not_knock_back_or_provoke() {
        let mut world = arena();
        spawn(&mut world, MobKind::Golem, 8.0);
        crate::explosion::explode(&mut world, 5, 62, 8, 2, Vec3::ZERO);
        assert!(world.mobs[0].health < 100.0);
        assert_eq!(world.mobs[0].velocity, Vec3::ZERO);
        assert!(world.mobs[0].grounded);
        assert!(world.mobs[0].golem_target.is_none());
    }

    #[test]
    fn golem_acquires_monsters_within_ten_blocks_but_follows_farther() {
        // nearest_attackable_target uses within_default = 10 for the monster family.
        let mut world = arena();
        spawn(&mut world, MobKind::Golem, 8.0);
        spawn(&mut world, MobKind::Zombie, 8.0 + 10.5);
        world.update_mobs(Vec3::new(7.0, 61.0, 8.0), 0.05, BlockType::Air);
        assert!(
            world.mobs[0].golem_target.is_none(),
            "golem acquired a monster beyond 10 blocks"
        );
        assert_eq!(world.mobs[1].health, 20.0);

        world.mobs[1].position = Vec3::new(8.0 + 9.5, 61.0, 8.0);
        world.update_mobs(Vec3::new(7.0, 61.0, 8.0), 0.05, BlockType::Air);
        assert_eq!(
            world.mobs[0].golem_target,
            Some(crate::skeleton_ai::TargetId::Mob(world.mobs[1].id))
        );
    }

    #[test]
    fn golem_retaliation_ignores_the_ten_block_acquisition_radius() {
        // hurt_by_target has no radius; follow_range = 64 is what bounds pursuit.
        let mut world = arena();
        spawn(&mut world, MobKind::Golem, 8.0);
        let shooter = spawn(&mut world, MobKind::Skeleton, 8.0 + 40.0);
        world.mobs[0].record_attacker(
            crate::skeleton_ai::TargetId::Mob(shooter),
            Some(MobKind::Skeleton),
            1.0,
        );
        world.update_mobs(Vec3::new(7.0, 61.0, 8.0), 0.05, BlockType::Air);
        assert_eq!(
            world.mobs[0].golem_target,
            Some(crate::skeleton_ai::TargetId::Mob(shooter))
        );
    }

    #[test]
    fn golem_attack_box_checks_horizontal_and_vertical_overlap() {
        let mut world = arena();
        let mut golem = Mob::new(MobKind::Golem, Vec3::new(8.0, 61.0, 8.0), Vec3::ZERO, 0);
        for (offset, attacks) in [
            (Vec3::new(1.7, 0.0, 1.7), true),
            (Vec3::X * 1.81, false),
            (Vec3::Y * 3.0, false),
        ] {
            let target = Mob::new(MobKind::Zombie, golem.position + offset, Vec3::ZERO, 0);
            golem.attack_cooldown = 0.0;
            golem.golem_target = None;
            assert_eq!(
                world
                    .tick_golem(&mut golem, &[(&target).into()], Vec3::ZERO)
                    .is_some(),
                attacks
            );
        }
    }
}
