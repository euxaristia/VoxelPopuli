use super::*;
use crate::mob_catalog::{Temper, can_spawn};

impl World {
    pub(super) fn mob_in_water(&self, pos: Vec3, hw: f32, height: f32) -> bool {
        for x in [pos.x - hw * 0.8, pos.x, pos.x + hw * 0.8] {
            for z in [pos.z - hw * 0.8, pos.z, pos.z + hw * 0.8] {
                for y in [pos.y + 0.05, pos.y + height * 0.8] {
                    if self.get_block(x.floor() as i32, y.floor() as i32, z.floor() as i32)
                        != BlockType::Water
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
        if self.mobs.iter().any(|m| {
            (m.position.y - pos.y).abs() < m.height().max(mob.height())
                && (m.position - pos).with_y(0.0).length() < m.half_width() + mob.half_width()
        }) {
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
        let biome = crate::chunk::biome_at(x as f32, z as f32, self.seed);
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
                        && (!hostiles_only || k.species().temper == Temper::Hostile)
                })
                .collect();
            if options.is_empty() {
                return;
            }
            let kind = options[random as usize % options.len()];
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
