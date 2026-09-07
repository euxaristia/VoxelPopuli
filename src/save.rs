use crate::block::BlockType;
use crate::container::{CHEST_SLOTS, Container, Furnace, SMELT_SECONDS};
use crate::inventory::{INVENTORY_SLOT_COUNT, ItemStack};
use crate::player::Player;
use crate::world::World;
use glam::{Vec2, Vec3};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 8] = b"VPOPSAV\0";
const VERSION: u32 = 2;
const MAX_EDITS: usize = 10_000_000;
const MAX_CONTAINERS: usize = 100_000;
const MAX_PENDING_STACKS: usize = 1_000_000;
const MAX_PATH_BYTES: usize = 32 * 1024;
pub const SAVE_FILE: &str = "world.vps";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GameSettings {
    pub view_distance: i32,
    pub fov: f32,
    pub fancy_graphics: bool,
    pub selected_skin: u8,
}

impl Default for GameSettings {
    fn default() -> Self {
        Self {
            view_distance: crate::world::DEFAULT_VIEW_DISTANCE,
            fov: 80.0,
            fancy_graphics: true,
            selected_skin: 0,
        }
    }
}

impl GameSettings {
    pub fn clamped(self) -> Self {
        Self {
            view_distance: self.view_distance.clamp(4, crate::world::MAX_VIEW_DISTANCE),
            fov: self.fov.clamp(60.0, 100.0),
            fancy_graphics: self.fancy_graphics,
            selected_skin: self.selected_skin.min(3),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct PlayerState {
    position: Vec3,
    velocity: Vec3,
    grounded: bool,
    air_seconds: f32,
    selected_slot: usize,
    health: i32,
    hunger: i32,
    saturation: f32,
    exhaustion: f32,
    sandbox: bool,
    hunger_timer: f32,
    equipped_armor: [Option<(BlockType, u16)>; 4],
    xp_level: u32,
    xp_progress: f32,
    total_xp: u32,
    flying: bool,
    damage_cooldown: f32,
    drowning_timer: f32,
    fall_distance: f32,
    spawn_point: Option<Vec3>,
}

impl PlayerState {
    fn capture(player: &Player) -> Self {
        Self {
            position: player.position,
            velocity: player.velocity,
            grounded: player.grounded,
            air_seconds: player.air_seconds,
            selected_slot: player.selected_slot,
            health: player.health,
            hunger: player.hunger,
            saturation: player.saturation,
            exhaustion: player.exhaustion,
            sandbox: player.sandbox,
            hunger_timer: player.hunger_timer,
            equipped_armor: player.equipped_armor,
            xp_level: player.xp_level,
            xp_progress: player.xp_progress,
            total_xp: player.total_xp,
            flying: player.flying,
            damage_cooldown: player.damage_cooldown,
            drowning_timer: player.drowning_timer,
            fall_distance: player.fall_distance,
            spawn_point: player.spawn_point,
        }
    }

    fn restore(&self) -> Player {
        let mut player = Player::new(self.position.y);
        player.position = self.position;
        player.velocity = self.velocity;
        player.grounded = self.grounded;
        player.air_seconds = self.air_seconds.clamp(0.0, 15.0);
        player.selected_slot = self.selected_slot.min(8);
        player.health = self.health.clamp(0, 20);
        player.hunger = self.hunger.clamp(0, 20);
        player.saturation = self.saturation.clamp(0.0, player.hunger as f32);
        player.exhaustion = self.exhaustion;
        player.sandbox = self.sandbox;
        player.hunger_timer = self.hunger_timer.max(0.0);
        player.equipped_armor = self.equipped_armor;
        player.xp_level = self.xp_level;
        player.xp_progress = self.xp_progress.clamp(0.0, 1.0);
        player.total_xp = self.total_xp;
        player.flying = self.flying && self.sandbox;
        player.damage_cooldown = self.damage_cooldown.max(0.0);
        player.drowning_timer = self.drowning_timer.max(0.0);
        player.fall_distance = self.fall_distance.max(0.0);
        player.spawn_point = self.spawn_point;
        player
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct GameSave {
    pub seed: u64,
    pub day_time: f32,
    player: PlayerState,
    pub inventory: [Option<ItemStack>; INVENTORY_SLOT_COUNT],
    pub camera_angle: Vec2,
    pub settings: GameSettings,
    pub import_world: Option<PathBuf>,
    pub edits: Vec<((i32, i32, i32), BlockType)>,
    pub containers: Vec<((i32, i32, i32), Container)>,
    pub cursor: Option<ItemStack>,
    pub crafting: [Option<ItemStack>; 9],
    pub pending_stacks: Vec<ItemStack>,
    pub dropped_items: Vec<crate::inventory::DroppedItem>,
}

impl GameSave {
    #[allow(clippy::too_many_arguments)]
    pub fn capture(
        world: &World,
        player: &Player,
        inventory: &[Option<ItemStack>; INVENTORY_SLOT_COUNT],
        camera_angle: Vec2,
        settings: GameSettings,
        import_world: Option<&Path>,
        cursor: Option<ItemStack>,
        crafting: &[Option<ItemStack>; crate::inventory::CRAFT_TABLE_SLOT_COUNT],
    ) -> io::Result<Self> {
        let mut edits = world
            .edits
            .read()
            .map_err(|_| io::Error::other("world edit lock poisoned"))?
            .iter()
            .map(|(position, block)| (*position, *block))
            .collect::<Vec<_>>();
        edits.sort_unstable_by_key(|(position, _)| *position);
        let mut containers = world
            .containers
            .iter()
            .map(|(pos, c)| (*pos, c.clone()))
            .collect::<Vec<_>>();
        containers.sort_unstable_by_key(|(position, _)| *position);
        Ok(Self {
            seed: world.seed,
            day_time: world.day_time,
            player: PlayerState::capture(player),
            inventory: *inventory,
            camera_angle,
            settings,
            import_world: import_world.map(Path::to_path_buf),
            edits,
            containers,
            cursor,
            crafting: std::array::from_fn(|i| crafting[i]),
            dropped_items: world.dropped_items.clone(),
            pending_stacks: world
                .pending_stacks
                .iter()
                .copied()
                .chain(
                    world
                        .pending_drops
                        .iter()
                        .map(|&(block, count)| ItemStack::new(block, count)),
                )
                .collect(),
        })
    }

    pub fn restore_player(&self) -> Player {
        self.player.restore()
    }

    pub fn write_to(&self, path: &Path) -> io::Result<()> {
        let bytes = self.encode()?;
        static NEXT_TEMP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT_TEMP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let temporary = path.with_extension(format!("vps.{}.{id}.tmp", std::process::id()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        let result = file.write_all(&bytes).and_then(|()| file.sync_all());
        drop(file);
        // Rename replaces the destination atomically. Never delete a good save to retry.
        let result = result.and_then(|()| std::fs::rename(&temporary, path));
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result
    }

    pub fn read_from(path: &Path) -> io::Result<Self> {
        Self::decode(&std::fs::read(path)?)
    }

    fn encode(&self) -> io::Result<Vec<u8>> {
        if self.edits.len() > MAX_EDITS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "too many world edits to save",
            ));
        }
        let mut out = Vec::with_capacity(4096 + self.edits.len() * 17);
        out.extend_from_slice(MAGIC);
        put_u32(&mut out, VERSION);
        put_u64(&mut out, self.seed);
        put_vec3(&mut out, self.player.position);
        put_vec3(&mut out, self.player.velocity);
        put_bool(&mut out, self.player.grounded);
        put_f32(&mut out, self.player.air_seconds);
        put_u32(&mut out, self.player.selected_slot as u32);
        put_i32(&mut out, self.player.health);
        put_i32(&mut out, self.player.hunger);
        put_f32(&mut out, self.player.saturation);
        put_f32(&mut out, self.player.hunger_timer);
        for armor in self.player.equipped_armor {
            put_armor(&mut out, armor);
        }
        put_u32(&mut out, self.player.xp_level);
        put_f32(&mut out, self.player.xp_progress);
        put_u32(&mut out, self.player.total_xp);
        put_bool(&mut out, self.player.flying);
        put_f32(&mut out, self.player.damage_cooldown);
        put_f32(&mut out, self.player.drowning_timer);
        put_f32(&mut out, self.player.fall_distance);
        put_optional_vec3(&mut out, self.player.spawn_point);
        put_f32(&mut out, self.camera_angle.x);
        put_f32(&mut out, self.camera_angle.y);
        put_i32(&mut out, self.settings.view_distance);
        put_f32(&mut out, self.settings.fov);
        put_bool(&mut out, self.settings.fancy_graphics);
        out.push(self.settings.selected_skin);
        let import_path = self
            .import_world
            .as_ref()
            .map(|path| path.to_string_lossy().into_owned());
        put_optional_string(&mut out, import_path.as_deref())?;
        put_u32(&mut out, INVENTORY_SLOT_COUNT as u32);
        for stack in self.inventory {
            put_stack(&mut out, stack)?;
        }
        put_u32(&mut out, self.edits.len() as u32);
        for ((x, y, z), block) in &self.edits {
            put_i32(&mut out, *x);
            put_i32(&mut out, *y);
            put_i32(&mut out, *z);
            out.push(*block as u8);
        }
        put_stack(&mut out, self.cursor)?;
        for stack in self.crafting {
            put_stack(&mut out, stack)?;
        }
        if self.containers.len() > MAX_CONTAINERS || self.pending_stacks.len() > MAX_PENDING_STACKS
        {
            return Err(invalid("too many containers or pending items"));
        }
        put_u32(&mut out, self.containers.len() as u32);
        for ((x, y, z), container) in &self.containers {
            put_i32(&mut out, *x);
            put_i32(&mut out, *y);
            put_i32(&mut out, *z);
            out.push(container.block() as u8);
            for &stack in container.slots() {
                put_stack(&mut out, stack)?;
            }
            if let Container::Furnace(furnace) = container {
                put_f32(&mut out, furnace.burn_remaining);
                put_f32(&mut out, furnace.burn_total);
                put_f32(&mut out, furnace.progress);
                put_bool(&mut out, furnace.cooking.is_some());
                if let Some(input) = furnace.cooking {
                    out.push(input as u8);
                }
            }
        }
        put_u32(&mut out, self.pending_stacks.len() as u32);
        for &stack in &self.pending_stacks {
            put_stack(&mut out, Some(stack))?;
        }
        put_f32(&mut out, self.player.exhaustion);
        put_bool(&mut out, self.player.sandbox);
        put_f32(&mut out, self.day_time);
        if self.dropped_items.len() > MAX_PENDING_STACKS {
            return Err(invalid("too many dropped items"));
        }
        put_u32(&mut out, self.dropped_items.len() as u32);
        for drop in &self.dropped_items {
            put_stack(&mut out, Some(drop.stack))?;
            put_vec3(&mut out, drop.position);
            put_vec3(&mut out, drop.velocity);
            put_f32(&mut out, drop.pickup_delay);
        }
        Ok(out)
    }

    fn decode(bytes: &[u8]) -> io::Result<Self> {
        let mut reader = Reader::new(bytes);
        if reader.take(MAGIC.len())? != MAGIC {
            return Err(invalid("not a VoxelPopuli save file"));
        }
        let version = reader.u32()?;
        if !(1..=VERSION).contains(&version) {
            return Err(invalid("unsupported save version"));
        }
        let seed = reader.u64()?;
        let position = reader.vec3()?;
        let velocity = reader.vec3()?;
        let grounded = reader.bool()?;
        let air_seconds = reader.f32()?;
        let selected_slot = reader.u32()? as usize;
        let health = reader.i32()?;
        let hunger = reader.i32()?;
        let saturation = reader.f32()?;
        let hunger_timer = reader.f32()?;
        let mut equipped_armor = [None; 4];
        for armor in &mut equipped_armor {
            *armor = reader.armor()?;
        }
        let xp_level = reader.u32()?;
        let xp_progress = reader.f32()?;
        let total_xp = reader.u32()?;
        let flying = reader.bool()?;
        let damage_cooldown = reader.f32()?;
        let drowning_timer = reader.f32()?;
        let fall_distance = reader.f32()?;
        let spawn_point = reader.optional_vec3()?;
        let camera_angle = Vec2::new(reader.f32()?, reader.f32()?);
        let settings = GameSettings {
            view_distance: reader.i32()?,
            fov: reader.f32()?,
            fancy_graphics: reader.bool()?,
            selected_skin: reader.u8()?,
        }
        .clamped();
        let import_world = reader.optional_string()?.map(PathBuf::from);
        let inventory_len = reader.u32()? as usize;
        if inventory_len != INVENTORY_SLOT_COUNT {
            return Err(invalid("save inventory has the wrong size"));
        }
        let mut inventory = [None; INVENTORY_SLOT_COUNT];
        for stack in &mut inventory {
            *stack = reader.stack()?;
        }
        let edit_count = reader.u32()? as usize;
        if edit_count > MAX_EDITS || edit_count > reader.remaining() / 13 {
            return Err(invalid("invalid world edit count"));
        }
        let mut edits = Vec::with_capacity(edit_count);
        for _ in 0..edit_count {
            let position = (reader.i32()?, reader.i32()?, reader.i32()?);
            edits.push((position, reader.block()?));
        }
        let mut cursor = None;
        let mut crafting = [None; 9];
        let mut containers = Vec::new();
        let mut pending_stacks = Vec::new();
        let mut exhaustion = 0.0;
        let mut sandbox = true; // Preserve the flight controls of version 1 worlds.
        let mut day_time = 570.0;
        let mut dropped_items = Vec::new();
        if version >= 2 {
            cursor = reader.stack()?;
            for stack in &mut crafting {
                *stack = reader.stack()?;
            }
            let count = reader.u32()? as usize;
            if count > MAX_CONTAINERS || count > reader.remaining() / 13 {
                return Err(invalid("invalid container count"));
            }
            let mut positions = std::collections::HashSet::new();
            for _ in 0..count {
                let position = (reader.i32()?, reader.i32()?, reader.i32()?);
                if !(0..crate::chunk::CHUNK_HEIGHT as i32).contains(&position.1)
                    || !positions.insert(position)
                {
                    return Err(invalid("invalid or duplicate container position"));
                }
                let container = match reader.block()? {
                    BlockType::Chest => {
                        let mut slots = [None; CHEST_SLOTS];
                        for stack in &mut slots {
                            *stack = reader.stack()?;
                        }
                        Container::Chest(Box::new(slots))
                    }
                    BlockType::Furnace => {
                        let mut slots = [None; 3];
                        for stack in &mut slots {
                            *stack = reader.stack()?;
                        }
                        let burn_remaining = reader.f32()?;
                        let burn_total = reader.f32()?;
                        let progress = reader.f32()?;
                        let cooking = reader.bool()?.then(|| reader.block()).transpose()?;
                        if !(0.0..=80.0).contains(&burn_total)
                            || !(0.0..=burn_total).contains(&burn_remaining)
                            || !(0.0..SMELT_SECONDS).contains(&progress)
                        {
                            return Err(invalid("invalid furnace timers"));
                        }
                        Container::Furnace(Furnace {
                            slots,
                            burn_remaining,
                            burn_total,
                            progress,
                            cooking,
                        })
                    }
                    _ => return Err(invalid("invalid container type")),
                };
                containers.push((position, container));
            }
            let count = reader.u32()? as usize;
            if count > MAX_PENDING_STACKS || count > reader.remaining() / 7 {
                return Err(invalid("invalid pending item count"));
            }
            for _ in 0..count {
                pending_stacks.push(
                    reader
                        .stack()?
                        .ok_or_else(|| invalid("empty pending item"))?,
                );
            }
            exhaustion = reader.f32()?;
            sandbox = reader.bool()?;
            day_time = reader.f32()?;
            if !(0.0..4.0).contains(&exhaustion) || !(0.0..1200.0).contains(&day_time) {
                return Err(invalid("invalid survival clock or exhaustion"));
            }
            let count = reader.u32()? as usize;
            if count > MAX_PENDING_STACKS || count > reader.remaining() / 35 {
                return Err(invalid("invalid dropped item count"));
            }
            for _ in 0..count {
                let stack = reader
                    .stack()?
                    .ok_or_else(|| invalid("empty dropped item"))?;
                let position = reader.vec3()?;
                let velocity = reader.vec3()?;
                let pickup_delay = reader.f32()?;
                if !(0.0..=0.75).contains(&pickup_delay) {
                    return Err(invalid("invalid pickup delay"));
                }
                dropped_items.push(crate::inventory::DroppedItem {
                    stack,
                    position,
                    velocity,
                    pickup_delay,
                });
            }
        }
        if reader.remaining() != 0 {
            return Err(invalid("trailing data in save file"));
        }
        Ok(Self {
            seed,
            day_time,
            player: PlayerState {
                position,
                velocity,
                grounded,
                air_seconds,
                selected_slot,
                health,
                hunger,
                saturation,
                exhaustion,
                sandbox,
                hunger_timer,
                equipped_armor,
                xp_level,
                xp_progress,
                total_xp,
                flying,
                damage_cooldown,
                drowning_timer,
                fall_distance,
                spawn_point,
            },
            inventory,
            camera_angle,
            settings,
            import_world,
            edits,
            containers,
            cursor,
            crafting,
            pending_stacks,
            dropped_items,
        })
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn put_bool(out: &mut Vec<u8>, value: bool) {
    out.push(value as u8);
}

fn put_u32(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_le_bytes());
}

fn put_f32(out: &mut Vec<u8>, value: f32) {
    put_u32(out, value.to_bits());
}

fn put_vec3(out: &mut Vec<u8>, value: Vec3) {
    put_f32(out, value.x);
    put_f32(out, value.y);
    put_f32(out, value.z);
}

fn put_optional_vec3(out: &mut Vec<u8>, value: Option<Vec3>) {
    put_bool(out, value.is_some());
    if let Some(value) = value {
        put_vec3(out, value);
    }
}

fn put_armor(out: &mut Vec<u8>, armor: Option<(BlockType, u16)>) {
    put_bool(out, armor.is_some());
    if let Some((block, durability)) = armor {
        out.push(block as u8);
        out.extend_from_slice(&durability.to_le_bytes());
    }
}

fn put_stack(out: &mut Vec<u8>, stack: Option<ItemStack>) -> io::Result<()> {
    if let Some(stack) = stack
        && !(1..=64).contains(&stack.count)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "inventory stack count must be in 1..=64",
        ));
    }
    put_bool(out, stack.is_some());
    if let Some(stack) = stack {
        out.push(stack.block as u8);
        put_u32(out, stack.count);
        put_bool(out, stack.durability.is_some());
        if let Some(durability) = stack.durability {
            out.extend_from_slice(&durability.to_le_bytes());
        }
    }
    Ok(())
}

fn put_optional_string(out: &mut Vec<u8>, value: Option<&str>) -> io::Result<()> {
    put_bool(out, value.is_some());
    if let Some(value) = value {
        if value.len() > MAX_PATH_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "path is too long",
            ));
        }
        put_u32(out, value.len() as u32);
        out.extend_from_slice(value.as_bytes());
    }
    Ok(())
}

struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len() - self.position
    }

    fn take(&mut self, count: usize) -> io::Result<&'a [u8]> {
        let end = self
            .position
            .checked_add(count)
            .filter(|end| *end <= self.bytes.len())
            .ok_or_else(|| invalid("truncated save file"))?;
        let bytes = &self.bytes[self.position..end];
        self.position = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take(1)?[0])
    }

    fn bool(&mut self) -> io::Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid("invalid boolean in save file")),
        }
    }

    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> io::Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn u64(&mut self) -> io::Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }

    fn f32(&mut self) -> io::Result<f32> {
        let value = f32::from_bits(self.u32()?);
        if !value.is_finite() {
            return Err(invalid("non-finite float in save file"));
        }
        Ok(value)
    }

    fn vec3(&mut self) -> io::Result<Vec3> {
        Ok(Vec3::new(self.f32()?, self.f32()?, self.f32()?))
    }

    fn optional_vec3(&mut self) -> io::Result<Option<Vec3>> {
        self.bool()?.then(|| self.vec3()).transpose()
    }

    fn block(&mut self) -> io::Result<BlockType> {
        let value = self.u8()?;
        if value as usize >= BlockType::COUNT {
            return Err(invalid("invalid block id in save file"));
        }
        Ok(BlockType::from_u8(value))
    }

    fn armor(&mut self) -> io::Result<Option<(BlockType, u16)>> {
        self.bool()?
            .then(|| Ok((self.block()?, self.u16()?)))
            .transpose()
    }

    fn stack(&mut self) -> io::Result<Option<ItemStack>> {
        self.bool()?
            .then(|| {
                let block = self.block()?;
                let count = self.u32()?;
                if count == 0 || count > 64 {
                    return Err(invalid("invalid item count in save file"));
                }
                let durability = self.bool()?.then(|| self.u16()).transpose()?;
                Ok(ItemStack {
                    block,
                    count,
                    durability,
                })
            })
            .transpose()
    }

    fn optional_string(&mut self) -> io::Result<Option<String>> {
        self.bool()?
            .then(|| {
                let length = self.u32()? as usize;
                if length > MAX_PATH_BYTES {
                    return Err(invalid("saved path is too long"));
                }
                String::from_utf8(self.take(length)?.to_vec())
                    .map_err(|_| invalid("saved path is not UTF-8"))
            })
            .transpose()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_save() -> GameSave {
        let mut inventory = [None; INVENTORY_SLOT_COUNT];
        inventory[0] = Some(ItemStack {
            block: BlockType::IronPickaxe,
            count: 1,
            durability: Some(123),
        });
        GameSave {
            seed: 42,
            day_time: 570.0,
            player: PlayerState {
                position: Vec3::new(-2.5, 70.0, 18.25),
                velocity: Vec3::new(1.0, -2.0, 3.0),
                grounded: false,
                air_seconds: 9.5,
                selected_slot: 3,
                health: 17,
                hunger: 12,
                saturation: 2.5,
                exhaustion: 0.0,
                sandbox: true,
                hunger_timer: 1.25,
                equipped_armor: [Some((BlockType::IronHelmet, 99)), None, None, None],
                xp_level: 4,
                xp_progress: 0.75,
                total_xp: 80,
                flying: false,
                damage_cooldown: 0.2,
                drowning_timer: 0.7,
                fall_distance: 2.0,
                spawn_point: Some(Vec3::new(1.0, 65.0, 2.0)),
            },
            inventory,
            camera_angle: Vec2::new(1.2, -0.4),
            settings: GameSettings {
                view_distance: 6,
                fov: 90.0,
                fancy_graphics: false,
                selected_skin: 2,
            },
            import_world: Some(PathBuf::from("example-world")),
            edits: vec![((1, 2, 3), BlockType::Torch), ((-4, 60, 8), BlockType::Air)],
            containers: Vec::new(),
            cursor: None,
            crafting: [None; 9],
            pending_stacks: Vec::new(),
            dropped_items: Vec::new(),
        }
    }

    #[test]
    fn save_round_trip_preserves_progress_and_durability() {
        let save = sample_save();
        assert_eq!(GameSave::decode(&save.encode().unwrap()).unwrap(), save);
    }

    #[test]
    fn save_preserves_containers_open_crafting_cursor_and_overflow() {
        let mut save = sample_save();
        let mut furnace = Furnace {
            slots: [
                Some(ItemStack::new(BlockType::IronOre, 8)),
                Some(ItemStack::new(BlockType::Coal, 1)),
                None,
            ],
            ..Furnace::default()
        };
        furnace.tick(14.0);
        let mut chest = [None; CHEST_SLOTS];
        chest[17] = save.inventory[0];
        save.containers = vec![
            ((0, 60, 0), Container::Chest(Box::new(chest))),
            ((1, 60, 0), Container::Furnace(furnace)),
        ];
        save.cursor = save.inventory[0];
        save.crafting[8] = Some(ItemStack::new(BlockType::Diamond, 2));
        save.pending_stacks = vec![save.inventory[0].unwrap()];
        save.dropped_items.push(crate::inventory::DroppedItem::new(
            save.inventory[0].unwrap(),
            Vec3::new(1.0, 65.0, 2.0),
            Vec3::Y,
        ));
        let mut restored = GameSave::decode(&save.encode().unwrap()).unwrap();
        assert_eq!(restored, save);
        let Container::Furnace(furnace) = &mut restored.containers[1].1 else {
            panic!("missing furnace")
        };
        furnace.tick(6.0);
        assert_eq!(
            furnace.slots[2],
            Some(ItemStack::new(BlockType::IronIngot, 2))
        );
        assert_eq!(furnace.burn_remaining, 60.0);
    }

    #[test]
    fn version_one_saves_still_load() {
        let save = sample_save();
        let mut bytes = save.encode().unwrap();
        // Empty version 2 extension: cursor, crafting, counts, and survival state.
        bytes.truncate(bytes.len() - 31);
        bytes[8..12].copy_from_slice(&1u32.to_le_bytes());
        assert_eq!(GameSave::decode(&bytes).unwrap(), save);
    }

    #[test]
    fn corrupt_furnace_timers_and_duplicate_containers_are_rejected() {
        let mut save = sample_save();
        save.containers.push((
            (0, 60, 0),
            Container::Furnace(Furnace {
                progress: 10.0,
                ..Furnace::default()
            }),
        ));
        assert!(GameSave::decode(&save.encode().unwrap()).is_err());
        save.containers[0].1 = Container::new(BlockType::Chest).unwrap();
        save.containers.push(save.containers[0].clone());
        assert!(GameSave::decode(&save.encode().unwrap()).is_err());
    }

    #[test]
    fn failed_replacement_preserves_the_existing_destination() {
        let directory =
            std::env::temp_dir().join(format!("voxelpopuli-save-replace-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let sentinel = directory.join("keep.txt");
        std::fs::write(&sentinel, b"existing data").unwrap();
        assert!(sample_save().write_to(&directory).is_err());
        assert_eq!(std::fs::read(&sentinel).unwrap(), b"existing data");
        std::fs::remove_file(sentinel).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn save_rejects_truncation_and_invalid_block_ids() {
        let bytes = sample_save().encode().unwrap();
        assert!(GameSave::decode(&bytes[..bytes.len() - 1]).is_err());

        let mut invalid = bytes;
        let last = invalid.len() - 1;
        invalid[last] = u8::MAX;
        assert!(GameSave::decode(&invalid).is_err());
    }

    #[test]
    fn save_file_round_trip_preserves_edits() {
        let dir =
            std::env::temp_dir().join(format!("voxelpopuli-save-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("world.vps");
        let save = sample_save();
        save.write_to(&path).unwrap();
        assert_eq!(GameSave::read_from(&path).unwrap(), save);
        let mut changed = save;
        changed.player.health = 4;
        changed.write_to(&path).unwrap();
        assert_eq!(GameSave::read_from(&path).unwrap(), changed);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn settings_clamp_rejects_out_of_range_values() {
        let settings = GameSettings {
            view_distance: 99,
            fov: 12.0,
            fancy_graphics: false,
            selected_skin: 9,
        }
        .clamped();
        assert_eq!(settings.view_distance, crate::world::MAX_VIEW_DISTANCE);
        assert_eq!(settings.fov, 60.0);
        assert_eq!(settings.selected_skin, 3);
    }

    #[test]
    fn decode_clamps_saved_settings() {
        let mut save = sample_save();
        save.settings.view_distance = 99;
        save.settings.fov = 12.0;
        save.settings.selected_skin = 9;
        let decoded = GameSave::decode(&save.encode().unwrap()).unwrap();
        assert_eq!(
            decoded.settings.view_distance,
            crate::world::MAX_VIEW_DISTANCE
        );
        assert_eq!(decoded.settings.fov, 60.0);
        assert_eq!(decoded.settings.selected_skin, 3);
    }

    #[test]
    fn encode_rejects_invalid_stack_counts() {
        let mut save = sample_save();
        save.inventory[0].as_mut().unwrap().count = 0;
        let error = save.encode().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);

        save.inventory[0].as_mut().unwrap().count = 65;
        let error = save.encode().unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }
}
