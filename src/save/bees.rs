use super::*;
use crate::bee::{BeeState, BlockPos};

#[derive(Clone, Debug, PartialEq)]
pub struct SavedBee {
    pub position: Vec3,
    pub home: Vec3,
    pub health: f32,
    pub growth: f32,
    pub love: f32,
    pub cooldown: f32,
    pub anger: f32,
    pub state: BeeState,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BeeSaveData {
    pub bees: Vec<SavedBee>,
    pub hives: Vec<(BlockPos, u8)>,
    pub raining: bool,
    pub weather_timer: f32,
    pub poison_time: f32,
    pub poison_tick: f32,
}

impl Default for BeeSaveData {
    fn default() -> Self {
        Self {
            bees: Vec::new(),
            hives: Vec::new(),
            raining: false,
            weather_timer: 900.0,
            poison_time: 0.0,
            poison_tick: 0.0,
        }
    }
}

impl BeeSaveData {
    pub fn capture(world: &World, player: &Player) -> Self {
        let mut hives: Vec<_> = world.hives.iter().map(|(p, h)| (*p, *h)).collect();
        hives.sort_unstable_by_key(|v| v.0);
        Self {
            hives,
            raining: world.raining,
            weather_timer: world.weather_timer,
            poison_time: player.poison_time,
            poison_tick: player.poison_tick,
            bees: world
                .mobs
                .iter()
                .filter(|m| m.kind == crate::mob::MobKind::Bee && m.health > 0.0)
                .map(|m| {
                    let mut state = m.bee.clone();
                    state.goal = None;
                    state.goal_age = 0.0;
                    state.path.clear();
                    state.alert_pending = false;
                    SavedBee {
                        position: m.position,
                        home: m.home,
                        health: m.health,
                        growth: m.animal.growth,
                        love: m.animal.love_time,
                        cooldown: m.animal.breed_cooldown,
                        anger: m.anger_time,
                        state,
                    }
                })
                .collect(),
        }
    }
    pub fn restore(&self, world: &mut World, player: &mut Player) {
        // Nests discovered during loading already have newly generated occupants.
        // Replace saved occupants, retaining bees belonging to previously unseen nests.
        let saved_hives: std::collections::HashSet<_> =
            self.hives.iter().map(|(p, _)| *p).collect();
        world.mobs.retain(|m| {
            m.kind != crate::mob::MobKind::Bee
                || m.bee.hive.is_some_and(|p| !saved_hives.contains(&p))
        });
        world.hives.extend(self.hives.iter().copied());
        world.raining = self.raining;
        world.weather_timer = self.weather_timer;
        player.poison_time = self.poison_time;
        player.poison_tick = self.poison_tick;
        for saved in &self.bees {
            let mut mob =
                crate::mob::Mob::new(crate::mob::MobKind::Bee, saved.position, saved.home, 0);
            mob.health = saved.health;
            mob.animal.growth = saved.growth;
            mob.animal.love_time = saved.love;
            mob.animal.breed_cooldown = saved.cooldown;
            mob.anger_time = saved.anger;
            mob.bee = saved.state.clone();
            world.mobs.push(mob);
        }
    }
    pub(super) fn encode(&self, out: &mut Vec<u8>) -> io::Result<()> {
        if self.bees.len() > crate::world::MOB_CAP || self.hives.len() > MAX_CONTAINERS {
            return Err(invalid("too many bees or hives"));
        }
        put_bool(out, self.raining);
        for v in [self.weather_timer, self.poison_time, self.poison_tick] {
            put_f32(out, v);
        }
        put_u32(out, self.hives.len() as u32);
        for (p, h) in &self.hives {
            put_pos(out, Some(*p));
            out.push(*h);
        }
        put_u32(out, self.bees.len() as u32);
        for b in &self.bees {
            put_vec3(out, b.position);
            put_vec3(out, b.home);
            for v in [
                b.health,
                b.growth,
                b.love,
                b.cooldown,
                b.anger,
                b.state.pollination,
                b.state.residence,
                b.state.search_time,
                b.state.retry,
                b.state.sting_death,
            ] {
                put_f32(out, v);
            }
            put_pos(out, b.state.hive);
            put_pos(out, b.state.flower);
            put_bool(out, b.state.inside);
            put_bool(out, b.state.nectar);
            out.push(b.state.crop_charges);
            out.extend_from_slice(&b.state.random.to_le_bytes());
        }
        Ok(())
    }
    pub(super) fn decode(r: &mut Reader<'_>) -> io::Result<Self> {
        let raining = r.bool()?;
        let weather_timer = timer(r, 1800.0)?;
        let poison_time = timer(r, 18.0)?;
        let poison_tick = timer(r, 1.25)?;
        let count = r.u32()? as usize;
        if count > MAX_CONTAINERS {
            return Err(invalid("too many hives"));
        }
        let mut hives = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for _ in 0..count {
            let p = read_pos(r)?.ok_or_else(|| invalid("missing hive position"))?;
            let h = r.u8()?;
            if h > 5 || !seen.insert(p) {
                return Err(invalid("invalid or duplicate hive"));
            }
            hives.push((p, h));
        }
        let count = r.u32()? as usize;
        if count > crate::world::MOB_CAP {
            return Err(invalid("too many bees"));
        }
        let mut bees = Vec::new();
        let mut occupancy = std::collections::HashMap::<BlockPos, usize>::new();
        for _ in 0..count {
            let position = r.vec3()?;
            let home = r.vec3()?;
            let health = timer(r, 10.0)?;
            let growth = timer(r, 1200.0)?;
            let love = timer(r, 30.0)?;
            let cooldown = timer(r, 300.0)?;
            let anger = timer(r, 25.0)?;
            let pollination = timer(r, 20.0)?;
            let residence = timer(r, 120.0)?;
            let search_time = timer(r, f32::MAX)?;
            let retry = timer(r, 20.0)?;
            let sting_death = timer(r, 60.0)?;
            let hive = read_pos(r)?;
            let flower = read_pos(r)?;
            let inside = r.bool()?;
            let nectar = r.bool()?;
            let crop_charges = r.u8()?;
            let random = u64::from_le_bytes(r.take(8)?.try_into().unwrap());
            if health <= 0.0
                || position.x.abs() > 30_000_000.0
                || position.z.abs() > 30_000_000.0
                || home.x.abs() > 30_000_000.0
                || home.z.abs() > 30_000_000.0
                || position.y < 1.0
                || position.y >= crate::chunk::CHUNK_HEIGHT as f32
                || crop_charges > 10
            {
                return Err(invalid("invalid bee state"));
            }
            if inside {
                let p = hive.ok_or_else(|| invalid("housed bee missing hive"))?;
                let n = occupancy.entry(p).or_default();
                *n += 1;
                if *n > 3 || !seen.contains(&p) {
                    return Err(invalid("invalid hive occupancy"));
                }
            }
            bees.push(SavedBee {
                position,
                home,
                health,
                growth,
                love,
                cooldown,
                anger,
                state: BeeState {
                    hive,
                    inside,
                    nectar,
                    pollination,
                    residence,
                    search_time,
                    retry,
                    sting_death,
                    crop_charges,
                    flower,
                    random,
                    ..Default::default()
                },
            });
        }
        Ok(Self {
            bees,
            hives,
            raining,
            weather_timer,
            poison_time,
            poison_tick,
        })
    }
}
fn timer(r: &mut Reader<'_>, max: f32) -> io::Result<f32> {
    let v = r.f32()?;
    if !(0.0..=max).contains(&v) {
        Err(invalid("invalid bee timer"))
    } else {
        Ok(v)
    }
}
fn put_pos(out: &mut Vec<u8>, p: Option<BlockPos>) {
    put_bool(out, p.is_some());
    if let Some((x, y, z)) = p {
        for v in [x, y, z] {
            out.extend_from_slice(&v.to_le_bytes());
        }
    }
}
fn read_pos(r: &mut Reader<'_>) -> io::Result<Option<BlockPos>> {
    if !r.bool()? {
        return Ok(None);
    }
    let p = (r.i32()?, r.i32()?, r.i32()?);
    if !(0..crate::chunk::CHUNK_HEIGHT as i32).contains(&p.1)
        || p.0.unsigned_abs() > 30_000_000
        || p.2.unsigned_abs() > 30_000_000
    {
        return Err(invalid("invalid bee block height"));
    }
    Ok(Some(p))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn restoring_empty_save_retains_newly_discovered_nest_occupants() {
        let mut world = World::simulation(42);
        let p = Vec3::new(8.5, 65.0, 8.5);
        let mut bee = crate::mob::Mob::new(crate::mob::MobKind::Bee, p, p, 0);
        bee.bee.hive = Some((8, 65, 8));
        bee.bee.inside = true;
        world.mobs.push(bee);
        world.hives.insert((8, 65, 8), 0);
        BeeSaveData::default().restore(&mut world, &mut Player::new(64.0));
        assert_eq!(world.mobs.len(), 1);
        assert_eq!(world.hives.len(), 1);
    }
    #[test]
    fn bee_hive_and_poison_state_survive_save_restore() {
        let mut world = World::simulation(42);
        let p = Vec3::new(8.5, 65.2, 9.6);
        let h = (8, 65, 8);
        let mut mob = crate::mob::Mob::new(crate::mob::MobKind::Bee, p, p, 0);
        mob.bee.hive = Some(h);
        mob.bee.inside = true;
        mob.bee.nectar = true;
        mob.bee.residence = 83.5;
        mob.animal.growth = 500.0;
        world.mobs.push(mob);
        world.hives.insert(h, 4);
        world.raining = true;
        let mut player = Player::new(64.0);
        player.poison_time = 7.5;
        player.poison_tick = 0.75;
        let saved = BeeSaveData::capture(&world, &player);
        let mut bytes = Vec::new();
        saved.encode(&mut bytes).unwrap();
        let decoded = BeeSaveData::decode(&mut Reader::new(&bytes)).unwrap();
        assert_eq!(decoded, saved);
        let mut restored = World::simulation(42);
        let mut p2 = Player::new(64.0);
        decoded.restore(&mut restored, &mut p2);
        assert_eq!(restored.mobs[0].bee.residence, 83.5);
        assert_eq!(restored.hives[&h], 4);
        assert!(restored.mobs[0].bee.inside && restored.raining);
        assert_eq!(p2.poison_time, 7.5);
        decoded.restore(&mut restored, &mut p2);
        assert_eq!(restored.mobs.len(), 1);
    }
    #[test]
    fn malformed_hive_state_and_truncation_are_rejected() {
        let mut state = BeeSaveData::default();
        state.hives.push(((1, 64, 1), 6));
        let mut bytes = Vec::new();
        state.encode(&mut bytes).unwrap();
        assert!(BeeSaveData::decode(&mut Reader::new(&bytes)).is_err());
        state.hives[0].1 = 5;
        bytes.clear();
        state.encode(&mut bytes).unwrap();
        for end in 0..bytes.len() {
            assert!(BeeSaveData::decode(&mut Reader::new(&bytes[..end])).is_err());
        }
    }
}
