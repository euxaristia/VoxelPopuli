package com.voxelpopuli.client.renderer;

import com.voxelpopuli.client.noise.Noise;

public class TextureGenerator {

    private static void sp(byte[] d, int x, int y, int r, int g, int b, int a) {
        if (x >= 0 && x < 256 && y >= 0 && y < 256) {
            int i = (y * 256 + x) * 4;
            d[i] = (byte) r;
            d[i + 1] = (byte) g;
            d[i + 2] = (byte) b;
            d[i + 3] = (byte) a;
        }
    }

    private static void nb(byte[] d, int tx, int ty, int r, int g, int b) {
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 20.0f;
                int nr = Math.clamp(r + (int) n, 0, 255);
                int ng = Math.clamp(g + (int) n, 0, 255);
                int nb = Math.clamp(b + (int) n, 0, 255);
                sp(d, tx * 16 + x, ty * 16 + y, nr, ng, nb, 255);
            }
        }
    }

    private static void drawPredefined(byte[] d, int tx, int ty, int[] packedPixels, boolean useAlpha) {
        for (int y = 0; y < 16; y++) {
            for (int x = 0; x < 16; x++) {
                int packed = packedPixels[y * 16 + x];
                int a = (packed >> 24) & 0xFF;
                int r = (packed >> 16) & 0xFF;
                int g = (packed >> 8) & 0xFF;
                int b = packed & 0xFF;
                if (!useAlpha || a > 0) {
                    sp(d, tx * 16 + x, ty * 16 + y, r, g, b, useAlpha ? a : 255);
                }
            }
        }
    }

    public static byte[] generateAtlasData() {
        byte[] data = new byte[256 * 256 * 4];

        // === ORIGINAL BLOCKS (Row 0 & 1) ===
        nb(data, 1, 0, 140, 140, 140); // Stone
        nb(data, 2, 0, 130, 90, 60);   // Dirt
        nb(data, 3, 0, 80, 160, 40);   // Grass Top
        nb(data, 0, 0, 100, 180, 50);  // Grass Side
        nb(data, 4, 0, 100, 70, 40);   // Oak Log Side
        nb(data, 5, 1, 110, 80, 50);   // Oak Log Top
        nb(data, 5, 0, 40, 120, 30);   // Oak Leaves
        nb(data, 7, 0, 120, 120, 130); // Bedrock
        nb(data, 1, 1, 60, 60, 60);    // Coal Ore base
        nb(data, 2, 1, 140, 140, 140); // Powdered Snow
        nb(data, 13, 12, 40, 80, 200); // Water
        nb(data, 8, 0, 240, 245, 250); // Snow
        nb(data, 9, 0, 240, 245, 250); // SnowyGrass Top
        nb(data, 11, 0, 60, 40, 25);   // Spruce Log Side
        nb(data, 12, 0, 30, 80, 30);   // Spruce Leaves
        nb(data, 6, 1, 230, 230, 230); // Iron Block
        nb(data, 7, 1, 210, 160, 130); // Raw Iron Item
        nb(data, 8, 1, 245, 245, 245); // Iron Ingot Item

        // Flint & Steel Item
        drawPredefined(data, 9, 1, TextureArrays.FS_P, true);

        // Iron Ore
        nb(data, 3, 1, 140, 140, 140); // base stone
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                if ((x * 7 + y * 3) % 11 == 0) {
                    sp(data, 3 * 16 + x, 16 + y, 220, 180, 150, 255);
                }
            }
        }

        // TNT
        drawPredefined(data, 4, 1, TextureArrays.TNT_PIXELS, false);

        // Nuke
        drawPredefined(data, 6, 7, TextureArrays.NUKE_PIXELS, false);

        // SnowyGrass Side: dirt with white snow strip on top
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 20.0f;
                int r, g, b;
                if (y < 3) {
                    r = Math.clamp(240 + (int) n, 0, 255);
                    g = Math.clamp(245 + (int) n, 0, 255);
                    b = Math.clamp(250 + (int) n, 0, 255);
                } else {
                    r = Math.clamp(130 + (int) n, 0, 255);
                    g = Math.clamp(90 + (int) n, 0, 255);
                    b = Math.clamp(60 + (int) n, 0, 255);
                }
                sp(data, 10 * 16 + x, y, r, g, b, 255);
            }
        }

        // Sand
        drawPredefined(data, 6, 0, TextureArrays.SAND_PIXELS, false);

        // === NEW BLOCK TEXTURES (Phase 2) ===
        // Cobblestone (0,2)
        nb(data, 0, 2, 125, 125, 125);
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                if ((x * 5 + y * 7) % 13 == 0 || (x * 3 + y * 11) % 17 == 0) {
                    float n = Noise.noise2d(x * 0.3f, y * 0.3f) * 15.0f;
                    int v = Math.clamp(100 + (int) n, 0, 255);
                    sp(data, x, 2 * 16 + y, v, v, v, 255);
                }
            }
        }

        // Oak Planks (1,2)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                float n = Noise.noise2d(x * 0.8f, y * 0.15f) * 15.0f;
                int l = (y % 4 == 0) ? -15 : 0;
                int r = Math.clamp(180 + (int) n + l, 0, 255);
                int g = Math.clamp(140 + (int) n + l, 0, 255);
                int b = Math.clamp(90 + (int) n + l, 0, 255);
                sp(data, 16 + x, 2 * 16 + y, r, g, b, 255);
            }
        }

        // Crafting Table Top (2,2)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 10.0f;
                boolean g = x == 0 || y == 0 || x == 8 || y == 8;
                int r, gr, b;
                if (g) {
                    r = 60 + (int) n;
                    gr = 45 + (int) n;
                    b = 30 + (int) n;
                } else {
                    r = 170 + (int) n;
                    gr = 130 + (int) n;
                    b = 80 + (int) n;
                }
                sp(data, 2 * 16 + x, 2 * 16 + y, Math.clamp(r, 0, 255), Math.clamp(gr, 0, 255), Math.clamp(b, 0, 255), 255);
            }
        }

        // Crafting Table Side (3,2)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                float n = Noise.noise2d(x * 0.8f, y * 0.15f) * 15.0f;
                int s = (y < 2) ? -40 : ((y % 4 == 0) ? -15 : 0);
                int r = Math.clamp(170 + (int) n + s, 0, 255);
                int g = Math.clamp(125 + (int) n + s, 0, 255);
                int b = Math.clamp(75 + (int) n + s, 0, 255);
                sp(data, 3 * 16 + x, 2 * 16 + y, r, g, b, 255);
            }
        }

        // Furnace Front (4,2)
        nb(data, 4, 2, 130, 130, 130);
        for (int x = 5; x < 11; x++) {
            for (int y = 6; y < 13; y++) {
                sp(data, 4 * 16 + x, 2 * 16 + y, 30, 20, 15, 255);
            }
        }

        // Furnace Side (5,2)
        nb(data, 5, 2, 135, 135, 135);

        // Glass (6,2)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                boolean edge = x == 0 || x == 15 || y == 0 || y == 15;
                boolean inner = x == 1 || x == 14 || y == 1 || y == 14;
                if (edge) {
                    sp(data, 6 * 16 + x, 2 * 16 + y, 180, 210, 230, 200);
                } else if (inner) {
                    sp(data, 6 * 16 + x, 2 * 16 + y, 200, 225, 240, 100);
                } else {
                    sp(data, 6 * 16 + x, 2 * 16 + y, 220, 235, 248, 40);
                }
            }
        }

        // Gold Ore (7,2)
        nb(data, 7, 2, 140, 140, 140);
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                if ((x * 7 + y * 5) % 11 == 0) {
                    sp(data, 7 * 16 + x, 2 * 16 + y, 250, 220, 50, 255);
                }
            }
        }

        // Diamond Ore (8,2)
        nb(data, 8, 2, 140, 140, 140);
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                if ((x * 7 + y * 3) % 11 == 0) {
                    sp(data, 8 * 16 + x, 2 * 16 + y, 80, 220, 230, 255);
                }
            }
        }

        // Obsidian (9,2)
        nb(data, 9, 2, 20, 15, 30);

        // Brick (10,2)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 10.0f;
                boolean m = y % 4 == 0 || (x + (y / 4) * 8) % 8 == 0;
                int r, g, b;
                if (m) {
                    r = 180 + (int) n;
                    g = 170 + (int) n;
                    b = 150 + (int) n;
                } else {
                    r = 170 + (int) n;
                    g = 85 + (int) n;
                    b = 60 + (int) n;
                }
                sp(data, 10 * 16 + x, 2 * 16 + y, Math.clamp(r, 0, 255), Math.clamp(g, 0, 255), Math.clamp(b, 0, 255), 255);
            }
        }

        // Stone Brick (11,2)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 10.0f;
                boolean m = y % 8 == 0 || (x + (y / 8) * 8) % 8 == 0;
                int v = m ? (110 + (int) n) : (145 + (int) n);
                sp(data, 11 * 16 + x, 2 * 16 + y, Math.clamp(v, 0, 255), Math.clamp(v, 0, 255), Math.clamp(v, 0, 255), 255);
            }
        }

        // Sandstone (12,2) and Wool (13,2)
        nb(data, 12, 2, 215, 205, 160);
        nb(data, 13, 2, 235, 235, 235);

        // Bookshelf (14,2)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 8.0f;
                if (y < 2 || y > 13) {
                    sp(data, 14 * 16 + x, 2 * 16 + y, Math.clamp(180 + (int) n, 0, 255), Math.clamp(140 + (int) n, 0, 255), Math.clamp(90 + (int) n, 0, 255), 255);
                } else {
                    int c = (x / 3) % 4;
                    int r, g, b;
                    switch (c) {
                        case 0 -> { r = 150 + (int) n; g = 40 + (int) n; b = 40 + (int) n; }
                        case 1 -> { r = 40 + (int) n; g = 100 + (int) n; b = 40 + (int) n; }
                        case 2 -> { r = 40 + (int) n; g = 40 + (int) n; b = 140 + (int) n; }
                        default -> { r = 130 + (int) n; g = 100 + (int) n; b = 40 + (int) n; }
                    }
                    sp(data, 14 * 16 + x, 2 * 16 + y, Math.clamp(r, 0, 255), Math.clamp(g, 0, 255), Math.clamp(b, 0, 255), 255);
                }
            }
        }

        // Sponge (15,2)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 15.0f;
                boolean h = (x * 5 + y * 7) % 9 == 0;
                int r, g, b;
                if (h) {
                    r = 160 + (int) n;
                    g = 150 + (int) n;
                    b = 50 + (int) n;
                } else {
                    r = 210 + (int) n;
                    g = 200 + (int) n;
                    b = 80 + (int) n;
                }
                sp(data, 15 * 16 + x, 2 * 16 + y, Math.clamp(r, 0, 255), Math.clamp(g, 0, 255), Math.clamp(b, 0, 255), 255);
            }
        }

        // Chest Side (0,7)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 10.0f;
                boolean latch = x >= 6 && x <= 9 && y >= 5 && y <= 8;
                boolean edge = x == 0 || x == 15 || y == 0 || y == 15;
                int r, g, b;
                if (latch) {
                    r = 80; g = 80; b = 80;
                } else if (edge) {
                    r = 100 + (int) n; g = 70 + (int) n; b = 40 + (int) n;
                } else {
                    r = 160 + (int) n; g = 120 + (int) n; b = 60 + (int) n;
                }
                sp(data, x, 7 * 16 + y, Math.clamp(r, 0, 255), Math.clamp(g, 0, 255), Math.clamp(b, 0, 255), 255);
            }
        }

        // Chest Top (1,7) and Mossy Cobblestone (2,7)
        nb(data, 1, 7, 150, 110, 55);
        nb(data, 2, 7, 125, 125, 125);
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                if ((x * 3 + y * 7) % 5 == 0) {
                    float n = Noise.noise2d(x * 0.8f, y * 0.8f) * 10.0f;
                    int r = Math.clamp(50 + (int) n, 0, 255);
                    int g = Math.clamp(110 + (int) n, 0, 255);
                    int b = Math.clamp(40 + (int) n, 0, 255);
                    sp(data, 2 * 16 + x, 7 * 16 + y, r, g, b, 255);
                }
            }
        }

        // Lapis Block (3,7) and Lapis Ore (4,7)
        nb(data, 3, 7, 30, 50, 160);
        nb(data, 4, 7, 140, 140, 140);
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                if ((x * 7 + y * 3) % 11 == 0) {
                    sp(data, 4 * 16 + x, 7 * 16 + y, 30, 60, 180, 255);
                }
            }
        }

        // Torch (5,7)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                boolean flame = x >= 6 && x <= 9 && y >= 2 && y <= 5;
                boolean stick = x >= 7 && x <= 8 && y >= 6 && y <= 14;
                if (flame) {
                    sp(data, 5 * 16 + x, 7 * 16 + y, 255, 200, 50, 255);
                } else if (stick) {
                    sp(data, 5 * 16 + x, 7 * 16 + y, 130, 90, 50, 255);
                }
            }
        }

        // === MATERIAL ITEM SPRITES (Row 4) ===
        // Stick (0,4)
        for (int i = 0; i < 12; i++) {
            sp(data, 4 + i, 4 * 16 + 3 + i, 130, 90, 50, 255);
            sp(data, 5 + i, 4 * 16 + 3 + i, 110, 75, 40, 255);
        }

        // Coal (1,4)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                int d = (x - 8) * (x - 8) + (y - 8) * (y - 8);
                if (d < 30) {
                    float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 15.0f;
                    int v = Math.clamp(25 + (int) n, 0, 255);
                    sp(data, 16 + x, 4 * 16 + y, v, v, v, 255);
                }
            }
        }

        // Diamond (2,4)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                if (Math.abs(x - 8) + Math.abs(y - 8) < 7) {
                    float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 15.0f;
                    int r = Math.clamp(80 + (int) n, 0, 255);
                    int g = Math.clamp(220 + (int) n, 0, 255);
                    int b = Math.clamp(230 + (int) n, 0, 255);
                    sp(data, 2 * 16 + x, 4 * 16 + y, r, g, b, 255);
                }
            }
        }

        // Gold Ingot (3,4)
        for (int x = 2; x < 14; x++) {
            for (int y = 5; y < 11; y++) {
                float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 10.0f;
                int r, g, b;
                if (y < 7) {
                    r = 255;
                    g = 230 + (int) n;
                    b = 50 + (int) n;
                } else {
                    r = 220 + (int) n;
                    g = 190 + (int) n;
                    b = 30 + (int) n;
                }
                sp(data, 3 * 16 + x, 4 * 16 + y, r, Math.clamp(g, 0, 255), Math.clamp(b, 0, 255), 255);
            }
        }

        // Lapis Lazuli (4,4)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                if ((x - 8) * (x - 8) + (y - 8) * (y - 8) < 28) {
                    float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 15.0f;
                    int r = Math.clamp(30 + (int) n, 0, 255);
                    int g = Math.clamp(60 + (int) n, 0, 255);
                    int b = Math.clamp(180 + (int) n, 0, 255);
                    sp(data, 4 * 16 + x, 4 * 16 + y, r, g, b, 255);
                }
            }
        }

        // String (5,4)
        for (int y = 2; y < 14; y++) {
            sp(data, 5 * 16 + 8, 4 * 16 + y, 230, 230, 230, 255);
            if (y % 3 == 0) {
                sp(data, 5 * 16 + 7, 4 * 16 + y, 200, 200, 200, 255);
            }
        }

        // Gunpowder (6,4)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                if ((x * 3 + y * 7 + 5) % 4 < 2 && (x - 8) * (x - 8) + (y - 8) * (y - 8) < 35) {
                    float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 10.0f;
                    int v = Math.clamp(60 + (int) n, 0, 255);
                    sp(data, 6 * 16 + x, 4 * 16 + y, v, v, v, 255);
                }
            }
        }

        // Leather (7,4)
        for (int x = 0; x < 16; x++) {
            for (int y = 0; y < 16; y++) {
                if (Math.abs(x - 8) < 5 && Math.abs(y - 8) < 6) {
                    float n = Noise.noise2d(x * 0.5f, y * 0.5f) * 12.0f;
                    int r = Math.clamp(160 + (int) n, 0, 255);
                    int g = Math.clamp(100 + (int) n, 0, 255);
                    int b = Math.clamp(50 + (int) n, 0, 255);
                    sp(data, 7 * 16 + x, 4 * 16 + y, r, g, b, 255);
                }
            }
        }

        // === TOOL SPRITES ===
        int[][] matColors = {
            {160, 120, 60},  // Wood
            {130, 130, 130}, // Stone
            {210, 210, 210}, // Iron
            {80, 220, 230},  // Diamond
            {250, 220, 50}   // Gold
        };

        // Slots mapping for tools:
        // tx, ty, mat, shape (0: Pickaxe, 1: Axe, 2: Shovel, 3: Sword, 4: Hoe)
        int[][] toolLayout = {
            {0, 5, 0, 0}, {1, 5, 0, 1}, {2, 5, 0, 2}, {3, 5, 0, 3}, {4, 5, 0, 4}, // Wood
            {5, 5, 1, 0}, {6, 5, 1, 1}, {7, 5, 1, 2}, {8, 5, 1, 3}, {9, 5, 1, 4}, // Stone
            {0, 6, 2, 0}, {1, 6, 2, 1}, {2, 6, 2, 2}, {3, 6, 2, 3}, {4, 6, 2, 4}, // Iron
            {5, 6, 3, 0}, {6, 6, 3, 1}, {7, 6, 3, 2}, {8, 6, 3, 3}, {9, 6, 3, 4}, // Diamond
            {10, 6, 4, 0}, {11, 6, 4, 1}, {12, 6, 4, 2}, {13, 6, 4, 3}, {14, 6, 4, 4} // Gold
        };

        for (int[] tool : toolLayout) {
            int tx = tool[0];
            int ty = tool[1];
            int mat = tool[2];
            int shape = tool[3];

            int hr = matColors[mat][0];
            int hg = matColors[mat][1];
            int hb = matColors[mat][2];

            // Draw Stick helper
            if (shape != 3) { // Sword draws its own handle
                for (int i = 0; i < 7; i++) {
                    sp(data, tx * 16 + 3 + i, ty * 16 + 15 - i, 130, 90, 50, 255);
                    sp(data, tx * 16 + 4 + i, ty * 16 + 15 - i, 110, 75, 40, 255);
                }
            }

            switch (shape) {
                case 0 -> { // Pickaxe
                    for (int x = 2; x < 14; x++) {
                        sp(data, tx * 16 + x, ty * 16 + 2, hr, hg, hb, 255);
                        sp(data, tx * 16 + x, ty * 16 + 3, hr, hg, hb, 255);
                    }
                    sp(data, tx * 16 + 2, ty * 16 + 4, hr, hg, hb, 255);
                    sp(data, tx * 16 + 3, ty * 16 + 4, hr, hg, hb, 255);
                    sp(data, tx * 16 + 12, ty * 16 + 4, hr, hg, hb, 255);
                    sp(data, tx * 16 + 13, ty * 16 + 4, hr, hg, hb, 255);
                }
                case 1 -> { // Axe
                    for (int x = 3; x < 9; x++) {
                        sp(data, tx * 16 + x, ty * 16 + 1, hr, hg, hb, 255);
                        sp(data, tx * 16 + x, ty * 16 + 2, hr, hg, hb, 255);
                    }
                    for (int y = 3; y < 7; y++) {
                        sp(data, tx * 16 + 3, ty * 16 + y, hr, hg, hb, 255);
                        sp(data, tx * 16 + 4, ty * 16 + y, hr, hg, hb, 255);
                    }
                    sp(data, tx * 16 + 5, ty * 16 + 3, hr, hg, hb, 255);
                    sp(data, tx * 16 + 5, ty * 16 + 4, hr, hg, hb, 255);
                }
                case 2 -> { // Shovel
                    for (int y = 1; y < 6; y++) {
                        sp(data, tx * 16 + 7, ty * 16 + y, hr, hg, hb, 255);
                        sp(data, tx * 16 + 8, ty * 16 + y, hr, hg, hb, 255);
                        sp(data, tx * 16 + 9, ty * 16 + y, hr, hg, hb, 255);
                    }
                    sp(data, tx * 16 + 8, ty * 16, hr, hg, hb, 255);
                }
                case 3 -> { // Sword
                    for (int x = 5; x < 11; x++) {
                        sp(data, tx * 16 + x, ty * 16 + 8, 80, 80, 80, 255);
                    }
                    for (int y = 1; y < 8; y++) {
                        sp(data, tx * 16 + 7, ty * 16 + y, hr, hg, hb, 255);
                        sp(data, tx * 16 + 8, ty * 16 + y, hr, hg, hb, 255);
                    }
                    sp(data, tx * 16 + 8, ty * 16, hr, hg, hb, 255);
                    for (int y = 9; y < 14; y++) {
                        sp(data, tx * 16 + 7, ty * 16 + y, 130, 90, 50, 255);
                        sp(data, tx * 16 + 8, ty * 16 + y, 110, 75, 40, 255);
                    }
                }
                case 4 -> { // Hoe
                    for (int x = 8; x < 14; x++) {
                        sp(data, tx * 16 + x, ty * 16 + 1, hr, hg, hb, 255);
                        sp(data, tx * 16 + x, ty * 16 + 2, hr, hg, hb, 255);
                    }
                    sp(data, tx * 16 + 8, ty * 16 + 3, hr, hg, hb, 255);
                    sp(data, tx * 16 + 9, ty * 16 + 3, hr, hg, hb, 255);
                }
            }
        }

        // === CRACK STAGE TEXTURES (Row 3, columns 0-9) ===
        for (int stage = 0; stage < 10; stage++) {
            float density = (stage + 1) / 12.0f;
            for (int x = 0; x < 16; x++) {
                for (int y = 0; y < 16; y++) {
                    int hash = (x * 374761393 + y * 668265263 + stage * 1274126177);
                    hash = hash * 1103515245 + 12345;
                    float frac = Math.abs(hash % 1000) / 1000.0f;
                    if (frac < density) {
                        int alpha = Math.min(255, 80 + stage * 18);
                        sp(data, stage * 16 + x, 3 * 16 + y, 0, 0, 0, alpha);
                    }
                }
            }
        }

        return data;
    }
}
