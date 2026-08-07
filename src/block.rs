use glam::{Vec3, Vec4};

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
    pub color: Vec4,
    pub life: f32,
    pub max_life: f32,
    pub scale: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct ArrowEntity {
    pub position: Vec3,
    pub velocity: Vec3,
    pub life: f32,
    pub in_ground: bool,
    pub damage: f32,
    pub is_critical: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct XpOrbEntity {
    pub position: Vec3,
    pub velocity: Vec3,
    pub xp_value: u32,
    pub life: f32,
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

    // === World-depth blocks/items (74-82) ===
    Lava,
    Cactus,
    Clay,
    Farmland,
    Wheat,
    RedstoneOre,
    RedstoneDust,
    MobSpawner,
    Bell,

    // === MC 1.0 Food Items (83-89) ===
    Apple,
    GoldenApple,
    RawPorkchop,
    CookedPorkchop,
    RawBeef,
    Steak,
    Bread,

    // === MC 1.0 Armor Items (90-105) ===
    LeatherHelmet,
    LeatherChestplate,
    LeatherLeggings,
    LeatherBoots,
    IronHelmet,
    IronChestplate,
    IronLeggings,
    IronBoots,
    GoldHelmet,
    GoldChestplate,
    GoldLeggings,
    GoldBoots,
    DiamondHelmet,
    DiamondChestplate,
    DiamondLeggings,
    DiamondBoots,

    // === MC 1.0 Combat & Ranged (106-107) ===
    Bow,
    Arrow,

    // === MC 1.0 Fluid Buckets, Beds & Doors (108-115) ===
    Bucket,
    WaterBucket,
    LavaBucket,
    Bed,
    OakDoor,
    IronDoor,
    Lever,
    StoneButton,

    // === MC 1.0 Redstone Components (116-119) ===
    RedstoneWire,
    RedstoneTorch,
    RedstoneLamp,
    RedstoneBlock,

    // === MC 1.0 Pistons (120-122) ===
    Piston,
    StickyPiston,
    PistonHead,
}

#[allow(dead_code)]
impl BlockType {
    pub const COUNT: usize = 123;

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
                | BlockType::Apple
                | BlockType::GoldenApple
                | BlockType::RawPorkchop
                | BlockType::CookedPorkchop
                | BlockType::RawBeef
                | BlockType::Steak
                | BlockType::Bread
                | BlockType::LeatherHelmet
                | BlockType::LeatherChestplate
                | BlockType::LeatherLeggings
                | BlockType::LeatherBoots
                | BlockType::IronHelmet
                | BlockType::IronChestplate
                | BlockType::IronLeggings
                | BlockType::IronBoots
                | BlockType::GoldHelmet
                | BlockType::GoldChestplate
                | BlockType::GoldLeggings
                | BlockType::GoldBoots
                | BlockType::DiamondHelmet
                | BlockType::DiamondChestplate
                | BlockType::DiamondLeggings
                | BlockType::DiamondBoots
                | BlockType::Bow
                | BlockType::Arrow
                | BlockType::Bucket
                | BlockType::WaterBucket
                | BlockType::LavaBucket
        )
    }

    pub fn is_solid(&self) -> bool {
        match self {
            BlockType::Air
            | BlockType::Water
            | BlockType::Lava
            | BlockType::SnowLayer
            | BlockType::Torch
            | BlockType::Bed
            | BlockType::OakDoor
            | BlockType::IronDoor
            | BlockType::Lever
            | BlockType::StoneButton
            | BlockType::RedstoneWire
            | BlockType::RedstoneTorch
            | BlockType::PistonHead
            | BlockType::Bell => false,
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
                | BlockType::Bed
                | BlockType::OakDoor
                | BlockType::IronDoor
                | BlockType::Lever
                | BlockType::StoneButton
                | BlockType::RedstoneWire
                | BlockType::RedstoneTorch
                | BlockType::PistonHead
                | BlockType::Bell
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count() {
        assert_eq!(BlockType::COUNT, 123);
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
        assert_eq!(BlockType::from_u8(BlockType::COUNT as u8), BlockType::Air);
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
    fn test_arrow_entity() {
        let arr = ArrowEntity {
            position: Vec3::new(10.0, 64.0, 10.0),
            velocity: Vec3::new(0.0, 10.0, 0.0),
            life: 60.0,
            in_ground: false,
            damage: 10.0,
            is_critical: true,
        };
        assert_eq!(arr.damage, 10.0);
        assert!(arr.is_critical);
        assert!(!arr.in_ground);
    }

    #[test]
    fn test_xp_orb_entity() {
        let orb = XpOrbEntity {
            position: Vec3::new(5.0, 64.0, 5.0),
            velocity: Vec3::new(0.0, 2.0, 0.0),
            xp_value: 5,
            life: 300.0,
        };
        assert_eq!(orb.xp_value, 5);
        assert_eq!(orb.life, 300.0);
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
