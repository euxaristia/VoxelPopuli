use super::*;
use crate::bee::{BlockPos, center, is_flower, is_hive};

impl World {
    fn bee_path(&self, mob: &Mob, destination: Vec3) -> Vec<Vec3> {
        use std::cmp::Reverse;
        use std::collections::{BinaryHeap, HashMap};
        let start = mob.position.floor().as_ivec3();
        let end = destination.floor().as_ivec3();
        let key = |p: glam::IVec3| (p.x, p.y, p.z);
        let mut open = BinaryHeap::new();
        let mut costs = HashMap::new();
        let mut parents = HashMap::new();
        costs.insert(key(start), 0i32);
        open.push(Reverse((0i32, 0i32, key(start))));
        for _ in 0..512 {
            let Some(Reverse((_, cost, p))) = open.pop() else {
                break;
            };
            if costs.get(&p).copied() != Some(cost) {
                continue;
            }
            if p == key(end) {
                let mut cursor = p;
                let mut path = vec![destination];
                while cursor != key(start) {
                    path.push(center(cursor) + Vec3::Y * 0.2);
                    cursor = parents[&cursor];
                }
                path.reverse();
                return path;
            }
            let p3 = glam::IVec3::new(p.0, p.1, p.2);
            for delta in [
                glam::IVec3::X,
                -glam::IVec3::X,
                glam::IVec3::Y,
                -glam::IVec3::Y,
                glam::IVec3::Z,
                -glam::IVec3::Z,
            ] {
                let next = p3 + delta;
                if (next - start).abs().max_element() > 24
                    || !self.bee_clear(mob, center(key(next)) + Vec3::Y * 0.2)
                {
                    continue;
                }
                let next_cost = cost + 1;
                if costs.get(&key(next)).is_some_and(|c| *c <= next_cost) {
                    continue;
                }
                costs.insert(key(next), next_cost);
                parents.insert(key(next), p);
                open.push(Reverse((
                    next_cost + (next - end).abs().element_sum(),
                    next_cost,
                    key(next),
                )));
            }
        }
        Vec::new()
    }

