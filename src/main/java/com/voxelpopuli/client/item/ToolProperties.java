package com.voxelpopuli.client.item;

import com.voxelpopuli.client.block.BlockType;

public class ToolProperties {
    public enum ToolType {
        NONE, PICKAXE, AXE, SHOVEL, SWORD, HOE
    }

    public enum ToolMaterial {
        NONE(0), WOOD(1), GOLD(2), STONE(3), IRON(4), DIAMOND(5);

        public final int tier;
        ToolMaterial(int tier) {
            this.tier = tier;
        }

        public boolean gte(ToolMaterial other) {
            return this.tier >= other.tier;
        }
    }

    public static class BlockProperties {
        public final float hardness;
        public final ToolType toolType;
        public final ToolMaterial minTier;
        public final boolean requiresTool;
        public final BlockType drop;
        public final int dropCount;

        public BlockProperties(float hardness, ToolType toolType, ToolMaterial minTier, boolean requiresTool, BlockType drop, int dropCount) {
            this.hardness = hardness;
            this.toolType = toolType;
            this.minTier = minTier;
            this.requiresTool = requiresTool;
            this.drop = drop;
            this.dropCount = dropCount;
        }
    }

    public final ToolType toolType;
    public final ToolMaterial material;
    public final float speed;
    public final int durability;

    public ToolProperties(ToolType toolType, ToolMaterial material, float speed, int durability) {
        this.toolType = toolType;
        this.material = material;
        this.speed = speed;
        this.durability = durability;
    }

