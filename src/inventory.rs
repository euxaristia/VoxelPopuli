use crate::block::BlockType;
use crate::crafting;
use crate::item;
use crate::world::{self, World};
use glam::Vec3;
use rand::RngExt;

pub const INVENTORY_SLOT_COUNT: usize = 45;
pub const CRAFT_TABLE_SLOT_COUNT: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemStack {
    pub block: BlockType,
    pub count: u32,
    pub durability: Option<u16>,
}

impl ItemStack {
    pub fn new(b: BlockType, n: u32) -> Self {
        Self {
            block: b,
            count: n,
            durability: None,
        }
    }

    pub fn new_tool(b: BlockType) -> Self {
        let dur = item::tool_properties(b).map(|t| t.durability);
        Self {
            block: b,
            count: 1,
            durability: dur,
        }
    }

    pub fn with_count(mut self, count: u32) -> Self {
        self.count = count;
        self
    }

    fn can_stack_with(self, other: Self) -> bool {
        self.block == other.block && self.durability == other.durability
    }
}

pub fn stack_max(b: BlockType) -> u32 {
    item::max_stack_size(b)
}

/// Create starting inventory with default items for gameplay & testing.
pub fn create_starting_inventory() -> [Option<ItemStack>; INVENTORY_SLOT_COUNT] {
    let mut slots = [None::<ItemStack>; INVENTORY_SLOT_COUNT];
    slots[0] = Some(ItemStack::new(BlockType::TNT, 64));
    slots[1] = Some(ItemStack::new_tool(BlockType::FlintAndSteel));
    slots[2] = Some(ItemStack::new(BlockType::Torch, 64));
    slots[3] = Some(ItemStack::new(BlockType::Bread, 16));
    slots[4] = Some(ItemStack::new(BlockType::CookedPorkchop, 16));
    slots[5] = Some(ItemStack::new(BlockType::IronChestplate, 1));
    slots[6] = Some(ItemStack::new(BlockType::Bow, 1));
    slots[7] = Some(ItemStack::new(BlockType::Arrow, 64));
    slots
}

/// Add items when mining – hotbar first, then main inventory.
pub fn inv_add(
    slots: &mut [Option<ItemStack>; INVENTORY_SLOT_COUNT],
    block: BlockType,
    amt: u32,
) -> u32 {
    inv_add_stack(slots, ItemStack::new(block, amt)).map_or(0, |stack| stack.count)
}

fn inv_add_stack(
    slots: &mut [Option<ItemStack>; INVENTORY_SLOT_COUNT],
    stack: ItemStack,
) -> Option<ItemStack> {
    let sm = stack_max(stack.block);
    let mut remaining = stack.count;
    for pass in 0..2u8 {
        for i in (0..9).chain(9..36) {
            if remaining == 0 {
                return None;
            }
            match slots[i] {
                Some(s) if s.can_stack_with(stack) && s.count < sm && pass == 0 => {
                    let add = (sm - s.count).min(remaining);
                    slots[i] = Some(s.with_count(s.count + add));
                    remaining -= add;
                }
                None if pass == 1 => {
                    let add = remaining.min(sm);
                    slots[i] = Some(stack.with_count(add));
                    remaining -= add;
                }
                _ => {}
            }
        }
    }
    Some(stack.with_count(remaining))
}

/// Add a tool item to inventory (preserves durability).
#[allow(dead_code)]
pub fn inv_add_tool(slots: &mut [Option<ItemStack>; INVENTORY_SLOT_COUNT], tool: ItemStack) {
    let _ = inv_add_stack(slots, tool);
}

fn bucket_for_liquid(block: BlockType, liquid_level: u8) -> Option<BlockType> {
    match block {
        BlockType::Lava => Some(BlockType::LavaBucket),
        BlockType::Water => Some(BlockType::WaterBucket),
        _ if liquid_level > 0 => Some(BlockType::WaterBucket),
        _ => None,
    }
}

pub fn is_hoe_item(block: BlockType) -> bool {
    matches!(
        block,
        BlockType::WoodHoe
            | BlockType::StoneHoe
            | BlockType::IronHoe
            | BlockType::DiamondHoe
            | BlockType::GoldHoe
    )
}