    fn bee_clear(&self, mob: &Mob, p: Vec3) -> bool {
        if p.y < 1.0 || p.y + mob.height() >= CHUNK_HEIGHT as f32 {
            return false;
        }
        let r = mob.half_width();
        for x in (p.x - r).floor() as i32..=(p.x + r).floor() as i32 {
            for z in (p.z - r).floor() as i32..=(p.z + r).floor() as i32 {
                if self.get_chunk(x.div_euclid(16), z.div_euclid(16)).is_none() {
                    return false;
                }
                for y in p.y.floor() as i32..=(p.y + mob.height()).floor() as i32 {
                    let b = self.get_block(x, y, z);
                    if b.is_solid()
                        || matches!(
                            b,
                            BlockType::Water
                                | BlockType::Lava
                                | BlockType::Fire
                                | BlockType::Cactus
                                | BlockType::Campfire
                                | BlockType::OakDoor
                                | BlockType::IronDoor
                        )
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    fn bee_route_clear(&self, mob: &Mob, p: Vec3) -> bool {
        let steps = (mob.position.distance(p) / 0.2).ceil().max(1.0) as usize;
        (1..=steps).all(|i| self.bee_clear(mob, mob.position.lerp(p, i as f32 / steps as f32)))
    }

    fn bee_hover_goal(&self, mob: &mut Mob) -> Option<Vec3> {
        for _ in 0..24 {
            let x = (mob.position.x + (mob.bee.random(mob.id) * 2.0 - 1.0) * 8.0).floor() as i32;
            let z = (mob.position.z + (mob.bee.random(mob.id) * 2.0 - 1.0) * 8.0).floor() as i32;
            let top = (mob.position.y as i32 + 7).min(CHUNK_HEIGHT as i32 - 2);
            let bottom = (mob.position.y as i32 - 9).max(1);
            let height = 1.0 + mob.bee.random(mob.id) * 3.0;
            for y in (bottom..=top).rev() {
                if !self.get_block(x, y, z).is_solid()
                    && self.get_block(x, y, z) != BlockType::Water
                {
                    continue;
                }
                let p = Vec3::new(x as f32 + 0.5, y as f32 + 1.0 + height, z as f32 + 0.5);
                if (p.y - mob.position.y).abs() <= 8.0
                    && p.distance_squared(mob.position) > 1.0
                    && mob
                        .bee
                        .hive
                        .is_none_or(|h| p.distance_squared(center(h)) <= 22.0 * 22.0)
                    && self.bee_route_clear(mob, p)
                {
                    return Some(p);
                }
                break;
            }
        }
        // A bee displaced high above terrain must be able to descend gradually.
        let down = mob.position - Vec3::Y * 2.0;
        self.bee_route_clear(mob, down).then_some(down)
    }

    fn bee_flower(&self, mob: &mut Mob) -> Option<BlockPos> {
        let origin = mob.position.floor().as_ivec3();
        let mut chosen = None;
        let mut count = 0;
        for x in origin.x - 6..=origin.x + 6 {
            for z in origin.z - 6..=origin.z + 6 {
                for y in (origin.y - 4).max(1)..=(origin.y + 4).min(CHUNK_HEIGHT as i32 - 2) {
                    if is_flower(self.get_block(x, y, z))
                        && self.bee_clear(mob, center((x, y, z)) + Vec3::Y * 0.25)
                    {
                        count += 1;
                        if mob.bee.random(mob.id) < 1.0 / count as f32 {
                            chosen = Some((x, y, z));
                        }
                    }
                }
            }
        }
        chosen
    }

    pub fn hive_smoked(&self, pos: BlockPos) -> bool {
        for n in 1..=5 {
            let b = self.get_block(pos.0, pos.1 - n, pos.2);
            if b == BlockType::Campfire {
                return true;
            }
            if b.is_solid() {
                return false;
            }
        }
        false
    }

    pub fn disturb_hive(&mut self, pos: BlockPos) {
        for mob in &mut self.mobs {
            if mob.kind != MobKind::Bee || mob.bee.sting_death > 0.0 {
                continue;
            }
            if mob.bee.hive == Some(pos) || mob.position.distance_squared(center(pos)) < 20.0 * 20.0
            {
                mob.anger_time = 25.0;
                mob.bee.inside = false;
                mob.bee.hive = None;
                mob.bee.goal = None;
            }
        }
    }

    pub fn harvest_hive(&mut self, pos: BlockPos, tool: BlockType) -> Option<(BlockType, u32)> {
        if !is_hive(self.get_block(pos.0, pos.1, pos.2))
            || self.hives.get(&pos).copied().unwrap_or(0) < 5
        {
            return None;
        }
        let drop = match tool {
            BlockType::GlassBottle => (BlockType::HoneyBottle, 1),
            BlockType::Shears => (BlockType::Honeycomb, 3),
            _ => return None,
        };
        self.hives.insert(pos, 0);
        if !self.hive_smoked(pos) {
            self.disturb_hive(pos);
        }
        Some(drop)
    }

    pub(super) fn tick_bee(
        &mut self,
        mob: &mut Mob,
        others: &[skeletons::CreatureSnapshot],
        player: Vec3,
        held: BlockType,
        dt: f32,
    ) {
        let previous = mob.position;
        mob.anger_time = (mob.anger_time - dt).max(0.0);
        mob.animal.growth = (mob.animal.growth - dt).max(0.0);
        mob.animal.love_time = (mob.animal.love_time - dt).max(0.0);
        mob.animal.breed_cooldown = (mob.animal.breed_cooldown - dt).max(0.0);
        mob.bee.retry = (mob.bee.retry - dt).max(0.0);
        mob.bee.search_time = (mob.bee.search_time + dt).min(180.0);
        mob.bee.goal_age += dt;
        if mob.bee.sting_death > 0.0 {
            mob.anger_time = 0.0;
            mob.bee.sting_death = (mob.bee.sting_death - dt).max(0.0);
            if mob.bee.sting_death == 0.0 {
                mob.health = 0.0;
                return;
            }
        }
        if mob.bee.hive.is_some_and(|p| {
            self.get_chunk(p.0.div_euclid(16), p.2.div_euclid(16))
                .is_some()
                && !is_hive(self.get_block(p.0, p.1, p.2))
        }) {
            mob.bee.hive = None;
            mob.bee.inside = false;
        }
        let shelter = self.day_time >= 600.0 || self.raining;
        if mob.bee.inside {
            mob.velocity = Vec3::ZERO;
            mob.bee.residence = (mob.bee.residence - dt).max(0.0);
            if mob.bee.residence == 0.0
                && !shelter
                && self.bee_clear(mob, mob.position)
                && !others.iter().any(|o| {
                    o.alive && o.id != mob.id && o.position.distance_squared(mob.position) < 1.0
                })
            {
                if mob.bee.nectar {
                    if let Some(p) = mob.bee.hive {
                        let honey = self.hives.entry(p).or_default();
                        *honey = (*honey + 1).min(5);
                    }
                    mob.bee.nectar = false;
                }
                mob.bee.inside = false;
                mob.bee.flower = None;
                mob.bee.search_time = 0.0;
                mob.bee.goal = None;
                mob.bee.retry = 1.0;
            }
            return;
        }
        let mut target = None;
        let mut speed = mob.base_speed();
        if mob.anger_time > 0.0 && self.difficulty != crate::skeleton_ai::Difficulty::Peaceful {
            target = Some(player + Vec3::Y * 0.8);
            speed *= 1.4;
            if mob.position.distance(player + Vec3::Y * 0.8) < 1.1
                && self.bee_route_clear(mob, player + Vec3::Y * 0.8)
            {
                self.pending_hurt += 2;
                self.pending_hurt_origin = Some(mob.position);
                self.pending_poison = self.pending_poison.max(match self.difficulty {
                    crate::skeleton_ai::Difficulty::Hard => 18.0,
                    crate::skeleton_ai::Difficulty::Normal => 10.0,
                    _ => 0.0,
                });
                mob.anger_time = 0.0;
                mob.bee.sting_death = 10.0 + mob.bee.random(mob.id) * 50.0;
                mob.bee.nectar = false;
                mob.animation.attack();
            }
        } else if is_flower(held)
            && mob.position.distance(player) < 8.0
            && mob.bee.sting_death == 0.0
        {
            target = Some(player + Vec3::Y * 1.0);
            speed *= 1.25;
            if mob.position.distance(target.unwrap()) < 1.5 {
                speed = 0.0;
            }
        } else if mob.animal.love_time > 0.0 {
            target = others
                .iter()
                .filter(|o| o.kind == MobKind::Bee && o.id != mob.id && o.alive && !o.baby)
                .min_by(|a, b| {
                    a.position
                        .distance_squared(mob.position)
                        .total_cmp(&b.position.distance_squared(mob.position))
                })
                .map(|o| o.position);
        } else if mob.is_baby() && !shelter && !mob.bee.nectar {
            target = others
                .iter()
                .filter(|o| {
                    o.kind == MobKind::Bee
                        && !o.baby
                        && o.alive
                        && o.position.distance_squared(mob.position) > 4.0
                })
                .min_by(|a, b| {
                    a.position
                        .distance_squared(mob.position)
                        .total_cmp(&b.position.distance_squared(mob.position))
                })
                .map(|o| o.position);
            speed *= 1.1;
        }
        if target.is_none()
            && mob.bee.sting_death == 0.0
            && (shelter || mob.bee.nectar || mob.bee.search_time >= 180.0)
        {
            if mob.bee.hive.is_none() && mob.bee.retry == 0.0 {
                mob.bee.hive = self
                    .hives
                    .keys()
                    .copied()
                    .filter(|p| {
                        (center(*p) - mob.position)
                            .abs()
                            .cmple(Vec3::new(16.0, 10.0, 16.0))
                            .all()
                            && others
                                .iter()
                                .filter(|o| {
                                    o.kind == MobKind::Bee && o.hive == Some(*p) && o.inside
                                })
                                .count()
                                < 3
                    })
                    .min_by(|a, b| {
                        center(*a)
                            .distance_squared(mob.position)
                            .total_cmp(&center(*b).distance_squared(mob.position))
                    });
                mob.bee.retry = 1.0;
            }
            if let Some(h) = mob.bee.hive {
                let entrance = center(h) + Vec3::new(0.0, 0.2, 1.1);
                target = Some(entrance);
                if mob.position.distance(entrance) < 0.65 {
                    let occupied = others
                        .iter()
                        .filter(|o| o.kind == MobKind::Bee && o.inside && o.hive == Some(h))
                        .count();
                    if occupied < 3 {
                        mob.position = entrance;
                        mob.bee.inside = true;
                        mob.bee.residence = if mob.bee.nectar { 120.0 } else { 30.0 };
                        mob.velocity = Vec3::ZERO;
                        return;
                    }
                    mob.bee.hive = None;
                    mob.bee.retry = 5.0 + mob.bee.random(mob.id) * 15.0;
                    target = None;
                }
            }
        }
        if target.is_none() && !shelter && !mob.bee.nectar && mob.bee.sting_death == 0.0 {
            if mob
                .bee
                .flower
                .is_some_and(|p| !is_flower(self.get_block(p.0, p.1, p.2)))
            {
                mob.bee.flower = None;
                mob.bee.pollination = 0.0;
            }
            if mob.bee.flower.is_none() && mob.bee.retry == 0.0 {
                if mob.bee.random(mob.id) < 0.5 {
                    mob.bee.flower = self.bee_flower(mob);
                }
                mob.bee.retry = 1.0;
            }
            if let Some(f) = mob.bee.flower {
                let p = center(f) + Vec3::Y * 0.25;
                target = Some(p);
                if mob.position.distance(p) < 1.0 {
                    speed = 0.0;
                    mob.bee.pollination += dt;
                    if mob.bee.pollination >= 20.0 {
                        mob.bee.nectar = true;
                        mob.bee.crop_charges = 10;
                        mob.bee.pollination = 0.0;
                        mob.bee.search_time = 0.0;
                        mob.bee.goal = None;
                    }
                } else {
                    mob.bee.pollination = 0.0;
                }
            }
        } else {
            mob.bee.pollination = 0.0;
        }
        if mob.bee.nectar
            && mob.bee.crop_charges > 0
            && mob.bee.random(mob.id) < 1.0 - 0.97f32.powf(dt * 20.0)
        {
            let p = mob.position.floor().as_ivec3();
            for y in (p.y - 2..=p.y).rev() {
                let next = match self.get_block(p.x, y, p.z) {
                    BlockType::WheatStage0 => BlockType::WheatStage1,
                    BlockType::WheatStage1 => BlockType::WheatStage2,
                    BlockType::WheatStage2 => BlockType::Wheat,
                    _ => continue,
                };
                self.set_block(p.x, y, p.z, next);
                mob.bee.crop_charges -= 1;
                break;
            }
        }
        let direct = target.filter(|p| self.bee_route_clear(mob, *p));
        if let Some(p) = direct {
            mob.bee.goal = Some(p);
            mob.bee.path.clear();
        } else if let Some(target) = target {
            if mob.bee.goal_age >= 1.0 {
                mob.bee.path = self.bee_path(mob, target);
                mob.bee.goal_age = 0.0;
            }
            while mob
                .bee
                .path
                .first()
                .is_some_and(|p| p.distance_squared(mob.position) < 0.16)
            {
                mob.bee.path.remove(0);
            }
            mob.bee.goal = mob.bee.path.first().copied();
            if mob.bee.goal.is_none() && mob.bee.goal_age == 0.0 {
                mob.bee.goal = self.bee_hover_goal(mob);
            }
        } else if mob.bee.goal.is_none_or(|p| {
            p.distance_squared(mob.position) < 0.16 || !self.bee_route_clear(mob, p)
        }) || mob.bee.goal_age > 4.0
        {
            mob.bee.goal = self.bee_hover_goal(mob);
            mob.bee.goal_age = 0.0;
        }
        let mut velocity = mob.bee.goal.map_or(Vec3::ZERO, |p| {
            (p - mob.position).normalize_or_zero()
                * speed.min(p.distance(mob.position) / dt.max(0.001))
        });
        // Local avoidance complements the block navigator; do not synchronize
        // bees to a common altitude or use a shared flock destination.
        for other in others.iter().filter(|o| o.id != mob.id && o.alive) {
            let delta = mob.position - other.position;
            let distance = delta.length();
            if distance > 0.001 && distance < 1.0 {
                velocity += delta / distance * (1.0 - distance) * 3.0;
            }
        }
        velocity = velocity.clamp_length_max(speed.max(0.5));
        mob.velocity = Vec3::ZERO;
        for axis in [1, 0, 2] {
            let mut p = mob.position;
            p[axis] += velocity[axis] * dt;
            if self.bee_route_clear(mob, p)
                && !others.iter().any(|o| {
                    o.alive
                        && o.id != mob.id
                        && p.y < o.position.y + o.height
                        && p.y + mob.height() > o.position.y
                        && (p.x - o.position.x).abs() < mob.half_width() + o.half_width
                        && (p.z - o.position.z).abs() < mob.half_width() + o.half_width
                })
            {
                mob.position = p;
                mob.velocity[axis] = velocity[axis];
            } else {
                mob.bee.goal = None;
            }
        }
        if mob.velocity.with_y(0.0).length_squared() > 0.001 {
            mob.yaw = mob.velocity.z.atan2(mob.velocity.x);
        }
        mob.grounded = false;
        mob.animate_movement(previous, dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arena() -> World {
        let mut world = World::simulation(42);
        let mut chunk = Chunk::new(0, 0, 42);
        for column in chunk.blocks.iter_mut() {
            for (y, row) in column.iter_mut().enumerate() {
                row.fill(if y == 63 {
                    BlockType::Stone
                } else {
                    BlockType::Air
                });
            }
        }
        world.insert_chunk(chunk);
        world.spawned_natural_chunks.insert((0, 0));
        world
    }

    #[test]
    fn pollination_hive_processing_and_honey_form_a_complete_cycle() {
        let mut world = arena();
        let flower = (5, 64, 5);
        let hive = (9, 65, 5);
        world.set_block(flower.0, flower.1, flower.2, BlockType::Poppy);
        world.set_block(hive.0, hive.1, hive.2, BlockType::Beehive);
        let p = center(flower) + Vec3::Y * 0.25;
        let mut bee = Mob::new(MobKind::Bee, p, p, 0);
        bee.bee.flower = Some(flower);
        world.mobs.push(bee);
        let player = Vec3::new(1.0, 64.0, 1.0);
        for _ in 0..500 {
            world.update_mobs(player, 0.05, BlockType::Air);
        }
        assert!(
            world.mobs[0].bee.inside,
            "nectar bee did not return to the hive: {:?}",
            world.mobs[0].position
        );
        assert!(world.mobs[0].bee.nectar);
        assert_eq!(world.hives[&hive], 0);
        world.mobs[0].bee.residence = 0.05;
        world.update_mobs(player, 0.1, BlockType::Air);
        assert_eq!(world.hives[&hive], 1);
        assert!(!world.mobs[0].bee.nectar && !world.mobs[0].bee.inside);
    }

    #[test]
    fn shelter_and_blocked_exit_keep_occupants_inside() {
        let mut world = arena();
        let h = (8, 65, 8);
        world.set_block(h.0, h.1, h.2, BlockType::Beehive);
        let p = center(h) + Vec3::new(0.0, 0.2, 1.1);
        let mut bee = Mob::new(MobKind::Bee, p, p, 0);
        bee.bee.hive = Some(h);
        bee.bee.inside = true;
        world.mobs.push(bee);
        world.raining = true;
        world.update_mobs(p, 0.1, BlockType::Air);
        assert!(world.mobs[0].bee.inside);
        world.raining = false;
        world.day_time = 700.0;
        world.update_mobs(p, 0.1, BlockType::Air);
        assert!(world.mobs[0].bee.inside);
        world.day_time = 0.0;
        world.set_block(8, 65, 9, BlockType::Stone);
        world.update_mobs(p, 0.1, BlockType::Air);
        assert!(world.mobs[0].bee.inside);
        world.set_block(8, 65, 9, BlockType::Air);
        world.update_mobs(p, 0.1, BlockType::Air);
        assert!(!world.mobs[0].bee.inside);
    }

    #[test]
    fn fourth_bee_cannot_enter_a_full_hive() {
        let mut world = arena();
        let h = (8, 65, 8);
        world.set_block(8, 65, 8, BlockType::Beehive);
        let p = center(h) + Vec3::new(0.0, 0.2, 1.1);
        for _ in 0..3 {
            let mut b = Mob::new(MobKind::Bee, p, p, 0);
            b.bee.hive = Some(h);
            b.bee.inside = true;
            b.bee.residence = 120.0;
            world.mobs.push(b);
        }
        let mut b = Mob::new(MobKind::Bee, p, p, 0);
        b.bee.hive = Some(h);
        b.bee.nectar = true;
        world.mobs.push(b);
        world.update_mobs(Vec3::ZERO, 0.1, BlockType::Air);
        assert_eq!(world.mobs.iter().filter(|m| m.bee.inside).count(), 3);
        assert!(world.mobs[3].bee.retry >= 5.0 && world.mobs[3].bee.hive.is_none());
    }

    #[test]
    fn harvesting_requires_full_honey_and_smoke_prevents_anger() {
        let mut world = arena();
        let h = (8, 66, 8);
        world.set_block(8, 66, 8, BlockType::BeeNest);
        let p = center(h) + Vec3::Z * 1.1;
        let mut b = Mob::new(MobKind::Bee, p, p, 0);
        b.bee.hive = Some(h);
        world.mobs.push(b);
        assert_eq!(world.harvest_hive(h, BlockType::Shears), None);
        world.hives.insert(h, 5);
        world.set_block(8, 64, 8, BlockType::Campfire);
        assert_eq!(
            world.harvest_hive(h, BlockType::Shears),
            Some((BlockType::Honeycomb, 3))
        );
        assert_eq!(world.mobs[0].anger_time, 0.0);
        assert_eq!(world.hives[&h], 0);
        world.hives.insert(h, 5);
        world.set_block(8, 65, 8, BlockType::Stone);
        assert_eq!(
            world.harvest_hive(h, BlockType::GlassBottle),
            Some((BlockType::HoneyBottle, 1))
        );
        assert_eq!(world.mobs[0].anger_time, 25.0);
    }

    #[test]
    fn bees_sting_once_and_die_after_their_timer() {
        let mut world = arena();
        let p = Vec3::new(8.0, 65.0, 8.0);
        let mut b = Mob::new(MobKind::Bee, p, p, 0);
        b.anger_time = 25.0;
        world.mobs.push(b);
        world.update_mobs(p - Vec3::Y * 0.8, 0.05, BlockType::Air);
        assert_eq!(world.pending_hurt, 2);
        assert_eq!(world.pending_poison, 10.0);
        assert!((10.0..=60.0).contains(&world.mobs[0].bee.sting_death));
        for _ in 0..20 {
            world.update_mobs(p, 0.05, BlockType::Air);
        }
        assert_eq!(world.pending_hurt, 2);
        world.mobs[0].bee.sting_death = 0.01;
        world.update_mobs(p, 0.05, BlockType::Air);
        assert!(world.mobs.is_empty());
    }

    #[test]
    fn hover_navigation_routes_around_a_wall_and_rejects_water() {
        let mut world = arena();
        let start = Vec3::new(3.5, 65.2, 8.5);
        let end = Vec3::new(10.5, 65.2, 8.5);
        for y in 64..70 {
            for z in 5..=10 {
                world.set_block(7, y, z, BlockType::Stone);
            }
        }
        let bee = Mob::new(MobKind::Bee, start, start, 0);
        assert!(!world.bee_route_clear(&bee, end));
        let path = world.bee_path(&bee, end);
        assert!(!path.is_empty());
        assert!(path.iter().all(|p| world.bee_clear(&bee, *p)));
        world.set_block(3, 65, 8, BlockType::Water);
        assert!(!world.bee_clear(&bee, start));
    }

    #[test]
    fn bees_leave_a_shared_spawn_line_and_choose_independent_heights() {
        let mut world = arena();
        for i in 0..8 {
            let p = Vec3::new(4.0 + i as f32 * 0.7, 65.5, 8.0);
            world.mobs.push(Mob::new(MobKind::Bee, p, p, 0));
        }
        for _ in 0..200 {
            world.update_mobs(Vec3::new(8.0, 64.0, 8.0), 0.05, BlockType::Air);
        }
        let lo = world
            .mobs
            .iter()
            .map(|m| m.position.y)
            .fold(f32::INFINITY, f32::min);
        let hi = world
            .mobs
            .iter()
            .map(|m| m.position.y)
            .fold(f32::NEG_INFINITY, f32::max);
        assert!(
            hi - lo > 0.5,
            "bees are still locked to one height: {lo}..{hi}"
        );
        assert!(
            world
                .mobs
                .iter()
                .filter(|m| (m.position.z - 8.0).abs() > 1.0)
                .count()
                >= 4
        );
        assert_eq!(world.pending_hurt, 0, "unprovoked bees attacked the player");
    }

    #[test]
    fn bee_hover_tracks_current_ground_not_destroyed_spawn_height() {
        let mut world = arena();
        let p = Vec3::new(8.0, 67.0, 8.0);
        world
            .mobs
            .push(Mob::new(MobKind::Bee, p, p + Vec3::Y * 15.0, 0));
        for _ in 0..200 {
            world.update_mobs(p, 0.05, BlockType::Air);
        }
        assert!(
            (65.0..=68.1).contains(&world.mobs[0].position.y),
            "bee followed obsolete home altitude: {:?}",
            world.mobs[0].position
        );
    }
}
