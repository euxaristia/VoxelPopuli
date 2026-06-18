use glam::Vec3;

#[derive(Clone, Copy, Debug)]
pub struct ActiveExplosive {
    pub position: Vec3,
    pub velocity: Vec3,
    pub fuse: f32, // remaining time in seconds
    pub initial_fuse: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Particle {
    pub position: Vec3,
    pub velocity: Vec3,
    pub life: f32,
    pub max_life: f32,
    pub scale: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
#[repr(u8)]
#[allow(dead_code, clippy::upper_case_acronyms)]
pub enum BlockType {
    // === Original blocks (0-21) ===
    Air = 0,
    Stone,
    Grass,
    Dirt,
    OakLog,
    OakLeaves,
    Bedrock,
    Water,
    Sand,
    Gravel,
    CoalOre,
    #[allow(dead_code)]
    PowderedSnow,
    SnowyGrass,
    SpruceLog,
    SpruceLeaves,
    SnowLayer,
    IronOre,
    RawIron,
    IronIngot,
    IronBlock,
    TNT,
    FlintAndSteel,

    // === New placeable blocks (22-40) ===
    Cobblestone,
    OakPlanks,
    CraftingTable,
    Furnace,
    Chest,
    Torch,
    GoldOre,
    DiamondOre,
    Glass,
    Bookshelf,
    MossyCobblestone,
    Obsidian,
    Sponge,
    Wool,
    LapisOre,
    LapisBlock,
    Sandstone,
    Brick,
    StoneBrick,

    // === New material items (41-48) ===
    Stick,
    Coal,
    GoldIngot,
    Diamond,
    LapisLazuli,
    String,
    Gunpowder,
    Leather,

    // === Tools: Pickaxes (49-53) ===
    WoodPickaxe,
    StonePickaxe,
    IronPickaxe,
    DiamondPickaxe,
    GoldPickaxe,

    // === Tools: Axes (54-58) ===
    WoodAxe,
    StoneAxe,
    IronAxe,
    DiamondAxe,
    GoldAxe,

    // === Tools: Shovels (59-63) ===
    WoodShovel,
    StoneShovel,
    IronShovel,
    DiamondShovel,
    GoldShovel,

    // === Tools: Swords (64-68) ===
    WoodSword,
    StoneSword,
    IronSword,
    DiamondSword,
    GoldSword,

    // === Tools: Hoes (69-73) ===
    WoodHoe,
    StoneHoe,
    IronHoe,
    DiamondHoe,
    GoldHoe,

    // === World-depth blocks/items (74-81) ===
    Lava,
    Cactus,
    Clay,
    Farmland,
    Wheat,
    RedstoneOre,
    RedstoneDust,
    MobSpawner,
}

#[allow(dead_code)]
impl BlockType {
    pub const COUNT: usize = 82;

    pub fn from_u8(value: u8) -> Self {
        if (value as usize) < Self::COUNT {
            // SAFETY: BlockType is repr(u8) with contiguous variants 0..COUNT
            unsafe { std::mem::transmute::<u8, BlockType>(value) }
        } else {
            BlockType::Air
        }
    }

    pub fn is_item(&self) -> bool {
        matches!(
            self,
            BlockType::RawIron
                | BlockType::IronIngot
                | BlockType::FlintAndSteel
                | BlockType::Stick
                | BlockType::Coal
                | BlockType::GoldIngot
                | BlockType::Diamond
                | BlockType::LapisLazuli
                | BlockType::String
                | BlockType::Gunpowder
                | BlockType::Leather
                | BlockType::Wheat
                | BlockType::RedstoneDust
                | BlockType::WoodPickaxe
                | BlockType::StonePickaxe
                | BlockType::IronPickaxe
                | BlockType::DiamondPickaxe
                | BlockType::GoldPickaxe
                | BlockType::WoodAxe
                | BlockType::StoneAxe
                | BlockType::IronAxe
                | BlockType::DiamondAxe
                | BlockType::GoldAxe
                | BlockType::WoodShovel
                | BlockType::StoneShovel
                | BlockType::IronShovel
                | BlockType::DiamondShovel
                | BlockType::GoldShovel
                | BlockType::WoodSword
                | BlockType::StoneSword
                | BlockType::IronSword
                | BlockType::DiamondSword
                | BlockType::GoldSword
                | BlockType::WoodHoe
                | BlockType::StoneHoe
                | BlockType::IronHoe
                | BlockType::DiamondHoe
                | BlockType::GoldHoe
        )
    }

    pub fn is_solid(&self) -> bool {
        match self {
            BlockType::Air
            | BlockType::Water
            | BlockType::Lava
            | BlockType::SnowLayer
            | BlockType::Torch => false,
            _ => !self.is_item(),
        }
    }

    /// Returns true if this block type is a tool with durability.
    pub fn is_tool(&self) -> bool {
        matches!(
            self,
            BlockType::WoodPickaxe
                | BlockType::StonePickaxe
                | BlockType::IronPickaxe
                | BlockType::DiamondPickaxe
                | BlockType::GoldPickaxe
                | BlockType::WoodAxe
                | BlockType::StoneAxe
                | BlockType::IronAxe
                | BlockType::DiamondAxe
                | BlockType::GoldAxe
                | BlockType::WoodShovel
                | BlockType::StoneShovel
                | BlockType::IronShovel
                | BlockType::DiamondShovel
                | BlockType::GoldShovel
                | BlockType::WoodSword
                | BlockType::StoneSword
                | BlockType::IronSword
                | BlockType::DiamondSword
                | BlockType::GoldSword
                | BlockType::WoodHoe
                | BlockType::StoneHoe
                | BlockType::IronHoe
                | BlockType::DiamondHoe
                | BlockType::GoldHoe
                | BlockType::FlintAndSteel
        )
    }

    /// Returns true if this block should be rendered in the transparent pass.
    pub fn is_transparent(&self) -> bool {
        matches!(
            self,
            BlockType::OakLeaves
                | BlockType::SpruceLeaves
                | BlockType::SnowLayer
                | BlockType::Glass
                | BlockType::Torch
                | BlockType::Wheat
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count() {
        assert_eq!(BlockType::COUNT, 82);
    }

    #[test]
    fn test_from_u8_all_valid() {
        for i in 0..BlockType::COUNT as u8 {
            let b = BlockType::from_u8(i);
            assert_eq!(b as u8, i, "roundtrip failed for {i}");
        }
    }

    #[test]
    fn test_from_u8_out_of_range() {
        assert_eq!(BlockType::from_u8(82), BlockType::Air);
        assert_eq!(BlockType::from_u8(128), BlockType::Air);
        assert_eq!(BlockType::from_u8(255), BlockType::Air);
    }

    #[test]
    fn test_air_and_water_not_solid() {
        assert!(!BlockType::Air.is_solid());
        assert!(!BlockType::Water.is_solid());
        assert!(!BlockType::Lava.is_solid());
        assert!(!BlockType::SnowLayer.is_solid());
        assert!(!BlockType::Torch.is_solid());
    }

    #[test]
    fn test_blocks_are_solid() {
        assert!(BlockType::Stone.is_solid());
        assert!(BlockType::Dirt.is_solid());
        assert!(BlockType::Cobblestone.is_solid());
        assert!(BlockType::OakPlanks.is_solid());
        assert!(BlockType::CraftingTable.is_solid());
        assert!(BlockType::Furnace.is_solid());
        assert!(BlockType::Obsidian.is_solid());
    }

    #[test]
    fn test_tools_not_solid() {
        assert!(!BlockType::WoodPickaxe.is_solid());
        assert!(!BlockType::DiamondSword.is_solid());
        assert!(!BlockType::GoldHoe.is_solid());
    }

    #[test]
    fn test_is_item_materials() {
        assert!(BlockType::Stick.is_item());
        assert!(BlockType::Coal.is_item());
        assert!(BlockType::Diamond.is_item());
        assert!(BlockType::GoldIngot.is_item());
        assert!(BlockType::IronIngot.is_item());
        assert!(BlockType::LapisLazuli.is_item());
        assert!(BlockType::String.is_item());
        assert!(BlockType::Gunpowder.is_item());
        assert!(BlockType::Leather.is_item());
        assert!(BlockType::Wheat.is_item());
        assert!(BlockType::RedstoneDust.is_item());
    }

    #[test]
    fn test_blocks_not_items() {
        assert!(!BlockType::Stone.is_item());
        assert!(!BlockType::Dirt.is_item());
        assert!(!BlockType::Air.is_item());
        assert!(!BlockType::Water.is_item());
    }

    #[test]
    fn test_all_25_tools_are_tools() {
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
            assert!(t.is_tool(), "{t:?} should be a tool");
            assert!(t.is_item(), "{t:?} should be an item");
            assert!(!t.is_solid(), "{t:?} should not be solid");
        }
    }

    #[test]
    fn test_flint_and_steel_is_tool() {
        assert!(BlockType::FlintAndSteel.is_tool());
    }

    #[test]
    fn test_transparent_blocks() {
        assert!(BlockType::OakLeaves.is_transparent());
        assert!(BlockType::SpruceLeaves.is_transparent());
        assert!(BlockType::SnowLayer.is_transparent());
        assert!(BlockType::Glass.is_transparent());
        assert!(BlockType::Torch.is_transparent());
        assert!(BlockType::Wheat.is_transparent());
    }

    #[test]
    fn test_opaque_blocks_not_transparent() {
        assert!(!BlockType::Stone.is_transparent());
        assert!(!BlockType::Dirt.is_transparent());
        assert!(!BlockType::Cobblestone.is_transparent());
        assert!(!BlockType::OakPlanks.is_transparent());
        assert!(!BlockType::Air.is_transparent());
    }
}
