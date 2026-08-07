use crate::block::BlockType;

// ── Recipe types ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub enum RecipeShape {
    /// Shaped recipe: width x height grid, read left-to-right top-to-bottom.
    /// Air = empty cell.
    Shaped {
        width: u8,
        height: u8,
        pattern: &'static [BlockType],
    },
    /// Shapeless: any arrangement of these ingredients.
    Shapeless { ingredients: &'static [BlockType] },
}

#[derive(Clone, Debug)]
pub struct Recipe {
    pub shape: RecipeShape,
    pub output: BlockType,
    pub output_count: u8,
    pub mirror: bool, // if true, also check horizontal mirror for shaped
}

// ── MC 1.0 Recipe Database ──────────────────────────────────────────────────

use BlockType::*;

const RECIPES: &[Recipe] = &[
    // === Basic materials ===
    // Oak Log -> 4 Oak Planks (shapeless)
    Recipe {
        shape: RecipeShape::Shapeless {
            ingredients: &[OakLog],
        },
        output: OakPlanks,
        output_count: 4,
        mirror: false,
    },
    // Spruce Log -> 4 Oak Planks (in 1.0, all logs give same planks)
    Recipe {
        shape: RecipeShape::Shapeless {
            ingredients: &[SpruceLog],
        },
        output: OakPlanks,
        output_count: 4,
        mirror: false,
    },
    // 2 Planks vertical -> 4 Sticks
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 2,
            pattern: &[OakPlanks, OakPlanks],
        },
        output: Stick,
        output_count: 4,
        mirror: false,
    },
    // 3 Iron Ingots V-shape -> Bucket
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 2,
            pattern: &[IronIngot, Air, IronIngot, Air, IronIngot, Air],
        },
        output: Bucket,
        output_count: 1,
        mirror: false,
    },
    // 3 Wool top row, 3 OakPlanks bottom row -> Bed
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 2,
            pattern: &[Wool, Wool, Wool, OakPlanks, OakPlanks, OakPlanks],
        },
        output: Bed,
        output_count: 1,
        mirror: false,
    },
    // 6 OakPlanks 2x3 -> OakDoor
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 3,
            pattern: &[
                OakPlanks, OakPlanks, OakPlanks, OakPlanks, OakPlanks, OakPlanks,
            ],
        },
        output: OakDoor,
        output_count: 1,
        mirror: false,
    },
    // 6 IronIngots 2x3 -> IronDoor
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 3,
            pattern: &[
                IronIngot, IronIngot, IronIngot, IronIngot, IronIngot, IronIngot,
            ],
        },
        output: IronDoor,
        output_count: 1,
        mirror: false,
    },
    // 1 Stick over 1 Cobblestone -> Lever
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 2,
            pattern: &[Stick, Cobblestone],
        },
        output: Lever,
        output_count: 1,
        mirror: false,
    },
    // 1 Stone -> StoneButton
    Recipe {
        shape: RecipeShape::Shapeless {
            ingredients: &[Stone],
        },
        output: StoneButton,
        output_count: 1,
        mirror: false,
    },
    // 4 Planks 2x2 -> Crafting Table
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 2,
            pattern: &[OakPlanks, OakPlanks, OakPlanks, OakPlanks],
        },
        output: CraftingTable,
        output_count: 1,
        mirror: false,
    },
    // === 3x3 recipes (require crafting table) ===
    // Furnace: 8 cobblestone ring
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                Cobblestone,
                Cobblestone,
                Cobblestone,
                Cobblestone,
                Air,
                Cobblestone,
                Cobblestone,
                Cobblestone,
                Cobblestone,
            ],
        },
        output: Furnace,
        output_count: 1,
        mirror: false,
    },
    // Chest: 8 planks ring
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                OakPlanks, OakPlanks, OakPlanks, OakPlanks, Air, OakPlanks, OakPlanks, OakPlanks,
                OakPlanks,
            ],
        },
        output: Chest,
        output_count: 1,
        mirror: false,
    },
    // === Torches ===
    // Coal + Stick vertical -> 4 Torches
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 2,
            pattern: &[Coal, Stick],
        },
        output: Torch,
        output_count: 4,
        mirror: false,
    },
    // === Wooden tools ===
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                OakPlanks, OakPlanks, OakPlanks, Air, Stick, Air, Air, Stick, Air,
            ],
        },
        output: WoodPickaxe,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 3,
            pattern: &[OakPlanks, OakPlanks, OakPlanks, Stick, Air, Stick],
        },
        output: WoodAxe,
        output_count: 1,
        mirror: true,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 3,
            pattern: &[OakPlanks, Stick, Stick],
        },
        output: WoodShovel,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 3,
            pattern: &[OakPlanks, OakPlanks, Stick],
        },
        output: WoodSword,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 3,
            pattern: &[OakPlanks, OakPlanks, Air, Stick, Air, Stick],
        },
        output: WoodHoe,
        output_count: 1,
        mirror: true,
    },
    // === Stone tools ===
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                Cobblestone,
                Cobblestone,
                Cobblestone,
                Air,
                Stick,
                Air,
                Air,
                Stick,
                Air,
            ],
        },
        output: StonePickaxe,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 3,
            pattern: &[Cobblestone, Cobblestone, Cobblestone, Stick, Air, Stick],
        },
        output: StoneAxe,
        output_count: 1,
        mirror: true,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 3,
            pattern: &[Cobblestone, Stick, Stick],
        },
        output: StoneShovel,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 3,
            pattern: &[Cobblestone, Cobblestone, Stick],
        },
        output: StoneSword,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 3,
            pattern: &[Cobblestone, Cobblestone, Air, Stick, Air, Stick],
        },
        output: StoneHoe,
        output_count: 1,
        mirror: true,
    },
    // === Iron tools ===
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                IronIngot, IronIngot, IronIngot, Air, Stick, Air, Air, Stick, Air,
            ],
        },
        output: IronPickaxe,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 3,
            pattern: &[IronIngot, IronIngot, IronIngot, Stick, Air, Stick],
        },
        output: IronAxe,
        output_count: 1,
        mirror: true,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 3,
            pattern: &[IronIngot, Stick, Stick],
        },
        output: IronShovel,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 3,
            pattern: &[IronIngot, IronIngot, Stick],
        },
        output: IronSword,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 3,
            pattern: &[IronIngot, IronIngot, Air, Stick, Air, Stick],
        },
        output: IronHoe,
        output_count: 1,
        mirror: true,
    },
    // === Diamond tools ===
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[Diamond, Diamond, Diamond, Air, Stick, Air, Air, Stick, Air],
        },
        output: DiamondPickaxe,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 3,
            pattern: &[Diamond, Diamond, Diamond, Stick, Air, Stick],
        },
        output: DiamondAxe,
        output_count: 1,
        mirror: true,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 3,
            pattern: &[Diamond, Stick, Stick],
        },
        output: DiamondShovel,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 3,
            pattern: &[Diamond, Diamond, Stick],
        },
        output: DiamondSword,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 3,
            pattern: &[Diamond, Diamond, Air, Stick, Air, Stick],
        },
        output: DiamondHoe,
        output_count: 1,
        mirror: true,
    },
    // === Gold tools ===
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                GoldIngot, GoldIngot, GoldIngot, Air, Stick, Air, Air, Stick, Air,
            ],
        },
        output: GoldPickaxe,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 3,
            pattern: &[GoldIngot, GoldIngot, GoldIngot, Stick, Air, Stick],
        },
        output: GoldAxe,
        output_count: 1,
        mirror: true,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 3,
            pattern: &[GoldIngot, Stick, Stick],
        },
        output: GoldShovel,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 3,
            pattern: &[GoldIngot, GoldIngot, Stick],
        },
        output: GoldSword,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 3,
            pattern: &[GoldIngot, GoldIngot, Air, Stick, Air, Stick],
        },
        output: GoldHoe,
        output_count: 1,
        mirror: true,
    },
    // === Block conversions ===
    // Iron Block from 9 ingots
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                IronIngot, IronIngot, IronIngot, IronIngot, IronIngot, IronIngot, IronIngot,
                IronIngot, IronIngot,
            ],
        },
        output: IronBlock,
        output_count: 1,
        mirror: false,
    },
    // Iron Block -> 9 Iron Ingots (shapeless)
    Recipe {
        shape: RecipeShape::Shapeless {
            ingredients: &[IronBlock],
        },
        output: IronIngot,
        output_count: 9,
        mirror: false,
    },
    // Lapis Block from 9 lapis
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                LapisLazuli,
                LapisLazuli,
                LapisLazuli,
                LapisLazuli,
                LapisLazuli,
                LapisLazuli,
                LapisLazuli,
                LapisLazuli,
                LapisLazuli,
            ],
        },
        output: LapisBlock,
        output_count: 1,
        mirror: false,
    },
    // Lapis Block -> 9 Lapis (shapeless)
    Recipe {
        shape: RecipeShape::Shapeless {
            ingredients: &[LapisBlock],
        },
        output: LapisLazuli,
        output_count: 9,
        mirror: false,
    },
    // Gold Ore block from 9 Gold Ingots
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                GoldIngot, GoldIngot, GoldIngot, GoldIngot, GoldIngot, GoldIngot, GoldIngot,
                GoldIngot, GoldIngot,
            ],
        },
        output: GoldOre,
        output_count: 1,
        mirror: false,
    },
    // Gold Ore -> 9 Gold Ingots
    Recipe {
        shape: RecipeShape::Shapeless {
            ingredients: &[GoldOre],
        },
        output: GoldIngot,
        output_count: 9,
        mirror: false,
    },
    // Diamond Ore block from 9 Diamonds
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                Diamond, Diamond, Diamond, Diamond, Diamond, Diamond, Diamond, Diamond, Diamond,
            ],
        },
        output: DiamondOre,
        output_count: 1,
        mirror: false,
    },
    // Diamond Ore -> 9 Diamonds
    Recipe {
        shape: RecipeShape::Shapeless {
            ingredients: &[DiamondOre],
        },
        output: Diamond,
        output_count: 9,
        mirror: false,
    },
    // Coal Ore block from 9 Coal
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[Coal, Coal, Coal, Coal, Coal, Coal, Coal, Coal, Coal],
        },
        output: CoalOre,
        output_count: 1,
        mirror: false,
    },
    // Coal Ore -> 9 Coal
    Recipe {
        shape: RecipeShape::Shapeless {
            ingredients: &[CoalOre],
        },
        output: Coal,
        output_count: 9,
        mirror: false,
    },
    // Redstone Ore block from 9 Redstone Dust
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                RedstoneDust,
                RedstoneDust,
                RedstoneDust,
                RedstoneDust,
                RedstoneDust,
                RedstoneDust,
                RedstoneDust,
                RedstoneDust,
                RedstoneDust,
            ],
        },
        output: RedstoneOre,
        output_count: 1,
        mirror: false,
    },
    // Redstone Ore -> 9 Redstone Dust
    Recipe {
        shape: RecipeShape::Shapeless {
            ingredients: &[RedstoneOre],
        },
        output: RedstoneDust,
        output_count: 9,
        mirror: false,
    },
    // Sandstone from 4 sand
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 2,
            pattern: &[Sand, Sand, Sand, Sand],
        },
        output: Sandstone,
        output_count: 1,
        mirror: false,
    },
    // Stone Brick from 4 stone
    Recipe {
        shape: RecipeShape::Shaped {
            width: 2,
            height: 2,
            pattern: &[Stone, Stone, Stone, Stone],
        },
        output: StoneBrick,
        output_count: 4,
        mirror: false,
    },
    // Brick block from 4 bricks (we don't have clay brick items, so skip for now)

    // === Flint & Steel ===
    Recipe {
        shape: RecipeShape::Shapeless {
            ingredients: &[IronIngot, Gravel],
        },
        output: FlintAndSteel,
        output_count: 1,
        mirror: false,
    },
    // === Mossy Cobblestone ===
    Recipe {
        shape: RecipeShape::Shapeless {
            ingredients: &[Cobblestone, OakLeaves],
        },
        output: MossyCobblestone,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shapeless {
            ingredients: &[Cobblestone, SpruceLeaves],
        },
        output: MossyCobblestone,
        output_count: 1,
        mirror: false,
    },
    // === TNT ===
    // TNT: X pattern of gunpowder and sand
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                Gunpowder, Sand, Gunpowder, Sand, Gunpowder, Sand, Gunpowder, Sand, Gunpowder,
            ],
        },
        output: TNT,
        output_count: 1,
        mirror: false,
    },
    // === Bookshelf ===
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                OakPlanks, OakPlanks, OakPlanks, OakPlanks, OakPlanks,
                OakPlanks, // Should be books, but we don't have paper/books yet
                OakPlanks, OakPlanks, OakPlanks,
            ],
        },
        output: Bookshelf,
        output_count: 1,
        mirror: false,
    },
    // === MC 1.0 Food & Ranged Recipes ===
    // Bread: 3 Wheat in a row
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 1,
            pattern: &[Wheat, Wheat, Wheat],
        },
        output: Bread,
        output_count: 1,
        mirror: false,
    },
    // Golden Apple: Apple surrounded by 8 Gold Ingots
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                GoldIngot, GoldIngot, GoldIngot, GoldIngot, Apple, GoldIngot, GoldIngot, GoldIngot,
                GoldIngot,
            ],
        },
        output: GoldenApple,
        output_count: 1,
        mirror: false,
    },
    // Bow: 3 Sticks & 3 Strings
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[Stick, String, Air, Stick, Air, String, Stick, String, Air],
        },
        output: Bow,
        output_count: 1,
        mirror: true,
    },
    // Arrow: Stick & Stick
    Recipe {
        shape: RecipeShape::Shaped {
            width: 1,
            height: 2,
            pattern: &[Stick, Stick],
        },
        output: Arrow,
        output_count: 4,
        mirror: false,
    },
    // === MC 1.0 Armor Recipes ===
    // Helmets (5 materials: top 2 rows)
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 2,
            pattern: &[Leather, Leather, Leather, Leather, Air, Leather],
        },
        output: LeatherHelmet,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 2,
            pattern: &[IronIngot, IronIngot, IronIngot, IronIngot, Air, IronIngot],
        },
        output: IronHelmet,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 2,
            pattern: &[GoldIngot, GoldIngot, GoldIngot, GoldIngot, Air, GoldIngot],
        },
        output: GoldHelmet,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 2,
            pattern: &[Diamond, Diamond, Diamond, Diamond, Air, Diamond],
        },
        output: DiamondHelmet,
        output_count: 1,
        mirror: false,
    },
    // Chestplates (8 materials)
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                Leather, Air, Leather, Leather, Leather, Leather, Leather, Leather, Leather,
            ],
        },
        output: LeatherChestplate,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                IronIngot, Air, IronIngot, IronIngot, IronIngot, IronIngot, IronIngot, IronIngot,
                IronIngot,
            ],
        },
        output: IronChestplate,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                GoldIngot, Air, GoldIngot, GoldIngot, GoldIngot, GoldIngot, GoldIngot, GoldIngot,
                GoldIngot,
            ],
        },
        output: GoldChestplate,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                Diamond, Air, Diamond, Diamond, Diamond, Diamond, Diamond, Diamond, Diamond,
            ],
        },
        output: DiamondChestplate,
        output_count: 1,
        mirror: false,
    },
    // Leggings (7 materials)
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                Leather, Leather, Leather, Leather, Air, Leather, Leather, Air, Leather,
            ],
        },
        output: LeatherLeggings,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                IronIngot, IronIngot, IronIngot, IronIngot, Air, IronIngot, IronIngot, Air,
                IronIngot,
            ],
        },
        output: IronLeggings,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                GoldIngot, GoldIngot, GoldIngot, GoldIngot, Air, GoldIngot, GoldIngot, Air,
                GoldIngot,
            ],
        },
        output: GoldLeggings,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 3,
            pattern: &[
                Diamond, Diamond, Diamond, Diamond, Air, Diamond, Diamond, Air, Diamond,
            ],
        },
        output: DiamondLeggings,
        output_count: 1,
        mirror: false,
    },
    // Boots (4 materials)
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 2,
            pattern: &[Leather, Air, Leather, Leather, Air, Leather],
        },
        output: LeatherBoots,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 2,
            pattern: &[IronIngot, Air, IronIngot, IronIngot, Air, IronIngot],
        },
        output: IronBoots,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 2,
            pattern: &[GoldIngot, Air, GoldIngot, GoldIngot, Air, GoldIngot],
        },
        output: GoldBoots,
        output_count: 1,
        mirror: false,
    },
    Recipe {
        shape: RecipeShape::Shaped {
            width: 3,
            height: 2,
            pattern: &[Diamond, Air, Diamond, Diamond, Air, Diamond],
        },
        output: DiamondBoots,
        output_count: 1,
        mirror: false,
    },
];

