use crate::block::BlockType;
use crate::item;
use crate::sprint;
use crate::world::World;
use glam::Vec3;

pub const STANDING_EYE_HEIGHT: f32 = 1.62;
const CROUCHING_EYE_HEIGHT: f32 = 1.27;

fn spawn_clear(block: BlockType) -> bool {
    matches!(block, BlockType::Air | BlockType::SnowLayer)
}

/// New worlds start above the terrain, including snow, rather than in a cave.
pub fn surface_spawn_near(
    target: Vec3,
    mut block_at: impl FnMut(i32, i32, i32) -> BlockType,
) -> Option<Vec3> {
    let (x, z) = (target.x.floor() as i32, target.z.floor() as i32);
    let mut water_fallback = None;
    for radius in 0i32..=32 {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx.abs().max(dz.abs()) != radius {
                    continue;
                }
                for y in (0..crate::chunk::CHUNK_HEIGHT as i32).rev() {
                    let floor = block_at(x + dx, y, z + dz);
                    if spawn_clear(floor) {
                        continue;
                    }
                    let position =
                        Vec3::new((x + dx) as f32 + 0.5, (y + 1) as f32, (z + dz) as f32 + 0.5);
                    if floor == BlockType::Water {
                        water_fallback.get_or_insert(position);
                    }
                    if floor.is_solid()
                        && floor != BlockType::Cactus
                        && spawn_clear(block_at(x + dx, y + 1, z + dz))
                        && spawn_clear(block_at(x + dx, y + 2, z + dz))
                    {
                        return Some(position);
                    }
                    // A blocked or liquid surface must not send us down into a cave.
                    break;
                }
            }
        }
    }
    water_fallback
}

pub fn safe_spawn_near(
    target: Vec3,
    mut block_at: impl FnMut(i32, i32, i32) -> BlockType,
) -> Option<Vec3> {
    let (x, z) = (target.x.floor() as i32, target.z.floor() as i32);
    let preferred_y = (target.y.floor() as i32).clamp(1, crate::chunk::CHUNK_HEIGHT as i32 - 2);
    let mut heights = (1..crate::chunk::CHUNK_HEIGHT as i32 - 1).collect::<Vec<_>>();
    heights.sort_unstable_by_key(|y| (y - preferred_y).abs());
    for radius in 0i32..=4 {
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                if dx.abs().max(dz.abs()) != radius {
                    continue;
                }
                for &y in &heights {
                    let floor = block_at(x + dx, y - 1, z + dz);
                    if floor.is_solid()
                        && floor != BlockType::Cactus
                        && spawn_clear(block_at(x + dx, y, z + dz))
                        && spawn_clear(block_at(x + dx, y + 1, z + dz))
                    {
                        return Some(Vec3::new(
                            (x + dx) as f32 + 0.5,
                            y as f32,
                            (z + dz) as f32 + 0.5,
                        ));
                    }
                }
            }
        }
    }
    None
}

pub struct Player {
    pub poison_time: f32,
    pub poison_tick: f32,
    pub position: Vec3,
    pub velocity: Vec3,
    pub grounded: bool,
    pub air_seconds: f32,
    pub inventory_open: bool,
    pub selected_slot: usize,
    pub health: i32,
    pub hunger: i32,
    pub saturation: f32,
    pub exhaustion: f32,
    pub sandbox: bool,
    pub hunger_timer: f32,
    pub equipped_armor: [Option<(BlockType, u16)>; 4],
    pub xp_level: u32,
    pub xp_progress: f32,
    pub total_xp: u32,
    pub flying: bool,
    pub sprinting: bool,
    pub sneaking: bool,
    pub crouch_amount: f32,
    pub last_space_release: f64,
    pub space_was_pressed: bool,
    pub damage_cooldown: f32,
    pub drowning_timer: f32,
    pub fall_distance: f32,
    pub spawn_point: Option<Vec3>,
    pub attack_cooldown: f32,
    pub hurt_time: f32,
    pub hurt_direction: f32,
}

impl Player {
    pub fn new(spawn_y: f32) -> Self {
        Self {
            position: Vec3::new(32.5, spawn_y, 32.5),
            velocity: Vec3::ZERO,
            grounded: false,
            air_seconds: 15.0,
            inventory_open: false,
            selected_slot: 0,
            health: 20,
            hunger: 20,
            saturation: 5.0,
            exhaustion: 0.0,
            sandbox: false,
            hunger_timer: 0.0,
            equipped_armor: [None, None, None, None],
            xp_level: 0,
            xp_progress: 0.0,
            total_xp: 0,
            flying: false,
            sprinting: false,
            sneaking: false,
            crouch_amount: 0.0,
            poison_time: 0.0,
            poison_tick: 0.0,
            last_space_release: 0.0,
            space_was_pressed: false,
            damage_cooldown: 0.0,
            drowning_timer: 1.0,
            fall_distance: 0.0,
            spawn_point: None,
            attack_cooldown: 0.0,
            hurt_time: 0.0,
            hurt_direction: 0.0,
        }
    }

