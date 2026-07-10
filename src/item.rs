use crate::block::BlockType;

// ── Tool classification ─────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ToolType {
    None,
    Pickaxe,
    Axe,
    Shovel,
    Sword,
    Hoe,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum ToolMaterial {
    None = 0,
    Wood = 1,
    Gold = 2, // Gold is tier 2 in MC 1.0 (same harvest level as wood)
    Stone = 3,
    Iron = 4,
    Diamond = 5,
}

// ── Block properties ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct BlockProperties {
    pub hardness: f32,          // -1.0 = unbreakable
    pub tool_type: ToolType,    // preferred tool
    pub min_tier: ToolMaterial, // minimum tool tier to get drops
    pub requires_tool: bool,    // if true, wrong/no tool = no drop + 3.33x slower
    pub drop: BlockType,        // what it drops (Air = nothing)
    pub drop_count: u8,
}

pub fn block_properties(b: BlockType) -> BlockProperties {
    use BlockType::*;
    use ToolMaterial as TM;
    use ToolType as TT;

    let s = |hardness, tool_type, min_tier, requires_tool, drop, drop_count| BlockProperties {
        hardness,
        tool_type,
        min_tier,
        requires_tool,
        drop,
        drop_count,
    };

    match b {
        // Instant break
        Torch => s(0.0, TT::None, TM::None, false, Torch, 1),
        SnowLayer => s(0.1, TT::Shovel, TM::None, false, SnowLayer, 1),

        // Fragile blocks
        Glass => s(0.3, TT::None, TM::None, false, Air, 0), // Glass drops nothing in 1.0
        OakLeaves => s(0.2, TT::None, TM::None, false, Air, 0),
        SpruceLeaves => s(0.2, TT::None, TM::None, false, Air, 0),

        // Soft blocks (shovel)
        Dirt => s(0.5, TT::Shovel, TM::None, false, Dirt, 1),
        Sand => s(0.5, TT::Shovel, TM::None, false, Sand, 1),
        Grass => s(0.6, TT::Shovel, TM::None, false, Dirt, 1),
        SnowyGrass => s(0.6, TT::Shovel, TM::None, false, Dirt, 1),
        Gravel => s(0.6, TT::Shovel, TM::None, false, Gravel, 1),
        PowderedSnow => s(0.25, TT::Shovel, TM::None, false, PowderedSnow, 1),
        Clay => s(0.6, TT::Shovel, TM::None, false, Clay, 1),
        Farmland => s(0.6, TT::Shovel, TM::None, false, Dirt, 1),
        Sponge => s(0.6, TT::None, TM::None, false, Sponge, 1),
        Wool => s(0.8, TT::None, TM::None, false, Wool, 1),
        Wheat => s(0.0, TT::None, TM::None, false, Wheat, 1),
        Cactus => s(0.4, TT::None, TM::None, false, Cactus, 1),

        // Wood blocks (axe)
        OakLog => s(2.0, TT::Axe, TM::None, false, OakLog, 1),
        SpruceLog => s(2.0, TT::Axe, TM::None, false, SpruceLog, 1),
        OakPlanks => s(2.0, TT::Axe, TM::None, false, OakPlanks, 1),
        CraftingTable => s(2.5, TT::Axe, TM::None, false, CraftingTable, 1),
        Chest => s(2.5, TT::Axe, TM::None, false, Chest, 1),
        Bookshelf => s(1.5, TT::Axe, TM::None, false, Bookshelf, 1),

        // Stone blocks (pickaxe required)
        Stone => s(1.5, TT::Pickaxe, TM::Wood, true, Cobblestone, 1),
        Cobblestone => s(2.0, TT::Pickaxe, TM::Wood, true, Cobblestone, 1),
        StoneBrick => s(1.5, TT::Pickaxe, TM::Wood, true, StoneBrick, 1),
        Brick => s(2.0, TT::Pickaxe, TM::Wood, true, Brick, 1),
        MossyCobblestone => s(2.0, TT::Pickaxe, TM::Wood, true, MossyCobblestone, 1),
        Sandstone => s(0.8, TT::Pickaxe, TM::Wood, true, Sandstone, 1),
        Furnace => s(3.5, TT::Pickaxe, TM::Wood, true, Furnace, 1),

        // Ores (pickaxe required, varying tiers)
        CoalOre => s(3.0, TT::Pickaxe, TM::Wood, true, Coal, 1),
        IronOre => s(3.0, TT::Pickaxe, TM::Stone, true, IronOre, 1),
        GoldOre => s(3.0, TT::Pickaxe, TM::Iron, true, GoldOre, 1),
        DiamondOre => s(3.0, TT::Pickaxe, TM::Iron, true, Diamond, 1),
        LapisOre => s(3.0, TT::Pickaxe, TM::Stone, true, LapisLazuli, 4),
        RedstoneOre => s(3.0, TT::Pickaxe, TM::Iron, true, RedstoneDust, 4),

        // Metal/gem blocks
        IronBlock => s(5.0, TT::Pickaxe, TM::Stone, true, IronBlock, 1),
        LapisBlock => s(3.0, TT::Pickaxe, TM::Stone, true, LapisBlock, 1),

        // Obsidian (diamond pickaxe required)
        Obsidian => s(50.0, TT::Pickaxe, TM::Diamond, true, Obsidian, 1),

        // Special
        Bedrock => s(-1.0, TT::None, TM::None, false, Air, 0),
        Water => s(-1.0, TT::None, TM::None, false, Air, 0),
        Lava => s(-1.0, TT::None, TM::None, false, Air, 0),
        Air => s(0.0, TT::None, TM::None, false, Air, 0),
        MobSpawner => s(5.0, TT::Pickaxe, TM::Wood, true, Air, 0),

        // TNT drops itself
        TNT => s(0.0, TT::None, TM::None, false, TNT, 1),

        // Village bell
        Bell => s(5.0, TT::Pickaxe, TM::Wood, true, Bell, 1),

        // Items don't have block hardness (shouldn't be mined)
        _ => s(0.0, TT::None, TM::None, false, b, 1),
    }
}

// ── Tool properties ──────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug)]
pub struct ToolProperties {
    pub tool_type: ToolType,
    pub material: ToolMaterial,
    pub speed: f32,
    pub durability: u16,
}

