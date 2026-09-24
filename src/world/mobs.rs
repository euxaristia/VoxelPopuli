use super::*;
use crate::mob_catalog::{Temper, can_spawn};

/// Sweep the moving box against another creature, including fast crossings.
fn crosses_mob(mob: &Mob, start: Vec3, end: Vec3, other: &Mob) -> bool {
    if other.health <= 0.0 || other.bee.inside || mob.bee.inside {
        return false;
    }
    let radius = mob.half_width() + other.half_width() - 0.001;
    let min = other.position - Vec3::new(radius, mob.height() - 0.001, radius);
    let max = other.position + Vec3::new(radius, other.height() - 0.001, radius);
    let delta = end - start;
    let mut enter: f32 = 0.0;
    let mut leave: f32 = 1.0;
    for axis in 0..3 {
        if delta[axis].abs() < 1e-7 {
            if start[axis] <= min[axis] || start[axis] >= max[axis] {
                return false;
            }
            continue;
        }
        let a = (min[axis] - start[axis]) / delta[axis];
        let b = (max[axis] - start[axis]) / delta[axis];
        enter = enter.max(a.min(b));
        leave = leave.min(a.max(b));
    }
    enter < leave && leave > 0.0 && enter < 1.0
}

fn resolve_mob_overlaps(mobs: &mut [Mob], allowed: impl Fn(&Mob, Vec3) -> bool) {
    for _ in 0..8 {
        let mut changed = false;
        for i in 0..mobs.len() {
            for j in i + 1..mobs.len() {
                let (left, right) = mobs.split_at_mut(j);
                let (a, b) = (&mut left[i], &mut right[0]);
                if a.health <= 0.0
                    || a.bee.inside
                    || b.bee.inside
                    || b.health <= 0.0
                    || a.position.y >= b.position.y + b.height()
                    || b.position.y >= a.position.y + a.height()
                {
                    continue;
                }
                let delta = b.position - a.position;
                let radius = a.half_width() + b.half_width();
                let ox = radius - delta.x.abs();
                let oz = radius - delta.z.abs();
                if ox <= 0.001 || oz <= 0.001 {
                    continue;
                }
                for axis in if ox < oz { [0, 2] } else { [2, 0] } {
                    let amount = radius - delta[axis].abs() + 0.002;
                    let mut push = Vec3::ZERO;
                    push[axis] = if delta[axis] < 0.0 { -amount } else { amount };
                    let pa = a.position - push * 0.5;
                    let pb = b.position + push * 0.5;
                    if allowed(a, pa) && allowed(b, pb) {
                        a.position = pa;
                        b.position = pb;
                    } else if allowed(a, a.position - push) {
                        a.position -= push;
                    } else if allowed(b, b.position + push) {
                        b.position += push;
                    } else {
                        continue;
                    }
                    a.velocity[axis] = 0.0;
                    b.velocity[axis] = 0.0;
                    changed = true;
                    break;
                }
            }
        }
        if !changed {
            break;
        }
    }
}

impl World {
    pub(super) fn mob_path_occupied<'a>(
        &self,
        mob: &Mob,
        end: Vec3,
        others: impl Iterator<Item = &'a Mob>,
    ) -> bool {
        others
            .into_iter()
            .any(|other| crosses_mob(mob, mob.position, end, other))
    }

    /// Resolve saved overlaps and newborns without pushing creatures into terrain.
    pub(super) fn separate_mobs(&self, mobs: &mut [Mob]) {
        let allowed = |m: &Mob, p: Vec3| {
            let steps = ((p - m.position).length() / 0.1).ceil().max(1.0) as usize;
            (1..=steps).all(|step| {
                !self.mob_box_blocked(
                    m.position.lerp(p, step as f32 / steps as f32),
                    m.half_width(),
                    m.height(),
                )
            }) && (m.kind.species().motion != Motion::Swim
                || self.mob_in_water(p, m.half_width(), m.height()))
        };
        resolve_mob_overlaps(mobs, allowed);
    }
}