// ── Furnace Smelting & Fuel Database ─────────────────────────────────────────

/// Returns the smelted output item and count for a given raw block or item.
pub fn smelt_item(input: BlockType) -> Option<(BlockType, u8)> {
    match input {
        RawIron | IronOre => Some((IronIngot, 1)),
        GoldOre => Some((GoldIngot, 1)),
        Cobblestone => Some((Stone, 1)),
        Sand => Some((Glass, 1)),
        Clay => Some((Brick, 1)),
        OakLog | SpruceLog => Some((Coal, 1)),
        Cactus => Some((Sand, 1)),
        _ => None,
    }
}

/// Returns true if the block/item can be burned as fuel in a furnace.
pub fn is_furnace_fuel(fuel: BlockType) -> bool {
    fuel_burn_time(fuel) > 0.0
}

/// Returns the burn duration in seconds for a fuel item.
pub fn fuel_burn_time(fuel: BlockType) -> f32 {
    match fuel {
        Coal => 80.0,
        OakLog | SpruceLog | OakPlanks | CraftingTable | Chest | Bookshelf => 15.0,
        Stick => 5.0,
        WoodPickaxe | WoodAxe | WoodShovel | WoodSword | WoodHoe => 10.0,
        _ => 0.0,
    }
}

// ── Recipe matching engine ──────────────────────────────────────────────────