pub fn tool_properties(b: BlockType) -> Option<ToolProperties> {
    use BlockType::*;
    use ToolMaterial as TM;
    use ToolType as TT;

    let t = |tool_type, material: TM, speed, durability| {
        Some(ToolProperties {
            tool_type,
            material,
            speed,
            durability,
        })
    };

    match b {
        // Pickaxes
        WoodPickaxe => t(TT::Pickaxe, TM::Wood, 2.0, 59),
        StonePickaxe => t(TT::Pickaxe, TM::Stone, 4.0, 131),
        IronPickaxe => t(TT::Pickaxe, TM::Iron, 6.0, 250),
        DiamondPickaxe => t(TT::Pickaxe, TM::Diamond, 8.0, 1561),
        GoldPickaxe => t(TT::Pickaxe, TM::Gold, 12.0, 32),

        // Axes
        WoodAxe => t(TT::Axe, TM::Wood, 2.0, 59),
        StoneAxe => t(TT::Axe, TM::Stone, 4.0, 131),
        IronAxe => t(TT::Axe, TM::Iron, 6.0, 250),
        DiamondAxe => t(TT::Axe, TM::Diamond, 8.0, 1561),
        GoldAxe => t(TT::Axe, TM::Gold, 12.0, 32),

        // Shovels
        WoodShovel => t(TT::Shovel, TM::Wood, 2.0, 59),
        StoneShovel => t(TT::Shovel, TM::Stone, 4.0, 131),
        IronShovel => t(TT::Shovel, TM::Iron, 6.0, 250),
        DiamondShovel => t(TT::Shovel, TM::Diamond, 8.0, 1561),
        GoldShovel => t(TT::Shovel, TM::Gold, 12.0, 32),

        // Swords (act as 1.5x speed on all blocks in MC 1.0)
        WoodSword => t(TT::Sword, TM::Wood, 1.5, 59),
        StoneSword => t(TT::Sword, TM::Stone, 1.5, 131),
        IronSword => t(TT::Sword, TM::Iron, 1.5, 250),
        DiamondSword => t(TT::Sword, TM::Diamond, 1.5, 1561),
        GoldSword => t(TT::Sword, TM::Gold, 1.5, 32),

        // Hoes (no mining speed bonus)
        WoodHoe => t(TT::Hoe, TM::Wood, 1.0, 59),
        StoneHoe => t(TT::Hoe, TM::Stone, 1.0, 131),
        IronHoe => t(TT::Hoe, TM::Iron, 1.0, 250),
        DiamondHoe => t(TT::Hoe, TM::Diamond, 1.0, 1561),
        GoldHoe => t(TT::Hoe, TM::Gold, 1.0, 32),

        // Flint and Steel (durability-based item, not a mining tool)
        FlintAndSteel => t(TT::None, TM::None, 1.0, 64),

        _ => None,
    }
}

// ── Breaking time calculation (MC 1.0 formula) ──────────────────────────────

/// Returns time in seconds to break a block with the given held item.
/// Returns f32::INFINITY for unbreakable blocks.
pub fn breaking_time(block: BlockType, held: BlockType) -> f32 {
    let props = block_properties(block);

    // Unbreakable
    if props.hardness < 0.0 {
        return f32::INFINITY;
    }

    // Instant break
    if props.hardness == 0.0 {
        return 0.05; // 1 tick minimum
    }

    let tool = tool_properties(held);
    let mut speed = 1.0f32;
    let mut can_harvest = !props.requires_tool; // If no tool required, hand can harvest

    if let Some(tp) = tool {
        if tp.tool_type == props.tool_type {
            // Correct tool type: apply speed multiplier
            speed = tp.speed;
            // Check if tool tier meets minimum
            if tp.material >= props.min_tier {
                can_harvest = true;
            }
        } else if !props.requires_tool {
            // Wrong tool type but block doesn't require specific tool
            can_harvest = true;
        }
    }

    // MC 1.0 breaking time formula
    let base_time = if can_harvest {
        props.hardness * 1.5
    } else {
        props.hardness * 5.0 // 3.33x slower when can't harvest
    };

    base_time / speed
}

/// Returns what a block drops when mined with the given tool.
/// Returns (drop_type, drop_count). Air with count 0 means no drop.
pub fn get_drop(block: BlockType, held: BlockType) -> (BlockType, u8) {
    let props = block_properties(block);

    if props.requires_tool {
        if let Some(tp) = tool_properties(held)
            && tp.tool_type == props.tool_type
            && tp.material >= props.min_tier
        {
            return (props.drop, props.drop_count);
        }
        // Wrong tool or insufficient tier: no drop
        return (BlockType::Air, 0);
    }

    (props.drop, props.drop_count)
}

// ── Atlas UV mapping (centralized) ──────────────────────────────────────────

