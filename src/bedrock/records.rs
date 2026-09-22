use crate::{block::BlockType, inventory::ItemStack, java_compat::NbtTag};
use std::io;

pub fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

pub fn set(root: &mut NbtTag, key: &str, value: NbtTag) -> io::Result<()> {
    let NbtTag::Compound(fields) = root else {
        return Err(invalid("Expected compound record"));
    };
    if let Some((_, old)) = fields.iter_mut().find(|(name, _)| name == key) {
        *old = value;
    } else {
        fields.push((key.into(), value));
    }
    Ok(())
}

pub fn number(value: Option<&NbtTag>) -> Option<f64> {
    match value? {
        NbtTag::Byte(v) => Some(*v as f64),
        NbtTag::Short(v) => Some(*v as f64),
        NbtTag::Int(v) => Some(*v as f64),
        NbtTag::Long(v) => Some(*v as f64),
        NbtTag::Float(v) if v.is_finite() => Some(*v as f64),
        NbtTag::Double(v) if v.is_finite() => Some(*v),
        _ => None,
    }
}

pub fn item_name(block: BlockType) -> Option<String> {
    use BlockType::*;
    let special = match block {
        Air | Fire | PistonHead | WheatStage0 | WheatStage1 | WheatStage2 => return None,
        RawPorkchop => Some("porkchop"),
        RawBeef => Some("beef"),
        Steak => Some("cooked_beef"),
        RedstoneDust | RedstoneWire => Some("redstone"),
        Bed => Some("bed"),
        SnowLayer => Some("snow_layer"),
        SnowyGrass => Some("grass_block"),
        Water => Some("water_bucket"),
        Lava => Some("lava_bucket"),
        _ => None,
    };
    if let Some(name) = special {
        return Some(format!("minecraft:{name}"));
    }
    if let Some(NbtTag::String(name)) =
        super::blocks::state(block, 0).and_then(|tag| tag.get("name").cloned())
    {
        return Some(name);
    }
    let debug = format!("{block:?}");
    let mut name = std::string::String::new();
    for (i, c) in debug.chars().enumerate() {
        if c.is_uppercase() && i != 0 {
            name.push('_');
        }
        name.push(c.to_ascii_lowercase());
    }
    if name.starts_with("wood_") {
        name = name.replacen("wood_", "wooden_", 1);
    }
    if name.starts_with("gold_") && !matches!(block, GoldIngot) {
        name = name.replacen("gold_", "golden_", 1);
    }
    Some(format!("minecraft:{name}"))
}

fn durability(block: BlockType) -> Option<u16> {
    crate::item::tool_properties(block)
        .map(|p| p.durability)
        .or_else(|| crate::item::armor_properties(block).map(|p| p.durability))
}

pub fn item(stack: ItemStack, slot: u8) -> io::Result<NbtTag> {
    let name = item_name(stack.block).ok_or_else(|| invalid("Unsupported inventory item"))?;
    if stack.count == 0 || stack.count > 127 {
        return Err(invalid("Invalid Bedrock item count"));
    }
    let damage = durability(stack.block)
        .zip(stack.durability)
        .map_or(0, |(max, left)| max.saturating_sub(left));
    Ok(NbtTag::Compound(vec![
        ("Name".into(), NbtTag::String(name)),
        ("Count".into(), NbtTag::Byte(stack.count as i8)),
        ("Slot".into(), NbtTag::Byte(slot as i8)),
        (
            "Damage".into(),
            NbtTag::Short(if stack.block == BlockType::Bed { 14 } else { 0 }),
        ),
        ("WasPickedUp".into(), NbtTag::Byte(0)),
        (
            "tag".into(),
            NbtTag::Compound(if durability(stack.block).is_some() {
                vec![("Damage".into(), NbtTag::Int(damage as i32))]
            } else {
                vec![]
            }),
        ),
    ]))
}

