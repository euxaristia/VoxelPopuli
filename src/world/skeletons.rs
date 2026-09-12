use super::*;
use crate::skeleton_ai::{self, Goal, Target, TargetId};

#[derive(Clone, Copy)]
pub(super) struct CreatureSnapshot {
    pub id: u32,
    pub kind: MobKind,
    pub position: Vec3,
    pub height: f32,
    pub half_width: f32,
    pub baby: bool,
    pub alive: bool,
}

impl From<&Mob> for CreatureSnapshot {
    fn from(m: &Mob) -> Self {
        Self {
            id: m.id,
            kind: m.kind,
            position: m.position,
            height: m.height(),
            half_width: m.half_width(),
            baby: m.is_baby(),
            alive: m.health > 0.0,
        }
    }
}

impl World {
    fn skeleton_sunlight(&self, feet: Vec3) -> bool {
        if self.day_time >= 600.0 {
            return false;
        }
        let p = (feet + Vec3::Y * 1.9).floor().as_ivec3();
        (p.y..CHUNK_HEIGHT as i32).all(|y| !self.get_block(p.x, y, p.z).is_solid())
    }

    fn skeleton_sees(&self, from: Vec3, to: Vec3) -> bool {
        let delta = to - from;
        delta.length_squared() < 0.001 || !self.raycast(from, delta, delta.length()).hit
    }

    fn skeleton_path(&self, mob: &mut Mob, destination: Vec3, shelter: bool, dt: f32) {
        mob.skeleton.path_timer -= dt;
        let changed = mob
            .skeleton
            .path_goal
            .is_none_or(|p| p.distance_squared(destination) > 2.25);
        if changed || mob.skeleton.path_timer <= 0.0 {
            let start = mob.position.floor().as_ivec3();
            let goal = destination.floor().as_ivec3();
            let avoid_sun = !self.skeleton_sunlight(mob.position);
            let allow_water =
                shelter || self.get_block(start.x, start.y, start.z) == BlockType::Water;
            mob.skeleton.path = skeleton_ai::find_path(
                start,
                |p| {
                    if shelter {
                        !self.skeleton_sunlight(p.as_vec3())
                            || self.get_block(p.x, p.y, p.z) == BlockType::Water
                    } else {
                        (p - goal).as_vec3().length_squared() <= 2.0
                    }
                },
                |p| {
                    let pos = p.as_vec3() + Vec3::new(0.5, 0.0, 0.5);
                    let floor = self.get_block(p.x, p.y - 1, p.z);
                    (1..CHUNK_HEIGHT as i32 - 2).contains(&p.y)
                        && self
                            .get_chunk(
                                p.x.div_euclid(CHUNK_WIDTH as i32),
                                p.z.div_euclid(CHUNK_DEPTH as i32),
                            )
                            .is_some()
                        && floor.is_solid()
                        && floor != BlockType::Lava
                        && (allow_water || self.get_block(p.x, p.y, p.z) != BlockType::Water)
                        && self.get_block(p.x, p.y, p.z) != BlockType::Lava
                        && !self.mob_box_blocked(pos, mob.half_width(), mob.height())
                        && (!avoid_sun || !self.skeleton_sunlight(pos))
                },
            );
            mob.skeleton.path_goal = Some(destination);
            mob.skeleton.path_timer = 0.5;
        }
        while mob
            .skeleton
            .path
            .first()
            .is_some_and(|p| p.with_y(0.0).distance_squared(mob.position.with_y(0.0)) < 0.09)
        {
            mob.skeleton.path.remove(0);
        }
        if let Some(next) = mob.skeleton.path.first() {
            let direction = *next - mob.position;
            mob.yaw = direction.z.atan2(direction.x);
        } else {
            mob.walk_speed = 0.0;
        }
    }