/// Find a matching recipe for the given grid.
/// `grid` is a flat array of Option<BlockType>, `grid_width` x `grid_height`.
/// Returns (output_block, output_count) if a recipe matches.
pub fn find_recipe(
    grid: &[Option<BlockType>],
    grid_width: u8,
    grid_height: u8,
) -> Option<(BlockType, u8)> {
    // Extract bounding box of non-empty cells
    let (min_x, min_y, max_x, max_y) = bounding_box(grid, grid_width, grid_height);
    if min_x > max_x {
        return None; // Empty grid
    }

    let content_w = max_x - min_x + 1;
    let content_h = max_y - min_y + 1;

    for recipe in RECIPES {
        match &recipe.shape {
            RecipeShape::Shaped {
                width,
                height,
                pattern,
            } => {
                let rw = *width as usize;
                let rh = *height as usize;

                // Recipe must fit in the content bounding box
                if content_w != rw || content_h != rh {
                    continue;
                }

                // Try normal orientation
                if matches_shaped(grid, grid_width as usize, min_x, min_y, rw, rh, pattern) {
                    return Some((recipe.output, recipe.output_count));
                }

                // Try mirrored horizontally
                if recipe.mirror {
                    let mirrored = mirror_pattern(pattern, rw, rh);
                    if matches_shaped(grid, grid_width as usize, min_x, min_y, rw, rh, &mirrored) {
                        return Some((recipe.output, recipe.output_count));
                    }
                }
            }
            RecipeShape::Shapeless { ingredients } => {
                if matches_shapeless(grid, ingredients) {
                    return Some((recipe.output, recipe.output_count));
                }
            }
        }
    }
    None
}

