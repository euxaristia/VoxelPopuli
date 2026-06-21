package com.voxelpopuli.client.block;

public enum BlockType {
    // === Original blocks (0-22) ===
    AIR(0),
    STONE(1),
    GRASS(2),
    DIRT(3),
    OAK_LOG(4),
    OAK_LEAVES(5),
    BEDROCK(6),
    WATER(7),
    SAND(8),
    GRAVEL(9),
    COAL_ORE(10),
    POWDERED_SNOW(11),
    SNOWY_GRASS(12),
    SPRUCE_LOG(13),
    SPRUCE_LEAVES(14),
    SNOW_LAYER(15),
    IRON_ORE(16),
    RAW_IRON(17),
    IRON_INGOT(18),
    IRON_BLOCK(19),
    TNT(20),
    NUKE(21),
    FLINT_AND_STEEL(22),

    // === New placeable blocks (23-41) ===
    COBBLESTONE(23),
    OAK_PLANKS(24),
    CRAFTING_TABLE(25),
    FURNACE(26),
    CHEST(27),
    TORCH(28),
    GOLD_ORE(29),
    DIAMOND_ORE(30),
    GLASS(31),
    BOOKSHELF(32),
    MOSSY_COBBLESTONE(33),
    OBSIDIAN(34),
    SPONGE(35),
    WOOL(36),
    LAPIS_ORE(37),
    LAPIS_BLOCK(38),
    SANDSTONE(39),
    BRICK(40),
    STONE_BRICK(41),

    // === New material items (42-49) ===
    STICK(42),
    COAL(43),
    GOLD_INGOT(44),
    DIAMOND(45),
    LAPIS_LAZULI(46),
    STRING(47),
    GUNPOWDER(48),
    LEATHER(49),

    // === Tools: Pickaxes (50-54) ===
    WOOD_PICKAXE(50),
    STONE_PICKAXE(51),
    IRON_PICKAXE(52),
    DIAMOND_PICKAXE(53),
    GOLD_PICKAXE(54),

    // === Tools: Axes (55-59) ===
    WOOD_AXE(55),
    STONE_AXE(56),
    IRON_AXE(57),
    DIAMOND_AXE(58),
    GOLD_AXE(59),

    // === Tools: Shovels (60-64) ===
    WOOD_SHOVEL(60),
    STONE_SHOVEL(61),
    IRON_SHOVEL(62),
    DIAMOND_SHOVEL(63),
    GOLD_SHOVEL(64),

    // === Tools: Swords (65-69) ===
    WOOD_SWORD(65),
    STONE_SWORD(66),
    IRON_SWORD(67),
    DIAMOND_SWORD(68),
    GOLD_SWORD(69),

    // === Tools: Hoes (70-74) ===
    WOOD_HOE(70),
    STONE_HOE(71),
    IRON_HOE(72),
    DIAMOND_HOE(73),
    GOLD_HOE(74);

    public final int id;

    BlockType(int id) {
        this.id = id;
    }

    public static final int COUNT = 75;
    private static final BlockType[] BY_ID = new BlockType[COUNT];

    static {
        for (BlockType b : values()) {
            if (b.id >= 0 && b.id < COUNT) {
                BY_ID[b.id] = b;
            }
        }
    }

    public static BlockType fromId(int id) {
        if (id >= 0 && id < COUNT) {
            BlockType b = BY_ID[id];
            return b != null ? b : AIR;
        }
        return AIR;
    }

    public boolean isItem() {
        return this == RAW_IRON
            || this == IRON_INGOT
            || this == FLINT_AND_STEEL
            || this == STICK
            || this == COAL
            || this == GOLD_INGOT
            || this == DIAMOND
            || this == LAPIS_LAZULI
            || this == STRING
            || this == GUNPOWDER
            || this == LEATHER
            || this.isTool();
    }

    public boolean isSolid() {
        if (this == AIR || this == WATER || this == SNOW_LAYER || this == TORCH) {
            return false;
        }
        return !this.isItem();
    }

    public boolean isTool() {
        return this == WOOD_PICKAXE
            || this == STONE_PICKAXE
            || this == IRON_PICKAXE
            || this == DIAMOND_PICKAXE
            || this == GOLD_PICKAXE
            || this == WOOD_AXE
            || this == STONE_AXE
            || this == IRON_AXE
            || this == DIAMOND_AXE
            || this == GOLD_AXE
            || this == WOOD_SHOVEL
            || this == STONE_SHOVEL
            || this == IRON_SHOVEL
            || this == DIAMOND_SHOVEL
            || this == GOLD_SHOVEL
            || this == WOOD_SWORD
            || this == STONE_SWORD
            || this == IRON_SWORD
            || this == DIAMOND_SWORD
            || this == GOLD_SWORD
            || this == WOOD_HOE
            || this == STONE_HOE
            || this == IRON_HOE
            || this == DIAMOND_HOE
            || this == GOLD_HOE
            || this == FLINT_AND_STEEL;
    }

    public boolean isTransparent() {
        return this == OAK_LEAVES
            || this == SPRUCE_LEAVES
            || this == SNOW_LAYER
            || this == GLASS
            || this == TORCH;
    }

