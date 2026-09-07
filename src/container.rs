use crate::block::BlockType;
use crate::crafting::{fuel_burn_time, smelt_item};
use crate::inventory::{INVENTORY_SLOT_COUNT, ItemStack, click_stack, inv_add_stack, move_stack};

pub const CHEST_SLOTS: usize = 27;
pub const SMELT_SECONDS: f32 = 10.0;

pub fn remove_replaced_container(
    containers: &mut std::collections::HashMap<(i32, i32, i32), Container>,
    pending: &mut Vec<ItemStack>,
    position: (i32, i32, i32),
    replacement: BlockType,
) {
    if containers
        .get(&position)
        .is_some_and(|c| c.block() != replacement)
        && let Some(container) = containers.remove(&position)
    {
        pending.extend(container.slots().iter().flatten().copied());
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Furnace {
    // Input, fuel, output.
    pub slots: [Option<ItemStack>; 3],
    pub burn_remaining: f32,
    pub burn_total: f32,
    pub progress: f32,
    pub cooking: Option<BlockType>,
}

impl Default for Furnace {
    fn default() -> Self {
        Self {
            slots: [None; 3],
            burn_remaining: 0.0,
            burn_total: 0.0,
            progress: 0.0,
            cooking: None,
        }
    }
}

impl Furnace {
    fn recipe(&self) -> Option<ItemStack> {
        let (block, count) = smelt_item(self.slots[0]?.block)?;
        let output = ItemStack::new(block, count as u32);
        match self.slots[2] {
            None => Some(output),
            Some(stack)
                if stack.can_stack_with(output)
                    && stack.count + output.count <= crate::inventory::stack_max(block) =>
            {
                Some(output)
            }
            _ => None,
        }
    }

    pub fn reset_changed_input(&mut self) {
        let input = self.slots[0].map(|stack| stack.block);
        if self.cooking != input {
            self.progress = 0.0;
            self.cooking = input;
        }
    }

    pub fn tick(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.reset_changed_input();
        let mut remaining = dt;
        while remaining > 0.0 {
            let recipe = self.recipe();
            if self.burn_remaining <= 0.0 && recipe.is_some() {
                let duration = self.slots[1].map_or(0.0, |s| fuel_burn_time(s.block));
                if duration > 0.0 {
                    consume_one(&mut self.slots[1]);
                    self.burn_remaining = duration;
                    self.burn_total = duration;
                }
            }
            if self.burn_remaining <= 0.0 || recipe.is_none() {
                self.burn_remaining = (self.burn_remaining - remaining).max(0.0);
                self.progress = 0.0;
                return;
            }
            let step = remaining
                .min(self.burn_remaining)
                .min(SMELT_SECONDS - self.progress);
            self.burn_remaining = (self.burn_remaining - step).max(0.0);
            self.progress += step;
            remaining -= step;
            if self.progress >= SMELT_SECONDS {
                let output = recipe.unwrap();
                self.slots[2] =
                    Some(output.with_count(self.slots[2].map_or(0, |s| s.count) + output.count));
                consume_one(&mut self.slots[0]);
                self.progress = 0.0;
                self.reset_changed_input();
            }
        }
    }
}

fn consume_one(slot: &mut Option<ItemStack>) {
    if let Some(stack) = *slot {
        *slot = (stack.count > 1).then(|| stack.with_count(stack.count - 1));
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Container {
    Chest(Box<[Option<ItemStack>; CHEST_SLOTS]>),
    Furnace(Furnace),
}

impl Container {
    pub fn new(block: BlockType) -> Option<Self> {
        match block {
            BlockType::Chest => Some(Self::Chest(Box::new([None; CHEST_SLOTS]))),
            BlockType::Furnace => Some(Self::Furnace(Furnace::default())),
            _ => None,
        }
    }

    pub fn block(&self) -> BlockType {
        match self {
            Self::Chest(_) => BlockType::Chest,
            Self::Furnace(_) => BlockType::Furnace,
        }
    }

    pub fn slots(&self) -> &[Option<ItemStack>] {
        match self {
            Self::Chest(slots) => slots.as_ref(),
            Self::Furnace(furnace) => &furnace.slots,
        }
    }

    fn slots_mut(&mut self) -> &mut [Option<ItemStack>] {
        match self {
            Self::Chest(slots) => slots.as_mut(),
            Self::Furnace(furnace) => &mut furnace.slots,
        }
    }

    fn accepts(&self, slot: usize, stack: ItemStack) -> bool {
        match self {
            Self::Chest(_) => slot < CHEST_SLOTS,
            Self::Furnace(_) => match slot {
                0 => smelt_item(stack.block).is_some(),
                1 => fuel_burn_time(stack.block) > 0.0,
                _ => false,
            },
        }
    }

    // Container slots start at 0; player inventory uses 100..136.
    pub fn click(
        &mut self,
        inventory: &mut [Option<ItemStack>; INVENTORY_SLOT_COUNT],
        cursor: &mut Option<ItemStack>,
        slot: usize,
        right: bool,
        shift: bool,
    ) {
        if (100..136).contains(&slot) {
            let source = &mut inventory[slot - 100];
            if !shift {
                click_stack(source, cursor, right);
                return;
            }
            if let Some(stack) = *source {
                let targets = (0..self.slots().len())
                    .filter(|&i| self.accepts(i, stack))
                    .collect::<Vec<_>>();
                *source = move_stack(self.slots_mut(), &targets, stack);
            }
        } else if slot < self.slots().len() {
            if shift {
                if let Some(stack) = self.slots_mut()[slot].take() {
                    self.slots_mut()[slot] = inv_add_stack(inventory, stack);
                }
            } else if matches!(self, Self::Furnace(_)) && slot == 2 {
                // Output can only be taken, including into a matching cursor stack.
                if let Some(output) = self.slots()[slot] {
                    let room = match *cursor {
                        None => crate::inventory::stack_max(output.block),
                        Some(held) if held.can_stack_with(output) => {
                            crate::inventory::stack_max(held.block).saturating_sub(held.count)
                        }
                        _ => 0,
                    };
                    let count = room.min(if right {
                        output.count.div_ceil(2)
                    } else {
                        output.count
                    });
                    if count > 0 {
                        *cursor = Some(output.with_count(cursor.map_or(0, |s| s.count) + count));
                        self.slots_mut()[slot] =
                            (output.count > count).then(|| output.with_count(output.count - count));
                    }
                }
            } else if cursor.is_none_or(|held| self.accepts(slot, held)) {
                click_stack(&mut self.slots_mut()[slot], cursor, right);
            }
        }
        if let Self::Furnace(furnace) = self {
            furnace.reset_changed_input();
        }
    }

    pub fn slot_rect(&self, slot: usize, px: f32, py: f32) -> (f32, f32, f32, f32) {
        if (100..136).contains(&slot) {
            return crate::inventory::slot_rect(slot - 100, px, py);
        }
        match self {
            Self::Chest(_) => (
                px + 10.0 + (slot % 9) as f32 * 38.0,
                py + 32.0 + (slot / 9) as f32 * 38.0,
                36.0,
                36.0,
            ),
            Self::Furnace(_) => match slot {
                0 => (px + 90.0, py + 30.0, 36.0, 36.0),
                1 => (px + 90.0, py + 108.0, 36.0, 36.0),
                2 => (px + 246.0, py + 66.0, 36.0, 36.0),
                _ => (0.0, 0.0, 0.0, 0.0),
            },
        }
    }

    pub fn slot_at_pos(&self, mx: f32, my: f32, px: f32, py: f32) -> Option<usize> {
        (0..self.slots().len()).chain(100..136).find(|&slot| {
            let (x, y, w, h) = self.slot_rect(slot, px, py);
            mx >= x && mx < x + w && my >= y && my < y + h
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use BlockType::*;

    fn furnace(input: BlockType, count: u32, fuel: BlockType, fuel_count: u32) -> super::Furnace {
        super::Furnace {
            slots: [
                Some(ItemStack::new(input, count)),
                Some(ItemStack::new(fuel, fuel_count)),
                None,
            ],
            ..super::Furnace::default()
        }
    }

    #[test]
    fn coal_smelts_exactly_eight_iron_ore() {
        let mut f = furnace(IronOre, 9, Coal, 1);
        f.tick(80.0);
        assert_eq!(f.slots[2], Some(ItemStack::new(IronIngot, 8)));
        assert_eq!(f.slots[0], Some(ItemStack::new(IronOre, 1)));
        assert_eq!(f.slots[1], None);
        assert_eq!(f.burn_remaining, 0.0);
        f.tick(20.0);
        assert_eq!(f.slots[2].unwrap().count, 8);
    }

    #[test]
    fn partial_fuels_continue_a_recipe_and_food_cooks() {
        let mut f = furnace(RawBeef, 1, Stick, 2);
        f.tick(9.0);
        assert_eq!(f.slots[2], None);
        f.tick(1.0);
        assert_eq!(f.slots[2], Some(ItemStack::new(Steak, 1)));
    }

    #[test]
    fn blocked_output_and_invalid_input_do_not_consume_fuel() {
        let mut f = furnace(IronOre, 2, Coal, 1);
        for output in [ItemStack::new(IronIngot, 64), ItemStack::new(Glass, 1)] {
            f.slots[2] = Some(output);
            f.tick(20.0);
            assert_eq!(f.slots[1], Some(ItemStack::new(Coal, 1)));
            assert_eq!(f.slots[2], Some(output));
        }
        f.slots[0] = Some(ItemStack::new(Dirt, 1));
        f.slots[2] = None;
        f.tick(20.0);
        assert_eq!(f.slots[1], Some(ItemStack::new(Coal, 1)));
    }

    #[test]
    fn changing_input_resets_progress_and_idle_fire_burns_down() {
        let mut f = furnace(IronOre, 1, Coal, 1);
        f.tick(9.0);
        f.slots[0] = Some(ItemStack::new(Sand, 1));
        f.tick(1.0);
        assert_eq!(f.progress, 1.0);
        assert_eq!(f.slots[2], None);
        f.slots[0] = None;
        f.tick(10.0);
        assert_eq!(f.progress, 0.0);
        assert_eq!(f.burn_remaining, 60.0);
    }

    #[test]
    fn shift_transfer_preserves_tools_and_full_inventory_preserves_output() {
        let mut c = Container::new(Chest).unwrap();
        let mut inv = [None; INVENTORY_SLOT_COUNT];
        let tool = ItemStack {
            durability: Some(7),
            ..ItemStack::new_tool(IronPickaxe)
        };
        inv[0] = Some(tool);
        c.click(&mut inv, &mut None, 100, false, true);
        assert_eq!(c.slots()[0], Some(tool));
        assert_eq!(inv[0], None);
        c.click(&mut inv, &mut None, 0, false, true);
        assert_eq!(inv[0], Some(tool));

        let mut f = furnace(IronOre, 1, Coal, 1);
        f.tick(10.0);
        let mut c = Container::Furnace(f);
        inv.fill(Some(ItemStack::new(Stone, 64)));
        c.click(&mut inv, &mut None, 2, false, true);
        assert_eq!(c.slots()[2], Some(ItemStack::new(IronIngot, 1)));
    }

    #[test]
    fn furnace_slots_filter_insertions_and_output_is_take_only() {
        let mut c = Container::new(Furnace).unwrap();
        let mut inv = [None; INVENTORY_SLOT_COUNT];
        let mut cursor = Some(ItemStack::new(Dirt, 12));
        for slot in 0..3 {
            c.click(&mut inv, &mut cursor, slot, false, false);
            assert_eq!(c.slots()[slot], None);
        }
        inv[0] = Some(ItemStack::new(Coal, 12));
        c.click(&mut inv, &mut None, 100, false, true);
        assert_eq!(c.slots()[1], Some(ItemStack::new(Coal, 12)));
        assert_eq!(inv[0], None);
    }

    #[test]
    fn slot_hitboxes_match_both_container_layouts() {
        for block in [Chest, Furnace] {
            let c = Container::new(block).unwrap();
            for slot in (0..c.slots().len()).chain(100..136) {
                let (x, y, _, _) = c.slot_rect(slot, 200.0, 100.0);
                assert_eq!(c.slot_at_pos(x + 18.0, y + 18.0, 200.0, 100.0), Some(slot));
            }
        }
    }
}