impl World {
    pub(super) fn mob_in_water(&self, pos: Vec3, hw: f32, height: f32) -> bool {
        let min = pos - Vec3::new(hw, 0.0, hw) + Vec3::splat(0.001);
        let max = pos + Vec3::new(hw, height, hw) - Vec3::splat(0.001);
        for x in min.x.floor() as i32..=max.x.floor() as i32 {
            for z in min.z.floor() as i32..=max.z.floor() as i32 {
                for y in min.y.floor() as i32..=max.y.floor() as i32 {
                    let surface = y as f32
                        + crate::chunk::water_render_height(self.get_liquid_level(x, y, z));
                    if self.get_block(x, y, z) != BlockType::Water
                        || max.y.min(y as f32 + 1.0) > surface
                    {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Natural encounters and catalogue summons share collision and capacity checks.
    pub fn spawn_mob(&mut self, kind: MobKind, pos: Vec3, variant: u8) -> Result<(), &'static str> {
        if self.difficulty == crate::skeleton_ai::Difficulty::Peaceful
            && kind.species().temper == Temper::Hostile
        {
            return Err("Hostile creatures cannot spawn on Peaceful difficulty.");
        }
        if self.mobs.len() >= MOB_CAP {
            return Err("Creature limit reached (48).");
        }
        let mob = Mob::new(kind, pos, pos, variant);
        if !pos.is_finite() || pos.y < 1.0 || pos.y + mob.height() >= CHUNK_HEIGHT as f32 {
            return Err("Choose a spot inside the world.");
        }
        if self.mob_box_blocked(pos, mob.half_width(), mob.height()) {
            return Err("This creature needs more room.");
        }
        if kind.species().motion == Motion::Swim
            && !self.mob_in_water(pos, mob.half_width(), mob.height())
        {
            return Err("This creature needs deeper water.");
        }
        if self.mobs.iter().any(|m| crosses_mob(&mob, pos, pos, m)) {
            return Err("Another creature is in the way.");
        }
        self.mobs.push(mob);
        Ok(())
    }

    pub fn summon_near(
        &mut self,
        kind: MobKind,
        player: Vec3,
        direction: Vec3,
    ) -> Result<(), &'static str> {
        if self.mobs.len() >= MOB_CAP {
            return Err("Creature limit reached (48).");
        }
        let forward = direction.with_y(0.0).normalize_or_zero();
        let side = Vec3::new(-forward.z, 0.0, forward.x);
        for distance in [4.0, 6.0, 8.0] {
            for offset in [0.0, -2.0, 2.0, -4.0, 4.0] {
                let center = player + forward * distance + side * offset;
                let x = center.x.floor() as i32;
                let z = center.z.floor() as i32;
                for y in ((player.y as i32 - 8).max(1)
                    ..=(player.y as i32 + 5).min(CHUNK_HEIGHT as i32 - 5))
                    .rev()
                {
                    let pos = Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5);
                    let supported = self.get_block(x, y - 1, z).is_solid();
                    if matches!(kind.species().motion, Motion::Walk | Motion::Hop) && !supported {
                        continue;
                    }
                    if kind.species().motion == Motion::Fly && y != player.y as i32 + 2 {
                        continue;
                    }
                    if kind.species().motion == Motion::Amphibious
                        && !supported
                        && self.get_block(x, y, z) != BlockType::Water
                    {
                        continue;
                    }
                    if self.spawn_mob(kind, pos, rand::random()).is_ok() {
                        return Ok(());
                    }
                }
            }
        }
        Err(if kind.species().motion == Motion::Swim {
            "Move closer to deep, open water."
        } else {
            "Find an open space in front of you."
        })
    }

    pub(super) fn spawn_surface_mob(&mut self, x: i32, z: i32, random: u64, hostiles_only: bool) {
        if self.mobs.len() >= MOB_CAP {
            return;
        }
        let biome =
            crate::chunk::biome_at_version(x as f32, z as f32, self.seed, self.generator_version);
        for y in (55..CHUNK_HEIGHT as i32 - 5).rev() {
            let floor = self.get_block(x, y, z);
            if floor == BlockType::Air {
                continue;
            }
            let water = floor == BlockType::Water;
            if !water
                && !matches!(
                    floor,
                    BlockType::Grass
                        | BlockType::SnowyGrass
                        | BlockType::Dirt
                        | BlockType::Sand
                        | BlockType::Stone
                )
            {
                return;
            }
            let options: Vec<_> = MobKind::ALL
                .iter()
                .copied()
                .filter(|&k| {
                    can_spawn(k, biome, water, false, self.day_time >= 600.0)
                        && (self.difficulty != crate::skeleton_ai::Difficulty::Peaceful
                            || k.species().temper != Temper::Hostile)
                        && (!hostiles_only || k.species().temper == Temper::Hostile)
                })
                .collect();
            if options.is_empty() {
                return;
            }
            let mut kind = options[random as usize % options.len()];
            if kind == MobKind::Zombie && ((random >> 24) % 20 == 0) {
                kind = MobKind::ZombieVillager;
            }
            let pos = Vec3::new(
                x as f32 + 0.5,
                if water {
                    y as f32 - kind.species().height - 0.5
                } else {
                    y as f32 + 1.0
                },
                z as f32 + 0.5,
            );
            let _ = self.spawn_mob(kind, pos, (random >> 16) as u8);
            return;
        }
    }

    pub(super) fn try_spawn_natural_mobs(&mut self, cx: i32, cz: i32) {
        if self.mobs.len() >= MOB_CAP
            || self.spawned_natural_chunks.contains(&(cx, cz))
            || self.get_chunk(cx, cz).is_none()
        {
            return;
        }
        self.spawned_natural_chunks.insert((cx, cz));
        let mut nests = Vec::new();
        if let Some(chunk) = self.get_chunk(cx, cz) {
            for x in 0..CHUNK_WIDTH {
                for z in 0..CHUNK_DEPTH {
                    for y in 1..CHUNK_HEIGHT {
                        if crate::bee::is_hive(chunk.blocks[x][y][z]) {
                            nests.push((
                                (cx * 16 + x as i32, y as i32, cz * 16 + z as i32),
                                chunk.blocks[x][y][z],
                            ));
                        }
                    }
                }
            }
        }
        for (pos, block) in nests {
            if self.hives.contains_key(&pos) {
                continue;
            }
            self.hives.insert(pos, 0);
            if block == BlockType::BeeNest {
                for _ in 0..(2 + crate::chunk::chunk_hash(self.seed, pos.0, pos.2, pos.1) % 2) {
                    if self.mobs.len() >= MOB_CAP {
                        break;
                    }
                    let p = crate::bee::center(pos) + Vec3::new(0.0, 0.2, 1.1);
                    let mut bee = Mob::new(MobKind::Bee, p, p, 0);
                    bee.bee.hive = Some(pos);
                    bee.bee.inside = true;
                    bee.bee.residence = 30.0;
                    self.mobs.push(bee);
                }
            }
        }
        let mut random =
            (self.seed ^ (cx as u64).wrapping_mul(0x9e3779b9) ^ (cz as u64).rotate_left(32))
                .wrapping_add(1);
        let mut next = || {
            random = random
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            random
        };
        // At most two surface creatures and one cave encounter per chunk.
        for _ in 0..next() % 3 {
            let x = cx * CHUNK_WIDTH as i32 + (next() % CHUNK_WIDTH as u64) as i32;
            let z = cz * CHUNK_DEPTH as i32 + (next() % CHUNK_DEPTH as u64) as i32;
            self.spawn_surface_mob(x, z, next(), false);
        }
        let x = cx * CHUNK_WIDTH as i32 + (next() % CHUNK_WIDTH as u64) as i32;
        let z = cz * CHUNK_DEPTH as i32 + (next() % CHUNK_DEPTH as u64) as i32;
        for y in 10..55 {
            if !self.get_block(x, y - 1, z).is_solid() {
                continue;
            }
            let feet = self.get_block(x, y, z);
            if feet != BlockType::Air && feet != BlockType::Water {
                continue;
            }
            let water = feet == BlockType::Water;
            let options: Vec<_> = MobKind::ALL
                .iter()
                .copied()
                .filter(|&k| can_spawn(k, crate::chunk::Biome::Plains, water, true, true))
                .collect();
            let n = next();
            let kind = options[n as usize % options.len()];
            if self
                .spawn_mob(
                    kind,
                    Vec3::new(x as f32 + 0.5, y as f32, z as f32 + 0.5),
                    (n >> 16) as u8,
                )
                .is_ok()
            {
                break;
            }
        }
        // Dungeon spawners keep their existing encounter behavior.
        let mut spawners = Vec::new();
        if let Some(chunk) = self.get_chunk(cx, cz) {
            for (x, column) in chunk.blocks.iter().enumerate() {
                for (y, row) in column.iter().enumerate().take(100).skip(10) {
                    for (z, &block) in row.iter().enumerate() {
                        if block == BlockType::MobSpawner {
                            spawners.push(Vec3::new(
                                (cx * CHUNK_WIDTH as i32 + x as i32) as f32 + 1.5,
                                y as f32 + 1.0,
                                (cz * CHUNK_DEPTH as i32 + z as i32) as f32 + 0.5,
                            ));
                        }
                    }
                }
            }
        }
        for pos in spawners {
            let kind = [MobKind::Zombie, MobKind::Skeleton, MobKind::Spider][next() as usize % 3];
            let _ = self.spawn_mob(kind, pos, next() as u8);
        }
    }
}

#[cfg(test)]
mod collision_tests {
    use super::*;

    fn pool() -> World {
        let mut world = World::simulation(42);
        let mut chunk = Chunk::new(0, 0, 42);
        for x in 0..16 {
            for z in 0..16 {
                chunk.set_block(x, 59, z, BlockType::Stone);
                for y in 60..64 {
                    chunk.set_block(x, y, z, BlockType::Water);
                }
            }
        }
        world.insert_chunk(chunk);
        world
    }

    #[test]
    fn aquatic_spawn_requires_the_entire_body_underwater() {
        for kind in MobKind::ALL
            .iter()
            .copied()
            .filter(|k| k.species().motion == Motion::Swim)
        {
            let mut world = pool();
            let height = kind.species().height;
            let pos = Vec3::new(8.0, 64.0 - height * 0.9, 8.0);
            assert!(world.spawn_mob(kind, pos, 0).is_err(), "{kind:?}");
        }
    }

    #[test]
    fn partially_submerged_swimmers_sink_back_into_water() {
        for kind in MobKind::ALL
            .iter()
            .copied()
            .filter(|k| k.species().motion == Motion::Swim)
        {
            let mut world = pool();
            let pos = Vec3::new(8.0, 63.75, 8.0);
            world.mobs.push(Mob::new(kind, pos, pos, 0));
            for _ in 0..90 {
                world.update_mobs(Vec3::new(8.0, 70.0, 8.0), 1.0 / 60.0, BlockType::Air);
            }
            let mob = &world.mobs[0];
            assert!(
                world.mob_in_water(mob.position, mob.half_width(), mob.height()),
                "{kind:?}: {:?}",
                mob.position
            );
        }
    }

    #[test]
    fn shallow_flow_does_not_count_as_full_water() {
        let mut world = pool();
        world.set_liquid_level(8, 63, 8, 8);
        assert!(!world.mob_in_water(Vec3::new(8.5, 63.5, 8.5), 0.2, 0.3));
    }

    #[test]
    fn aquatic_animals_fall_when_their_pool_is_drained() {
        for kind in MobKind::ALL
            .iter()
            .copied()
            .filter(|k| matches!(k.species().motion, Motion::Swim | Motion::Amphibious))
        {
            let mut world = pool();
            let pos = Vec3::new(8.0, 62.0, 8.0);
            world.mobs.push(Mob::new(kind, pos, pos, 0));
            for x in 0..16 {
                for z in 0..16 {
                    for y in 60..64 {
                        world.set_block(x, y, z, BlockType::Air);
                    }
                }
            }
            for _ in 0..20 {
                world.update_mobs(Vec3::new(8.0, 70.0, 8.0), 1.0 / 60.0, BlockType::Air);
            }
            assert!(world.mobs[0].position.y < 61.0, "{kind:?}");
            assert!(world.mobs[0].position.y >= 60.0, "{kind:?}");
        }
    }

    #[test]
    fn swimmers_stay_submerged_at_the_surface_and_shore() {
        for kind in MobKind::ALL
            .iter()
            .copied()
            .filter(|k| k.species().motion == Motion::Swim)
        {
            let mut world = pool();
            for x in 10..16 {
                for z in 0..16 {
                    for y in 60..64 {
                        world.set_block(x, y, z, BlockType::Air);
                    }
                }
            }
            let pos = Vec3::new(8.0, 64.0 - kind.species().height - 0.01, 8.0);
            world.spawn_mob(kind, pos, 0).unwrap();
            world.mobs[0].home.y = 70.0;
            for _ in 0..300 {
                world.update_mobs(Vec3::new(12.0, 70.0, 8.0), 1.0 / 60.0, BlockType::Air);
                let mob = &world.mobs[0];
                assert!(
                    world.mob_in_water(mob.position, mob.half_width(), mob.height()),
                    "{kind:?}: {:?}",
                    mob.position
                );
            }
        }
    }
    fn cow(p: Vec3) -> Mob {
        Mob::new(MobKind::Cow, p, p, 0)
    }

    #[test]
    fn movement_cannot_cross_another_mob_even_in_one_fast_step() {
        let a = cow(Vec3::new(0., 200., 0.));
        let b = cow(Vec3::new(2., 200., 0.));
        assert!(crosses_mob(&a, a.position, Vec3::new(4., 200., 0.), &b));
        assert!(!crosses_mob(&a, a.position, Vec3::new(-2., 200., 0.), &b));
        assert!(!crosses_mob(&a, a.position, Vec3::new(4., 200., 4.), &b));
    }

    #[test]
    fn collision_respects_vertical_clearance_babies_and_dead_mobs() {
        let a = cow(Vec3::new(0., 200., 0.));
        let mut b = cow(Vec3::new(2., 202., 0.));
        assert!(!crosses_mob(&a, a.position, Vec3::new(4., 200., 0.), &b));
        b.position = Vec3::new(0.7, 200., 0.);
        assert!(crosses_mob(&a, a.position, a.position, &b));
        b.animal.growth = 1200.;
        assert!(!crosses_mob(&a, a.position, a.position, &b));
        b.position.x = 0.1;
        b.health = 0.;
        assert!(!crosses_mob(&a, a.position, a.position, &b));
    }

    #[test]
    fn touching_mobs_can_move_apart_or_slide() {
        let a = cow(Vec3::new(0., 200., 0.));
        let b = cow(Vec3::new(a.half_width() * 2., 200., 0.));
        assert!(!crosses_mob(&a, a.position, a.position - Vec3::X, &b));
        assert!(!crosses_mob(&a, a.position, a.position + Vec3::Z, &b));
        assert!(crosses_mob(&a, a.position, a.position + Vec3::X, &b));
    }

    #[test]
    fn overlapping_mobs_separate_without_being_pushed_through_a_wall() {
        let mut mobs = vec![
            cow(Vec3::new(3.5, 200., 3.5)),
            cow(Vec3::new(3.5, 200., 3.5)),
        ];
        resolve_mob_overlaps(&mut mobs, |mob, p| p.z + mob.half_width() < 4.0);
        assert!(!crosses_mob(
            &mobs[0],
            mobs[0].position,
            mobs[0].position,
            &mobs[1]
        ));
        for mob in &mobs {
            assert!(mob.position.z + mob.half_width() < 4.);
        }
    }
}