/// Returns (tx, ty) atlas grid coordinates for a BlockType.
/// Used by both chunk meshing and UI rendering.
pub fn atlas_uv(b: BlockType) -> (u8, u8) {
    use BlockType::*;
    match b {
        // Row 0: original blocks
        Stone => (1, 0),
        Dirt => (2, 0),
        Grass => (3, 0),
        OakLog => (4, 0),
        OakLeaves => (5, 0),
        Sand => (6, 0),
        Gravel => (7, 0),
        PowderedSnow | SnowLayer => (8, 0),
        SnowyGrass => (10, 0),
        SpruceLog => (11, 0),
        SpruceLeaves => (12, 0),
        Water => (13, 12),
        Lava => (7, 7),

        // Row 1: original continued
        Bedrock => (1, 1),
        CoalOre => (2, 1),
        IronOre => (3, 1),
        TNT => (4, 1),
        IronBlock => (6, 1),
        Bell => (12, 1),
        RawIron => (7, 1),
        IronIngot => (8, 1),
        FlintAndSteel => (9, 1),

        // Row 2: new placeable blocks
        Cobblestone => (0, 2),
        OakPlanks => (1, 2),
        CraftingTable => (2, 2), // top face; sides use (3,2)
        Furnace => (4, 2),       // front face; sides use cobblestone
        Glass => (6, 2),
        GoldOre => (7, 2),
        DiamondOre => (8, 2),
        Obsidian => (9, 2),
        Brick => (10, 2),
        StoneBrick => (11, 2),
        Sandstone => (12, 2),
        Wool => (13, 2),
        Bookshelf => (14, 2),
        Sponge => (15, 2),
        Chest => (0, 7),
        MossyCobblestone => (2, 7),
        LapisOre => (4, 7),
        LapisBlock => (3, 7),
        Torch => (5, 7),
        Cactus => (8, 7),
        Clay => (9, 7),
        Farmland => (10, 7),
        Wheat => (11, 7),
        RedstoneOre => (12, 7),
        MobSpawner => (13, 7),

        // Row 4: material items
        Stick => (0, 4),
        Coal => (1, 4),
        Diamond => (2, 4),
        GoldIngot => (3, 4),
        LapisLazuli => (4, 4),
        String => (5, 4),
        Gunpowder => (6, 4),
        Leather => (7, 4),
        RedstoneDust => (8, 4),

        // Row 5: wood & stone tools
        WoodPickaxe => (0, 5),
        WoodAxe => (1, 5),
        WoodShovel => (2, 5),
        WoodSword => (3, 5),
        WoodHoe => (4, 5),
        StonePickaxe => (5, 5),
        StoneAxe => (6, 5),
        StoneShovel => (7, 5),
        StoneSword => (8, 5),
        StoneHoe => (9, 5),

        // Row 6: iron, diamond, gold tools
        IronPickaxe => (0, 6),
        IronAxe => (1, 6),
        IronShovel => (2, 6),
        IronSword => (3, 6),
        IronHoe => (4, 6),
        DiamondPickaxe => (5, 6),
        DiamondAxe => (6, 6),
        DiamondShovel => (7, 6),
        DiamondSword => (8, 6),
        DiamondHoe => (9, 6),
        GoldPickaxe => (10, 6),
        GoldAxe => (11, 6),
        GoldShovel => (12, 6),
        GoldSword => (13, 6),
        GoldHoe => (14, 6),

        // Default
        Air => (0, 0),
    }
}

/// Returns atlas (tx, ty) for the top face of a block (some blocks have different tops).
pub fn atlas_uv_top(b: BlockType) -> (u8, u8) {
    use BlockType::*;
    match b {
        Grass => (0, 0),              // green grass top
        SnowyGrass => (9, 0),         // snow top
        OakLog | SpruceLog => (5, 1), // log cross-section
        CraftingTable => (2, 2),      // grid pattern top
        Furnace => (0, 2),            // cobblestone top for furnace
        Chest => (1, 7),              // chest top
        Bookshelf => (1, 2),          // planks on top
        Farmland => (10, 7),
        TNT => (10, 1),               // stick-end bundle
        Cactus => (11, 1),            // rimmed fleshy top
        _ => atlas_uv(b),
    }
}

/// Returns atlas (tx, ty) for the side face of a block (some blocks have different sides).
#[allow(dead_code)]
pub fn atlas_uv_side(b: BlockType) -> (u8, u8) {
    use BlockType::*;
    match b {
        CraftingTable => (3, 2), // plank sides with dark stripe
        Furnace => (5, 2),       // stone sides
        Chest => (0, 7),         // chest side
        Farmland => (2, 0),      // dirt side
        _ => atlas_uv(b),
    }
}

/// Returns atlas (tx, ty) for the bottom face of a block.
#[allow(dead_code)]
pub fn atlas_uv_bottom(b: BlockType) -> (u8, u8) {
    use BlockType::*;
    match b {
        Grass | SnowyGrass => (2, 0), // dirt bottom
        OakLog | SpruceLog => (5, 1), // log cross-section
        CraftingTable => (1, 2),      // planks bottom
        Furnace | Chest => (0, 2),    // cobblestone / planks bottom
        Bookshelf => (1, 2),          // planks bottom
        Farmland => (2, 0),           // dirt bottom
        _ => atlas_uv(b),
    }
}