/// Returns (min_x, min_y, max_x, max_y) of non-empty cells.
/// If grid is empty, returns (usize::MAX, usize::MAX, 0, 0).
fn bounding_box(grid: &[Option<BlockType>], width: u8, height: u8) -> (usize, usize, usize, usize) {
    let w = width as usize;
    let h = height as usize;
    let mut min_x = usize::MAX;
    let mut min_y = usize::MAX;
    let mut max_x = 0usize;
    let mut max_y = 0usize;

    for y in 0..h {
        for x in 0..w {
            if grid[y * w + x].is_some() {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }
    }
    (min_x, min_y, max_x, max_y)
}

/// Check if a shaped pattern matches at the given offset in the grid.
fn matches_shaped(
    grid: &[Option<BlockType>],
    grid_w: usize,
    off_x: usize,
    off_y: usize,
    rw: usize,
    rh: usize,
    pattern: &[BlockType],
) -> bool {
    for ry in 0..rh {
        for rx in 0..rw {
            let grid_idx = (off_y + ry) * grid_w + (off_x + rx);
            let recipe_block = pattern[ry * rw + rx];
            let grid_block = grid[grid_idx];

            if recipe_block == BlockType::Air {
                if grid_block.is_some() {
                    return false;
                }
            } else {
                match grid_block {
                    Some(gb) if gb == recipe_block => {}
                    _ => return false,
                }
            }
        }
    }
    true
}

/// Mirror a shaped pattern horizontally.
fn mirror_pattern(pattern: &[BlockType], w: usize, h: usize) -> Vec<BlockType> {
    let mut mirrored = vec![BlockType::Air; w * h];
    for y in 0..h {
        for x in 0..w {
            mirrored[y * w + x] = pattern[y * w + (w - 1 - x)];
        }
    }
    mirrored
}

/// Check if grid contents match a shapeless recipe (multiset comparison).
fn matches_shapeless(grid: &[Option<BlockType>], ingredients: &[BlockType]) -> bool {
    let grid_items: Vec<BlockType> = grid.iter().filter_map(|s| *s).collect();
    if grid_items.len() != ingredients.len() {
        return false;
    }
    let mut remaining: Vec<BlockType> = ingredients.to_vec();
    for item in &grid_items {
        if let Some(pos) = remaining.iter().position(|&r| r == *item) {
            remaining.remove(pos);
        } else {
            return false;
        }
    }
    remaining.is_empty()
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn grid_2x2(
        a: Option<BlockType>,
        b: Option<BlockType>,
        c: Option<BlockType>,
        d: Option<BlockType>,
    ) -> Vec<Option<BlockType>> {
        vec![a, b, c, d]
    }

    fn grid_3x3(items: [Option<BlockType>; 9]) -> Vec<Option<BlockType>> {
        items.to_vec()
    }

    fn s(b: BlockType) -> Option<BlockType> {
        Some(b)
    }
    fn n() -> Option<BlockType> {
        None
    }

    #[test]
    fn test_log_to_planks() {
        let grid = grid_2x2(s(OakLog), n(), n(), n());
        let result = find_recipe(&grid, 2, 2);
        assert_eq!(result, Some((OakPlanks, 4)));
    }

    #[test]
    fn test_log_to_planks_any_position() {
        // Shapeless: log can be anywhere
        let grid = grid_2x2(n(), n(), n(), s(OakLog));
        let result = find_recipe(&grid, 2, 2);
        assert_eq!(result, Some((OakPlanks, 4)));
    }

    #[test]
    fn test_spruce_log_to_planks() {
        let grid = grid_2x2(s(SpruceLog), n(), n(), n());
        let result = find_recipe(&grid, 2, 2);
        assert_eq!(result, Some((OakPlanks, 4)));
    }

    #[test]
    fn test_sticks_recipe() {
        // 2 planks vertical (1x2): should work in any column of 2x2
        let grid = grid_2x2(s(OakPlanks), n(), s(OakPlanks), n());
        let result = find_recipe(&grid, 2, 2);
        assert_eq!(result, Some((Stick, 4)));
    }

    #[test]
    fn test_sticks_recipe_right_column() {
        let grid = grid_2x2(n(), s(OakPlanks), n(), s(OakPlanks));
        let result = find_recipe(&grid, 2, 2);
        assert_eq!(result, Some((Stick, 4)));
    }

    #[test]
    fn test_crafting_table_recipe() {
        let grid = grid_2x2(s(OakPlanks), s(OakPlanks), s(OakPlanks), s(OakPlanks));
        let result = find_recipe(&grid, 2, 2);
        assert_eq!(result, Some((CraftingTable, 1)));
    }

    #[test]
    fn test_empty_grid_no_recipe() {
        let grid = grid_2x2(n(), n(), n(), n());
        assert_eq!(find_recipe(&grid, 2, 2), None);
    }

    #[test]
    fn test_wrong_ingredients_no_recipe() {
        let grid = grid_2x2(s(Stone), s(Stone), s(Stone), s(Stone));
        // 4 stone in 2x2 = stone brick
        let result = find_recipe(&grid, 2, 2);
        assert_eq!(result, Some((StoneBrick, 4)));
    }

    #[test]
    fn test_furnace_3x3() {
        let grid = grid_3x3([
            s(Cobblestone),
            s(Cobblestone),
            s(Cobblestone),
            s(Cobblestone),
            n(),
            s(Cobblestone),
            s(Cobblestone),
            s(Cobblestone),
            s(Cobblestone),
        ]);
        let result = find_recipe(&grid, 3, 3);
        assert_eq!(result, Some((Furnace, 1)));
    }

    #[test]
    fn test_chest_3x3() {
        let grid = grid_3x3([
            s(OakPlanks),
            s(OakPlanks),
            s(OakPlanks),
            s(OakPlanks),
            n(),
            s(OakPlanks),
            s(OakPlanks),
            s(OakPlanks),
            s(OakPlanks),
        ]);
        let result = find_recipe(&grid, 3, 3);
        assert_eq!(result, Some((Chest, 1)));
    }

    #[test]
    fn test_wood_pickaxe_3x3() {
        let grid = grid_3x3([
            s(OakPlanks),
            s(OakPlanks),
            s(OakPlanks),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        let result = find_recipe(&grid, 3, 3);
        assert_eq!(result, Some((WoodPickaxe, 1)));
    }

    #[test]
    fn test_wood_axe_mirrored() {
        // Normal: planks on left
        let grid_normal = grid_3x3([
            s(OakPlanks),
            s(OakPlanks),
            n(),
            s(OakPlanks),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid_normal, 3, 3), Some((WoodAxe, 1)));

        // Mirrored: planks on right
        let grid_mirror = grid_3x3([
            n(),
            s(OakPlanks),
            s(OakPlanks),
            n(),
            s(Stick),
            s(OakPlanks),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid_mirror, 3, 3), Some((WoodAxe, 1)));
    }

    #[test]
    fn test_torch_recipe_3x3() {
        // Torch is 1x2 (coal on top, stick below), should work in 3x3
        let grid = grid_3x3([n(), s(Coal), n(), n(), s(Stick), n(), n(), n(), n()]);
        let result = find_recipe(&grid, 3, 3);
        assert_eq!(result, Some((Torch, 4)));
    }

    #[test]
    fn test_torch_left_position() {
        let grid = grid_3x3([s(Coal), n(), n(), s(Stick), n(), n(), n(), n(), n()]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((Torch, 4)));
    }

    #[test]
    fn test_diamond_pickaxe() {
        let grid = grid_3x3([
            s(Diamond),
            s(Diamond),
            s(Diamond),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((DiamondPickaxe, 1)));
    }

    #[test]
    fn test_tnt_recipe() {
        let grid = grid_3x3([
            s(Gunpowder),
            s(Sand),
            s(Gunpowder),
            s(Sand),
            s(Gunpowder),
            s(Sand),
            s(Gunpowder),
            s(Sand),
            s(Gunpowder),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((TNT, 1)));
    }

    #[test]
    fn test_sandstone_2x2() {
        let grid = grid_2x2(s(Sand), s(Sand), s(Sand), s(Sand));
        assert_eq!(find_recipe(&grid, 2, 2), Some((Sandstone, 1)));
    }

    #[test]
    fn test_iron_block_decompose() {
        // Shapeless: 1 iron block -> 9 ingots
        let grid = grid_2x2(s(IronBlock), n(), n(), n());
        assert_eq!(find_recipe(&grid, 2, 2), Some((IronIngot, 9)));
    }

    #[test]
    fn test_wood_sword() {
        let grid = grid_3x3([
            n(),
            s(OakPlanks),
            n(),
            n(),
            s(OakPlanks),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((WoodSword, 1)));
    }

    #[test]
    fn test_wood_shovel() {
        let grid = grid_3x3([
            n(),
            s(OakPlanks),
            n(),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((WoodShovel, 1)));
    }

    #[test]
    fn test_hoe_mirrored() {
        // Normal
        let grid = grid_3x3([
            s(IronIngot),
            s(IronIngot),
            n(),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((IronHoe, 1)));

        // Mirrored
        let grid = grid_3x3([
            n(),
            s(IronIngot),
            s(IronIngot),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((IronHoe, 1)));
    }

    #[test]
    fn test_partial_recipe_fails() {
        // Only 2 cobblestone, not enough for anything
        let grid = grid_3x3([
            s(Cobblestone),
            s(Cobblestone),
            n(),
            n(),
            n(),
            n(),
            n(),
            n(),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), None);
    }

    #[test]
    fn test_bounding_box_empty() {
        let grid = vec![None; 9];
        let (min_x, _min_y, max_x, _max_y) = bounding_box(&grid, 3, 3);
        assert!(min_x > max_x); // empty
    }

    #[test]
    fn test_bounding_box_single() {
        let mut grid = vec![None; 9];
        grid[4] = Some(Stone); // center of 3x3
        let (min_x, min_y, max_x, max_y) = bounding_box(&grid, 3, 3);
        assert_eq!((min_x, min_y, max_x, max_y), (1, 1, 1, 1));
    }

    #[test]
    fn test_mirror_pattern() {
        let pattern = vec![
            OakPlanks, OakPlanks, Air, OakPlanks, Stick, Air, Air, Stick, Air,
        ];
        let mirrored = mirror_pattern(&pattern, 3, 3);
        assert_eq!(
            mirrored,
            vec![
                Air, OakPlanks, OakPlanks, Air, Stick, OakPlanks, Air, Stick, Air
            ]
        );
    }

    // ── All 25 tool recipes exist ───────────────────────────────────────────

    #[test]
    fn test_all_wood_tool_recipes_exist() {
        // WoodPickaxe
        let grid = grid_3x3([
            s(OakPlanks),
            s(OakPlanks),
            s(OakPlanks),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((WoodPickaxe, 1)));

        // WoodAxe (normal orientation)
        let grid = grid_3x3([
            s(OakPlanks),
            s(OakPlanks),
            n(),
            s(OakPlanks),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((WoodAxe, 1)));

        // WoodShovel
        let grid = grid_3x3([
            n(),
            s(OakPlanks),
            n(),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((WoodShovel, 1)));

        // WoodSword
        let grid = grid_3x3([
            n(),
            s(OakPlanks),
            n(),
            n(),
            s(OakPlanks),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((WoodSword, 1)));

        // WoodHoe (normal orientation)
        let grid = grid_3x3([
            s(OakPlanks),
            s(OakPlanks),
            n(),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((WoodHoe, 1)));
    }

    #[test]
    fn test_all_stone_tool_recipes_exist() {
        let grid = grid_3x3([
            s(Cobblestone),
            s(Cobblestone),
            s(Cobblestone),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((StonePickaxe, 1)));

        let grid = grid_3x3([
            s(Cobblestone),
            s(Cobblestone),
            n(),
            s(Cobblestone),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((StoneAxe, 1)));

        let grid = grid_3x3([
            n(),
            s(Cobblestone),
            n(),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((StoneShovel, 1)));

        let grid = grid_3x3([
            n(),
            s(Cobblestone),
            n(),
            n(),
            s(Cobblestone),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((StoneSword, 1)));

        let grid = grid_3x3([
            s(Cobblestone),
            s(Cobblestone),
            n(),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((StoneHoe, 1)));
    }

    #[test]
    fn test_all_iron_tool_recipes_exist() {
        let grid = grid_3x3([
            s(IronIngot),
            s(IronIngot),
            s(IronIngot),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((IronPickaxe, 1)));

        let grid = grid_3x3([
            s(IronIngot),
            s(IronIngot),
            n(),
            s(IronIngot),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((IronAxe, 1)));

        let grid = grid_3x3([
            n(),
            s(IronIngot),
            n(),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((IronShovel, 1)));

        let grid = grid_3x3([
            n(),
            s(IronIngot),
            n(),
            n(),
            s(IronIngot),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((IronSword, 1)));

        let grid = grid_3x3([
            s(IronIngot),
            s(IronIngot),
            n(),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((IronHoe, 1)));
    }

    #[test]
    fn test_all_diamond_tool_recipes_exist() {
        let grid = grid_3x3([
            s(Diamond),
            s(Diamond),
            s(Diamond),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((DiamondPickaxe, 1)));

        let grid = grid_3x3([
            s(Diamond),
            s(Diamond),
            n(),
            s(Diamond),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((DiamondAxe, 1)));

        let grid = grid_3x3([n(), s(Diamond), n(), n(), s(Stick), n(), n(), s(Stick), n()]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((DiamondShovel, 1)));

        let grid = grid_3x3([
            n(),
            s(Diamond),
            n(),
            n(),
            s(Diamond),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((DiamondSword, 1)));

        let grid = grid_3x3([
            s(Diamond),
            s(Diamond),
            n(),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((DiamondHoe, 1)));
    }

    #[test]
    fn test_all_gold_tool_recipes_exist() {
        let grid = grid_3x3([
            s(GoldIngot),
            s(GoldIngot),
            s(GoldIngot),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((GoldPickaxe, 1)));

        let grid = grid_3x3([
            s(GoldIngot),
            s(GoldIngot),
            n(),
            s(GoldIngot),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((GoldAxe, 1)));

        let grid = grid_3x3([
            n(),
            s(GoldIngot),
            n(),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((GoldShovel, 1)));

        let grid = grid_3x3([
            n(),
            s(GoldIngot),
            n(),
            n(),
            s(GoldIngot),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((GoldSword, 1)));

        let grid = grid_3x3([
            s(GoldIngot),
            s(GoldIngot),
            n(),
            n(),
            s(Stick),
            n(),
            n(),
            s(Stick),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((GoldHoe, 1)));
    }

    // ── Stone brick recipe ──────────────────────────────────────────────────

    #[test]
    fn test_stone_brick_recipe() {
        let grid = grid_2x2(s(Stone), s(Stone), s(Stone), s(Stone));
        assert_eq!(find_recipe(&grid, 2, 2), Some((StoneBrick, 4)));
    }

    // ── Iron block recipe ───────────────────────────────────────────────────

    #[test]
    fn test_iron_block_recipe() {
        let grid = grid_3x3([
            s(IronIngot),
            s(IronIngot),
            s(IronIngot),
            s(IronIngot),
            s(IronIngot),
            s(IronIngot),
            s(IronIngot),
            s(IronIngot),
            s(IronIngot),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((IronBlock, 1)));
    }

    // ── Lapis block recipe ──────────────────────────────────────────────────

    #[test]
    fn test_lapis_block_recipe() {
        let grid = grid_3x3([
            s(LapisLazuli),
            s(LapisLazuli),
            s(LapisLazuli),
            s(LapisLazuli),
            s(LapisLazuli),
            s(LapisLazuli),
            s(LapisLazuli),
            s(LapisLazuli),
            s(LapisLazuli),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((LapisBlock, 1)));
    }

    // ── Bookshelf recipe ────────────────────────────────────────────────────

    #[test]
    fn test_bookshelf_recipe() {
        let grid = grid_3x3([
            s(OakPlanks),
            s(OakPlanks),
            s(OakPlanks),
            s(OakPlanks),
            s(OakPlanks),
            s(OakPlanks),
            s(OakPlanks),
            s(OakPlanks),
            s(OakPlanks),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((Bookshelf, 1)));
    }

    // ── Shapeless recipe matching regardless of position ────────────────────

    #[test]
    fn test_shapeless_oak_log_any_position_in_3x3() {
        // OakLog in every position of 3x3 grid should produce planks
        for i in 0..9 {
            let mut grid = vec![None; 9];
            grid[i] = Some(OakLog);
            assert_eq!(
                find_recipe(&grid, 3, 3),
                Some((OakPlanks, 4)),
                "OakLog at position {} should still produce planks",
                i
            );
        }
    }

    #[test]
    fn test_shapeless_iron_block_decompose_any_position() {
        for i in 0..4 {
            let mut grid = vec![None; 4];
            grid[i] = Some(IronBlock);
            assert_eq!(
                find_recipe(&grid, 2, 2),
                Some((IronIngot, 9)),
                "IronBlock at position {} should decompose",
                i
            );
        }
    }

    #[test]
    fn test_shapeless_lapis_block_decompose() {
        let grid = grid_2x2(n(), s(LapisBlock), n(), n());
        assert_eq!(find_recipe(&grid, 2, 2), Some((LapisLazuli, 9)));
    }

    // ── 2x2 recipe in 3x3 grid (bounding box extraction) ───────────────────

    #[test]
    fn test_2x2_crafting_table_in_3x3_top_left() {
        let grid = grid_3x3([
            s(OakPlanks),
            s(OakPlanks),
            n(),
            s(OakPlanks),
            s(OakPlanks),
            n(),
            n(),
            n(),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((CraftingTable, 1)));
    }

    #[test]
    fn test_2x2_crafting_table_in_3x3_bottom_right() {
        let grid = grid_3x3([
            n(),
            n(),
            n(),
            n(),
            s(OakPlanks),
            s(OakPlanks),
            n(),
            s(OakPlanks),
            s(OakPlanks),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((CraftingTable, 1)));
    }

    #[test]
    fn test_2x2_sandstone_in_3x3_center() {
        let grid = grid_3x3([n(), n(), n(), n(), s(Sand), s(Sand), n(), s(Sand), s(Sand)]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((Sandstone, 1)));
    }

    // ── Empty slots mixed with ingredients ──────────────────────────────────

    #[test]
    fn test_empty_slots_around_stick_recipe() {
        // Sticks: 1x2 planks vertical in a 3x3 grid, surrounded by empties
        let grid = grid_3x3([
            n(),
            n(),
            n(),
            n(),
            s(OakPlanks),
            n(),
            n(),
            s(OakPlanks),
            n(),
        ]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((Stick, 4)));
    }

    #[test]
    fn test_empty_slots_torch_bottom_right() {
        let grid = grid_3x3([n(), n(), n(), n(), n(), s(Coal), n(), n(), s(Stick)]);
        assert_eq!(find_recipe(&grid, 3, 3), Some((Torch, 4)));
    }

    // ── Single ingredient recipes ───────────────────────────────────────────

    #[test]
    fn test_single_ingredient_oak_log() {
        // Single OakLog -> 4 planks (shapeless, single ingredient)
        let grid = vec![Some(OakLog)];
        assert_eq!(find_recipe(&grid, 1, 1), Some((OakPlanks, 4)));
    }

    #[test]
    fn test_single_ingredient_spruce_log() {
        let grid = vec![Some(SpruceLog)];
        assert_eq!(find_recipe(&grid, 1, 1), Some((OakPlanks, 4)));
    }

    #[test]
    fn test_single_unrecognized_ingredient() {
        let grid = vec![Some(Dirt)];
        assert_eq!(find_recipe(&grid, 1, 1), None);
    }

    #[test]
    fn test_all_25_tools_have_recipes() {
        let tools = [
            WoodPickaxe,
            StonePickaxe,
            IronPickaxe,
            DiamondPickaxe,
            GoldPickaxe,
            WoodAxe,
            StoneAxe,
            IronAxe,
            DiamondAxe,
            GoldAxe,
            WoodShovel,
            StoneShovel,
            IronShovel,
            DiamondShovel,
            GoldShovel,
            WoodSword,
            StoneSword,
            IronSword,
            DiamondSword,
            GoldSword,
            WoodHoe,
            StoneHoe,
            IronHoe,
            DiamondHoe,
            GoldHoe,
        ];
        for tool in tools {
            let found = RECIPES.iter().any(|r| r.output == tool);
            assert!(found, "No recipe found for {tool:?}");
        }
    }

    #[test]
    fn test_2x2_recipe_works_in_3x3_grid() {
        // Planks recipe in top-left of 3x3 grid
        #[rustfmt::skip]
        let grid = vec![
            Some(OakLog), None, None,
            None, None, None,
            None, None, None,
        ];
        let result = find_recipe(&grid, 3, 3);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, OakPlanks);
    }

    #[test]
    fn test_2x2_recipe_works_in_3x3_bottom_right() {
        // Sticks recipe in bottom-right corner of 3x3 grid
        #[rustfmt::skip]
        let grid = vec![
            None, None, None,
            None, None, Some(OakPlanks),
            None, None, Some(OakPlanks),
        ];
        let result = find_recipe(&grid, 3, 3);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, Stick);
    }

    #[test]
    fn test_stone_pickaxe_recipe() {
        #[rustfmt::skip]
        let grid = vec![
            Some(Cobblestone), Some(Cobblestone), Some(Cobblestone),
            None, Some(Stick), None,
            None, Some(Stick), None,
        ];
        let result = find_recipe(&grid, 3, 3);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, StonePickaxe);
    }

    #[test]
    fn test_iron_sword_recipe() {
        #[rustfmt::skip]
        let grid = vec![
            None, Some(IronIngot), None,
            None, Some(IronIngot), None,
            None, Some(Stick), None,
        ];
        let result = find_recipe(&grid, 3, 3);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, IronSword);
    }

    #[test]
    fn test_diamond_axe_recipe() {
        #[rustfmt::skip]
        let grid = vec![
            Some(Diamond), Some(Diamond), None,
            Some(Diamond), Some(Stick), None,
            None, Some(Stick), None,
        ];
        let result = find_recipe(&grid, 3, 3);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, DiamondAxe);
    }

    #[test]
    fn test_gold_shovel_recipe() {
        #[rustfmt::skip]
        let grid = vec![
            None, Some(GoldIngot), None,
            None, Some(Stick), None,
            None, Some(Stick), None,
        ];
        let result = find_recipe(&grid, 3, 3);
        assert!(result.is_some());
        assert_eq!(result.unwrap().0, GoldShovel);
    }

    #[test]
    fn test_all_none_grid() {
        let grid = vec![None; 9];
        assert_eq!(find_recipe(&grid, 3, 3), None);
    }

    #[test]
    fn test_smelting_inputs() {
        assert_eq!(smelt_item(RawIron), Some((IronIngot, 1)));
        assert_eq!(smelt_item(IronOre), Some((IronIngot, 1)));
        assert_eq!(smelt_item(GoldOre), Some((GoldIngot, 1)));
        assert_eq!(smelt_item(Cobblestone), Some((Stone, 1)));
        assert_eq!(smelt_item(Sand), Some((Glass, 1)));
        assert_eq!(smelt_item(Clay), Some((Brick, 1)));
        assert_eq!(smelt_item(OakLog), Some((Coal, 1)));
        assert_eq!(smelt_item(Dirt), None);
    }

    #[test]
    fn test_furnace_fuels() {
        assert!(is_furnace_fuel(Coal));
        assert_eq!(fuel_burn_time(Coal), 80.0);
        assert!(is_furnace_fuel(OakLog));
        assert_eq!(fuel_burn_time(OakLog), 15.0);
        assert!(is_furnace_fuel(Stick));
        assert_eq!(fuel_burn_time(Stick), 5.0);
        assert!(is_furnace_fuel(WoodPickaxe));
        assert_eq!(fuel_burn_time(WoodPickaxe), 10.0);
        assert!(!is_furnace_fuel(Stone));
        assert_eq!(fuel_burn_time(Stone), 0.0);
    }

    #[test]
    fn test_flint_and_steel_recipe() {
        let grid = vec![s(IronIngot), s(Gravel)];
        let result = find_recipe(&grid, 2, 1);
        assert_eq!(result, Some((FlintAndSteel, 1)));
    }

    #[test]
    fn test_mossy_cobblestone_recipe() {
        let grid = vec![s(Cobblestone), s(OakLeaves)];
        let result = find_recipe(&grid, 2, 1);
        assert_eq!(result, Some((MossyCobblestone, 1)));
    }

    #[test]
    fn test_compacting_and_decrafting() {
        // 9 Coal -> CoalOre
        let grid = vec![s(Coal); 9];
        let result = find_recipe(&grid, 3, 3);
        assert_eq!(result, Some((CoalOre, 1)));

        // 1 CoalOre -> 9 Coal
        let grid = vec![s(CoalOre)];
        let result = find_recipe(&grid, 1, 1);
        assert_eq!(result, Some((Coal, 9)));

        // 9 Diamond -> DiamondOre
        let grid = vec![s(Diamond); 9];
        let result = find_recipe(&grid, 3, 3);
        assert_eq!(result, Some((DiamondOre, 1)));
    }

    #[test]
    fn test_bread_bow_and_armor_recipes() {
        // 3 Wheat -> Bread
        let grid = vec![s(Wheat), s(Wheat), s(Wheat)];
        let result = find_recipe(&grid, 3, 1);
        assert_eq!(result, Some((Bread, 1)));

        // Iron Chestplate (8 IronIngot)
        let grid = vec![
            s(IronIngot),
            None,
            s(IronIngot),
            s(IronIngot),
            s(IronIngot),
            s(IronIngot),
            s(IronIngot),
            s(IronIngot),
            s(IronIngot),
        ];
        let result = find_recipe(&grid, 3, 3);
        assert_eq!(result, Some((IronChestplate, 1)));
    }
}
