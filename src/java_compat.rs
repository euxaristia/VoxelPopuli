use crate::block::BlockType;
use crate::chunk::{CHUNK_DEPTH, CHUNK_HEIGHT, CHUNK_WIDTH};

pub const TARGET_NAME: &str = "Minecraft Java pre-1.18 Anvil";
pub const MIN_Y: i32 = 0;
pub const MAX_Y: i32 = 255;
pub const SECTION_HEIGHT: usize = 16;
pub const SECTIONS_PER_CHUNK: usize = CHUNK_HEIGHT / SECTION_HEIGHT;
pub const REGION_CHUNKS: i32 = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JavaProperty {
    pub name: &'static str,
    pub value: &'static str,
}

impl JavaProperty {
    const fn new(name: &'static str, value: &'static str) -> Self {
        Self { name, value }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JavaBlockState {
    pub name: &'static str,
    pub properties: &'static [JavaProperty],
}

impl JavaBlockState {
    const fn new(name: &'static str) -> Self {
        Self {
            name,
            properties: &[],
        }
    }

    const fn with_properties(name: &'static str, properties: &'static [JavaProperty]) -> Self {
        Self { name, properties }
    }
}

const GRASS_SNOWY: &[JavaProperty] = &[JavaProperty::new("snowy", "true")];
const LOG_Y_AXIS: &[JavaProperty] = &[JavaProperty::new("axis", "y")];
const LEAVES: &[JavaProperty] = &[
    JavaProperty::new("distance", "7"),
    JavaProperty::new("persistent", "false"),
];
const SOURCE_LIQUID: &[JavaProperty] = &[JavaProperty::new("level", "0")];
const SNOW_LAYER: &[JavaProperty] = &[JavaProperty::new("layers", "1")];
const FURNACE: &[JavaProperty] = &[
    JavaProperty::new("facing", "north"),
    JavaProperty::new("lit", "false"),
];
const CHEST: &[JavaProperty] = &[
    JavaProperty::new("facing", "north"),
    JavaProperty::new("type", "single"),
    JavaProperty::new("waterlogged", "false"),
];
const FARMLAND: &[JavaProperty] = &[JavaProperty::new("moisture", "7")];
const WHEAT: &[JavaProperty] = &[JavaProperty::new("age", "7")];
const REDSTONE_ORE: &[JavaProperty] = &[JavaProperty::new("lit", "false")];

pub fn classic_world_height_matches_chunks() -> bool {
    MIN_Y == 0 && MAX_Y == 255 && CHUNK_HEIGHT == 256 && SECTIONS_PER_CHUNK == 16
}

pub fn classic_chunk_dimensions() -> (usize, usize, usize) {
    (CHUNK_WIDTH, CHUNK_HEIGHT, CHUNK_DEPTH)
}

pub fn classic_java_block_state(block: BlockType) -> Option<JavaBlockState> {
    use BlockType::*;
    Some(match block {
        Air => JavaBlockState::new("minecraft:air"),
        Stone => JavaBlockState::new("minecraft:stone"),
        Grass => JavaBlockState::new("minecraft:grass_block"),
        Dirt => JavaBlockState::new("minecraft:dirt"),
        OakLog => JavaBlockState::with_properties("minecraft:oak_log", LOG_Y_AXIS),
        OakLeaves => JavaBlockState::with_properties("minecraft:oak_leaves", LEAVES),
        Bedrock => JavaBlockState::new("minecraft:bedrock"),
        Water => JavaBlockState::with_properties("minecraft:water", SOURCE_LIQUID),
        Sand => JavaBlockState::new("minecraft:sand"),
        Gravel => JavaBlockState::new("minecraft:gravel"),
        CoalOre => JavaBlockState::new("minecraft:coal_ore"),
        PowderedSnow => JavaBlockState::new("minecraft:powder_snow"),
        SnowyGrass => JavaBlockState::with_properties("minecraft:grass_block", GRASS_SNOWY),
        SpruceLog => JavaBlockState::with_properties("minecraft:spruce_log", LOG_Y_AXIS),
        SpruceLeaves => JavaBlockState::with_properties("minecraft:spruce_leaves", LEAVES),
        SnowLayer => JavaBlockState::with_properties("minecraft:snow", SNOW_LAYER),
        IronOre => JavaBlockState::new("minecraft:iron_ore"),
        IronBlock => JavaBlockState::new("minecraft:iron_block"),
        TNT => JavaBlockState::new("minecraft:tnt"),
        Cobblestone => JavaBlockState::new("minecraft:cobblestone"),
        OakPlanks => JavaBlockState::new("minecraft:oak_planks"),
        CraftingTable => JavaBlockState::new("minecraft:crafting_table"),
        Furnace => JavaBlockState::with_properties("minecraft:furnace", FURNACE),
        Chest => JavaBlockState::with_properties("minecraft:chest", CHEST),
        Torch => JavaBlockState::new("minecraft:torch"),
        GoldOre => JavaBlockState::new("minecraft:gold_ore"),
        DiamondOre => JavaBlockState::new("minecraft:diamond_ore"),
        Glass => JavaBlockState::new("minecraft:glass"),
        Bookshelf => JavaBlockState::new("minecraft:bookshelf"),
        MossyCobblestone => JavaBlockState::new("minecraft:mossy_cobblestone"),
        Obsidian => JavaBlockState::new("minecraft:obsidian"),
        Sponge => JavaBlockState::new("minecraft:sponge"),
        Wool => JavaBlockState::new("minecraft:white_wool"),
        LapisOre => JavaBlockState::new("minecraft:lapis_ore"),
        LapisBlock => JavaBlockState::new("minecraft:lapis_block"),
        Sandstone => JavaBlockState::new("minecraft:sandstone"),
        Brick => JavaBlockState::new("minecraft:bricks"),
        StoneBrick => JavaBlockState::new("minecraft:stone_bricks"),
        Lava => JavaBlockState::with_properties("minecraft:lava", SOURCE_LIQUID),
        Cactus => JavaBlockState::new("minecraft:cactus"),
        Clay => JavaBlockState::new("minecraft:clay"),
        Farmland => JavaBlockState::with_properties("minecraft:farmland", FARMLAND),
        Wheat => JavaBlockState::with_properties("minecraft:wheat", WHEAT),
        RedstoneOre => JavaBlockState::with_properties("minecraft:redstone_ore", REDSTONE_ORE),
        MobSpawner => JavaBlockState::new("minecraft:spawner"),

        RawIron | IronIngot | FlintAndSteel | Stick | Coal | GoldIngot | Diamond | LapisLazuli
        | String | Gunpowder | Leather | RedstoneDust | WoodPickaxe | StonePickaxe
        | IronPickaxe | DiamondPickaxe | GoldPickaxe | WoodAxe | StoneAxe | IronAxe
        | DiamondAxe | GoldAxe | WoodShovel | StoneShovel | IronShovel | DiamondShovel
        | GoldShovel | WoodSword | StoneSword | IronSword | DiamondSword | GoldSword | WoodHoe
        | StoneHoe | IronHoe | DiamondHoe | GoldHoe => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_target_uses_zero_to_255_world_height() {
        assert_eq!(TARGET_NAME, "Minecraft Java pre-1.18 Anvil");
        assert_eq!((MIN_Y, MAX_Y), (0, 255));
        assert_eq!(classic_chunk_dimensions(), (16, 256, 16));
        assert!(classic_world_height_matches_chunks());
    }

    #[test]
    fn every_world_block_has_classic_java_state_mapping() {
        for id in 0..BlockType::COUNT as u8 {
            let block = BlockType::from_u8(id);
            let expected_world_block = !block.is_item() || matches!(block, BlockType::Wheat);
            assert_eq!(
                classic_java_block_state(block).is_some(),
                expected_world_block,
                "{block:?} mapping mismatch"
            );
        }
    }

    #[test]
    fn classic_mapping_uses_real_java_names_for_special_blocks() {
        assert_eq!(
            classic_java_block_state(BlockType::SnowyGrass).unwrap(),
            JavaBlockState::with_properties("minecraft:grass_block", GRASS_SNOWY)
        );
        assert_eq!(
            classic_java_block_state(BlockType::Farmland).unwrap(),
            JavaBlockState::with_properties("minecraft:farmland", FARMLAND)
        );
        assert_eq!(
            classic_java_block_state(BlockType::Wheat).unwrap(),
            JavaBlockState::with_properties("minecraft:wheat", WHEAT)
        );
        assert_eq!(
            classic_java_block_state(BlockType::RedstoneDust),
            None,
            "inventory redstone dust is not a placed block in VoxelPopuli yet"
        );
    }
}