    pub fn total_armor_defense(&self) -> i32 {
        let mut total = 0;
        for (item, _) in self.equipped_armor.iter().flatten() {
            if let Some(props) = item::armor_properties(*item) {
                total += props.defense;
            }
        }
        total.min(20)
    }

    pub fn take_damage(&mut self, base_damage: i32) {
        self.take_damage_from(base_damage, None, 0.0);
    }

    pub fn take_damage_from(
        &mut self,
        base_damage: i32,
        source_pos: Option<Vec3>,
        player_yaw: f32,
    ) {
        if self.sandbox {
            return;
        }
        let defense = self.total_armor_defense();
        let reduction = (defense as f32 * 0.04).min(0.80);
        let actual_damage = ((base_damage as f32) * (1.0 - reduction)).round() as i32;
        let actual_damage = actual_damage.max(1);
        self.health = (self.health - actual_damage).max(0);
        self.hurt_time = 0.5;
        self.hurt_direction = if let Some(source) = source_pos {
            let dx = source.x - self.position.x;
            let dz = source.z - self.position.z;
            if dx.hypot(dz) > 1e-4 {
                let h = dx.atan2(dz) - player_yaw;
                (h + std::f32::consts::PI).rem_euclid(2.0 * std::f32::consts::PI)
                    - std::f32::consts::PI
            } else {
                0.0
            }
        } else {
            0.0
        };

        for slot in &mut self.equipped_armor {
            if let Some((_item, durability)) = slot {
                if *durability > 1 {
                    *durability -= 1;
                } else {
                    *slot = None;
                }
            }
        }
    }

    pub fn respawn(&mut self, fallback: Vec3) {
        self.position = self.spawn_point.unwrap_or(fallback);
        self.velocity = Vec3::ZERO;
        self.grounded = false;
        self.health = 20;
        self.hunger = 20;
        self.saturation = 5.0;
        self.exhaustion = 0.0;
        self.hunger_timer = 0.0;
        self.air_seconds = 15.0;
        self.damage_cooldown = 0.0;
        self.drowning_timer = 1.0;
        self.fall_distance = 0.0;
        self.flying = false;
        self.sprinting = false;
        self.sneaking = false;
        self.crouch_amount = 0.0;
        self.poison_time = 0.0;
        self.poison_tick = 0.0;
        self.attack_cooldown = 0.0;
        self.hurt_time = 0.0;
        self.hurt_direction = 0.0;
        self.inventory_open = false;
        self.space_was_pressed = false;
        self.last_space_release = f64::NEG_INFINITY;
    }

    pub fn exhaust(&mut self, amount: f32) {
        if self.sandbox || !amount.is_finite() || amount <= 0.0 {
            return;
        }
        self.exhaustion += amount;
        while self.exhaustion >= 4.0 {
            self.exhaustion -= 4.0;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else {
                self.hunger = (self.hunger - 1).max(0);
            }
        }
    }

    fn apply_starvation_tick(&mut self) {
        self.health = (self.health - 1).max(0);
    }

    pub fn eat_food(&mut self, props: item::FoodProperties) -> bool {
        if self.hunger >= 20 {
            return false;
        }
        self.hunger = (self.hunger + props.hunger_restored).min(20);
        self.saturation = (self.saturation + props.saturation_restored).min(self.hunger as f32);
        true
    }

    pub fn equip_armor(
        &mut self,
        stack: crate::inventory::ItemStack,
    ) -> Option<crate::inventory::ItemStack> {
        let props = item::armor_properties(stack.block)?;
        let slot = &mut self.equipped_armor[props.slot as usize];
        let previous = slot.map(|(block, durability)| crate::inventory::ItemStack {
            block,
            count: 1,
            durability: Some(durability),
        });
        *slot = Some((
            stack.block,
            stack
                .durability
                .unwrap_or(props.durability)
                .min(props.durability),
        ));
        previous
    }

    pub fn add_xp(&mut self, amount: u32) {
        self.total_xp += amount;
        let mut rem = amount as f32;
        while rem > 0.0 {
            let xp_needed = (7 + self.xp_level * 7) as f32;
            let current_xp = self.xp_progress * xp_needed;
            if current_xp + rem >= xp_needed {
                rem -= xp_needed - current_xp;
                self.xp_level += 1;
                self.xp_progress = 0.0;
            } else {
                self.xp_progress = (current_xp + rem) / xp_needed;
                rem = 0.0;
            }
        }
    }

    fn is_point_in_block(world: &World, p: Vec3) -> bool {
        let bx = p.x.floor() as i32;
        let by = p.y.floor() as i32;
        let bz = p.z.floor() as i32;
        let b = world.get_block(bx, by, bz);
        if b == BlockType::Cactus {
            let lx = p.x - bx as f32;
            let lz = p.z - bz as f32;
            return lx >= 1.0 / 16.0 && lx <= 15.0 / 16.0 && lz >= 1.0 / 16.0 && lz <= 15.0 / 16.0;
        }
        if matches!(b, BlockType::OakDoor | BlockType::IronDoor) {
            let lz = p.z - bz as f32;
            return lz >= 0.0 && lz <= 3.0 / 16.0;
        }
        b.is_solid()
    }