    pub(super) fn tick_skeleton(
        &mut self,
        mob: &mut Mob,
        peers: &[CreatureSnapshot],
        player: Vec3,
        dt: f32,
    ) -> Option<(u32, f32, u32)> {
        let block_at = |p: Vec3| {
            let p = p.floor().as_ivec3();
            self.get_block(p.x, p.y, p.z)
        };
        let in_water = block_at(mob.position + Vec3::Y * 0.05) == BlockType::Water;
        let underwater = block_at(mob.position + Vec3::Y * 1.7) == BlockType::Water;
        let powder = block_at(mob.position + Vec3::Y * 0.5) == BlockType::PowderedSnow;
        if mob.skeleton.environment(underwater, in_water, powder, dt)
            && mob.kind == MobKind::Skeleton
        {
            mob.kind = MobKind::Stray;
            mob.skeleton.converted = true;
            mob.skeleton.powder_time = 0.0;
            return None;
        }
        let sun = self.skeleton_sunlight(mob.position);
        if in_water {
            mob.skeleton.fire_time = 0.0;
        } else if sun && mob.skeleton.armor[0].is_none() {
            mob.skeleton.fire_time = 8.0;
        } else {
            mob.skeleton.fire_time = (mob.skeleton.fire_time - dt).max(0.0);
        }
        mob.skeleton.damage_timer += dt;
        if mob.skeleton.damage_timer >= 1.0 {
            mob.skeleton.damage_timer -= 1.0;
            if sun && let Some(helmet) = &mut mob.skeleton.armor[0] {
                let maximum = crate::item::armor_properties(helmet.block)
                    .unwrap()
                    .durability;
                let remaining = helmet.durability.unwrap_or(maximum).saturating_sub(1);
                helmet.durability = Some(remaining);
                if remaining == 0 {
                    mob.skeleton.armor[0] = None;
                }
            }
            if mob.skeleton.fire_time > 0.0 {
                mob.take_damage(1.0);
            }
            if block_at(mob.position + Vec3::Y * 0.1) == BlockType::Lava {
                mob.take_damage(4.0);
            }
        }
        if mob.health <= 0.0 {
            return None;
        }

        let eye = mob.position + Vec3::Y * 1.7;
        let mut targets = vec![Target {
            id: TargetId::Player,
            position: player,
            height: 1.8,
            half_width: 0.3,
            visible: mob.position.distance_squared(player) <= 256.0
                && self.skeleton_sees(eye, player + Vec3::Y * 1.2),
            eligible: self.player_targetable
                && (!self.player_sneaking
                    || mob.position.distance_squared(player) <= (16.0_f32 * 0.8).powi(2)),
        }];
        for peer in peers.iter().filter(|p| p.alive && p.id != mob.id) {
            let eligible = peer.kind == MobKind::Golem
                || (peer.kind == MobKind::Turtle
                    && peer.baby
                    && block_at(peer.position + Vec3::Y * 0.05) != BlockType::Water);
            if eligible
                || mob.skeleton.attacker == Some(TargetId::Mob(peer.id))
                || mob.skeleton.target == Some(TargetId::Mob(peer.id))
            {
                targets.push(Target {
                    id: TargetId::Mob(peer.id),
                    position: peer.position,
                    height: peer.height,
                    half_width: peer.half_width,
                    visible: mob.position.distance_squared(peer.position) <= 256.0
                        && self.skeleton_sees(eye, peer.position + Vec3::Y * peer.height * 0.7),
                    eligible,
                });
            }
        }
        // Creative/dead players cannot be retained as retaliation targets either.
        if !self.player_targetable {
            targets.retain(|t| t.id != TargetId::Player);
        }
        let target = mob.skeleton.select_target(mob.position, &targets, dt);
        let wolf = peers
            .iter()
            .filter(|p| p.alive && p.kind == MobKind::Wolf)
            .filter(|p| {
                p.position.distance_squared(mob.position)
                    < if mob.skeleton.wolf == Some(p.id) {
                        100.0
                    } else {
                        36.0
                    }
            })
            .filter(|p| self.skeleton_sees(eye, p.position + Vec3::Y * p.height * 0.5))
            .min_by(|a, b| {
                a.position
                    .distance_squared(mob.position)
                    .total_cmp(&b.position.distance_squared(mob.position))
            });
        mob.skeleton.wolf = wolf.map(|p| p.id);
        let pickup = self
            .dropped_items
            .iter()
            .enumerate()
            .filter(|(_, d)| {
                d.pickup_delay <= 0.0
                    && d.position.distance_squared(mob.position) <= 9.0
                    && mob.skeleton.equipment_slot(d.stack).is_some()
            })
            .min_by(|(_, a), (_, b)| {
                a.position
                    .distance_squared(mob.position)
                    .total_cmp(&b.position.distance_squared(mob.position))
            })
            .map(|(i, d)| (i, d.position));
        if target.is_none()
            && mob.skeleton.wander_goal.is_none()
            && rand::random::<f32>() < 1.0 - (119.0_f32 / 120.0).powf(dt * 20.0)
        {
            mob.skeleton.wander_goal = Some(
                mob.position
                    + Vec3::new(
                        rand::random::<f32>() * 20.0 - 10.0,
                        rand::random::<f32>() * 14.0 - 7.0,
                        rand::random::<f32>() * 20.0 - 10.0,
                    ),
            );
        }
        mob.skeleton.shelter_retry = (mob.skeleton.shelter_retry - dt).max(0.0);
        let mut shelter = false;
        if sun
            && !in_water
            && mob.skeleton.armor[0].is_none()
            && (target.is_none() || mob.skeleton.melee)
            && (mob.skeleton.goal == Goal::FleeSun || mob.skeleton.shelter_retry <= 0.0)
        {
            self.skeleton_path(mob, mob.position, true, dt);
            shelter = !mob.skeleton.path.is_empty();
            if !shelter {
                mob.skeleton.shelter_retry = 1.0;
            }
        }
        let goal = mob.skeleton.choose_goal(
            target.is_some(),
            shelter,
            wolf.is_some(),
            pickup.is_some(),
            mob.skeleton.wander_goal.is_some(),
        );
        if goal != mob.skeleton.goal && goal != Goal::FleeSun {
            mob.skeleton.path.clear();
            mob.skeleton.path_timer = 0.0;
        }
        mob.skeleton.goal = goal;
        mob.walk_speed = 0.0;
        match goal {
            Goal::Ranged | Goal::Melee => {
                let target = target.unwrap();
                let delta = target.position - mob.position;
                mob.yaw = delta.z.atan2(delta.x);
                let in_range = if goal == Goal::Ranged {
                    delta.length_squared() <= 225.0
                } else {
                    skeleton_ai::melee_reaches(mob.position, target)
                };
                if !in_range || !target.visible {
                    // Follow remembered positions, never track unseen movement through walls.
                    if goal == Goal::Melee || mob.skeleton.sight_ready {
                        mob.walk_speed =
                            mob.base_speed() * if goal == Goal::Melee { 1.25 } else { 1.0 };
                        self.skeleton_path(mob, mob.skeleton.last_seen, false, dt);
                    }
                } else if mob.attack_cooldown <= 0.0 {
                    if goal == Goal::Ranged {
                        let origin = mob.position + Vec3::Y * 1.4;
                        let aim = target.position + Vec3::Y * target.height * 0.7;
                        self.arrows.push(crate::block::ArrowEntity {
                            position: origin,
                            velocity: crate::mob::aimed_arrow_velocity(aim - origin),
                            life: 8.0,
                            in_ground: false,
                            damage: 4.0,
                            is_critical: false,
                            from_player: false,
                            owner: Some(mob.id),
                        });
                        mob.attack_cooldown = self.difficulty.shot_interval();
                    } else {
                        mob.attack_cooldown = 1.0;
                        mob.animation.attack();
                        let damage = mob
                            .skeleton
                            .weapon
                            .map_or(2.0, |s| crate::item::melee_damage(s.block).max(2.0));
                        match target.id {
                            TargetId::Player => self.pending_hurt += damage as i32,
                            TargetId::Mob(id) => return Some((id, damage, mob.id)),
                        }
                    }
                }
            }
            Goal::FleeSun => {
                mob.walk_speed = mob.base_speed();
            }
            Goal::AvoidWolf => {
                let away = (mob.position - wolf.unwrap().position)
                    .with_y(0.0)
                    .normalize_or(Vec3::X);
                mob.walk_speed = mob.base_speed() * 1.2;
                self.skeleton_path(mob, mob.position + away * 10.0, false, dt);
            }
            Goal::Pickup => {
                let (index, position) = pickup.unwrap();
                if position.distance_squared(mob.position) <= 4.0
                    && self.skeleton_sees(eye, position)
                {
                    let stack = self.dropped_items[index].stack;
                    if let Some(old) = mob.skeleton.equip(stack) {
                        if stack.count == 1 {
                            self.dropped_items.swap_remove(index);
                        } else {
                            self.dropped_items[index].stack.count -= 1;
                        }
                        if let Some(old) = old {
                            self.dropped_items.push(crate::inventory::DroppedItem::new(
                                old,
                                mob.position,
                                Vec3::ZERO,
                            ));
                        }
                    }
                } else {
                    mob.walk_speed = mob.base_speed();
                    self.skeleton_path(mob, position, false, dt);
                }
            }
            Goal::Wander => {
                mob.walk_speed = mob.base_speed();
                self.skeleton_path(mob, mob.skeleton.wander_goal.unwrap(), false, dt);
                if mob.skeleton.path.is_empty() {
                    mob.skeleton.wander_goal = None;
                }
            }
            Goal::Idle => {
                mob.skeleton.look_time = (mob.skeleton.look_time - dt).max(0.0);
                let look_roll = rand::random::<f32>() < 1.0 - 0.98_f32.powf(dt * 20.0);
                if player.distance_squared(mob.position) < 64.0
                    && self.skeleton_sees(eye, player + Vec3::Y * 1.2)
                    && (mob.skeleton.looking_at_player && mob.skeleton.look_time > 0.0 || look_roll)
                {
                    if !mob.skeleton.looking_at_player || mob.skeleton.look_time == 0.0 {
                        mob.skeleton.look_time = 2.0 + rand::random::<f32>() * 2.0;
                    }
                    mob.skeleton.looking_at_player = true;
                    let delta = player - mob.position;
                    mob.yaw = delta.z.atan2(delta.x);
                } else if mob.skeleton.look_time == 0.0 && look_roll {
                    mob.skeleton.looking_at_player = false;
                    mob.yaw = rand::random::<f32>() * std::f32::consts::TAU;
                    mob.skeleton.look_time = 20.0 + rand::random::<f32>() * 20.0;
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn arena() -> World {
        let mut world = World::simulation(42);
        world.day_time = 900.0;
        world.night_spawn_timer = 10000.0;
        for x in -1..=1 {
            for z in -1..=1 {
                let mut chunk = Chunk::new(x, z, 42);
                for column in chunk.blocks.iter_mut() {
                    column[60].fill(BlockType::OakPlanks);
                }
                let index = world.get_pool_index(x, z);
                world.chunks[index] = Some(Box::new(chunk));
                world.spawned_natural_chunks.insert((x, z));
            }
        }
        world
    }
    fn put(world: &mut World, x: i32, y: i32, z: i32, block: BlockType) {
        world
            .get_chunk_mut(x.div_euclid(16), z.div_euclid(16))
            .unwrap()
            .blocks[x.rem_euclid(16) as usize][y as usize][z.rem_euclid(16) as usize] = block;
    }
    fn spawn(world: &mut World, position: Vec3) -> u32 {
        let mut mob = Mob::new(MobKind::Skeleton, position, position, 0);
        mob.grounded = true;
        let id = mob.id;
        world.mobs.push(mob);
        id
    }
    #[test]
    fn live_skeleton_holds_position_across_reload_and_shoots_at_range() {
        let mut world = arena();
        let start = Vec3::new(0.5, 61.0, 0.5);
        let id = spawn(&mut world, start);
        let player = start + Vec3::X * 14.0;
        for _ in 0..32 {
            world.update_mobs(player, 0.1, BlockType::Air);
        }
        let mob = world.mobs.iter().find(|m| m.id == id).unwrap();
        assert_eq!(mob.position, start);
        assert_eq!(mob.skeleton.goal, Goal::Ranged);
        assert_eq!(world.arrows.len(), 2);
        assert!(world.arrows.iter().all(|a| a.owner == Some(id)));
    }
    #[test]
    fn live_skeleton_does_not_acquire_player_through_a_wall() {
        let mut world = arena();
        spawn(&mut world, Vec3::new(0.5, 61.0, 0.5));
        for y in 61..65 {
            put(&mut world, 3, y, 0, BlockType::Stone);
        }
        world.update_mobs(Vec3::new(6.5, 61.0, 0.5), 0.1, BlockType::Air);
        assert!(world.mobs[0].skeleton.target.is_none());
        assert!(world.arrows.is_empty());
    }
    #[test]
    fn live_skeleton_selects_golem_and_ignores_creative_player() {
        let mut world = arena();
        let id = spawn(&mut world, Vec3::new(0.5, 61.0, 0.5));
        let golem = Mob::new(MobKind::Golem, Vec3::new(6.5, 61.0, 0.5), Vec3::ZERO, 0);
        let golem_id = golem.id;
        world.mobs.push(golem);
        world.player_targetable = false;
        world.update_mobs(Vec3::new(2.5, 61.0, 0.5), 0.1, BlockType::Air);
        assert_eq!(
            world
                .mobs
                .iter()
                .find(|m| m.id == id)
                .unwrap()
                .skeleton
                .target,
            Some(TargetId::Mob(golem_id))
        );
        assert_eq!(world.arrows.len(), 1);
    }
    #[test]
    fn live_skeleton_flees_visible_wolf_without_combat_target() {
        let mut world = arena();
        let id = spawn(&mut world, Vec3::new(4.5, 61.0, 0.5));
        world.mobs.push(Mob::new(
            MobKind::Wolf,
            Vec3::new(2.5, 61.0, 0.5),
            Vec3::ZERO,
            0,
        ));
        world.player_targetable = false;
        world.update_mobs(Vec3::new(0.5, 61.0, 0.5), 0.1, BlockType::Air);
        let mob = world.mobs.iter().find(|m| m.id == id).unwrap();
        assert_eq!(mob.skeleton.goal, Goal::AvoidWolf);
        assert!(mob.position.x > 4.5);
    }
    #[test]
    fn live_skeleton_changes_to_melee_underwater() {
        let mut world = arena();
        let id = spawn(&mut world, Vec3::new(0.5, 61.0, 0.5));
        for y in 61..64 {
            put(&mut world, 0, y, 0, BlockType::Water);
        }
        world.update_mobs(Vec3::new(1.7, 61.0, 0.5), 0.1, BlockType::Air);
        assert!(
            world
                .mobs
                .iter()
                .find(|m| m.id == id)
                .unwrap()
                .skeleton
                .melee
        );
        assert!(world.arrows.is_empty());
        assert_eq!(world.pending_hurt, 2);
    }
    #[test]
    fn live_skeleton_seeks_reachable_shade_and_helmet_blocks_ignition() {
        let mut world = arena();
        world.day_time = 200.0;
        world.player_targetable = false;
        let id = spawn(&mut world, Vec3::new(0.5, 61.0, 0.5));
        for x in 3..7 {
            for z in -2..3 {
                put(&mut world, x, 64, z, BlockType::Stone);
            }
        }
        world.update_mobs(Vec3::new(10.5, 61.0, 0.5), 0.1, BlockType::Air);
        let mob = world.mobs.iter().find(|m| m.id == id).unwrap();
        assert_eq!(mob.skeleton.goal, Goal::FleeSun);
        assert!(!mob.skeleton.path.is_empty());
        assert!(mob.skeleton.fire_time > 0.0);
        let mob = world.mobs.iter_mut().find(|m| m.id == id).unwrap();
        mob.skeleton.armor[0] = Some(crate::inventory::ItemStack::new(BlockType::IronHelmet, 1));
        mob.skeleton.fire_time = 0.0;
        world.update_mobs(Vec3::new(10.5, 61.0, 0.5), 0.1, BlockType::Air);
        assert_eq!(
            world
                .mobs
                .iter()
                .find(|m| m.id == id)
                .unwrap()
                .skeleton
                .fire_time,
            0.0
        );
    }
    #[test]
    fn live_skeleton_picks_one_sword_and_switches_to_melee() {
        let mut world = arena();
        world.player_targetable = false;
        spawn(&mut world, Vec3::new(0.5, 61.0, 0.5));
        let mut drop = crate::inventory::DroppedItem::new(
            crate::inventory::ItemStack::new(BlockType::IronSword, 3),
            Vec3::new(1.5, 61.0, 0.5),
            Vec3::ZERO,
        );
        drop.pickup_delay = 0.0;
        world.dropped_items.push(drop);
        for _ in 0..2 {
            world.update_mobs(Vec3::new(10.5, 61.0, 0.5), 0.1, BlockType::Air);
        }
        assert_eq!(
            world.mobs[0].skeleton.weapon.unwrap().block,
            BlockType::IronSword
        );
        assert!(world.mobs[0].skeleton.melee);
        assert_eq!(
            world
                .dropped_items
                .iter()
                .find(|d| d.stack.block == BlockType::IronSword)
                .unwrap()
                .stack
                .count,
            2
        );
    }

    #[test]
    fn live_conversion_preserves_equipment_and_uses_skeleton_goals() {
        let mut world = arena();
        world.player_targetable = false;
        let id = spawn(&mut world, Vec3::new(0.5, 61.0, 0.5));
        world.mobs[0].skeleton.armor[0] =
            Some(crate::inventory::ItemStack::new(BlockType::IronHelmet, 1));
        for x in -1..=1 {
            for z in -1..=1 {
                for y in 61..65 {
                    put(&mut world, x, y, z, BlockType::PowderedSnow);
                }
            }
        }
        for _ in 0..202 {
            world.update_mobs(Vec3::new(8.5, 61.0, 0.5), 0.1, BlockType::Air);
        }
        let mob = world.mobs.iter().find(|m| m.id == id).unwrap();
        assert_eq!(mob.kind, MobKind::Stray);
        assert!(mob.uses_skeleton_ai());
        assert_eq!(mob.skeleton.armor[0].unwrap().block, BlockType::IronHelmet);
    }

    #[test]
    fn live_hard_difficulty_uses_two_second_reload_and_peaceful_removes_hostiles() {
        let mut world = arena();
        spawn(&mut world, Vec3::new(0.5, 61.0, 0.5));
        world.difficulty = crate::skeleton_ai::Difficulty::Hard;
        for _ in 0..22 {
            world.update_mobs(Vec3::new(8.5, 61.0, 0.5), 0.1, BlockType::Air);
        }
        assert_eq!(world.arrows.len(), 2);
        world.difficulty = crate::skeleton_ai::Difficulty::Peaceful;
        world.update_mobs(Vec3::new(8.5, 61.0, 0.5), 0.1, BlockType::Air);
        assert!(world.mobs.is_empty());
        assert!(
            world
                .spawn_mob(MobKind::Skeleton, Vec3::new(0.5, 61.0, 0.5), 0)
                .is_err()
        );
    }

    #[test]
    fn live_arrows_hit_at_low_frame_rates_and_retaliate_against_the_shooter() {
        for fps in [10.0, 30.0, 60.0, 144.0] {
            let mut world = arena();
            let owner = spawn(&mut world, Vec3::new(0.5, 61.0, 0.5));
            let victim = spawn(&mut world, Vec3::new(8.5, 61.0, 0.5));
            world.mobs.swap(0, 1);
            let origin = Vec3::new(0.5, 62.4, 0.5);
            let aim = Vec3::new(8.5, 62.3, 0.5);
            world.arrows.push(crate::block::ArrowEntity {
                position: origin,
                velocity: crate::mob::aimed_arrow_velocity(aim - origin),
                life: 8.0,
                in_ground: false,
                damage: 4.0,
                is_critical: false,
                from_player: false,
                owner: Some(owner),
            });
            for _ in 0..fps as usize {
                world.update_arrows(Vec3::new(14.5, 61.0, 0.5), 1.0 / fps);
            }
            let hit = world.mobs.iter().find(|m| m.id == victim).unwrap();
            assert_eq!(hit.health, 16.0, "{fps} fps");
            assert_eq!(hit.skeleton.attacker, Some(TargetId::Mob(owner)));
            assert_eq!(
                world.mobs.iter().find(|m| m.id == owner).unwrap().health,
                20.0
            );
        }
    }
}