/// Returns the max stack size for a block/item type.
pub fn max_stack_size(b: BlockType) -> u32 {
    if b.is_tool() { 1 } else { 64 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_type_roundtrip() {
        for i in 0..BlockType::COUNT as u8 {
            let b = BlockType::from_u8(i);
            assert_eq!(b as u8, i, "BlockType::from_u8({}) roundtrip failed", i);
        }
    }

    #[test]
    fn test_out_of_range_from_u8() {
        assert_eq!(BlockType::from_u8(255), BlockType::Air);
        assert_eq!(BlockType::from_u8(BlockType::COUNT as u8), BlockType::Air);
    }

    #[test]
    fn test_tools_are_items() {
        assert!(BlockType::WoodPickaxe.is_item());
        assert!(BlockType::DiamondSword.is_item());
        assert!(BlockType::GoldHoe.is_item());
        assert!(BlockType::Stick.is_item());
        assert!(BlockType::Coal.is_item());
    }

    #[test]
    fn test_blocks_are_solid() {
        assert!(BlockType::Stone.is_solid());
        assert!(BlockType::CraftingTable.is_solid());
        assert!(BlockType::Cobblestone.is_solid());
        assert!(!BlockType::Air.is_solid());
        assert!(!BlockType::Water.is_solid());
        assert!(!BlockType::Torch.is_solid());
    }

    #[test]
    fn test_tools_not_solid() {
        assert!(!BlockType::WoodPickaxe.is_solid());
        assert!(!BlockType::IronSword.is_solid());
    }

    #[test]
    fn test_breaking_time_bedrock_infinite() {
        assert_eq!(
            breaking_time(BlockType::Bedrock, BlockType::Air),
            f32::INFINITY
        );
        assert_eq!(
            breaking_time(BlockType::Bedrock, BlockType::DiamondPickaxe),
            f32::INFINITY
        );
    }

    #[test]
    fn test_breaking_time_dirt_by_hand() {
        let t = breaking_time(BlockType::Dirt, BlockType::Air);
        assert!(
            (t - 0.75).abs() < 0.01,
            "Dirt by hand: expected 0.75, got {}",
            t
        );
    }

    #[test]
    fn test_breaking_time_dirt_with_shovel() {
        let t = breaking_time(BlockType::Dirt, BlockType::WoodShovel);
        // 0.5 * 1.5 / 2.0 = 0.375
        assert!(
            (t - 0.375).abs() < 0.01,
            "Dirt with wood shovel: expected 0.375, got {}",
            t
        );
    }

    #[test]
    fn test_breaking_time_stone_by_hand() {
        // Stone requires pickaxe: 1.5 * 5.0 / 1.0 = 7.5
        let t = breaking_time(BlockType::Stone, BlockType::Air);
        assert!(
            (t - 7.5).abs() < 0.01,
            "Stone by hand: expected 7.5, got {}",
            t
        );
    }

    #[test]
    fn test_breaking_time_stone_with_wood_pickaxe() {
        // 1.5 * 1.5 / 2.0 = 1.125
        let t = breaking_time(BlockType::Stone, BlockType::WoodPickaxe);
        assert!(
            (t - 1.125).abs() < 0.01,
            "Stone with wood pick: expected 1.125, got {}",
            t
        );
    }

    #[test]
    fn test_breaking_time_stone_with_diamond_pickaxe() {
        // 1.5 * 1.5 / 8.0 = 0.28125
        let t = breaking_time(BlockType::Stone, BlockType::DiamondPickaxe);
        assert!(
            (t - 0.28125).abs() < 0.01,
            "Stone with diamond pick: expected 0.28125, got {}",
            t
        );
    }

    #[test]
    fn test_breaking_time_obsidian_with_diamond_pickaxe() {
        // 50.0 * 1.5 / 8.0 = 9.375
        let t = breaking_time(BlockType::Obsidian, BlockType::DiamondPickaxe);
        assert!(
            (t - 9.375).abs() < 0.01,
            "Obsidian with diamond pick: expected 9.375, got {}",
            t
        );
    }

    #[test]
    fn test_breaking_time_instant_break() {
        let t = breaking_time(BlockType::Torch, BlockType::Air);
        assert!(
            (t - 0.05).abs() < 0.01,
            "Torch should be near-instant, got {}",
            t
        );
    }

    #[test]
    fn test_drop_stone_with_pickaxe() {
        let (drop, count) = get_drop(BlockType::Stone, BlockType::WoodPickaxe);
        assert_eq!(drop, BlockType::Cobblestone);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_drop_stone_without_pickaxe() {
        let (drop, _) = get_drop(BlockType::Stone, BlockType::Air);
        assert_eq!(drop, BlockType::Air); // No drop without correct tool
    }

    #[test]
    fn test_drop_iron_ore_needs_stone_tier() {
        // Wood pickaxe can't mine iron ore
        let (drop, _) = get_drop(BlockType::IronOre, BlockType::WoodPickaxe);
        assert_eq!(drop, BlockType::Air);

        // Stone pickaxe can
        let (drop, count) = get_drop(BlockType::IronOre, BlockType::StonePickaxe);
        assert_eq!(drop, BlockType::IronOre);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_drop_diamond_ore_needs_iron_tier() {
        let (drop, _) = get_drop(BlockType::DiamondOre, BlockType::StonePickaxe);
        assert_eq!(drop, BlockType::Air);

        let (drop, count) = get_drop(BlockType::DiamondOre, BlockType::IronPickaxe);
        assert_eq!(drop, BlockType::Diamond);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_drop_glass_always_nothing() {
        let (drop, _) = get_drop(BlockType::Glass, BlockType::Air);
        assert_eq!(drop, BlockType::Air);
    }

    #[test]
    fn test_drop_grass_gives_dirt() {
        let (drop, count) = get_drop(BlockType::Grass, BlockType::Air);
        assert_eq!(drop, BlockType::Dirt);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_drop_coal_ore_with_pickaxe() {
        let (drop, count) = get_drop(BlockType::CoalOre, BlockType::WoodPickaxe);
        assert_eq!(drop, BlockType::Coal);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_drop_lapis_ore() {
        let (drop, count) = get_drop(BlockType::LapisOre, BlockType::StonePickaxe);
        assert_eq!(drop, BlockType::LapisLazuli);
        assert_eq!(count, 4);
    }

    #[test]
    fn test_tool_properties_exist_for_all_tools() {
        let tools = [
            BlockType::WoodPickaxe,
            BlockType::StonePickaxe,
            BlockType::IronPickaxe,
            BlockType::DiamondPickaxe,
            BlockType::GoldPickaxe,
            BlockType::WoodAxe,
            BlockType::StoneAxe,
            BlockType::IronAxe,
            BlockType::DiamondAxe,
            BlockType::GoldAxe,
            BlockType::WoodShovel,
            BlockType::StoneShovel,
            BlockType::IronShovel,
            BlockType::DiamondShovel,
            BlockType::GoldShovel,
            BlockType::WoodSword,
            BlockType::StoneSword,
            BlockType::IronSword,
            BlockType::DiamondSword,
            BlockType::GoldSword,
            BlockType::WoodHoe,
            BlockType::StoneHoe,
            BlockType::IronHoe,
            BlockType::DiamondHoe,
            BlockType::GoldHoe,
        ];
        for t in tools {
            assert!(
                tool_properties(t).is_some(),
                "tool_properties({:?}) should return Some",
                t
            );
        }
    }

    #[test]
    fn test_tool_properties_none_for_non_tools() {
        assert!(tool_properties(BlockType::Stone).is_none());
        assert!(tool_properties(BlockType::Dirt).is_none());
        assert!(tool_properties(BlockType::Air).is_none());
    }

    #[test]
    fn test_max_stack_size_tools() {
        assert_eq!(max_stack_size(BlockType::WoodPickaxe), 1);
        assert_eq!(max_stack_size(BlockType::DiamondSword), 1);
        assert_eq!(max_stack_size(BlockType::FlintAndSteel), 1);
    }

    #[test]
    fn test_max_stack_size_blocks() {
        assert_eq!(max_stack_size(BlockType::Stone), 64);
        assert_eq!(max_stack_size(BlockType::Dirt), 64);
        assert_eq!(max_stack_size(BlockType::Coal), 64);
    }

    #[test]
    fn test_atlas_uv_all_blocks_have_mapping() {
        // Ensure no block maps to a nonsensical position
        for i in 0..BlockType::COUNT as u8 {
            let b = BlockType::from_u8(i);
            let (tx, ty) = atlas_uv(b);
            assert!(
                tx < 16 && ty < 16,
                "atlas_uv({:?}) = ({}, {}) out of range",
                b,
                tx,
                ty
            );
        }
    }

    #[test]
    fn test_gold_tool_fast_but_fragile() {
        let gp = tool_properties(BlockType::GoldPickaxe).unwrap();
        assert_eq!(gp.speed, 12.0);
        assert_eq!(gp.durability, 32);

        let dp = tool_properties(BlockType::DiamondPickaxe).unwrap();
        assert_eq!(dp.speed, 8.0);
        assert_eq!(dp.durability, 1561);

        // Gold is faster but way less durable
        assert!(gp.speed > dp.speed);
        assert!(gp.durability < dp.durability);
    }

    #[test]
    fn test_wood_axe_on_log() {
        // OakLog: hardness 2.0, preferred Axe, no tool required
        // Wood axe: speed 2.0
        // 2.0 * 1.5 / 2.0 = 1.5
        let t = breaking_time(BlockType::OakLog, BlockType::WoodAxe);
        assert!(
            (t - 1.5).abs() < 0.01,
            "OakLog with wood axe: expected 1.5, got {}",
            t
        );
    }

    #[test]
    fn test_wrong_tool_on_stone() {
        // Using axe on stone: axe is wrong tool type, stone requires pickaxe
        // hardness 1.5 * 5.0 / 1.0 = 7.5 (no speed bonus, penalty applies)
        let t = breaking_time(BlockType::Stone, BlockType::WoodAxe);
        assert!(
            (t - 7.5).abs() < 0.01,
            "Stone with wood axe: expected 7.5, got {}",
            t
        );
    }

    // ── Tool material durability values ─────────────────────────────────────

    #[test]
    fn test_wood_tool_durability() {
        let tools = [
            BlockType::WoodPickaxe,
            BlockType::WoodAxe,
            BlockType::WoodShovel,
            BlockType::WoodSword,
            BlockType::WoodHoe,
        ];
        for t in tools {
            assert_eq!(
                tool_properties(t).unwrap().durability,
                59,
                "{:?} durability should be 59",
                t
            );
        }
    }

    #[test]
    fn test_stone_tool_durability() {
        let tools = [
            BlockType::StonePickaxe,
            BlockType::StoneAxe,
            BlockType::StoneShovel,
            BlockType::StoneSword,
            BlockType::StoneHoe,
        ];
        for t in tools {
            assert_eq!(
                tool_properties(t).unwrap().durability,
                131,
                "{:?} durability should be 131",
                t
            );
        }
    }

    #[test]
    fn test_iron_tool_durability() {
        let tools = [
            BlockType::IronPickaxe,
            BlockType::IronAxe,
            BlockType::IronShovel,
            BlockType::IronSword,
            BlockType::IronHoe,
        ];
        for t in tools {
            assert_eq!(
                tool_properties(t).unwrap().durability,
                250,
                "{:?} durability should be 250",
                t
            );
        }
    }

    #[test]
    fn test_diamond_tool_durability() {
        let tools = [
            BlockType::DiamondPickaxe,
            BlockType::DiamondAxe,
            BlockType::DiamondShovel,
            BlockType::DiamondSword,
            BlockType::DiamondHoe,
        ];
        for t in tools {
            assert_eq!(
                tool_properties(t).unwrap().durability,
                1561,
                "{:?} durability should be 1561",
                t
            );
        }
    }

    #[test]
    fn test_gold_tool_durability() {
        let tools = [
            BlockType::GoldPickaxe,
            BlockType::GoldAxe,
            BlockType::GoldShovel,
            BlockType::GoldSword,
            BlockType::GoldHoe,
        ];
        for t in tools {
            assert_eq!(
                tool_properties(t).unwrap().durability,
                32,
                "{:?} durability should be 32",
                t
            );
        }
    }

    // ── Tool speed multipliers ──────────────────────────────────────────────

    #[test]
    fn test_hand_speed_multiplier() {
        // Hand (Air) has no tool properties, effective speed is 1.0
        assert!(tool_properties(BlockType::Air).is_none());
    }

    #[test]
    fn test_wood_tool_speed() {
        // Pickaxe/Axe/Shovel: 2.0, Sword: 1.5, Hoe: 1.0
        assert_eq!(tool_properties(BlockType::WoodPickaxe).unwrap().speed, 2.0);
        assert_eq!(tool_properties(BlockType::WoodAxe).unwrap().speed, 2.0);
        assert_eq!(tool_properties(BlockType::WoodShovel).unwrap().speed, 2.0);
        assert_eq!(tool_properties(BlockType::WoodSword).unwrap().speed, 1.5);
        assert_eq!(tool_properties(BlockType::WoodHoe).unwrap().speed, 1.0);
    }

    #[test]
    fn test_stone_tool_speed() {
        assert_eq!(tool_properties(BlockType::StonePickaxe).unwrap().speed, 4.0);
        assert_eq!(tool_properties(BlockType::StoneAxe).unwrap().speed, 4.0);
        assert_eq!(tool_properties(BlockType::StoneShovel).unwrap().speed, 4.0);
        assert_eq!(tool_properties(BlockType::StoneSword).unwrap().speed, 1.5);
        assert_eq!(tool_properties(BlockType::StoneHoe).unwrap().speed, 1.0);
    }

    #[test]
    fn test_iron_tool_speed() {
        assert_eq!(tool_properties(BlockType::IronPickaxe).unwrap().speed, 6.0);
        assert_eq!(tool_properties(BlockType::IronAxe).unwrap().speed, 6.0);
        assert_eq!(tool_properties(BlockType::IronShovel).unwrap().speed, 6.0);
        assert_eq!(tool_properties(BlockType::IronSword).unwrap().speed, 1.5);
        assert_eq!(tool_properties(BlockType::IronHoe).unwrap().speed, 1.0);
    }

    #[test]
    fn test_diamond_tool_speed() {
        assert_eq!(
            tool_properties(BlockType::DiamondPickaxe).unwrap().speed,
            8.0
        );
        assert_eq!(tool_properties(BlockType::DiamondAxe).unwrap().speed, 8.0);
        assert_eq!(
            tool_properties(BlockType::DiamondShovel).unwrap().speed,
            8.0
        );
        assert_eq!(tool_properties(BlockType::DiamondSword).unwrap().speed, 1.5);
        assert_eq!(tool_properties(BlockType::DiamondHoe).unwrap().speed, 1.0);
    }

    #[test]
    fn test_gold_tool_speed() {
        assert_eq!(tool_properties(BlockType::GoldPickaxe).unwrap().speed, 12.0);
        assert_eq!(tool_properties(BlockType::GoldAxe).unwrap().speed, 12.0);
        assert_eq!(tool_properties(BlockType::GoldShovel).unwrap().speed, 12.0);
        assert_eq!(tool_properties(BlockType::GoldSword).unwrap().speed, 1.5);
        assert_eq!(tool_properties(BlockType::GoldHoe).unwrap().speed, 1.0);
    }

    // ── Breaking time for every ore with correct/wrong tool tiers ───────────

    #[test]
    fn test_coal_ore_breaking_times() {
        // Coal ore: hardness 3.0, requires Wood pickaxe
        // With wood pick: 3.0 * 1.5 / 2.0 = 2.25
        let t = breaking_time(BlockType::CoalOre, BlockType::WoodPickaxe);
        assert!((t - 2.25).abs() < 0.01, "Coal ore + wood pick: {}", t);
        // With hand (wrong): 3.0 * 5.0 / 1.0 = 15.0
        let t = breaking_time(BlockType::CoalOre, BlockType::Air);
        assert!((t - 15.0).abs() < 0.01, "Coal ore + hand: {}", t);
    }

    #[test]
    fn test_iron_ore_breaking_times() {
        // Iron ore: hardness 3.0, requires Stone pickaxe
        // With stone pick: 3.0 * 1.5 / 4.0 = 1.125
        let t = breaking_time(BlockType::IronOre, BlockType::StonePickaxe);
        assert!((t - 1.125).abs() < 0.01, "Iron ore + stone pick: {}", t);
        // With wood pick (too low tier): correct tool type but can't harvest
        // 3.0 * 5.0 / 2.0 = 7.5
        let t = breaking_time(BlockType::IronOre, BlockType::WoodPickaxe);
        assert!((t - 7.5).abs() < 0.01, "Iron ore + wood pick: {}", t);
    }

    #[test]
    fn test_gold_ore_breaking_times() {
        // Gold ore: hardness 3.0, requires Iron pickaxe
        // With iron pick: 3.0 * 1.5 / 6.0 = 0.75
        let t = breaking_time(BlockType::GoldOre, BlockType::IronPickaxe);
        assert!((t - 0.75).abs() < 0.01, "Gold ore + iron pick: {}", t);
        // With stone pick (too low): 3.0 * 5.0 / 4.0 = 3.75
        let t = breaking_time(BlockType::GoldOre, BlockType::StonePickaxe);
        assert!((t - 3.75).abs() < 0.01, "Gold ore + stone pick: {}", t);
    }

    #[test]
    fn test_diamond_ore_breaking_times() {
        // Diamond ore: hardness 3.0, requires Iron pickaxe
        // With iron pick: 3.0 * 1.5 / 6.0 = 0.75
        let t = breaking_time(BlockType::DiamondOre, BlockType::IronPickaxe);
        assert!((t - 0.75).abs() < 0.01, "Diamond ore + iron pick: {}", t);
        // With diamond pick: 3.0 * 1.5 / 8.0 = 0.5625
        let t = breaking_time(BlockType::DiamondOre, BlockType::DiamondPickaxe);
        assert!(
            (t - 0.5625).abs() < 0.01,
            "Diamond ore + diamond pick: {}",
            t
        );
        // With stone pick (too low): 3.0 * 5.0 / 4.0 = 3.75
        let t = breaking_time(BlockType::DiamondOre, BlockType::StonePickaxe);
        assert!((t - 3.75).abs() < 0.01, "Diamond ore + stone pick: {}", t);
    }

    #[test]
    fn test_lapis_ore_breaking_times() {
        // Lapis ore: hardness 3.0, requires Stone pickaxe
        // With stone pick: 3.0 * 1.5 / 4.0 = 1.125
        let t = breaking_time(BlockType::LapisOre, BlockType::StonePickaxe);
        assert!((t - 1.125).abs() < 0.01, "Lapis ore + stone pick: {}", t);
        // With wood pick (too low): 3.0 * 5.0 / 2.0 = 7.5
        let t = breaking_time(BlockType::LapisOre, BlockType::WoodPickaxe);
        assert!((t - 7.5).abs() < 0.01, "Lapis ore + wood pick: {}", t);
    }

    // ── Multi-face atlas UV tests ───────────────────────────────────────────

    #[test]
    fn test_grass_has_different_top_side_bottom() {
        let top = atlas_uv_top(BlockType::Grass);
        let side = atlas_uv(BlockType::Grass); // side uses default atlas_uv
        let bottom = atlas_uv_bottom(BlockType::Grass);
        assert_ne!(top, side, "Grass top and side should differ");
        assert_ne!(top, bottom, "Grass top and bottom should differ");
        assert_ne!(side, bottom, "Grass side and bottom should differ");
    }

    #[test]
    fn test_oak_log_has_different_top_side() {
        let top = atlas_uv_top(BlockType::OakLog);
        let side = atlas_uv(BlockType::OakLog);
        assert_ne!(top, side, "OakLog top and side should differ");
    }

    #[test]
    fn test_crafting_table_has_different_top_side() {
        let top = atlas_uv_top(BlockType::CraftingTable);
        let side = atlas_uv_side(BlockType::CraftingTable);
        assert_ne!(top, side, "CraftingTable top and side should differ");
    }

    #[test]
    fn test_furnace_has_different_top_side() {
        let top = atlas_uv_top(BlockType::Furnace);
        let side = atlas_uv_side(BlockType::Furnace);
        assert_ne!(top, side, "Furnace top and side should differ");
    }

    #[test]
    fn test_grass_bottom_is_dirt_texture() {
        let grass_bottom = atlas_uv_bottom(BlockType::Grass);
        let dirt = atlas_uv(BlockType::Dirt);
        assert_eq!(grass_bottom, dirt, "Grass bottom should be dirt texture");
    }

    // ── is_transparent tests ────────────────────────────────────────────────

    #[test]
    fn test_is_transparent_glass() {
        assert!(BlockType::Glass.is_transparent());
    }

    #[test]
    fn test_is_transparent_torch() {
        assert!(BlockType::Torch.is_transparent());
    }

    #[test]
    fn test_is_transparent_oak_leaves() {
        assert!(BlockType::OakLeaves.is_transparent());
    }

    #[test]
    fn test_is_transparent_spruce_leaves() {
        assert!(BlockType::SpruceLeaves.is_transparent());
    }

    #[test]
    fn test_is_transparent_snow_layer() {
        assert!(BlockType::SnowLayer.is_transparent());
    }

    // ── is_tool for all 25 tool variants + FlintAndSteel ────────────────────

    #[test]
    fn test_is_tool_all_25_tools() {
        let all_tools = [
            BlockType::WoodPickaxe,
            BlockType::StonePickaxe,
            BlockType::IronPickaxe,
            BlockType::DiamondPickaxe,
            BlockType::GoldPickaxe,
            BlockType::WoodAxe,
            BlockType::StoneAxe,
            BlockType::IronAxe,
            BlockType::DiamondAxe,
            BlockType::GoldAxe,
            BlockType::WoodShovel,
            BlockType::StoneShovel,
            BlockType::IronShovel,
            BlockType::DiamondShovel,
            BlockType::GoldShovel,
            BlockType::WoodSword,
            BlockType::StoneSword,
            BlockType::IronSword,
            BlockType::DiamondSword,
            BlockType::GoldSword,
            BlockType::WoodHoe,
            BlockType::StoneHoe,
            BlockType::IronHoe,
            BlockType::DiamondHoe,
            BlockType::GoldHoe,
        ];
        for t in all_tools {
            assert!(t.is_tool(), "{:?} should be a tool", t);
        }
    }

    #[test]
    fn test_is_tool_flint_and_steel() {
        assert!(BlockType::FlintAndSteel.is_tool());
    }

    #[test]
    fn test_is_tool_false_for_blocks() {
        assert!(!BlockType::Stone.is_tool());
        assert!(!BlockType::Dirt.is_tool());
        assert!(!BlockType::Air.is_tool());
        assert!(!BlockType::Coal.is_tool());
        assert!(!BlockType::Diamond.is_tool());
        assert!(!BlockType::Stick.is_tool());
    }

    // ── Breaking time for Air (edge case) ───────────────────────────────────

    #[test]
    fn test_breaking_time_air() {
        // Air has hardness 0.0, should be instant (0.05)
        let t = breaking_time(BlockType::Air, BlockType::Air);
        assert!(
            (t - 0.05).abs() < 0.01,
            "Air should break instantly (0.05), got {}",
            t
        );
    }

    // ── Block properties for new blocks ─────────────────────────────────────

    #[test]
    fn test_cobblestone_properties() {
        let p = block_properties(BlockType::Cobblestone);
        assert_eq!(p.hardness, 2.0);
        assert_eq!(p.tool_type, ToolType::Pickaxe);
        assert_eq!(p.min_tier, ToolMaterial::Wood);
        assert!(p.requires_tool);
        assert_eq!(p.drop, BlockType::Cobblestone);
    }

    #[test]
    fn test_oak_planks_properties() {
        let p = block_properties(BlockType::OakPlanks);
        assert_eq!(p.hardness, 2.0);
        assert_eq!(p.tool_type, ToolType::Axe);
        assert!(!p.requires_tool);
        assert_eq!(p.drop, BlockType::OakPlanks);
    }

    #[test]
    fn test_glass_properties() {
        let p = block_properties(BlockType::Glass);
        assert_eq!(p.hardness, 0.3);
        assert_eq!(p.drop, BlockType::Air);
        assert_eq!(p.drop_count, 0);
    }

    #[test]
    fn test_obsidian_properties() {
        let p = block_properties(BlockType::Obsidian);
        assert_eq!(p.hardness, 50.0);
        assert_eq!(p.min_tier, ToolMaterial::Diamond);
        assert!(p.requires_tool);
    }

    #[test]
    fn test_sandstone_properties() {
        let p = block_properties(BlockType::Sandstone);
        assert_eq!(p.hardness, 0.8);
        assert_eq!(p.tool_type, ToolType::Pickaxe);
        assert_eq!(p.min_tier, ToolMaterial::Wood);
    }

    #[test]
    fn test_brick_properties() {
        let p = block_properties(BlockType::Brick);
        assert_eq!(p.hardness, 2.0);
        assert_eq!(p.tool_type, ToolType::Pickaxe);
        assert!(p.requires_tool);
    }

    #[test]
    fn test_stone_brick_properties() {
        let p = block_properties(BlockType::StoneBrick);
        assert_eq!(p.hardness, 1.5);
        assert_eq!(p.tool_type, ToolType::Pickaxe);
        assert_eq!(p.min_tier, ToolMaterial::Wood);
    }

    #[test]
    fn test_bookshelf_properties() {
        let p = block_properties(BlockType::Bookshelf);
        assert_eq!(p.hardness, 1.5);
        assert_eq!(p.tool_type, ToolType::Axe);
        assert!(!p.requires_tool);
    }

    #[test]
    fn test_wool_properties() {
        let p = block_properties(BlockType::Wool);
        assert_eq!(p.hardness, 0.8);
        assert!(!p.requires_tool);
    }

    #[test]
    fn test_sponge_properties() {
        let p = block_properties(BlockType::Sponge);
        assert_eq!(p.hardness, 0.6);
        assert!(!p.requires_tool);
        assert_eq!(p.drop, BlockType::Sponge);
    }

    #[test]
    fn test_iron_block_properties() {
        let p = block_properties(BlockType::IronBlock);
        assert_eq!(p.hardness, 5.0);
        assert_eq!(p.min_tier, ToolMaterial::Stone);
        assert!(p.requires_tool);
    }

    #[test]
    fn test_lapis_block_properties() {
        let p = block_properties(BlockType::LapisBlock);
        assert_eq!(p.hardness, 3.0);
        assert_eq!(p.min_tier, ToolMaterial::Stone);
        assert!(p.requires_tool);
    }

    #[test]
    fn test_mossy_cobblestone_properties() {
        let p = block_properties(BlockType::MossyCobblestone);
        assert_eq!(p.hardness, 2.0);
        assert_eq!(p.tool_type, ToolType::Pickaxe);
        assert!(p.requires_tool);
    }

    #[test]
    fn test_tnt_properties() {
        let p = block_properties(BlockType::TNT);
        assert_eq!(p.hardness, 0.0);
        assert_eq!(p.drop, BlockType::TNT);
    }

    #[test]
    fn test_crafting_table_properties() {
        let p = block_properties(BlockType::CraftingTable);
        assert_eq!(p.hardness, 2.5);
        assert_eq!(p.tool_type, ToolType::Axe);
        assert!(!p.requires_tool);
    }

    #[test]
    fn test_furnace_properties() {
        let p = block_properties(BlockType::Furnace);
        assert_eq!(p.hardness, 3.5);
        assert_eq!(p.tool_type, ToolType::Pickaxe);
        assert!(p.requires_tool);
    }

    #[test]
    fn test_chest_properties() {
        let p = block_properties(BlockType::Chest);
        assert_eq!(p.hardness, 2.5);
        assert_eq!(p.tool_type, ToolType::Axe);
        assert!(!p.requires_tool);
    }

    #[test]
    fn test_drop_gold_ore_needs_iron_tier() {
        let (drop, _) = get_drop(BlockType::GoldOre, BlockType::StonePickaxe);
        assert_eq!(drop, BlockType::Air);
        let (drop, count) = get_drop(BlockType::GoldOre, BlockType::IronPickaxe);
        assert_eq!(drop, BlockType::GoldOre);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_drop_obsidian_needs_diamond_tier() {
        let (drop, _) = get_drop(BlockType::Obsidian, BlockType::IronPickaxe);
        assert_eq!(drop, BlockType::Air);
        let (drop, count) = get_drop(BlockType::Obsidian, BlockType::DiamondPickaxe);
        assert_eq!(drop, BlockType::Obsidian);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_flint_and_steel_properties() {
        let tp = tool_properties(BlockType::FlintAndSteel).unwrap();
        assert_eq!(tp.tool_type, ToolType::None);
        assert_eq!(tp.durability, 64);
        assert_eq!(tp.speed, 1.0);
    }

    #[test]
    fn test_tool_durabilities() {
        assert_eq!(
            tool_properties(BlockType::WoodPickaxe).unwrap().durability,
            59
        );
        assert_eq!(
            tool_properties(BlockType::StoneAxe).unwrap().durability,
            131
        );
        assert_eq!(
            tool_properties(BlockType::IronShovel).unwrap().durability,
            250
        );
        assert_eq!(
            tool_properties(BlockType::DiamondSword).unwrap().durability,
            1561
        );
        assert_eq!(tool_properties(BlockType::GoldHoe).unwrap().durability, 32);
    }

    #[test]
    fn test_tool_speeds() {
        assert_eq!(tool_properties(BlockType::WoodPickaxe).unwrap().speed, 2.0);
        assert_eq!(tool_properties(BlockType::StoneAxe).unwrap().speed, 4.0);
        assert_eq!(tool_properties(BlockType::IronShovel).unwrap().speed, 6.0);
        assert_eq!(
            tool_properties(BlockType::DiamondPickaxe).unwrap().speed,
            8.0
        );
        assert_eq!(tool_properties(BlockType::GoldPickaxe).unwrap().speed, 12.0);
        // Hoes have speed 1.0 (not mining tools)
        assert_eq!(tool_properties(BlockType::DiamondHoe).unwrap().speed, 1.0);
    }

    #[test]
    fn test_atlas_uv_multiface_blocks() {
        // Grass: top != side (atlas_uv returns side face)
        assert_ne!(atlas_uv_top(BlockType::Grass), atlas_uv(BlockType::Grass));
        // OakLog: top != side
        assert_ne!(atlas_uv_top(BlockType::OakLog), atlas_uv(BlockType::OakLog));
        // CraftingTable: top != side face
        assert_ne!(
            atlas_uv_top(BlockType::CraftingTable),
            atlas_uv_side(BlockType::CraftingTable)
        );
        // Furnace: front != side
        assert_ne!(
            atlas_uv(BlockType::Furnace),
            atlas_uv_side(BlockType::Furnace)
        );
    }

    #[test]
    fn test_atlas_uv_bottom_grass_is_dirt() {
        assert_eq!(atlas_uv_bottom(BlockType::Grass), atlas_uv(BlockType::Dirt));
    }

    #[test]
    fn test_breaking_time_ores() {
        // Coal ore: pickaxe required, any tier
        let t = breaking_time(BlockType::CoalOre, BlockType::WoodPickaxe);
        assert!(t < breaking_time(BlockType::CoalOre, BlockType::Air));

        // Iron ore: stone+ pickaxe
        let t_stone = breaking_time(BlockType::IronOre, BlockType::StonePickaxe);
        let t_hand = breaking_time(BlockType::IronOre, BlockType::Air);
        assert!(t_stone < t_hand);

        // Diamond ore: iron+ pickaxe
        let t_iron = breaking_time(BlockType::DiamondOre, BlockType::IronPickaxe);
        let t_wood = breaking_time(BlockType::DiamondOre, BlockType::WoodPickaxe);
        assert!(t_iron < t_wood);

        // Gold ore: iron+ pickaxe
        let t_iron = breaking_time(BlockType::GoldOre, BlockType::IronPickaxe);
        assert!(t_iron.is_finite());
    }

    #[test]
    fn test_glass_drops_nothing() {
        let (drop, count) = get_drop(BlockType::Glass, BlockType::DiamondPickaxe);
        assert_eq!(drop, BlockType::Air);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_obsidian_breaking_time() {
        // Obsidian is very slow even with diamond pickaxe
        let t = breaking_time(BlockType::Obsidian, BlockType::DiamondPickaxe);
        assert!(t > 5.0, "Obsidian should take >5s with diamond, got {t}");
        assert!(t.is_finite());
    }

    #[test]
    fn test_world_depth_block_properties() {
        assert_eq!(
            breaking_time(BlockType::Lava, BlockType::Air),
            f32::INFINITY
        );
        assert_eq!(
            get_drop(BlockType::Farmland, BlockType::Air),
            (BlockType::Dirt, 1)
        );
        assert_eq!(
            get_drop(BlockType::Cactus, BlockType::Air),
            (BlockType::Cactus, 1)
        );
        assert_eq!(
            get_drop(BlockType::RedstoneOre, BlockType::IronPickaxe),
            (BlockType::RedstoneDust, 4)
        );
        assert_eq!(
            get_drop(BlockType::MobSpawner, BlockType::DiamondPickaxe),
            (BlockType::Air, 0)
        );
        assert_eq!(atlas_uv(BlockType::Lava), (7, 7));
        assert_eq!(atlas_uv(BlockType::RedstoneDust), (8, 4));
        assert_eq!(
            atlas_uv_side(BlockType::Farmland),
            atlas_uv(BlockType::Dirt)
        );
    }
}