pub fn decode_item(tag: &NbtTag) -> io::Result<Option<(usize, ItemStack)>> {
    let count = number(tag.get("Count")).unwrap_or(0.0);
    if count == 0.0 {
        return Ok(None);
    }
    if !(1.0..=127.0).contains(&count) || count.fract() != 0.0 {
        return Err(invalid("Invalid inventory count"));
    }
    let Some(NbtTag::String(name)) = tag.get("Name") else {
        return Err(invalid("Unsupported numeric item identifier"));
    };
    let block = (0..BlockType::COUNT)
        .map(|id| BlockType::from_u16(id as u16))
        .filter(|block| !matches!(block, BlockType::Water | BlockType::Lava))
        .find(|block| item_name(*block).as_ref() == Some(name))
        .ok_or_else(|| invalid(&format!("Unsupported Bedrock inventory item: {name}")))?;
    let slot = number(tag.get("Slot")).ok_or_else(|| invalid("Missing inventory slot"))?;
    if !(0.0..=127.0).contains(&slot) || slot.fract() != 0.0 {
        return Err(invalid("Invalid inventory slot"));
    }
    if let Some(NbtTag::Compound(fields)) = tag.get("tag") {
        if fields.iter().any(|(key, _)| key != "Damage") {
            return Err(invalid(
                "This inventory contains item data VoxelPopuli cannot yet preserve while moving items (such as enchantments or custom names)",
            ));
        }
    }
    let used = number(tag.get("tag").and_then(|tag| tag.get("Damage"))).unwrap_or(0.0);
    if used < 0.0 || used > u16::MAX as f64 || used.fract() != 0.0 {
        return Err(invalid("Invalid item damage"));
    }
    let remaining = durability(block).map(|max| max.saturating_sub(used as u16));
    Ok(Some((
        slot as usize,
        ItemStack {
            block,
            count: count as u32,
            durability: remaining,
        },
    )))
}

pub fn inventory(slots: &[Option<ItemStack>]) -> io::Result<NbtTag> {
    Ok(NbtTag::List {
        element_type: 10,
        tags: slots
            .iter()
            .enumerate()
            .filter_map(|(i, stack)| stack.map(|stack| item(stack, i as u8)))
            .collect::<io::Result<Vec<_>>>()?,
    })
}

pub fn decode_inventory(tag: &NbtTag, slots: &mut [Option<ItemStack>]) -> io::Result<()> {
    let NbtTag::List {
        element_type: 10,
        tags,
    } = tag
    else {
        return Err(invalid("Invalid inventory list"));
    };
    let mut decoded = vec![None; slots.len()];
    for tag in tags {
        if let Some((slot, item)) = decode_item(tag)? {
            let dest = decoded
                .get_mut(slot)
                .ok_or_else(|| invalid("Inventory slot outside supported range"))?;
            if dest.is_some() {
                return Err(invalid("Duplicate inventory slot"));
            }
            *dest = Some(item);
        }
    }
    slots.copy_from_slice(&decoded);
    Ok(())
}

pub fn encode(root: &NbtTag) -> io::Result<Vec<u8>> {
    super::nbt::encode("", root)
}
pub fn decode(bytes: &[u8]) -> io::Result<NbtTag> {
    let (_, tag, used) = super::nbt::decode(bytes)?;
    if used != bytes.len() {
        return Err(invalid("Trailing record data"));
    }
    Ok(tag)
}

pub fn decode_stream(mut bytes: &[u8]) -> io::Result<Vec<NbtTag>> {
    let mut records = Vec::new();
    while !bytes.is_empty() {
        if records.len() >= 65_536 {
            return Err(invalid("Too many block entities in one chunk"));
        }
        let (_, record, used) = super::nbt::decode(bytes)?;
        records.push(record);
        bytes = &bytes[used..];
    }
    Ok(records)
}

pub fn position(root: &NbtTag) -> io::Result<(i32, i32, i32)> {
    let coordinate = |key| match root.get(key) {
        Some(NbtTag::Int(value)) => Ok(*value),
        _ => Err(invalid("Invalid block entity coordinate")),
    };
    Ok((coordinate("x")?, coordinate("y")?, coordinate("z")?))
}