    pub fn eye_position(&self) -> Vec3 {
        self.position
            + Vec3::Y
                * (STANDING_EYE_HEIGHT
                    + (CROUCHING_EYE_HEIGHT - STANDING_EYE_HEIGHT) * self.crouch_amount)
    }

    fn body_height(&self) -> f32 {
        if self.sneaking { 1.5 } else { 1.8 }
    }

    fn check_collision(world: &World, pos: Vec3, height: f32) -> bool {
        let w = 0.22;
        let h = height - 0.05;
        for x_off in [-w, w] {
            for z_off in [-w, w] {
                for yi in 0..=2 {
                    let y = 0.1 + yi as f32 * ((h - 0.1) / 2.0);
                    if Self::is_point_in_block(
                        world,
                        Vec3::new(pos.x + x_off, pos.y + y, pos.z + z_off),
                    ) {
                        return true;
                    }
                }
            }
        }
        false
    }

    pub fn intersects_block(&self, bx: i32, by: i32, bz: i32) -> bool {
        let w = 0.3;
        let player_min = self.position - Vec3::new(w, 0.0, w);
        let player_max = self.position + Vec3::new(w, self.body_height(), w);
        let block_min = Vec3::new(bx as f32, by as f32, bz as f32);
        let block_max = block_min + Vec3::ONE;

        player_max.x > block_min.x
            && player_min.x < block_max.x
            && player_max.y > block_min.y
            && player_min.y < block_max.y
            && player_max.z > block_min.z
            && player_min.z < block_max.z
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update(
        &mut self,
        world: &World,
        facing: Vec3,
        move_input: Vec3,
        dt: f32,
        is_sprinting: bool,
        is_jumping: bool,
        is_sneaking: bool,
        current_time: f64,
    ) {
        if self.attack_cooldown > 0.0 {
            self.attack_cooldown = (self.attack_cooldown - dt).max(0.0);
        }
        if !self.sandbox && self.poison_time > 0.0 {
            let elapsed = dt.min(self.poison_time);
            self.poison_time = (self.poison_time - dt).max(0.0);
            self.poison_tick += elapsed;
            while self.poison_tick >= 1.25 {
                self.poison_tick -= 1.25;
                self.health = (self.health - 1).max(1).min(self.health);
            }
        } else {
            self.poison_tick = 0.0;
        }
        if !self.sandbox {
            self.hunger_timer += dt;
            if self.hunger_timer >= 4.0 {
                self.hunger_timer -= 4.0;
                if self.hunger >= 18 && self.health < 20 {
                    self.health = (self.health + 1).min(20);
                    self.exhaust(6.0);
                } else if self.hunger == 0 {
                    self.apply_starvation_tick();
                }
            }
        }

        let in_lava = world.get_block(
            self.position.x.floor() as i32,
            (self.position.y + 0.9).floor() as i32,
            self.position.z.floor() as i32,
        ) == BlockType::Lava
            || world.get_block(
                self.position.x.floor() as i32,
                (self.position.y + 0.1).floor() as i32,
                self.position.z.floor() as i32,
            ) == BlockType::Lava;
        let touching_cactus = !self.sandbox && {
            let min_x = (self.position.x - 0.31).floor() as i32;
            let max_x = (self.position.x + 0.31).floor() as i32;
            let min_y = (self.position.y - 0.05).floor() as i32;
            let max_y = (self.position.y + self.body_height()).floor() as i32;
            let min_z = (self.position.z - 0.31).floor() as i32;
            let max_z = (self.position.z + 0.31).floor() as i32;
            let mut found = false;
            'check: for bx in min_x..=max_x {
                for by in min_y..=max_y {
                    for bz in min_z..=max_z {
                        if world.get_block(bx, by, bz) == BlockType::Cactus {
                            found = true;
                            break 'check;
                        }
                    }
                }
            }
            found
        };
        if in_lava {
            self.damage_cooldown -= dt;
            if self.damage_cooldown <= 0.0 {
                self.take_damage(2);
                self.damage_cooldown = 0.5;
            }
        } else if touching_cactus {
            self.damage_cooldown -= dt;
            if self.damage_cooldown <= 0.0 {
                self.take_damage(1);
                self.damage_cooldown = 0.5;
            }
        } else if self.damage_cooldown > 0.0 {
            self.damage_cooldown = (self.damage_cooldown - dt).max(0.0);
        }
        self.hurt_time = (self.hurt_time - dt).max(0.0);

        let controls_enabled = !self.inventory_open;
        let is_jumping = is_jumping && controls_enabled;
        let is_sneaking = is_sneaking && controls_enabled;
        let move_input = if controls_enabled {
            move_input
        } else {
            Vec3::ZERO
        };

        let space_just_pressed = is_jumping && !self.space_was_pressed;
        if self.sandbox && space_just_pressed && !self.grounded {
            let time_since_last = current_time - self.last_space_release;
            if time_since_last < 0.35 {
                self.flying = !self.flying;
                self.velocity.y = 0.0;
            }
        }
        if !is_jumping && self.space_was_pressed {
            self.last_space_release = current_time;
        }
        self.space_was_pressed = is_jumping;

        if self.grounded && self.flying {
            self.flying = false;
        }

        self.sneaking = !self.flying
            && (is_sneaking || (self.sneaking && Self::check_collision(world, self.position, 1.8)));
        let target = if self.sneaking { 1.0 } else { 0.0 };
        self.crouch_amount += (target - self.crouch_amount) * (1.0 - (-20.0 * dt).exp());

        let waist_in_w = world.get_block(
            self.position.x.floor() as i32,
            (self.position.y + 0.9).floor() as i32,
            self.position.z.floor() as i32,
        ) == BlockType::Water;
        let feet_in_w = world.get_block(
            self.position.x.floor() as i32,
            (self.position.y + 0.1).floor() as i32,
            self.position.z.floor() as i32,
        ) == BlockType::Water;
        let head_in_w = world.get_block(
            self.position.x.floor() as i32,
            self.eye_position().y.floor() as i32,
            self.position.z.floor() as i32,
        ) == BlockType::Water;
        let in_water = waist_in_w || feet_in_w;

        self.sprinting = is_sprinting
            && controls_enabled
            && !self.sneaking
            && !self.flying
            && !in_water
            && !in_lava
            && self.health > 0
            && (self.sandbox || self.hunger > 6)
            && move_input.dot(facing) > 0.0;
        let position_before_move = self.position;

        let mut mv = move_input;
        if mv.length_squared() > 0.1 {
            mv = mv.normalize();
            let speed = if self.flying {
                10.92
            } else if in_water {
                2.0
            } else if self.sprinting {
                sprint::SPRINT_SPEED
            } else if self.sneaking {
                sprint::WALK_SPEED * 0.3
            } else {
                sprint::WALK_SPEED
            };
            mv *= speed;
        }

        if self.flying {
            self.fall_distance = 0.0;
            let fly_drag = 0.09f32.powf(dt);
            self.velocity.x = self.velocity.x * (1.0 - fly_drag) + mv.x * fly_drag;
            self.velocity.z = self.velocity.z * (1.0 - fly_drag) + mv.z * fly_drag;

            let fly_speed = 7.8;
            if is_jumping {
                self.velocity.y = fly_speed;
            } else if is_sneaking {
                self.velocity.y = -fly_speed;
            } else {
                self.velocity.y *= 0.6f32.powf(dt * 20.0);
            }
        } else {
            let friction_multiplier = if in_water {
                0.8
            } else if !self.grounded {
                0.98
            } else {
                0.6
            };
            self.velocity.x = self.velocity.x * (1.0 - friction_multiplier * dt * 20.0)
                + mv.x * friction_multiplier * dt * 20.0;
            self.velocity.z = self.velocity.z * (1.0 - friction_multiplier * dt * 20.0)
                + mv.z * friction_multiplier * dt * 20.0;

            if in_water {
                self.fall_distance = 0.0;
                if is_jumping {
                    self.velocity.y += 0.04 * 20.0;
                    if self.velocity.y > 2.0 {
                        self.velocity.y = 2.0;
                    }
                } else {
                    self.velocity.y -= 0.02 * 20.0;
                    if self.velocity.y < -2.0 {
                        self.velocity.y = -2.0;
                    }
                }
            } else {
                self.velocity.y -= 32.0 * dt;
                let fall_drag = 0.98f32.powf(dt * 20.0);
                self.velocity.y *= fall_drag;
                if is_jumping && self.grounded {
                    self.velocity.y = 8.4;
                    self.grounded = false;
                    if self.sprinting {
                        let boost =
                            facing.with_y(0.0).normalize_or_zero() * sprint::SPRINT_JUMP_BOOST;
                        self.velocity.x += boost.x;
                        self.velocity.z += boost.z;
                        self.exhaust(sprint::SPRINT_JUMP_EXHAUSTION);
                    } else {
                        self.exhaust(sprint::JUMP_EXHAUSTION);
                    }
                }
            }
        }

        let dy = self.velocity.y * dt;
        if self.velocity.y < 0.0 && !self.flying && !in_water {
            self.fall_distance += -dy;
        }
        self.position.y += dy;
        if Self::check_collision(world, self.position, self.body_height()) {
            if self.velocity.y <= 0.0 {
                self.grounded = true;
                let fall_damage = (self.fall_distance - 3.0).floor() as i32;
                if fall_damage > 0 {
                    self.take_damage(fall_damage);
                }
                self.fall_distance = 0.0;
            }
            self.position.y -= dy;
            self.velocity.y = 0.0;
        } else if self.velocity.y != 0.0 {
            self.grounded = false;
        }

        let sprinted = self.sprinting;
        let dx = self.velocity.x * dt;
        self.position.x += dx;
        if Self::check_collision(world, self.position, self.body_height()) {
            self.position.x -= dx;
            self.velocity.x = 0.0;
            self.sprinting = false;
        }

        let dz = self.velocity.z * dt;
        self.position.z += dz;
        if Self::check_collision(world, self.position, self.body_height()) {
            self.position.z -= dz;
            self.velocity.z = 0.0;
            self.sprinting = false;
        }

        // Charge accepted movement, never attempted movement into a wall.
        let displacement = self.position - position_before_move;
        if !self.flying && in_water {
            self.exhaust(displacement.length() * sprint::SWIM_EXHAUSTION_PER_BLOCK);
        } else if self.grounded && sprinted {
            self.exhaust(displacement.with_y(0.0).length() * sprint::SPRINT_EXHAUSTION_PER_BLOCK);
        }
        if !self.sandbox && self.hunger <= 6 {
            self.sprinting = false;
        }

        if head_in_w {
            self.air_seconds = (self.air_seconds - dt).max(0.0);
            if self.air_seconds <= 0.0 {
                self.drowning_timer -= dt;
                if self.drowning_timer <= 0.0 {
                    self.take_damage(2);
                    self.drowning_timer = 1.0;
                }
            }
        } else {
            self.air_seconds = (self.air_seconds + dt * 3.0).min(15.0);
            self.drowning_timer = 1.0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn movement_fixture() -> (World, Player) {
        let mut world = World::simulation(42);
        let mut chunk = crate::chunk::Chunk::new(0, 0, 42);
        for x in 0..16 {
            for z in 0..16 {
                chunk.blocks[x][63][z] = BlockType::Stone;
            }
        }
        world.insert_chunk(chunk);
        let mut player = Player::new(63.9);
        player.position = Vec3::new(8.5, 63.9, 8.5);
        player.grounded = true;
        (world, player)
    }

    #[test]
    fn bee_poison_uses_one_point_ticks_and_cannot_kill() {
        let (world, mut p) = movement_fixture();
        p.health = 3;
        p.poison_time = 10.0;
        for i in 0..100 {
            p.update(
                &world,
                Vec3::Z,
                Vec3::ZERO,
                0.1,
                false,
                false,
                false,
                i as f64 * 0.1,
            );
        }
        assert_eq!(p.health, 1);
        p.sandbox = true;
        p.health = 20;
        p.poison_time = 10.0;
        for i in 0..100 {
            p.update(
                &world,
                Vec3::Z,
                Vec3::ZERO,
                0.1,
                false,
                false,
                false,
                i as f64 * 0.1,
            );
        }
        assert_eq!(p.health, 20);
    }

    #[test]
    fn sprint_exhaustion_tracks_ground_distance() {
        let (world, mut player) = movement_fixture();
        let start = player.position;
        player.update(&world, Vec3::X, Vec3::X, 0.05, true, false, false, 0.0);
        let distance = (player.position - start).with_y(0.0).length();
        assert!(distance > 0.0);
        assert!((player.exhaustion - distance * 0.1).abs() < 0.00001);
    }

    #[test]
    fn blocked_sprint_does_not_consume_exhaustion() {
        let (mut world, mut player) = movement_fixture();
        for y in 64..=66 {
            world.set_block(9, y, 8, BlockType::Stone);
        }
        player.position.x = 8.75;
        player.update(&world, Vec3::X, Vec3::X, 0.05, true, false, false, 0.0);
        assert_eq!(player.position.x, 8.75);
        assert_eq!(player.exhaustion, 0.0);
        assert!(!player.sprinting);
    }

    #[test]
    fn jump_exhaustion_is_charged_once_at_takeoff() {
        let (world, mut player) = movement_fixture();
        player.update(&world, Vec3::X, Vec3::ZERO, 0.05, false, true, false, 0.0);
        assert!((player.exhaustion - 0.05).abs() < 0.00001);
        player.update(&world, Vec3::X, Vec3::ZERO, 0.05, false, true, false, 0.05);
        assert!((player.exhaustion - 0.05).abs() < 0.00001);
    }

    #[test]
    fn sprint_jump_costs_point_two_and_adds_forward_impulse() {
        let (world, mut player) = movement_fixture();
        player.update(&world, Vec3::X, Vec3::X, 0.05, true, true, false, 0.0);
        assert!((player.exhaustion - 0.2).abs() < 0.00001);
        assert!(player.velocity.x > 5.612);
    }

    #[test]
    fn sprint_jump_boost_follows_facing_while_strafing() {
        let (world, mut player) = movement_fixture();
        player.update(
            &world,
            Vec3::Z,
            Vec3::X + Vec3::Z,
            0.05,
            true,
            true,
            false,
            0.0,
        );
        assert!((player.velocity.z - player.velocity.x - 4.0).abs() < 0.00001);
    }

    #[test]
    fn sideways_and_backward_movement_cannot_sprint() {
        for movement in [Vec3::X, -Vec3::Z] {
            let (world, mut player) = movement_fixture();
            player.update(&world, Vec3::Z, movement, 0.05, true, false, false, 0.0);
            assert!(!player.sprinting);
            assert_eq!(player.exhaustion, 0.0);
        }
    }

    #[test]
    fn airborne_sprint_does_not_charge_ground_distance() {
        let (world, mut player) = movement_fixture();
        player.position.y = 70.0;
        player.grounded = false;
        player.update(&world, Vec3::X, Vec3::X, 0.05, true, false, false, 0.0);
        assert_eq!(player.exhaustion, 0.0);
    }

    #[test]
    fn sneaking_cannot_apply_sprint_speed_or_exhaustion() {
        let (world, mut player) = movement_fixture();
        player.update(&world, Vec3::X, Vec3::X, 0.05, true, false, true, 0.0);
        assert!(player.velocity.x <= sprint::WALK_SPEED * 0.3);
        assert_eq!(player.exhaustion, 0.0);
    }

    #[test]
    fn sneaking_lowers_view_and_releasing_restores_it() {
        let (world, mut player) = movement_fixture();
        for i in 0..20 {
            player.update(
                &world,
                Vec3::X,
                Vec3::ZERO,
                0.05,
                false,
                false,
                true,
                i as f64 * 0.05,
            );
        }
        assert!(player.sneaking);
        assert!((player.eye_position().y - player.position.y - 1.27).abs() < 0.001);
        assert_eq!(player.body_height(), 1.5);
        for i in 0..20 {
            player.update(
                &world,
                Vec3::X,
                Vec3::ZERO,
                0.05,
                false,
                false,
                false,
                1.0 + i as f64 * 0.05,
            );
        }
        assert!(!player.sneaking);
        assert!((player.eye_position().y - player.position.y - 1.62).abs() < 0.001);
    }

    #[test]
    fn crouch_remains_when_ceiling_blocks_standing() {
        let (mut world, mut player) = movement_fixture();
        player.position.y = 64.4;
        player.sneaking = true;
        world.set_block(8, 66, 8, BlockType::Stone);
        player.update(&world, Vec3::X, Vec3::ZERO, 0.0, true, false, false, 1.0);
        assert!(player.sneaking);
        assert!(!player.sprinting);
        assert!(!Player::check_collision(&world, player.position, 1.5));
        assert!(Player::check_collision(&world, player.position, 1.8));
        world.set_block(8, 66, 8, BlockType::Air);
        player.update(&world, Vec3::X, Vec3::ZERO, 0.0, false, false, false, 1.1);
        assert!(!player.sneaking);
    }

    #[test]
    fn sandbox_sprinting_does_not_consume_food() {
        let (world, mut player) = movement_fixture();
        player.sandbox = true;
        player.update(&world, Vec3::X, Vec3::X, 0.05, true, true, false, 0.0);
        player.exhaust(8.0);
        assert_eq!(
            (player.hunger, player.saturation, player.exhaustion),
            (20, 5.0, 0.0)
        );
    }

    #[test]
    fn sprint_stops_at_six_food_and_eating_allows_it_again() {
        let (world, mut player) = movement_fixture();
        player.hunger = 7;
        player.saturation = 0.0;
        player.exhaustion = 3.999;
        player.update(&world, Vec3::X, Vec3::X, 0.05, true, false, false, 0.0);
        assert_eq!(player.hunger, 6);
        assert!(!player.sprinting);
        player.update(&world, Vec3::X, Vec3::X, 0.05, true, false, false, 0.05);
        assert!(!player.sprinting);
        assert!(player.eat_food(item::food_properties(BlockType::Apple).unwrap()));
        player.update(&world, Vec3::X, Vec3::X, 0.05, true, false, false, 0.1);
        assert!(player.sprinting);
    }

    #[test]
    fn double_tap_forward_reaches_sprint_speed_without_holding_control() {
        let (world, mut player) = movement_fixture();
        let mut input = sprint::SprintInput::default();
        for (time, forward) in [(0.0, 1.0), (0.05, 0.0), (0.1, 1.0)] {
            let request = input.request(forward, false, player.sprinting, true, time);
            player.update(
                &world,
                Vec3::X,
                Vec3::X * forward,
                0.05,
                request,
                false,
                false,
                time,
            );
        }
        assert!(player.sprinting);
        for tick in 3..20 {
            let time = tick as f64 * 0.05;
            let request = input.request(1.0, false, player.sprinting, true, time);
            player.update(&world, Vec3::X, Vec3::X, 0.05, request, false, false, time);
        }
        assert!((player.velocity.x - sprint::SPRINT_SPEED).abs() < 0.001);
        let request = input.request(0.0, false, player.sprinting, true, 1.0);
        player.update(
            &world,
            Vec3::X,
            Vec3::ZERO,
            0.05,
            request,
            false,
            false,
            1.0,
        );
        assert!(!player.sprinting);
    }

    #[test]
    fn inventory_and_flight_cancel_sprint_without_spending_food() {
        let (world, mut player) = movement_fixture();
        player.sprinting = true;
        player.inventory_open = true;
        player.update(&world, Vec3::X, Vec3::X, 0.05, true, true, false, 0.0);
        assert!(!player.sprinting);
        assert_eq!(player.exhaustion, 0.0);
        player.inventory_open = false;
        player.sandbox = true;
        player.grounded = false;
        player.flying = true;
        player.position.y = 70.0;
        player.hunger = 0;
        player.hunger_timer = 3.99;
        player.update(&world, Vec3::X, Vec3::X, 0.05, true, false, false, 0.05);
        assert!(!player.sprinting);
        assert_eq!(player.exhaustion, 0.0);
        assert_eq!(player.health, 20);
    }

    #[test]
    fn swimming_uses_swim_exhaustion_instead_of_sprint_exhaustion() {
        let (mut world, mut player) = movement_fixture();
        for x in 8..=9 {
            for y in 64..=65 {
                world.set_block(x, y, 8, BlockType::Water);
            }
        }
        player.position.y = 64.0;
        let start = player.position;
        player.update(&world, Vec3::X, Vec3::X, 0.05, true, false, false, 0.0);
        assert!(!player.sprinting);
        assert!((player.exhaustion - player.position.distance(start) * 0.01).abs() < 0.00001);
    }

    #[test]
    fn starvation_can_reach_zero_health() {
        let mut player = Player::new(64.0);
        player.health = 1;
        player.hunger = 0;
        player.apply_starvation_tick();
        assert_eq!(player.health, 0);
    }

    #[test]
    fn exhaustion_consumes_saturation_then_hunger_and_keeps_fractional_work() {
        let mut player = Player::new(64.0);
        player.saturation = 1.0;
        player.exhaust(4.5);
        assert_eq!(player.saturation, 0.0);
        assert_eq!(player.hunger, 20);
        player.exhaust(3.5);
        assert_eq!(player.hunger, 19);
        assert_eq!(player.exhaustion, 0.0);
        assert!(!player.sandbox);
    }

    #[test]
    fn swapping_armor_cannot_repair_it() {
        let mut player = Player::new(64.0);
        let iron = crate::inventory::ItemStack::new(BlockType::IronHelmet, 1);
        let gold = crate::inventory::ItemStack::new(BlockType::GoldHelmet, 1);
        assert!(player.equip_armor(iron).is_none());
        player.take_damage(2);
        let worn = player.equipped_armor[0].unwrap().1;
        let returned = player.equip_armor(gold).unwrap();
        assert_eq!(returned.durability, Some(worn));
        player.equip_armor(returned);
        assert_eq!(
            player.equipped_armor[0],
            Some((BlockType::IronHelmet, worn))
        );
    }

    #[test]
    fn respawn_finds_standing_room_when_the_bed_location_is_obstructed() {
        let spawn = safe_spawn_near(Vec3::new(0.5, 61.0, 0.5), |x, y, z| {
            if y == 60 || (x == 0 && z == 0 && y >= 61) {
                BlockType::Stone
            } else {
                BlockType::Air
            }
        })
        .unwrap();
        assert_ne!((spawn.x, spawn.z), (0.5, 0.5));
        assert_eq!(spawn.y, 61.0);
        assert!(safe_spawn_near(Vec3::ZERO, |_, _, _| BlockType::Air).is_none());
    }

    #[test]
    fn snowy_surface_spawn_does_not_select_the_cave_underneath() {
        let blocks = |_: i32, y, _: i32| match y {
            0..=60 | 64..=130 => BlockType::Stone,
            131 => BlockType::SnowLayer,
            _ => BlockType::Air,
        };
        let target = Vec3::new(0.5, 150.0, 0.5);
        assert_eq!(safe_spawn_near(target, blocks).unwrap().y, 131.0);
        assert_eq!(surface_spawn_near(target, blocks).unwrap().y, 131.0);
    }

    #[test]
    fn surface_spawn_searches_for_land_instead_of_an_underwater_cave() {
        let spawn = surface_spawn_near(Vec3::new(0.5, 150.0, 0.5), |x, y, _| {
            if y <= 60 || (64..=125).contains(&y) {
                return BlockType::Stone;
            }
            if (126..=130).contains(&y) && x <= 0 {
                return BlockType::Water;
            }
            BlockType::Air
        })
        .unwrap();
        assert_eq!(spawn.x, 1.5);
        assert_eq!(spawn.y, 126.0);
    }

    #[test]
    fn ocean_spawn_falls_back_to_the_water_surface() {
        let spawn = surface_spawn_near(Vec3::new(0.5, 150.0, 0.5), |_, y, _| {
            if y <= 124 {
                BlockType::Water
            } else {
                BlockType::Air
            }
        })
        .unwrap();
        assert_eq!(spawn, Vec3::new(0.5, 125.0, 0.5));
    }

    #[test]
    fn reported_black_screen_seed_spawns_above_the_ocean() {
        let mut chunk = crate::chunk::Chunk::new(2, 2, -112651535689168126i64 as u64);
        chunk.generate();
        assert_eq!(chunk.get_block(0, 123, 0), BlockType::Water);
        let spawn = surface_spawn_near(Vec3::new(32.5, 150.0, 32.5), |x, y, z| {
            if !(32..48).contains(&x) || !(32..48).contains(&z) || !(0..256).contains(&y) {
                return BlockType::Air;
            }
            chunk.get_block((x - 32) as usize, y as usize, (z - 32) as usize)
        })
        .unwrap();
        assert!(spawn.y >= 124.0, "spawned underground at {spawn:?}");
    }

    #[test]
    fn respawn_uses_bed_and_resets_survival_state() {
        let mut player = Player::new(64.0);
        player.spawn_point = Some(Vec3::new(10.5, 70.0, -4.5));
        player.health = 0;
        player.hunger = 0;
        player.air_seconds = 0.0;
        player.velocity = Vec3::splat(5.0);
        player.exhaustion = 3.5;
        player.hunger_timer = 3.9;
        player.inventory_open = true;

        player.respawn(Vec3::new(32.5, 80.0, 32.5));

        assert_eq!(player.position, Vec3::new(10.5, 70.0, -4.5));
        assert_eq!(player.velocity, Vec3::ZERO);
        assert_eq!((player.health, player.hunger), (20, 20));
        assert_eq!(player.air_seconds, 15.0);
        assert_eq!(player.exhaustion, 0.0);
        assert_eq!(player.hunger_timer, 0.0);
        assert!(!player.inventory_open);
    }

    #[test]
    fn test_cactus_contact_damage() {
        let mut world = World::simulation(42);
        world.set_block(10, 63, 10, BlockType::Sand);
        world.set_block(10, 64, 10, BlockType::Cactus);
        let mut player = Player::new(65.0);
        // Position player touching cactus
        player.position = Vec3::new(10.5, 65.0, 10.5);
        player.health = 20;
        player.sandbox = false;
        player.damage_cooldown = 0.0;

        player.update(&world, Vec3::X, Vec3::ZERO, 0.1, false, false, false, 0.0);
        assert_eq!(
            player.health, 19,
            "Player standing on cactus takes 1 damage"
        );
        assert_eq!(player.damage_cooldown, 0.5, "Damage cooldown set to 0.5s");

        // Another tick while on cooldown should not apply extra damage
        player.update(&world, Vec3::X, Vec3::ZERO, 0.1, false, false, false, 0.1);
        assert_eq!(player.health, 19);

        // After cooldown expires, damage applies again
        player.update(&world, Vec3::X, Vec3::ZERO, 0.45, false, false, false, 0.55);
        assert_eq!(player.health, 18);
    }

    #[test]
    fn test_cactus_sandbox_immunity() {
        let mut world = World::simulation(42);
        world.set_block(10, 63, 10, BlockType::Sand);
        world.set_block(10, 64, 10, BlockType::Cactus);
        let mut player = Player::new(65.0);
        player.position = Vec3::new(10.5, 65.0, 10.5);
        player.health = 20;
        player.sandbox = true;
        player.damage_cooldown = 0.0;

        player.update(&world, Vec3::X, Vec3::ZERO, 0.1, false, false, false, 0.0);
        assert_eq!(
            player.health, 20,
            "Sandbox player is immune to cactus damage"
        );
    }

    #[test]
    fn test_player_hurt_tilt_and_timer() {
        let mut player = Player::new(64.0);
        assert_eq!(player.hurt_time, 0.0);
        player.take_damage(2);
        assert_eq!(player.hurt_time, 0.5);
        assert_eq!(player.hurt_direction, 0.0);

        let world = World::simulation(42);
        player.update(&world, Vec3::X, Vec3::ZERO, 0.2, false, false, false, 0.0);
        assert!((player.hurt_time - 0.3).abs() < 1e-4);

        // Directional damage test: attacker to the right (+X relative to facing +Z)
        player.take_damage_from(
            2,
            Some(Vec3::new(
                player.position.x + 5.0,
                player.position.y,
                player.position.z,
            )),
            0.0,
        );
        assert_eq!(player.hurt_time, 0.5);
        assert!((player.hurt_direction - std::f32::consts::FRAC_PI_2).abs() < 1e-4);

        player.respawn(Vec3::new(32.5, 64.0, 32.5));
        assert_eq!(player.hurt_time, 0.0);
        assert_eq!(player.hurt_direction, 0.0);
    }
}
