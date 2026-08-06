use crate::block::BlockType;
use crate::item;
use crate::world::World;
use glam::Vec3;

pub struct Player {
    pub position: Vec3,
    pub velocity: Vec3,
    pub grounded: bool,
    pub air_seconds: f32,
    pub inventory_open: bool,
    pub selected_slot: usize,
    pub health: i32,
    pub hunger: i32,
    pub saturation: f32,
    pub hunger_timer: f32,
    pub equipped_armor: [Option<(BlockType, u16)>; 4],
    pub xp_level: u32,
    pub xp_progress: f32,
    pub total_xp: u32,
    pub flying: bool,
    pub last_space_release: f64,
    pub space_was_pressed: bool,
    pub damage_cooldown: f32,
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
            hunger_timer: 0.0,
            equipped_armor: [None, None, None, None],
            xp_level: 0,
            xp_progress: 0.0,
            total_xp: 0,
            flying: false,
            last_space_release: 0.0,
            space_was_pressed: false,
            damage_cooldown: 0.0,
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
        let defense = self.total_armor_defense();
        let reduction = (defense as f32 * 0.04).min(0.80);
        let actual_damage = ((base_damage as f32) * (1.0 - reduction)).round() as i32;
        let actual_damage = actual_damage.max(1);
        self.health = (self.health - actual_damage).max(0);

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

    pub fn eat_food(&mut self, props: item::FoodProperties) -> bool {
        if self.hunger >= 20 {
            return false;
        }
        self.hunger = (self.hunger + props.hunger_restored).min(20);
        self.saturation = (self.saturation + props.saturation_restored).min(self.hunger as f32);
        true
    }

    pub fn equip_armor(&mut self, item: BlockType) -> Option<BlockType> {
        if let Some(props) = item::armor_properties(item) {
            let slot_idx = props.slot as usize;
            let prev = self.equipped_armor[slot_idx].map(|(b, _)| b);
            self.equipped_armor[slot_idx] = Some((item, props.durability));
            prev
        } else {
            None
        }
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
        let b = world.get_block(p.x.floor() as i32, p.y.floor() as i32, p.z.floor() as i32);
        b.is_solid()
    }

    fn check_collision(world: &World, pos: Vec3) -> bool {
        let w = 0.22;
        let h = 1.75;
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
        let player_max = self.position + Vec3::new(w, 1.8, w);
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
        move_input: Vec3,
        dt: f32,
        is_sprinting: bool,
        is_jumping: bool,
        is_sneaking: bool,
        current_time: f64,
    ) {
        self.hunger_timer += dt;
        if self.hunger_timer >= 4.0 {
            self.hunger_timer -= 4.0;
            if self.hunger >= 18 && self.health < 20 {
                self.health = (self.health + 1).min(20);
                if self.saturation > 0.0 {
                    self.saturation = (self.saturation - 1.5).max(0.0);
                } else {
                    self.hunger = (self.hunger - 1).max(0);
                }
            } else if self.hunger == 0 {
                self.health = (self.health - 1).max(1);
            }
        }

        let effective_sprint = is_sprinting && self.hunger > 6;
        if effective_sprint && move_input.length_squared() > 0.01 {
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 0.05 * dt).max(0.0);
            } else if self.hunger > 0 {
                self.saturation = 0.0;
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
        if in_lava {
            self.damage_cooldown -= dt;
            if self.damage_cooldown <= 0.0 {
                self.take_damage(2);
                self.damage_cooldown = 0.5;
            }
        } else if self.damage_cooldown > 0.0 {
            self.damage_cooldown = (self.damage_cooldown - dt).max(0.0);
        }

        if self.inventory_open {
            return;
        }

        let space_just_pressed = is_jumping && !self.space_was_pressed;
        if space_just_pressed && !self.grounded {
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
            (self.position.y + 1.6).floor() as i32,
            self.position.z.floor() as i32,
        ) == BlockType::Water;
        let in_water = waist_in_w || feet_in_w;

        let mut mv = move_input;
        if mv.length_squared() > 0.1 {
            mv = mv.normalize();
            let speed = if self.flying {
                10.92
            } else if in_water {
                2.0
            } else if effective_sprint {
                5.612
            } else {
                4.317
            };
            mv *= speed;
        }

        if self.flying {
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
                }
            }
        }

        let dy = self.velocity.y * dt;
        self.position.y += dy;
        if Self::check_collision(world, self.position) {
            if self.velocity.y <= 0.0 {
                self.grounded = true;
            }
            self.position.y -= dy;
            self.velocity.y = 0.0;
        } else if self.velocity.y != 0.0 {
            self.grounded = false;
        }

        let dx = self.velocity.x * dt;
        self.position.x += dx;
        if Self::check_collision(world, self.position) {
            self.position.x -= dx;
        }

        let dz = self.velocity.z * dt;
        self.position.z += dz;
        if Self::check_collision(world, self.position) {
            self.position.z -= dz;
        }

        if head_in_w {
            self.air_seconds = (self.air_seconds - dt).max(0.0);
        } else {
            self.air_seconds = (self.air_seconds + dt * 3.0).min(15.0);
        }
    }
}