pub fn decode_container(root: &NbtTag) -> io::Result<Option<crate::container::Container>> {
    use crate::container::{CHEST_SLOTS, Container, Furnace};
    let Some(NbtTag::String(id)) = root.get("id") else {
        return Ok(None);
    };
    if !matches!(id.as_str(), "Chest" | "Furnace") {
        return Ok(None);
    }
    if root.get("LootTable").is_some() {
        return Err(invalid(
            "Unopened loot-table containers are not yet supported",
        ));
    }
    let empty = NbtTag::List {
        element_type: 10,
        tags: vec![],
    };
    let items = root.get("Items").unwrap_or(&empty);
    Ok(Some(if id == "Chest" {
        let mut slots = [None; CHEST_SLOTS];
        decode_inventory(items, &mut slots)?;
        Container::Chest(Box::new(slots))
    } else {
        let mut furnace = Furnace::default();
        decode_inventory(items, &mut furnace.slots)?;
        furnace.burn_remaining =
            (number(root.get("BurnTime")).unwrap_or(0.0) as f32 / 20.0).max(0.0);
        furnace.burn_total = (number(root.get("BurnDuration")).unwrap_or(0.0) as f32 / 20.0)
            .max(furnace.burn_remaining);
        furnace.progress =
            (number(root.get("CookTime")).unwrap_or(0.0) as f32 / 20.0).clamp(0.0, 10.0);
        furnace.cooking = furnace.slots[0].map(|stack| stack.block);
        Container::Furnace(furnace)
    }))
}

pub fn container(
    root: &mut NbtTag,
    position: (i32, i32, i32),
    container: &crate::container::Container,
) -> io::Result<()> {
    use crate::container::Container;
    let (id, slots) = match container {
        Container::Chest(slots) => ("Chest", slots.as_slice()),
        Container::Furnace(furnace) => ("Furnace", furnace.slots.as_slice()),
    };
    set(root, "id", NbtTag::String(id.into()))?;
    for (key, value) in [("x", position.0), ("y", position.1), ("z", position.2)] {
        set(root, key, NbtTag::Int(value))?;
    }
    set(root, "Items", inventory(slots)?)?;
    if let Container::Furnace(furnace) = container {
        for (key, seconds) in [
            ("BurnTime", furnace.burn_remaining),
            ("BurnDuration", furnace.burn_total),
            ("CookTime", furnace.progress),
        ] {
            set(
                root,
                key,
                NbtTag::Short((seconds * 20.0).clamp(0.0, i16::MAX as f32) as i16),
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inventory_round_trip_preserves_slot_count_and_remaining_durability() {
        let mut tool = ItemStack::new_tool(BlockType::IronPickaxe);
        tool.durability = Some(tool.durability.unwrap() - 42);
        let original = [
            Some(ItemStack::new(BlockType::OakLog, 42)),
            None,
            Some(tool),
        ];
        let bytes = encode(&inventory(&original).unwrap()).unwrap();
        let mut restored = [None; 3];
        decode_inventory(&decode(&bytes).unwrap(), &mut restored).unwrap();
        assert_eq!(restored, original);
    }
    #[test]
    fn unsupported_item_data_and_duplicate_slots_fail_without_mutating_inventory() {
        let mut custom = item(ItemStack::new(BlockType::Stone, 3), 0).unwrap();
        set(
            &mut custom,
            "tag",
            NbtTag::Compound(vec![("display".into(), NbtTag::Compound(vec![]))]),
        )
        .unwrap();
        assert!(decode_item(&custom).is_err());
        let original = [Some(ItemStack::new(BlockType::Dirt, 1))];
        let mut slots = original;
        let entry = item(ItemStack::new(BlockType::Stone, 1), 0).unwrap();
        assert!(
            decode_inventory(
                &NbtTag::List {
                    element_type: 10,
                    tags: vec![entry.clone(), entry]
                },
                &mut slots
            )
            .is_err()
        );
        assert_eq!(slots, original);
    }
}