    public static BlockProperties getBlockProperties(BlockType b) {
        return switch (b) {
            // Instant break
            case TORCH -> new BlockProperties(0.0f, ToolType.NONE, ToolMaterial.NONE, false, BlockType.TORCH, 1);
            case SNOW_LAYER -> new BlockProperties(0.1f, ToolType.SHOVEL, ToolMaterial.NONE, false, BlockType.SNOW_LAYER, 1);

            // Fragile blocks
            case GLASS -> new BlockProperties(0.3f, ToolType.NONE, ToolMaterial.NONE, false, BlockType.AIR, 0);
            case OAK_LEAVES -> new BlockProperties(0.2f, ToolType.NONE, ToolMaterial.NONE, false, BlockType.AIR, 0);
            case SPRUCE_LEAVES -> new BlockProperties(0.2f, ToolType.NONE, ToolMaterial.NONE, false, BlockType.AIR, 0);

            // Soft blocks (shovel)
            case DIRT -> new BlockProperties(0.5f, ToolType.SHOVEL, ToolMaterial.NONE, false, BlockType.DIRT, 1);
            case SAND -> new BlockProperties(0.5f, ToolType.SHOVEL, ToolMaterial.NONE, false, BlockType.SAND, 1);
            case GRASS -> new BlockProperties(0.6f, ToolType.SHOVEL, ToolMaterial.NONE, false, BlockType.DIRT, 1);
            case SNOWY_GRASS -> new BlockProperties(0.6f, ToolType.SHOVEL, ToolMaterial.NONE, false, BlockType.DIRT, 1);
            case GRAVEL -> new BlockProperties(0.6f, ToolType.SHOVEL, ToolMaterial.NONE, false, BlockType.GRAVEL, 1);
            case POWDERED_SNOW -> new BlockProperties(0.25f, ToolType.SHOVEL, ToolMaterial.NONE, false, BlockType.POWDERED_SNOW, 1);
            case SPONGE -> new BlockProperties(0.6f, ToolType.NONE, ToolMaterial.NONE, false, BlockType.SPONGE, 1);
            case WOOL -> new BlockProperties(0.8f, ToolType.NONE, ToolMaterial.NONE, false, BlockType.WOOL, 1);

            // Wood blocks (axe)
            case OAK_LOG -> new BlockProperties(2.0f, ToolType.AXE, ToolMaterial.NONE, false, BlockType.OAK_LOG, 1);
            case SPRUCE_LOG -> new BlockProperties(2.0f, ToolType.AXE, ToolMaterial.NONE, false, BlockType.SPRUCE_LOG, 1);
            case OAK_PLANKS -> new BlockProperties(2.0f, ToolType.AXE, ToolMaterial.NONE, false, BlockType.OAK_PLANKS, 1);
            case CRAFTING_TABLE -> new BlockProperties(2.5f, ToolType.AXE, ToolMaterial.NONE, false, BlockType.CRAFTING_TABLE, 1);
            case CHEST -> new BlockProperties(2.5f, ToolType.AXE, ToolMaterial.NONE, false, BlockType.CHEST, 1);
            case BOOKSHELF -> new BlockProperties(1.5f, ToolType.AXE, ToolMaterial.NONE, false, BlockType.BOOKSHELF, 1);

            // Stone blocks (pickaxe required)
            case STONE -> new BlockProperties(1.5f, ToolType.PICKAXE, ToolMaterial.WOOD, true, BlockType.COBBLESTONE, 1);
            case COBBLESTONE -> new BlockProperties(2.0f, ToolType.PICKAXE, ToolMaterial.WOOD, true, BlockType.COBBLESTONE, 1);
            case STONE_BRICK -> new BlockProperties(1.5f, ToolType.PICKAXE, ToolMaterial.WOOD, true, BlockType.STONE_BRICK, 1);
            case BRICK -> new BlockProperties(2.0f, ToolType.PICKAXE, ToolMaterial.WOOD, true, BlockType.BRICK, 1);
            case MOSSY_COBBLESTONE -> new BlockProperties(2.0f, ToolType.PICKAXE, ToolMaterial.WOOD, true, BlockType.MOSSY_COBBLESTONE, 1);
            case SANDSTONE -> new BlockProperties(0.8f, ToolType.PICKAXE, ToolMaterial.WOOD, true, BlockType.SANDSTONE, 1);
            case FURNACE -> new BlockProperties(3.5f, ToolType.PICKAXE, ToolMaterial.WOOD, true, BlockType.FURNACE, 1);

            // Ores (pickaxe required, varying tiers)
            case COAL_ORE -> new BlockProperties(3.0f, ToolType.PICKAXE, ToolMaterial.WOOD, true, BlockType.COAL, 1);
            case IRON_ORE -> new BlockProperties(3.0f, ToolType.PICKAXE, ToolMaterial.STONE, true, BlockType.IRON_ORE, 1);
            case GOLD_ORE -> new BlockProperties(3.0f, ToolType.PICKAXE, ToolMaterial.IRON, true, BlockType.GOLD_ORE, 1);
            case DIAMOND_ORE -> new BlockProperties(3.0f, ToolType.PICKAXE, ToolMaterial.IRON, true, BlockType.DIAMOND, 1);
            case LAPIS_ORE -> new BlockProperties(3.0f, ToolType.PICKAXE, ToolMaterial.STONE, true, BlockType.LAPIS_LAZULI, 4);

            // Metal/gem blocks
            case IRON_BLOCK -> new BlockProperties(5.0f, ToolType.PICKAXE, ToolMaterial.STONE, true, BlockType.IRON_BLOCK, 1);
            case LAPIS_BLOCK -> new BlockProperties(3.0f, ToolType.PICKAXE, ToolMaterial.STONE, true, BlockType.LAPIS_BLOCK, 1);

            // Obsidian (diamond pickaxe required)
            case OBSIDIAN -> new BlockProperties(50.0f, ToolType.PICKAXE, ToolMaterial.DIAMOND, true, BlockType.OBSIDIAN, 1);

            // Special
            case BEDROCK -> new BlockProperties(-1.0f, ToolType.NONE, ToolMaterial.NONE, false, BlockType.AIR, 0);
            case WATER -> new BlockProperties(-1.0f, ToolType.NONE, ToolMaterial.NONE, false, BlockType.AIR, 0);
            case AIR -> new BlockProperties(0.0f, ToolType.NONE, ToolMaterial.NONE, false, BlockType.AIR, 0);

            // TNT/Nuke drop themselves
            case TNT -> new BlockProperties(0.0f, ToolType.NONE, ToolMaterial.NONE, false, BlockType.TNT, 1);
            case NUKE -> new BlockProperties(0.0f, ToolType.NONE, ToolMaterial.NONE, false, BlockType.NUKE, 1);

            // Items don't have block hardness (shouldn't be mined)
            default -> new BlockProperties(0.0f, ToolType.NONE, ToolMaterial.NONE, false, b, 1);
        };
    }

