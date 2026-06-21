package com.voxelpopuli.client.item;

import com.voxelpopuli.client.block.BlockType;
import java.util.*;

public class Crafting {

    public static abstract class RecipeShape {
        public static class Shaped extends RecipeShape {
            public final int width;
            public final int height;
            public final BlockType[] pattern;

            public Shaped(int width, int height, BlockType[] pattern) {
                this.width = width;
                this.height = height;
                this.pattern = pattern;
            }
        }

        public static class Shapeless extends RecipeShape {
            public final BlockType[] ingredients;

            public Shapeless(BlockType[] ingredients) {
                this.ingredients = ingredients;
            }
        }
    }

    public static class Recipe {
        public final RecipeShape shape;
        public final BlockType output;
        public final int outputCount;
        public final boolean mirror;

        public Recipe(RecipeShape shape, BlockType output, int outputCount, boolean mirror) {
            this.shape = shape;
            this.output = output;
            this.outputCount = outputCount;
            this.mirror = mirror;
        }
    }

    public static final List<Recipe> RECIPES = new ArrayList<>();

    static {
        // === Basic materials ===
        RECIPES.add(new Recipe(
            new RecipeShape.Shapeless(new BlockType[]{BlockType.OAK_LOG}),
            BlockType.OAK_PLANKS, 4, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shapeless(new BlockType[]{BlockType.SPRUCE_LOG}),
            BlockType.OAK_PLANKS, 4, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(1, 2, new BlockType[]{BlockType.OAK_PLANKS, BlockType.OAK_PLANKS}),
            BlockType.STICK, 4, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(2, 2, new BlockType[]{BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, BlockType.OAK_PLANKS}),
            BlockType.CRAFTING_TABLE, 1, false
        ));
        
        // === 3x3 recipes (require crafting table) ===
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(3, 3, new BlockType[]{
                BlockType.COBBLESTONE, BlockType.COBBLESTONE, BlockType.COBBLESTONE, 
                BlockType.COBBLESTONE, BlockType.AIR, BlockType.COBBLESTONE, 
                BlockType.COBBLESTONE, BlockType.COBBLESTONE, BlockType.COBBLESTONE
            }),
            BlockType.FURNACE, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(3, 3, new BlockType[]{
                BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, 
                BlockType.OAK_PLANKS, BlockType.AIR, BlockType.OAK_PLANKS, 
                BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, BlockType.OAK_PLANKS
            }),
            BlockType.CHEST, 1, false
        ));
        
        // === Torches ===
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(1, 2, new BlockType[]{BlockType.COAL, BlockType.STICK}),
            BlockType.TORCH, 4, false
        ));
        
        // === Wooden tools ===
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(3, 3, new BlockType[]{
                BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, 
                BlockType.AIR, BlockType.STICK, BlockType.AIR, 
                BlockType.AIR, BlockType.STICK, BlockType.AIR
            }),
            BlockType.WOOD_PICKAXE, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(2, 3, new BlockType[]{
                BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, 
                BlockType.OAK_PLANKS, BlockType.STICK, 
                BlockType.AIR, BlockType.STICK
            }),
            BlockType.WOOD_AXE, 1, true
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(1, 3, new BlockType[]{BlockType.OAK_PLANKS, BlockType.STICK, BlockType.STICK}),
            BlockType.WOOD_SHOVEL, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(1, 3, new BlockType[]{BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, BlockType.STICK}),
            BlockType.WOOD_SWORD, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(2, 3, new BlockType[]{
                BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, 
                BlockType.AIR, BlockType.STICK, 
                BlockType.AIR, BlockType.STICK
            }),
            BlockType.WOOD_HOE, 1, true
        ));
        
        // === Stone tools ===
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(3, 3, new BlockType[]{
                BlockType.COBBLESTONE, BlockType.COBBLESTONE, BlockType.COBBLESTONE, 
                BlockType.AIR, BlockType.STICK, BlockType.AIR, 
                BlockType.AIR, BlockType.STICK, BlockType.AIR
            }),
            BlockType.STONE_PICKAXE, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(2, 3, new BlockType[]{
                BlockType.COBBLESTONE, BlockType.COBBLESTONE, 
                BlockType.COBBLESTONE, BlockType.STICK, 
                BlockType.AIR, BlockType.STICK
            }),
            BlockType.STONE_AXE, 1, true
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(1, 3, new BlockType[]{BlockType.COBBLESTONE, BlockType.STICK, BlockType.STICK}),
            BlockType.STONE_SHOVEL, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(1, 3, new BlockType[]{BlockType.COBBLESTONE, BlockType.COBBLESTONE, BlockType.STICK}),
            BlockType.STONE_SWORD, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(2, 3, new BlockType[]{
                BlockType.COBBLESTONE, BlockType.COBBLESTONE, 
                BlockType.AIR, BlockType.STICK, 
                BlockType.AIR, BlockType.STICK
            }),
            BlockType.STONE_HOE, 1, true
        ));
        
        // === Iron tools ===
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(3, 3, new BlockType[]{
                BlockType.IRON_INGOT, BlockType.IRON_INGOT, BlockType.IRON_INGOT, 
                BlockType.AIR, BlockType.STICK, BlockType.AIR, 
                BlockType.AIR, BlockType.STICK, BlockType.AIR
            }),
            BlockType.IRON_PICKAXE, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(2, 3, new BlockType[]{
                BlockType.IRON_INGOT, BlockType.IRON_INGOT, 
                BlockType.IRON_INGOT, BlockType.STICK, 
                BlockType.AIR, BlockType.STICK
            }),
            BlockType.IRON_AXE, 1, true
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(1, 3, new BlockType[]{BlockType.IRON_INGOT, BlockType.STICK, BlockType.STICK}),
            BlockType.IRON_SHOVEL, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(1, 3, new BlockType[]{BlockType.IRON_INGOT, BlockType.IRON_INGOT, BlockType.STICK}),
            BlockType.IRON_SWORD, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(2, 3, new BlockType[]{
                BlockType.IRON_INGOT, BlockType.IRON_INGOT, 
                BlockType.AIR, BlockType.STICK, 
                BlockType.AIR, BlockType.STICK
            }),
            BlockType.IRON_HOE, 1, true
        ));
        
        // === Diamond tools ===
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(3, 3, new BlockType[]{
                BlockType.DIAMOND, BlockType.DIAMOND, BlockType.DIAMOND, 
                BlockType.AIR, BlockType.STICK, BlockType.AIR, 
                BlockType.AIR, BlockType.STICK, BlockType.AIR
            }),
            BlockType.DIAMOND_PICKAXE, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(2, 3, new BlockType[]{
                BlockType.DIAMOND, BlockType.DIAMOND, 
                BlockType.DIAMOND, BlockType.STICK, 
                BlockType.AIR, BlockType.STICK
            }),
            BlockType.DIAMOND_AXE, 1, true
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(1, 3, new BlockType[]{BlockType.DIAMOND, BlockType.STICK, BlockType.STICK}),
            BlockType.DIAMOND_SHOVEL, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(1, 3, new BlockType[]{BlockType.DIAMOND, BlockType.DIAMOND, BlockType.STICK}),
            BlockType.DIAMOND_SWORD, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(2, 3, new BlockType[]{
                BlockType.DIAMOND, BlockType.DIAMOND, 
                BlockType.AIR, BlockType.STICK, 
                BlockType.AIR, BlockType.STICK
            }),
            BlockType.DIAMOND_HOE, 1, true
        ));
        
        // === Gold tools ===
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(3, 3, new BlockType[]{
                BlockType.GOLD_INGOT, BlockType.GOLD_INGOT, BlockType.GOLD_INGOT, 
                BlockType.AIR, BlockType.STICK, BlockType.AIR, 
                BlockType.AIR, BlockType.STICK, BlockType.AIR
            }),
            BlockType.GOLD_PICKAXE, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(2, 3, new BlockType[]{
                BlockType.GOLD_INGOT, BlockType.GOLD_INGOT, 
                BlockType.GOLD_INGOT, BlockType.STICK, 
                BlockType.AIR, BlockType.STICK
            }),
            BlockType.GOLD_AXE, 1, true
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(1, 3, new BlockType[]{BlockType.GOLD_INGOT, BlockType.STICK, BlockType.STICK}),
            BlockType.GOLD_SHOVEL, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(1, 3, new BlockType[]{BlockType.GOLD_INGOT, BlockType.GOLD_INGOT, BlockType.STICK}),
            BlockType.GOLD_SWORD, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(2, 3, new BlockType[]{
                BlockType.GOLD_INGOT, BlockType.GOLD_INGOT, 
                BlockType.AIR, BlockType.STICK, 
                BlockType.AIR, BlockType.STICK
            }),
            BlockType.GOLD_HOE, 1, true
        ));
        
        // === Blocks ===
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(3, 3, new BlockType[]{
                BlockType.IRON_INGOT, BlockType.IRON_INGOT, BlockType.IRON_INGOT, 
                BlockType.IRON_INGOT, BlockType.IRON_INGOT, BlockType.IRON_INGOT, 
                BlockType.IRON_INGOT, BlockType.IRON_INGOT, BlockType.IRON_INGOT
            }),
            BlockType.IRON_BLOCK, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shapeless(new BlockType[]{BlockType.IRON_BLOCK}),
            BlockType.IRON_INGOT, 9, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(3, 3, new BlockType[]{
                BlockType.LAPIS_LAZULI, BlockType.LAPIS_LAZULI, BlockType.LAPIS_LAZULI, 
                BlockType.LAPIS_LAZULI, BlockType.LAPIS_LAZULI, BlockType.LAPIS_LAZULI, 
                BlockType.LAPIS_LAZULI, BlockType.LAPIS_LAZULI, BlockType.LAPIS_LAZULI
            }),
            BlockType.LAPIS_BLOCK, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shapeless(new BlockType[]{BlockType.LAPIS_BLOCK}),
            BlockType.LAPIS_LAZULI, 9, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(2, 2, new BlockType[]{
                BlockType.SAND, BlockType.SAND, 
                BlockType.SAND, BlockType.SAND
            }),
            BlockType.SANDSTONE, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(2, 2, new BlockType[]{
                BlockType.STONE, BlockType.STONE, 
                BlockType.STONE, BlockType.STONE
            }),
            BlockType.STONE_BRICK, 4, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(3, 3, new BlockType[]{
                BlockType.GUNPOWDER, BlockType.SAND, BlockType.GUNPOWDER, 
                BlockType.SAND, BlockType.GUNPOWDER, BlockType.SAND, 
                BlockType.GUNPOWDER, BlockType.SAND, BlockType.GUNPOWDER
            }),
            BlockType.TNT, 1, false
        ));
        RECIPES.add(new Recipe(
            new RecipeShape.Shaped(3, 3, new BlockType[]{
                BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, 
                BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, 
                BlockType.OAK_PLANKS, BlockType.OAK_PLANKS, BlockType.OAK_PLANKS
            }),
            BlockType.BOOKSHELF, 1, false
        ));
    }

    public static Recipe findRecipe(ItemStack[] grid, int gridWidth, int gridHeight) {
        BlockType[] blocks = new BlockType[grid.length];
        for (int i = 0; i < grid.length; i++) {
            blocks[i] = (grid[i] != null) ? grid[i].block : BlockType.AIR;
        }

        int[] bbox = boundingBox(blocks, gridWidth, gridHeight);
        int minX = bbox[0], minY = bbox[1], maxX = bbox[2], maxY = bbox[3];
        if (minX > maxX) {
            return null; // Empty
        }

        int contentW = maxX - minX + 1;
        int contentH = maxY - minY + 1;

        for (Recipe recipe : RECIPES) {
            if (recipe.shape instanceof RecipeShape.Shaped shaped) {
                int rw = shaped.width;
                int rh = shaped.height;

                if (contentW != rw || contentH != rh) {
                    continue;
                }

                if (matchesShaped(blocks, gridWidth, minX, minY, rw, rh, shaped.pattern)) {
                    return recipe;
                }

                if (recipe.mirror) {
                    BlockType[] mirrored = mirrorPattern(shaped.pattern, rw, rh);
                    if (matchesShaped(blocks, gridWidth, minX, minY, rw, rh, mirrored)) {
                        return recipe;
                    }
                }
            } else if (recipe.shape instanceof RecipeShape.Shapeless shapeless) {
                if (matchesShapeless(blocks, shapeless.ingredients)) {
                    return recipe;
                }
            }
        }
        return null;
    }

    private static int[] boundingBox(BlockType[] grid, int width, int height) {
        int minX = Integer.MAX_VALUE;
        int minY = Integer.MAX_VALUE;
        int maxX = -1;
        int maxY = -1;

        for (int y = 0; y < height; y++) {
            for (int x = 0; x < width; x++) {
                if (grid[y * width + x] != BlockType.AIR) {
                    if (x < minX) minX = x;
                    if (y < minY) minY = y;
                    if (x > maxX) maxX = x;
                    if (y > maxY) maxY = y;
                }
            }
        }
        return new int[]{minX, minY, maxX, maxY};
    }

    private static boolean matchesShaped(BlockType[] grid, int gridW, int offX, int offY, int rw, int rh, BlockType[] pattern) {
        for (int ry = 0; ry < rh; ry++) {
            for (int rx = 0; rx < rw; rx++) {
                int gridIdx = (offY + ry) * gridW + (offX + rx);
                BlockType recipeBlock = pattern[ry * rw + rx];
                BlockType gridBlock = grid[gridIdx];

                if (recipeBlock == BlockType.AIR) {
                    if (gridBlock != BlockType.AIR) {
                        return false;
                    }
                } else {
                    if (gridBlock != recipeBlock) {
                        return false;
                    }
                }
            }
        }
        return true;
    }

    private static BlockType[] mirrorPattern(BlockType[] pattern, int w, int h) {
        BlockType[] mirrored = new BlockType[w * h];
        for (int y = 0; y < h; y++) {
            for (int x = 0; x < w; x++) {
                mirrored[y * w + x] = pattern[y * w + (w - 1 - x)];
            }
        }
        return mirrored;
    }

    private static boolean matchesShapeless(BlockType[] grid, BlockType[] ingredients) {
        List<BlockType> gridItems = new ArrayList<>();
        for (BlockType b : grid) {
            if (b != BlockType.AIR) {
                gridItems.add(b);
            }
        }
        if (gridItems.size() != ingredients.length) {
            return false;
        }
        List<BlockType> remaining = new ArrayList<>();
        for (BlockType ing : ingredients) {
            remaining.add(ing);
        }
        for (BlockType item : gridItems) {
            int idx = remaining.indexOf(item);
            if (idx >= 0) {
                remaining.remove(idx);
            } else {
                return false;
            }
        }
        return remaining.isEmpty();
    }
}