    public int[] getAtlasUV() {
        return switch (this) {
            case STONE -> new int[]{1, 0};
            case DIRT -> new int[]{2, 0};
            case GRASS -> new int[]{3, 0};
            case OAK_LOG -> new int[]{4, 0};
            case OAK_LEAVES -> new int[]{5, 0};
            case SAND -> new int[]{6, 0};
            case GRAVEL -> new int[]{7, 0};
            case POWDERED_SNOW, SNOW_LAYER -> new int[]{8, 0};
            case SNOWY_GRASS -> new int[]{10, 0};
            case SPRUCE_LOG -> new int[]{11, 0};
            case SPRUCE_LEAVES -> new int[]{12, 0};
            case WATER -> new int[]{13, 12};
            case BEDROCK -> new int[]{1, 1};
            case COAL_ORE -> new int[]{2, 1};
            case IRON_ORE -> new int[]{3, 1};
            case TNT -> new int[]{4, 1};
            case IRON_BLOCK -> new int[]{6, 1};
            case RAW_IRON -> new int[]{7, 1};
            case IRON_INGOT -> new int[]{8, 1};
            case FLINT_AND_STEEL -> new int[]{9, 1};
            case COBBLESTONE -> new int[]{0, 2};
            case OAK_PLANKS -> new int[]{1, 2};
            case CRAFTING_TABLE -> new int[]{2, 2};
            case FURNACE -> new int[]{4, 2};
            case GLASS -> new int[]{6, 2};
            case GOLD_ORE -> new int[]{7, 2};
            case DIAMOND_ORE -> new int[]{8, 2};
            case OBSIDIAN -> new int[]{9, 2};
            case BRICK -> new int[]{10, 2};
            case STONE_BRICK -> new int[]{11, 2};
            case SANDSTONE -> new int[]{12, 2};
            case WOOL -> new int[]{13, 2};
            case BOOKSHELF -> new int[]{14, 2};
            case SPONGE -> new int[]{15, 2};
            case NUKE -> new int[]{6, 7};
            case CHEST -> new int[]{0, 7};
            case MOSSY_COBBLESTONE -> new int[]{2, 7};
            case LAPIS_ORE -> new int[]{4, 7};
            case LAPIS_BLOCK -> new int[]{3, 7};
            case TORCH -> new int[]{5, 7};
            case STICK -> new int[]{0, 4};
            case COAL -> new int[]{1, 4};
            case DIAMOND -> new int[]{2, 4};
            case GOLD_INGOT -> new int[]{3, 4};
            case LAPIS_LAZULI -> new int[]{4, 4};
            case STRING -> new int[]{5, 4};
            case GUNPOWDER -> new int[]{6, 4};
            case LEATHER -> new int[]{7, 4};
            case WOOD_PICKAXE -> new int[]{0, 5};
            case WOOD_AXE -> new int[]{1, 5};
            case WOOD_SHOVEL -> new int[]{2, 5};
            case WOOD_SWORD -> new int[]{3, 5};
            case WOOD_HOE -> new int[]{4, 5};
            case STONE_PICKAXE -> new int[]{5, 5};
            case STONE_AXE -> new int[]{6, 5};
            case STONE_SHOVEL -> new int[]{7, 5};
            case STONE_SWORD -> new int[]{8, 5};
            case STONE_HOE -> new int[]{9, 5};
            case IRON_PICKAXE -> new int[]{0, 6};
            case IRON_AXE -> new int[]{1, 6};
            case IRON_SHOVEL -> new int[]{2, 6};
            case IRON_SWORD -> new int[]{3, 6};
            case IRON_HOE -> new int[]{4, 6};
            case DIAMOND_PICKAXE -> new int[]{5, 6};
            case DIAMOND_AXE -> new int[]{6, 6};
            case DIAMOND_SHOVEL -> new int[]{7, 6};
            case DIAMOND_SWORD -> new int[]{8, 6};
            case DIAMOND_HOE -> new int[]{9, 6};
            case GOLD_PICKAXE -> new int[]{10, 6};
            case GOLD_AXE -> new int[]{11, 6};
            case GOLD_SHOVEL -> new int[]{12, 6};
            case GOLD_SWORD -> new int[]{13, 6};
            case GOLD_HOE -> new int[]{14, 6};
            default -> new int[]{0, 0};
        };
    }

    public int[] getAtlasUVTop() {
        return switch (this) {
            case GRASS -> new int[]{0, 0};
            case SNOWY_GRASS -> new int[]{9, 0};
            case OAK_LOG, SPRUCE_LOG -> new int[]{5, 1};
            case CRAFTING_TABLE -> new int[]{2, 2};
            case FURNACE -> new int[]{0, 2};
            case CHEST -> new int[]{1, 7};
            case BOOKSHELF -> new int[]{1, 2};
            default -> getAtlasUV();
        };
    }

    public int[] getAtlasUVSide() {
        return switch (this) {
            case CRAFTING_TABLE -> new int[]{3, 2};
            case FURNACE -> new int[]{5, 2};
            case CHEST -> new int[]{0, 7};
            default -> getAtlasUV();
        };
    }

    public int[] getAtlasUVBottom() {
        return switch (this) {
            case GRASS, SNOWY_GRASS -> BlockType.DIRT.getAtlasUV();
            case OAK_LOG, SPRUCE_LOG -> new int[]{5, 1};
            case CRAFTING_TABLE -> new int[]{1, 2};
            case FURNACE, CHEST -> new int[]{0, 2};
            case BOOKSHELF -> new int[]{1, 2};
            default -> getAtlasUV();
        };
    }

    public int getMaxStackSize() {
        return isTool() ? 1 : 64;
    }
}