    public static ToolProperties getProperties(BlockType b) {
        return switch (b) {
            // Pickaxes
            case WOOD_PICKAXE -> new ToolProperties(ToolType.PICKAXE, ToolMaterial.WOOD, 2.0f, 59);
            case STONE_PICKAXE -> new ToolProperties(ToolType.PICKAXE, ToolMaterial.STONE, 4.0f, 131);
            case IRON_PICKAXE -> new ToolProperties(ToolType.PICKAXE, ToolMaterial.IRON, 6.0f, 250);
            case DIAMOND_PICKAXE -> new ToolProperties(ToolType.PICKAXE, ToolMaterial.DIAMOND, 8.0f, 1561);
            case GOLD_PICKAXE -> new ToolProperties(ToolType.PICKAXE, ToolMaterial.GOLD, 12.0f, 32);

            // Axes
            case WOOD_AXE -> new ToolProperties(ToolType.AXE, ToolMaterial.WOOD, 2.0f, 59);
            case STONE_AXE -> new ToolProperties(ToolType.AXE, ToolMaterial.STONE, 4.0f, 131);
            case IRON_AXE -> new ToolProperties(ToolType.AXE, ToolMaterial.IRON, 6.0f, 250);
            case DIAMOND_AXE -> new ToolProperties(ToolType.AXE, ToolMaterial.DIAMOND, 8.0f, 1561);
            case GOLD_AXE -> new ToolProperties(ToolType.AXE, ToolMaterial.GOLD, 12.0f, 32);

            // Shovels
            case WOOD_SHOVEL -> new ToolProperties(ToolType.SHOVEL, ToolMaterial.WOOD, 2.0f, 59);
            case STONE_SHOVEL -> new ToolProperties(ToolType.SHOVEL, ToolMaterial.STONE, 4.0f, 131);
            case IRON_SHOVEL -> new ToolProperties(ToolType.SHOVEL, ToolMaterial.IRON, 6.0f, 250);
            case DIAMOND_SHOVEL -> new ToolProperties(ToolType.SHOVEL, ToolMaterial.DIAMOND, 8.0f, 1561);
            case GOLD_SHOVEL -> new ToolProperties(ToolType.SHOVEL, ToolMaterial.GOLD, 12.0f, 32);

            // Swords
            case WOOD_SWORD -> new ToolProperties(ToolType.SWORD, ToolMaterial.WOOD, 1.5f, 59);
            case STONE_SWORD -> new ToolProperties(ToolType.SWORD, ToolMaterial.STONE, 1.5f, 131);
            case IRON_SWORD -> new ToolProperties(ToolType.SWORD, ToolMaterial.IRON, 1.5f, 250);
            case DIAMOND_SWORD -> new ToolProperties(ToolType.SWORD, ToolMaterial.DIAMOND, 1.5f, 1561);
            case GOLD_SWORD -> new ToolProperties(ToolType.SWORD, ToolMaterial.GOLD, 1.5f, 32);

            // Hoes
            case WOOD_HOE -> new ToolProperties(ToolType.HOE, ToolMaterial.WOOD, 1.0f, 59);
            case STONE_HOE -> new ToolProperties(ToolType.HOE, ToolMaterial.STONE, 1.0f, 131);
            case IRON_HOE -> new ToolProperties(ToolType.HOE, ToolMaterial.IRON, 1.0f, 250);
            case DIAMOND_HOE -> new ToolProperties(ToolType.HOE, ToolMaterial.DIAMOND, 1.0f, 1561);
            case GOLD_HOE -> new ToolProperties(ToolType.HOE, ToolMaterial.GOLD, 1.0f, 32);

            // Flint and Steel
            case FLINT_AND_STEEL -> new ToolProperties(ToolType.NONE, ToolMaterial.NONE, 1.0f, 64);

            default -> null;
        };
    }

    public static float breakingTime(BlockType block, BlockType held) {
        BlockProperties props = getBlockProperties(block);

        if (props.hardness < 0.0f) {
            return Float.POSITIVE_INFINITY;
        }

        if (props.hardness == 0.0f) {
            return 0.05f; // 1 tick minimum
        }

        ToolProperties tp = getProperties(held);
        float speed = 1.0f;
        boolean canHarvest = !props.requiresTool;

        if (tp != null) {
            if (tp.toolType == props.toolType) {
                speed = tp.speed;
                if (tp.material.gte(props.minTier)) {
                    canHarvest = true;
                }
            } else if (!props.requiresTool) {
                canHarvest = true;
            }
        }

        float baseTime = canHarvest ? props.hardness * 1.5f : props.hardness * 5.0f;
        return baseTime / speed;
    }

    public static BlockType getDrop(BlockType block, BlockType held) {
        BlockProperties props = getBlockProperties(block);
        if (props.requiresTool) {
            ToolProperties tp = getProperties(held);
            if (tp != null && tp.toolType == props.toolType && tp.material.gte(props.minTier)) {
                return props.drop;
            }
            return BlockType.AIR;
        }
        return props.drop;
    }

    public static int getDropCount(BlockType block, BlockType held) {
        BlockProperties props = getBlockProperties(block);
        if (props.requiresTool) {
            ToolProperties tp = getProperties(held);
            if (tp != null && tp.toolType == props.toolType && tp.material.gte(props.minTier)) {
                return props.dropCount;
            }
            return 0;
        }
        return props.dropCount;
    }
}