pub fn damage_selected_tool(
    slots: &mut [Option<ItemStack>; INVENTORY_SLOT_COUNT],
    selected_slot: usize,
) {
    if let Some(ref mut s) = slots[selected_slot]
        && s.block.is_tool()
        && let Some(ref mut dur) = s.durability
    {
        *dur = dur.saturating_sub(1);
        if *dur == 0 {
            slots[selected_slot] = None;
        }
    }
}

pub fn try_till_farmland(
    world: &mut World,
    slots: &mut [Option<ItemStack>; INVENTORY_SLOT_COUNT],
    selected_slot: usize,
    res: &world::RaycastResult,
) -> bool {
    let Some(held) = slots[selected_slot].map(|s| s.block) else {
        return false;
    };
    if !is_hoe_item(held) {
        return false;
    }

    let target = world.get_block(res.x, res.y, res.z);
    let above = world.get_block(res.x, res.y + 1, res.z);
    if matches!(
        target,
        BlockType::Grass | BlockType::Dirt | BlockType::SnowyGrass
    ) && above == BlockType::Air
    {
        world.set_block(res.x, res.y, res.z, BlockType::Farmland);
        damage_selected_tool(slots, selected_slot);
        return true;
    }
    false
}

use crate::player::Player;

pub fn prime_tnt(world: &mut World, x: i32, y: i32, z: i32) {
    world.set_block(x, y, z, BlockType::Air);
    let mut rng = rand::rng();
    let vx = (rng.random_range(0.0..1.0) * 0.04 - 0.02) * 20.0;
    let vz = (rng.random_range(0.0..1.0) * 0.04 - 0.02) * 20.0;
    world.explosives.push(crate::block::ActiveExplosive {
        position: Vec3::new(x as f32, y as f32, z as f32),
        velocity: Vec3::new(vx, 1.2, vz),
        fuse: 4.0,
        initial_fuse: 4.0,
    });
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Axis {
    X,
    Y,
    Z,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LockStage {
    /// 1 block placed: allows block #2 to be placed in any direction from P1
    InitialPlacement((i32, i32, i32)),
    /// 2+ blocks placed: locked to strict 1D line
    Line {
        axis: Axis,
        fixed_a: i32,
        fixed_b: i32,
    },
}

#[derive(Clone, Copy, Debug)]
pub struct LinearPlacementLock {
    pub stage: LockStage,
    pub last_placed: (i32, i32, i32),
}

impl LinearPlacementLock {
    pub fn new(pos: (i32, i32, i32)) -> Self {
        Self {
            stage: LockStage::InitialPlacement(pos),
            last_placed: pos,
        }
    }

    pub fn matches(&self, pos: (i32, i32, i32)) -> bool {
        let (x, y, z) = pos;
        match self.stage {
            LockStage::InitialPlacement((x1, y1, z1)) => {
                let dx = (x - x1).abs();
                let dy = (y - y1).abs();
                let dz = (z - z1).abs();
                dx + dy + dz == 1
            }
            LockStage::Line {
                axis,
                fixed_a,
                fixed_b,
            } => match axis {
                Axis::X => y == fixed_a && z == fixed_b,
                Axis::Y => x == fixed_a && z == fixed_b,
                Axis::Z => x == fixed_a && y == fixed_b,
            },
        }
    }

    pub fn register_placement(&mut self, pos: (i32, i32, i32)) {
        let (x1, y1, z1) = self.last_placed;
        let (x2, y2, z2) = pos;

        if let LockStage::InitialPlacement(_) = self.stage {
            let dx = (x2 - x1).abs();
            let dy = (y2 - y1).abs();
            let dz = (z2 - z1).abs();

            if dy > 0 {
                self.stage = LockStage::Line {
                    axis: Axis::Y,
                    fixed_a: x2,
                    fixed_b: z2,
                };
            } else if dx > 0 {
                self.stage = LockStage::Line {
                    axis: Axis::X,
                    fixed_a: y2,
                    fixed_b: z2,
                };
            } else if dz > 0 {
                self.stage = LockStage::Line {
                    axis: Axis::Z,
                    fixed_a: x2,
                    fixed_b: y2,
                };
            }
        }
        self.last_placed = pos;
    }
}

pub fn try_place_block_with_lock(
    world: &mut World,
    inv_slots: &mut [Option<ItemStack>; INVENTORY_SLOT_COUNT],
    selected_slot: usize,
    eye_pos: Vec3,
    look_dir: Vec3,
    player: &Player,
    lock: &mut Option<LinearPlacementLock>,
) -> bool {
    let res = world.raycast(eye_pos, look_dir, 8.0);
    if !res.hit {
        return false;
    }

    if try_till_farmland(world, inv_slots, selected_slot, &res) {
        return true;
    }

    let Some(s) = &mut inv_slots[selected_slot] else {
        return false;
    };

    if s.block == BlockType::FlintAndSteel {
        let target = world.get_block(res.x, res.y, res.z);
        if target == BlockType::TNT {
            prime_tnt(world, res.x, res.y, res.z);
            damage_selected_tool(inv_slots, selected_slot);
            return true;
        }
        let (nx, ny, nz) = (res.x + res.nx, res.y + res.ny, res.z + res.nz);
        if !player.intersects_block(nx, ny, nz) && world.try_ignite(nx, ny, nz) {
            damage_selected_tool(inv_slots, selected_slot);
            return true;
        }
        return false;
    }

    if s.block == BlockType::Bucket {
        let target = world.get_block(res.x, res.y, res.z);
        let liquid_level = world.get_liquid_level(res.x, res.y, res.z);
        if let Some(filled_bucket) = bucket_for_liquid(target, liquid_level) {
            world.set_block(res.x, res.y, res.z, BlockType::Air);
            world.set_liquid_level(res.x, res.y, res.z, 0);
            if filled_bucket == BlockType::WaterBucket {
                world.schedule_water_neighbors(res.x, res.y, res.z);
            }
            s.block = filled_bucket;
            return true;
        }
    } else if s.block == BlockType::WaterBucket {
        let (nx, ny, nz) = (res.x + res.nx, res.y + res.ny, res.z + res.nz);
        world.set_block(nx, ny, nz, BlockType::Water);
        world.set_liquid_level(nx, ny, nz, 1);
        world.schedule_water_neighbors(nx, ny, nz);
        s.block = BlockType::Bucket;
        return true;
    } else if s.block == BlockType::LavaBucket {
        let (nx, ny, nz) = (res.x + res.nx, res.y + res.ny, res.z + res.nz);
        world.set_block(nx, ny, nz, BlockType::Lava);
        world.set_liquid_level(nx, ny, nz, 1);
        s.block = BlockType::Bucket;
        return true;
    }

    if !s.block.is_item() {
        let (nx, ny, nz) = (res.x + res.nx, res.y + res.ny, res.z + res.nz);

        if let Some(l) = lock {
            if !l.matches((nx, ny, nz)) {
                return false;
            }
        }

        if world.get_block(nx, ny, nz) == BlockType::Air && !player.intersects_block(nx, ny, nz) {
            world.set_block(nx, ny, nz, s.block);

            if lock.is_none() {
                *lock = Some(LinearPlacementLock::new((nx, ny, nz)));
            } else if let Some(l) = lock {
                l.register_placement((nx, ny, nz));
            }

            s.count -= 1;
            if s.count == 0 {
                inv_slots[selected_slot] = None;
            }
            return true;
        }
    }

    false
}

/// Full MC 1.0 slot-click mechanics.
pub fn inv_click(
    slots: &mut [Option<ItemStack>; INVENTORY_SLOT_COUNT],
    cursor: &mut Option<ItemStack>,
    slot: usize,
    right: bool,
    shift: bool,
) {
    // Crafting output: pick up only (no placing)
    if slot == 44 {
        if !right
            && cursor.is_none()
            && let Some(output) = slots[44].take()
        {
            *cursor = Some(output);
            // Consume one from each crafting input
            for slot in slots.iter_mut().take(44).skip(40) {
                if let Some(s) = slot {
                    s.count -= 1;
                    if s.count == 0 {
                        *slot = None;
                    }
                }
            }
            // Re-check recipe
            update_craft_output_2x2(slots);
        }
        return;
    }
    // Armor slots: simple swap for now
    if (36..40).contains(&slot) {
        if !shift {
            std::mem::swap(&mut slots[slot], cursor);
        }
        return;
    }
    if shift {
        if let Some(s) = slots[slot] {
            slots[slot] = None;
            let sm = stack_max(s.block);
            let (a, b) = if slot < 9 {
                (9usize, 36usize)
            } else {
                (0usize, 9usize)
            };
            let mut rem = s.count;
            for slot_ref in slots.iter_mut().take(b).skip(a) {
                if rem == 0 {
                    break;
                }
                if let Some(d) = *slot_ref
                    && d.can_stack_with(s)
                    && d.count < sm
                {
                    let add = (sm - d.count).min(rem);
                    *slot_ref = Some(d.with_count(d.count + add));
                    rem -= add;
                }
            }
            for slot_ref in slots.iter_mut().take(b).skip(a) {
                if rem == 0 {
                    break;
                }
                if slot_ref.is_none() {
                    let n = rem.min(sm);
                    *slot_ref = Some(s.with_count(n));
                    rem -= n;
                }
            }
            if rem > 0 {
                slots[slot] = Some(s.with_count(rem));
            }
        }
        // Update crafting output if we touched crafting slots
        if (40..44).contains(&slot) {
            update_craft_output_2x2(slots);
        }
        return;
    }
    if right {
        if cursor.is_none() {
            if let Some(s) = slots[slot] {
                let half = s.count.div_ceil(2);
                *cursor = Some(s.with_count(half));
                let left = s.count - half;
                slots[slot] = if left > 0 {
                    Some(s.with_count(left))
                } else {
                    None
                };
            }
        } else {
            let held = cursor.unwrap();
            let sm = stack_max(held.block);
            let ok = match slots[slot] {
                None => true,
                Some(d) => d.can_stack_with(held) && d.count < sm,
            };
            if ok {
                match slots[slot] {
                    None => {
                        slots[slot] = Some(held.with_count(1));
                    }
                    Some(d) => {
                        slots[slot] = Some(d.with_count(d.count + 1));
                    }
                }
                let nc = held.count - 1;
                *cursor = if nc > 0 {
                    Some(held.with_count(nc))
                } else {
                    None
                };
            }
        }
    } else {
        match (*cursor, slots[slot]) {
            (None, _) => {
                *cursor = slots[slot].take();
            }
            (Some(h), None) => {
                slots[slot] = Some(h);
                *cursor = None;
            }
            (Some(h), Some(d)) if h.can_stack_with(d) => {
                let sm = stack_max(d.block);
                let add = (sm - d.count).min(h.count);
                slots[slot] = Some(d.with_count(d.count + add));
                let nc = h.count - add;
                *cursor = if nc > 0 { Some(h.with_count(nc)) } else { None };
            }
            _ => {
                std::mem::swap(&mut slots[slot], cursor);
            }
        }
    }
    // Update crafting output if we touched crafting slots
    if (40..44).contains(&slot) {
        update_craft_output_2x2(slots);
    }
}

/// Check 2x2 crafting grid and update output slot.
pub fn update_craft_output_2x2(slots: &mut [Option<ItemStack>; INVENTORY_SLOT_COUNT]) {
    let grid: Vec<Option<BlockType>> = (40..44).map(|i| slots[i].map(|s| s.block)).collect();
    if let Some((block, count)) = crafting::find_recipe(&grid, 2, 2) {
        if block.is_tool() {
            slots[44] = Some(ItemStack::new_tool(block));
        } else {
            slots[44] = Some(ItemStack::new(block, count as u32));
        }
    } else {
        slots[44] = None;
    }
}

/// Check 3x3 crafting grid and update output slot.
pub fn update_craft_output_3x3(slots: &mut [Option<ItemStack>; CRAFT_TABLE_SLOT_COUNT]) {
    let grid: Vec<Option<BlockType>> = (0..9).map(|i| slots[i].map(|s| s.block)).collect();
    if let Some((block, count)) = crafting::find_recipe(&grid, 3, 3) {
        if block.is_tool() {
            slots[9] = Some(ItemStack::new_tool(block));
        } else {
            slots[9] = Some(ItemStack::new(block, count as u32));
        }
    } else {
        slots[9] = None;
    }
}

/// Returns (x, y, w, h) of a slot in the inventory panel.
pub fn slot_rect(slot: usize, px: f32, py: f32) -> (f32, f32, f32, f32) {
    let ss = 36.0f32;
    let st = 38.0f32; // size, stride
    match slot {
        0..=8 => (px + 10.0 + slot as f32 * st, py + 294.0, ss, ss),
        9..=35 => {
            let i = slot - 9;
            (
                px + 10.0 + (i % 9) as f32 * st,
                py + 168.0 + (i / 9) as f32 * st,
                ss,
                ss,
            )
        }
        36..=39 => (px + 10.0, py + 10.0 + (slot - 36) as f32 * st, ss, ss),
        40..=43 => {
            let i = slot - 40;
            (
                px + 195.0 + (i % 2) as f32 * st,
                py + 28.0 + (i / 2) as f32 * st,
                ss,
                ss,
            )
        }
        44 => (px + 304.0, py + 42.0, ss, ss),
        _ => (0.0, 0.0, 0.0, 0.0),
    }
}

pub fn slot_at_pos(mx: f32, my: f32, px: f32, py: f32) -> Option<usize> {
    for s in (0..45).filter(|&s| !(36..40).contains(&s)) {
        // skip armor for now
        let (x, y, w, h) = slot_rect(s, px, py);
        if mx >= x && mx < x + w && my >= y && my < y + h {
            return Some(s);
        }
    }
    None
}

/// Crafting table UI layout.
/// Returns (x,y,w,h) for crafting table slots relative to panel origin.
/// Slots 0-8: 3x3 grid, 9: output, 100-126: inventory (mapped to inv 9-35), 127-135: hotbar overlay (mapped to inv 0-8)
pub fn ct_slot_rect(slot: usize, px: f32, py: f32) -> (f32, f32, f32, f32) {
    let ss = 36.0f32;
    let st = 38.0f32;
    match slot {
        0..=8 => {
            let col = slot % 3;
            let row = slot / 3;
            (
                px + 16.0 + col as f32 * st,
                py + 18.0 + row as f32 * st,
                ss,
                ss,
            )
        }
        9 => (px + 200.0, py + 52.0, ss, ss),
        100..=126 => {
            let i = slot - 100;
            (
                px + 10.0 + (i % 9) as f32 * st,
                py + 168.0 + (i / 9) as f32 * st,
                ss,
                ss,
            )
        }
        127..=135 => {
            let i = slot - 127;
            (px + 10.0 + i as f32 * st, py + 294.0, ss, ss)
        }
        _ => (0.0, 0.0, 0.0, 0.0),
    }
}

pub fn ct_slot_at_pos(mx: f32, my: f32, px: f32, py: f32) -> Option<usize> {
    for s in 0..=9 {
        let (x, y, w, h) = ct_slot_rect(s, px, py);
        if mx >= x && mx < x + w && my >= y && my < y + h {
            return Some(s);
        }
    }
    for s in 100..=126 {
        let (x, y, w, h) = ct_slot_rect(s, px, py);
        if mx >= x && mx < x + w && my >= y && my < y + h {
            return Some(s);
        }
    }
    for s in 127..=135 {
        let (x, y, w, h) = ct_slot_rect(s, px, py);
        if mx >= x && mx < x + w && my >= y && my < y + h {
            return Some(s);
        }
    }
    None
}

/// Handle click in crafting table UI.
pub fn ct_click(
    ct_slots: &mut [Option<ItemStack>; CRAFT_TABLE_SLOT_COUNT],
    inv_slots: &mut [Option<ItemStack>; INVENTORY_SLOT_COUNT],
    cursor: &mut Option<ItemStack>,
    ct_slot: usize,
    right: bool,
) {
    if ct_slot == 9 {
        if !right
            && cursor.is_none()
            && let Some(output) = ct_slots[9].take()
        {
            *cursor = Some(output);
            for slot in ct_slots.iter_mut().take(9) {
                if let Some(s) = slot {
                    s.count -= 1;
                    if s.count == 0 {
                        *slot = None;
                    }
                }
            }
            update_craft_output_3x3(ct_slots);
        }
        return;
    }

    let slot_ref: &mut Option<ItemStack> = if ct_slot <= 8 {
        &mut ct_slots[ct_slot]
    } else if (100..=126).contains(&ct_slot) {
        &mut inv_slots[ct_slot - 100 + 9]
    } else if (127..=135).contains(&ct_slot) {
        &mut inv_slots[ct_slot - 127]
    } else {
        return;
    };

    if right {
        if cursor.is_none() {
            if let Some(s) = *slot_ref {
                let half = s.count.div_ceil(2);
                *cursor = Some(s.with_count(half));
                let left = s.count - half;
                *slot_ref = if left > 0 {
                    Some(s.with_count(left))
                } else {
                    None
                };
            }
        } else {
            let held = cursor.unwrap();
            let sm = stack_max(held.block);
            let ok = match *slot_ref {
                None => true,
                Some(d) => d.can_stack_with(held) && d.count < sm,
            };
            if ok {
                match *slot_ref {
                    None => *slot_ref = Some(held.with_count(1)),
                    Some(d) => *slot_ref = Some(d.with_count(d.count + 1)),
                }
                let nc = held.count - 1;
                *cursor = if nc > 0 {
                    Some(held.with_count(nc))
                } else {
                    None
                };
            }
        }
    } else {
        match (*cursor, *slot_ref) {
            (None, _) => {
                *cursor = slot_ref.take();
            }
            (Some(h), None) => {
                *slot_ref = Some(h);
                *cursor = None;
            }
            (Some(h), Some(d)) if h.can_stack_with(d) => {
                let sm = stack_max(d.block);
                let add = (sm - d.count).min(h.count);
                *slot_ref = Some(d.with_count(d.count + add));
                let nc = h.count - add;
                *cursor = if nc > 0 { Some(h.with_count(nc)) } else { None };
            }
            _ => {
                std::mem::swap(slot_ref, cursor);
            }
        }
    }

    if ct_slot <= 8 {
        update_craft_output_3x3(ct_slots);
    }
}

/// Return crafting table items to inventory on close.
pub fn ct_close(
    ct_slots: &mut [Option<ItemStack>; CRAFT_TABLE_SLOT_COUNT],
    inv_slots: &mut [Option<ItemStack>; INVENTORY_SLOT_COUNT],
) {
    for slot in ct_slots.iter_mut().take(9) {
        if let Some(s) = slot.take() {
            *slot = inv_add_stack(inv_slots, s);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_starting_inventory_initialization() {
        let inv = create_starting_inventory();
        assert_eq!(inv[0].unwrap().block, BlockType::TNT);
        assert_eq!(inv[1].unwrap().block, BlockType::FlintAndSteel);
        assert_eq!(inv[2].unwrap().block, BlockType::Torch);
        assert_eq!(inv[2].unwrap().count, 64);
    }

    #[test]
    fn test_inv_add_stacks_properly() {
        let mut inv = [None::<ItemStack>; INVENTORY_SLOT_COUNT];
        assert_eq!(inv_add(&mut inv, BlockType::Dirt, 30), 0);
        assert_eq!(inv[0].unwrap().count, 30);
        inv_add(&mut inv, BlockType::Dirt, 40);
        assert_eq!(inv[0].unwrap().count, 64);
        assert_eq!(inv[1].unwrap().count, 6);
    }

    #[test]
    fn crafting_close_keeps_items_when_inventory_is_full() {
        let mut inv = [Some(ItemStack::new(BlockType::Stone, 64)); INVENTORY_SLOT_COUNT];
        let mut ct = [None::<ItemStack>; CRAFT_TABLE_SLOT_COUNT];
        ct[0] = Some(ItemStack::new(BlockType::Dirt, 3));

        ct_close(&mut ct, &mut inv);

        assert_eq!(ct[0], Some(ItemStack::new(BlockType::Dirt, 3)));
    }

    #[test]
    fn test_lava_fills_lava_bucket_even_with_liquid_level() {
        assert_eq!(
            bucket_for_liquid(BlockType::Lava, 1),
            Some(BlockType::LavaBucket)
        );
        assert_eq!(
            bucket_for_liquid(BlockType::Water, 1),
            Some(BlockType::WaterBucket)
        );
    }

    #[test]
    fn test_right_click_preserves_used_tool_durability() {
        let mut inv = [None::<ItemStack>; INVENTORY_SLOT_COUNT];
        let mut cursor = None;
        let mut tool = ItemStack::new_tool(BlockType::IronPickaxe);
        tool.durability = Some(17);
        inv[0] = Some(tool);

        inv_click(&mut inv, &mut cursor, 0, true, false);
        inv_click(&mut inv, &mut cursor, 1, true, false);
        damage_selected_tool(&mut inv, 1);

        assert_eq!(cursor, None);
        assert_eq!(inv[1].unwrap().durability, Some(16));
    }

    #[test]
    fn test_shift_click_preserves_used_tool_durability() {
        let mut inv = [None::<ItemStack>; INVENTORY_SLOT_COUNT];
        let mut cursor = None;
        let mut tool = ItemStack::new_tool(BlockType::StoneAxe);
        tool.durability = Some(23);
        inv[0] = Some(tool);

        inv_click(&mut inv, &mut cursor, 0, false, true);
        damage_selected_tool(&mut inv, 9);

        assert_eq!(inv[0], None);
        assert_eq!(inv[9].unwrap().durability, Some(22));
    }

    #[test]
    fn test_crafting_table_click_and_close_preserve_used_tool_durability() {
        let mut inv = [None::<ItemStack>; INVENTORY_SLOT_COUNT];
        let mut ct = [None::<ItemStack>; CRAFT_TABLE_SLOT_COUNT];
        let mut cursor = None;
        let mut tool = ItemStack::new_tool(BlockType::DiamondShovel);
        tool.durability = Some(31);
        inv[9] = Some(tool);

        ct_click(&mut ct, &mut inv, &mut cursor, 100, true);
        ct_click(&mut ct, &mut inv, &mut cursor, 0, true);
        ct_close(&mut ct, &mut inv);
        damage_selected_tool(&mut inv, 0);

        assert_eq!(cursor, None);
        assert_eq!(ct[0], None);
        assert_eq!(inv[0].unwrap().durability, Some(30));
    }

    #[test]
    fn test_merge_preserves_stack_durability_metadata() {
        let mut inv = [None::<ItemStack>; INVENTORY_SLOT_COUNT];
        let mut cursor = Some(ItemStack {
            block: BlockType::Dirt,
            count: 3,
            durability: Some(42),
        });
        inv[0] = Some(ItemStack {
            block: BlockType::Dirt,
            count: 2,
            durability: Some(42),
        });

        inv_click(&mut inv, &mut cursor, 0, false, false);

        assert_eq!(cursor, None);
        assert_eq!(inv[0].unwrap().count, 5);
        assert_eq!(inv[0].unwrap().durability, Some(42));
    }

    #[test]
    fn test_is_hoe_item() {
        assert!(is_hoe_item(BlockType::WoodHoe));
        assert!(is_hoe_item(BlockType::DiamondHoe));
        assert!(!is_hoe_item(BlockType::IronPickaxe));
    }

    #[test]
    fn test_linear_placement_lock_plane_to_line_transition() {
        let mut lock = LinearPlacementLock::new((10, 64, 5));
        assert_eq!(lock.stage, LockStage::InitialPlacement((10, 64, 5)));

        // Single block placement allows initial placement adjacent in any direction (including vertical)
        assert!(lock.matches((10, 64, 6))); // Z extension
        assert!(lock.matches((11, 64, 5))); // X extension
        assert!(lock.matches((10, 65, 5))); // Y vertical extension
        assert!(!lock.matches((11, 65, 5))); // Diagonal rejected
        assert!(!lock.matches((10, 64, 7))); // Non-adjacent rejected

        // Register 2nd block along Y axis (vertical pillar)
        lock.register_placement((10, 65, 5));
        assert_eq!(
            lock.stage,
            LockStage::Line {
                axis: Axis::Y,
                fixed_a: 10,
                fixed_b: 5
            }
        );

        // 3rd block: must stay on vertical Y axis (x=10, z=5)
        assert!(lock.matches((10, 66, 5)));
        assert!(!lock.matches((11, 66, 5))); // Drift along X rejected
    }
}
