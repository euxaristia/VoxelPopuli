package com.voxelpopuli.client.world;

import com.voxelpopuli.client.block.BlockType;
import com.voxelpopuli.client.noise.Noise;
import com.voxelpopuli.client.renderer.Mesh;
import java.util.ArrayDeque;
import java.util.Queue;
import java.util.Random;

public class Chunk {
    public static final int WIDTH = 16;
    public static final int HEIGHT = 256;
    public static final int DEPTH = 16;

    public static final byte WATER_SOURCE = 1;
    public static final byte WATER_FALL_BIT = 8;
    public static final byte WATER_VERTEX_ALPHA = (byte) 180;

    // Flat arrays using fast index: (x << 12) | (y << 4) | z
    public final BlockType[] blocks = new BlockType[WIDTH * HEIGHT * DEPTH];
    public final byte[] light = new byte[WIDTH * HEIGHT * DEPTH];
    public final byte[] liquidLevels = new byte[WIDTH * HEIGHT * DEPTH];

    public Mesh meshOpaque;
    public Mesh meshTransparent;
    public Mesh meshWater;

    public MeshData pendingOpaque;
    public MeshData pendingTransparent;
    public MeshData pendingWater;

    public boolean dirty = true;
    public volatile boolean meshingInProgress = false;
    public final int x;
    public final int z;

    public Chunk(int x, int z) {
        this.x = x;
        this.z = z;
        java.util.Arrays.fill(blocks, BlockType.AIR);
    }

    private int getIndex(int x, int y, int z) {
        return (x << 12) | (y << 4) | z;
    }

    public BlockType getBlock(int x, int y, int z) {
        return blocks[(x << 12) | (y << 4) | z];
    }

    public void setBlock(int x, int y, int z, BlockType block) {
        blocks[(x << 12) | (y << 4) | z] = block;
        if (block == BlockType.WATER) {
            liquidLevels[(x << 12) | (y << 4) | z] = WATER_SOURCE;
        } else {
            liquidLevels[(x << 12) | (y << 4) | z] = 0;
        }
        dirty = true;
    }

    public byte getLight(int x, int y, int z) {
        return light[(x << 12) | (y << 4) | z];
    }

    public void setLight(int x, int y, int z, byte val) {
        light[(x << 12) | (y << 4) | z] = val;
        dirty = true;
    }

    public byte getLiquidLevel(int x, int y, int z) {
        return liquidLevels[(x << 12) | (y << 4) | z];
    }

    public void setLiquidLevel(int x, int y, int z, byte val) {
        liquidLevels[(x << 12) | (y << 4) | z] = val;
        dirty = true;
    }

    public static Biome getBiome(float worldX, float worldZ) {
        float temperature = Noise.perlin2d(worldX + 5000.0f, worldZ + 5000.0f, 0.002f, 2);
        float humidity = Noise.perlin2d(worldX + 10000.0f, worldZ + 10000.0f, 0.002f, 2);
        float mountainNoise = Noise.perlin2d(worldX + 20000.0f, worldZ + 20000.0f, 0.001f, 3);
        float hillsNoise = Noise.perlin2d(worldX + 30000.0f, worldZ + 30000.0f, 0.005f, 2);

        if (mountainNoise > 0.45f) {
            return Biome.MOUNTAINS;
        } else if (hillsNoise > 0.35f) {
            return Biome.HIGH_HILLS;
        } else if (temperature < -0.15f) {
            if (humidity > 0.0f) {
                return Biome.SNOWY_TAIGA;
            } else {
                return Biome.SNOWY_TUNDRA;
            }
        } else if (temperature > 0.15f) {
            return Biome.DESERT;
        } else {
            return Biome.PLAINS;
        }
    }

    public static boolean waterIsSource(byte level) {
        return level == WATER_SOURCE;
    }

    public static boolean waterIsFalling(byte level) {
        return level >= 9 && level <= 16;
    }

    public static byte waterDecay(byte level) {
        if (level == 0) return 0;
        return (byte) ((level - 1) & 7);
    }

    public static float waterHeight(byte level) {
        if (level == 0) return 0.0f;
        if (level > WATER_FALL_BIT) return 1.0f;
        int decay = waterDecay(level);
        return (8 - decay) / 9.0f;
    }

    public static float waterRenderHeight(byte level) {
        if (level == 0) return 0.0f;
        if (level > WATER_FALL_BIT || level == WATER_SOURCE) return 1.0f;
        int decay = waterDecay(level);
        return (8 - decay) / 9.0f;
    }

    public void generate() {
        long chunkSeed = 42L
            ^ (0x9E3779B97F4A7C15L * (long) this.x)
            ^ (0xC2B2AE3D27D4EB4FL * (long) this.z);
        Random rng = new Random(chunkSeed);

        // PASS 1: Terrain (biome-aware)
        for (int x = 0; x < WIDTH; x++) {
            for (int z = 0; z < DEPTH; z++) {
                float worldX = (this.x * WIDTH + x);
                float worldZ = (this.z * DEPTH + z);
                Biome biome = getBiome(worldX, worldZ);

                float continental = Noise.perlin2d(worldX, worldZ, 0.005f, 3);
                float detail = Noise.perlin2d(worldX, worldZ, 0.03f, 4);

                float baseH;
                float heightScale;

                switch (biome) {
                    case MOUNTAINS -> {
                        float ridged = Noise.perlin2d(worldX, worldZ, 0.01f, 3);
                        ridged = 1.0f - Math.abs(ridged);
                        ridged = ridged * ridged;
                        baseH = continental * 50.0f + 140.0f + ridged * 60.0f;
                        heightScale = 30.0f;
                    }
                    case HIGH_HILLS -> {
                        baseH = continental * 40.0f + 135.0f;
                        heightScale = 25.0f;
                    }
                    default -> {
                        baseH = continental * 40.0f + 128.0f;
                        heightScale = 15.0f;
                    }
                }

                int height = (int) (baseH + detail * heightScale);

                // Fjord carving
                if ((biome == Biome.MOUNTAINS || biome == Biome.HIGH_HILLS) && continental < 0.1f) {
                    float fjordNoise = Math.abs(Noise.perlin2d(worldX, worldZ, 0.02f, 2));
                    if (fjordNoise < 0.15f) {
                        float depthFactor = (0.15f - fjordNoise) / 0.15f;
                        height = (int) (height * (1.0f - depthFactor) + 110.0f * depthFactor);
                    }
                }

                height = Math.clamp(height, 0, HEIGHT - 1);

                for (int y = 0; y < HEIGHT; y++) {
                    BlockType b;
                    if (y == 0) {
                        b = BlockType.BEDROCK;
                    } else if (y < height - 4) {
                        b = BlockType.STONE;
                    } else if (y < height) {
                        if (biome == Biome.DESERT) {
                            b = BlockType.SAND;
                        } else if (biome == Biome.MOUNTAINS) {
                            b = (y > 180) ? BlockType.STONE : BlockType.DIRT;
                        } else {
                            b = (height < 126) ? BlockType.SAND : BlockType.DIRT;
                        }
                    } else if (y == height) {
                        if (height < 126) {
                            b = BlockType.SAND;
                        } else {
                            switch (biome) {
                                case DESERT -> b = BlockType.SAND;
                                case SNOWY_TUNDRA, SNOWY_TAIGA -> b = BlockType.SNOWY_GRASS;
                                case MOUNTAINS -> {
                                    if (height > 185) {
                                        b = BlockType.POWDERED_SNOW;
                                    } else if (height > 160) {
                                        b = BlockType.STONE;
                                    } else {
                                        b = BlockType.GRASS;
                                    }
                                }
                                default -> b = BlockType.GRASS;
                            }
                        }
                    } else if (y < 124) {
                        b = BlockType.WATER;
                    } else {
                        b = BlockType.AIR;
                    }
                    blocks[getIndex(x, y, z)] = b;
                    if (b == BlockType.WATER) {
                        liquidLevels[getIndex(x, y, z)] = WATER_SOURCE;
                    }
                }
            }
        }

        // PASS 2: Coal Ores
        for (int i = 0; i < 150; i++) {
            int cx = rng.nextInt(WIDTH);
            int cy = rng.nextInt(HEIGHT);
            int cz = rng.nextInt(DEPTH);
            int veinSize = rng.nextInt(17) + 1;
            for (int v = 0; v < veinSize; v++) {
                if (cx >= 0 && cx < WIDTH && cz >= 0 && cz < DEPTH && cy >= 0 && cy < HEIGHT) {
                    int idx = getIndex(cx, cy, cz);
                    if (blocks[idx] == BlockType.STONE) {
                        blocks[idx] = BlockType.COAL_ORE;
                    }
                }
                int dir = rng.nextInt(6);
                switch (dir) {
                    case 0 -> cx++;
                    case 1 -> cx--;
                    case 2 -> cy++;
                    case 3 -> cy--;
                    case 4 -> cz++;
                    case 5 -> cz--;
                }
            }
        }

        // PASS 3: Iron Ores
        for (int i = 0; i < 80; i++) {
            int cx = rng.nextInt(WIDTH);
            int cy = rng.nextInt(64);
            int cz = rng.nextInt(DEPTH);
            int veinSize = rng.nextInt(9) + 1;
            for (int v = 0; v < veinSize; v++) {
                if (cx >= 0 && cx < WIDTH && cz >= 0 && cz < DEPTH && cy >= 0 && cy < HEIGHT) {
                    int idx = getIndex(cx, cy, cz);
                    if (blocks[idx] == BlockType.STONE) {
                        blocks[idx] = BlockType.IRON_ORE;
                    }
                }
                int dir = rng.nextInt(6);
                switch (dir) {
                    case 0 -> cx++;
                    case 1 -> cx--;
                    case 2 -> cy++;
                    case 3 -> cy--;
                    case 4 -> cz++;
                    case 5 -> cz--;
                }
            }
        }

        // PASS 4: Gravel
        for (int i = 0; i < 8; i++) {
            int cx = rng.nextInt(WIDTH);
            int cy = rng.nextInt(64) + 64;
            int cz = rng.nextInt(DEPTH);
            int veinSize = rng.nextInt(32) + 16;
            for (int v = 0; v < veinSize; v++) {
                if (cx >= 0 && cx < WIDTH && cz >= 0 && cz < DEPTH && cy >= 0 && cy < HEIGHT) {
                    int idx = getIndex(cx, cy, cz);
                    BlockType b = blocks[idx];
                    if (b == BlockType.STONE || b == BlockType.DIRT) {
                        blocks[idx] = BlockType.GRAVEL;
                    }
                }
                int dir = rng.nextInt(6);
                switch (dir) {
                    case 0 -> cx++;
                    case 1 -> cx--;
                    case 2 -> cy++;
                    case 3 -> cy--;
                    case 4 -> cz++;
                    case 5 -> cz--;
                }
            }
        }

        // PASS 5: Caves (Perlin Worm)
        int worldSeed = 42;
        java.util.function.Function<Long, Float> caveRng = (s) -> {
            s ^= s << 13;
            s ^= s >>> 17;
            s ^= s << 5;
            long val = Math.abs(s % 10000);
            return val / 10000.0f;
        };

        for (int cx = -1; cx <= 1; cx++) {
            for (int cz = -1; cz <= 1; cz++) {
                int neighborX = this.x + cx;
                int neighborZ = this.z + cz;

                long baseSeed = ((long) worldSeed * 341873128712L) + ((long) neighborX * 132897987541L) + ((long) neighborZ * 341873128712L);
                int numCaves = (int) (caveRng.apply(baseSeed) * 15.0f);

                for (int i = 0; i < numCaves; i++) {
                    long caveSeed = baseSeed + i * 9283711L;

                    float startX = (neighborX * WIDTH) + caveRng.apply(caveSeed) * 16.0f;
                    float startY = caveRng.apply(caveSeed + 1) * 120.0f + 8.0f;
                    float startZ = (neighborZ * DEPTH) + caveRng.apply(caveSeed + 2) * 16.0f;

                    float length = caveRng.apply(caveSeed + 3) * 100.0f + 10.0f;
                    float radius = caveRng.apply(caveSeed + 4) * 2.0f + 1.5f;

                    if (caveRng.apply(caveSeed + 5) < 0.25f) {
                        radius *= 2.0f;
                        length = 0.0f;
                    }

                    if (caveRng.apply(caveSeed + 6) < 0.10f) {
                        radius *= (caveRng.apply(caveSeed + 7) * 2.0f + 1.0f);
                    }

                    float currentX = startX;
                    float currentY = startY;
                    float currentZ = startZ;

                    float yaw = caveRng.apply(caveSeed + 8) * (float) Math.PI * 2.0f;
                    float pitch = (caveRng.apply(caveSeed + 9) - 0.5f) * (float) Math.PI * 0.5f;

                    float step = 0;
                    long seedState = caveSeed + 10;

                    while (step < length || length == 0.0f) {
                        float rad = radius * (1.0f + (float) Math.sin(step / Math.max(1.0f, length)));

                        int minCx = (int) Math.floor(currentX - rad) - (this.x * WIDTH);
                        int maxCx = (int) Math.ceil(currentX + rad) - (this.x * WIDTH);
                        int minCy = (int) Math.floor(currentY - rad);
                        int maxCy = (int) Math.ceil(currentY + rad);
                        int minCz = (int) Math.floor(currentZ - rad) - (this.z * DEPTH);
                        int maxCz = (int) Math.ceil(currentZ + rad) - (this.z * DEPTH);

                        for (int bx = Math.max(0, minCx); bx <= Math.min(WIDTH - 1, maxCx); bx++) {
                            for (int by = Math.max(0, minCy); by <= Math.min(HEIGHT - 1, maxCy); by++) {
                                for (int bz = Math.max(0, minCz); bz <= Math.min(DEPTH - 1, maxCz); bz++) {
                                    float globalX = (this.x * WIDTH + bx);
                                    float globalY = by;
                                    float globalZ = (this.z * DEPTH + bz);

                                    float dx = globalX - currentX;
                                    float dy = (globalY - currentY) * 1.5f;
                                    float dz = globalZ - currentZ;

                                    if (dx * dx + dy * dy + dz * dz <= rad * rad) {
                                        int idx = getIndex(bx, by, bz);
                                        BlockType currentBlock = blocks[idx];
                                        if (currentBlock == BlockType.STONE
                                            || currentBlock == BlockType.DIRT
                                            || currentBlock == BlockType.GRAVEL
                                            || currentBlock == BlockType.GRASS
                                            || currentBlock == BlockType.SNOWY_GRASS
                                            || currentBlock == BlockType.POWDERED_SNOW
                                            || currentBlock == BlockType.SNOW_LAYER) {
                                            blocks[idx] = BlockType.AIR;
                                        }
                                    }
                                }
                            }
                        }

                        if (length == 0.0f) break;

                        currentX += Math.cos(yaw) * Math.cos(pitch);
                        currentY += Math.sin(pitch);
                        currentZ += Math.sin(yaw) * Math.cos(pitch);

                        seedState += 1;
                        yaw += (caveRng.apply(seedState) - 0.5f) * 0.4f;
                        seedState += 1;
                        pitch += (caveRng.apply(seedState) - 0.5f) * 0.4f;
                        pitch = Math.clamp(pitch, -(float) Math.PI * 0.4f, (float) Math.PI * 0.4f);

                        step += 1.0f;
                    }
                }
            }
        }

        // PASS 6: Trees
        java.util.List<int[]> treePositions = new java.util.ArrayList<>();
        int treeAttempts = 0;
        int minDist = 0;

        float cX = (this.x * WIDTH + 8);
        float cZ = (this.z * DEPTH + 8);
        Biome centerBiome = getBiome(cX, cZ);

        if (centerBiome == Biome.PLAINS || centerBiome == Biome.HIGH_HILLS) {
            treeAttempts = 2;
            minDist = 4;
        } else if (centerBiome == Biome.SNOWY_TAIGA) {
            treeAttempts = 8;
            minDist = 5;
        }

        for (int i = 0; i < treeAttempts; i++) {
            int tx = rng.nextInt(WIDTH - 10) + 5;
            int tz = rng.nextInt(DEPTH - 10) + 5;

            boolean tooClose = false;
            for (int[] pos : treePositions) {
                int dx = tx - pos[0];
                int dz = tz - pos[1];
                if (dx * dx + dz * dz < minDist * minDist) {
                    tooClose = true;
                    break;
                }
            }
            if (tooClose) continue;

            float worldX = (this.x * WIDTH + tx);
            float worldZ = (this.z * DEPTH + tz);
            Biome biome = getBiome(worldX, worldZ);
            int seed = (int) (worldX * 73.1f + worldZ * 91.7f + i * 111.1f);

            int surfaceY = 0;
            for (int y = HEIGHT - 33; y >= 60; y--) {
                BlockType b = blocks[getIndex(tx, y, tz)];
                if (b == BlockType.GRASS || b == BlockType.SNOWY_GRASS) {
                    surfaceY = y;
                    break;
                } else if (b != BlockType.AIR && b != BlockType.WATER && b != BlockType.SNOW_LAYER) {
                    break;
                }
            }

            if (surfaceY > 0) {
                if (biome == Biome.PLAINS) {
                    int treeH = 4 + (seed % 3);
                    if (treeH < 0) treeH += 3;
                    for (int h = 1; h <= treeH; h++) {
                        if (surfaceY + h < HEIGHT) {
                            blocks[getIndex(tx, surfaceY + h, tz)] = BlockType.OAK_LOG;
                        }
                    }
                    for (int ly = -2; ly <= 1; ly++) {
                        int rad = (ly < 0) ? 2 : 1;
                        for (int lx = -rad; lx <= rad; lx++) {
                            for (int lz = -rad; lz <= rad; lz++) {
                                if (ly < 0 && Math.abs(lx) == rad && Math.abs(lz) == rad && Math.abs((seed + lx + lz) % 3) == 0) {
                                    continue;
                                }
                                if (ly == 1 && (Math.abs(lx) + Math.abs(lz) > 1)) {
                                    continue;
                                }
                                int lyIdx = surfaceY + treeH + ly + 1;
                                int bx = tx + lx;
                                int bz = tz + lz;
                                if (lyIdx >= 0 && lyIdx < HEIGHT && bx >= 0 && bx < WIDTH && bz >= 0 && bz < DEPTH) {
                                    int bidx = getIndex(bx, lyIdx, bz);
                                    if (blocks[bidx] == BlockType.AIR) {
                                        blocks[bidx] = BlockType.OAK_LEAVES;
                                    }
                                }
                            }
                        }
                    }
                    treePositions.add(new int[]{tx, tz});
                } else if (biome == Biome.SNOWY_TAIGA) {
                    int treeH = 7 + (seed % 4);
                    if (treeH < 0) treeH += 4;
                    for (int h = 1; h <= treeH; h++) {
                        if (surfaceY + h < HEIGHT) {
                            blocks[getIndex(tx, surfaceY + h, tz)] = BlockType.SPRUCE_LOG;
                        }
                    }
                    for (int yOff = 0; yOff <= treeH; yOff++) {
                        int ly = surfaceY + treeH + 1 - yOff;
                        if (ly >= HEIGHT || ly <= surfaceY + 1) continue;
                        int rad = switch (yOff) {
                            case 0 -> 0;
                            case 1 -> 1;
                            case 2 -> 0;
                            case 3 -> 1;
                            case 4 -> 2;
                            case 5 -> 1;
                            case 6, 7 -> 2;
                            default -> 2;
                        };
                        for (int lx = -rad; lx <= rad; lx++) {
                            for (int lz = -rad; lz <= rad; lz++) {
                                if (rad > 0 && Math.abs(lx) == rad && Math.abs(lz) == rad) {
                                    continue;
                                }
                                int bx = tx + lx;
                                int bz = tz + lz;
                                if (bx >= 0 && bx < WIDTH && bz >= 0 && bz < DEPTH) {
                                    int bidx = getIndex(bx, ly, bz);
                                    if (blocks[bidx] == BlockType.AIR) {
                                        blocks[bidx] = BlockType.SPRUCE_LEAVES;
                                    }
                                }
                            }
                        }
                    }
                    treePositions.add(new int[]{tx, tz});
                }
            }
        }

        calculateLighting();
    }

    public void calculateLighting() {
        // top-down sunlight
        for (int x = 0; x < WIDTH; x++) {
            for (int z = 0; z < DEPTH; z++) {
                byte sunlight = 15;
                for (int y = HEIGHT - 1; y >= 0; y--) {
                    BlockType b = blocks[getIndex(x, y, z)];
                    if (b == BlockType.AIR || b == BlockType.WATER || b.isTransparent()) {
                        light[getIndex(x, y, z)] = sunlight;
                    } else {
                        sunlight = 0;
                        light[getIndex(x, y, z)] = 0;
                    }
                }
            }
        }

        // Horizontal Light Propagation (BFS queue)
        Queue<Integer> queue = new ArrayDeque<>();
        for (int i = 0; i < light.length; i++) {
            if (light[i] > 1) {
                queue.add(i);
            }
        }

        while (!queue.isEmpty()) {
            int idx = queue.poll();
            int cx = (idx >>> 12) & 15;
            int cy = (idx >>> 4) & 255;
            int cz = idx & 15;

            byte currentLight = light[idx];
            byte dropLight = (byte) (currentLight - 1);
            if (dropLight <= 0) continue;

            int[] dx = {1, -1, 0, 0, 0, 0};
            int[] dy = {0, 0, 1, -1, 0, 0};
            int[] dz = {0, 0, 0, 0, 1, -1};

            for (int d = 0; d < 6; d++) {
                int nx = cx + dx[d];
                int ny = cy + dy[d];
                int nz = cz + dz[d];

                if (nx >= 0 && nx < WIDTH && ny >= 0 && ny < HEIGHT && nz >= 0 && nz < DEPTH) {
                    int nidx = getIndex(nx, ny, nz);
                    BlockType nb = blocks[nidx];
                    boolean occluding = nb != BlockType.AIR && nb != BlockType.WATER && !nb.isTransparent();
                    if (!occluding && light[nidx] < dropLight) {
                        light[nidx] = dropLight;
                        queue.add(nidx);
                    }
                }
            }
        }
    }

    public static class FloatList {
        public float[] data = new float[1024];
        public int size = 0;

        public void add(float val) {
            if (size >= data.length) {
                float[] newData = new float[data.length * 2];
                System.arraycopy(data, 0, newData, 0, data.length);
                data = newData;
            }
            data[size++] = val;
        }

        public void add(float u, float v) {
            add(u); add(v);
        }

        public void add(float x, float y, float z) {
            add(x); add(y); add(z);
        }

        public float[] toArray() {
            float[] res = new float[size];
            System.arraycopy(data, 0, res, 0, size);
            return res;
        }
    }

    public static class ByteList {
        public byte[] data = new byte[1024];
        public int size = 0;

        public void add(byte val) {
            if (size >= data.length) {
                byte[] newData = new byte[data.length * 2];
                System.arraycopy(data, 0, newData, 0, data.length);
                data = newData;
            }
            data[size++] = val;
        }

        public void add(byte r, byte g, byte b, byte a) {
            add(r); add(g); add(b); add(a);
        }

        public byte[] toArray() {
            byte[] res = new byte[size];
            System.arraycopy(data, 0, res, 0, size);
            return res;
        }
    }

    public static class MeshData {
        public float[] v;
        public float[] t;
        public float[] n;
        public byte[] c;

        public MeshData(float[] v, float[] t, float[] n, byte[] c) {
            this.v = v;
            this.t = t;
            this.n = n;
            this.c = c;
        }
    }

    public MeshData[] calculateMeshData(World world) {
        FloatList vOp = new FloatList();
        FloatList tOp = new FloatList();
        FloatList nOp = new FloatList();
        ByteList cOp = new ByteList();

        FloatList vWa = new FloatList();
        FloatList tWa = new FloatList();
        FloatList nWa = new FloatList();
        ByteList cWa = new ByteList();

        java.util.function.BiFunction<BlockType, BlockType, Boolean> shouldDrawFace = (curr, neigh) -> {
            if (neigh == BlockType.AIR) return true;
            if (curr == BlockType.WATER) return neigh != BlockType.WATER;
            if (neigh == BlockType.WATER && curr != BlockType.WATER) return true;
            if (neigh.isTransparent() && curr != neigh) return true;
            return false;
        };

        java.util.function.Function<BlockType, Boolean> isSolid = (b) -> {
            return b != BlockType.AIR && b != BlockType.WATER && !b.isTransparent();
        };

        java.util.function.BiFunction<Byte, Byte, Byte> saturatingSub = (b1, b2) -> {
            int res = (b1 & 0xFF) - (b2 & 0xFF);
            return (byte) Math.max(0, res);
        };

        // Standard Voxel AO calculation
        java.util.function.BiFunction<Boolean, Boolean, java.util.function.Function<Boolean, Float>> calcAO = (s1, s2) -> (c) -> {
            if (s1 && s2) return 0.4f;
            int occ = 0;
            if (s1) occ++;
            if (s2) occ++;
            if (c) occ++;
            return switch (occ) {
                case 0 -> 1.0f;
                case 1 -> 0.8f;
                case 2 -> 0.6f;
                default -> 0.4f;
            };
        };

        java.util.function.Function<Byte, Float> calcLightF = (lightVal) -> {
            float l = lightVal & 0xFF;
            float f = (float) Math.pow(0.85f, 15.0f - l);
            if (f < 0.1f) f = 0.1f;
            return f;
        };

        // Vertex light averager
        java.util.function.BiFunction<Float, Float, java.util.function.BiFunction<Float, Float, Float>> calcVertexLight = (l0, l1) -> (l2, l3) -> {
            float avg = (l0 + l1 + l2 + l3) / 4.0f;
            return Math.max(0.1f, avg);
        };

        float ts = 1.0f / 16.0f;
        float pad = 0.5f / 256.0f;

        for (int x = 0; x < WIDTH; x++) {
            for (int y = 0; y < HEIGHT; y++) {
                for (int z = 0; z < DEPTH; z++) {
                    BlockType block = getBlock(x, y, z);
                    if (block == BlockType.AIR) continue;

                    float fx = (x + this.x * WIDTH);
                    float fy = y;
                    float fz = (z + this.z * DEPTH);

                    int wx = (int) fx;
                    int wy = (int) fy;
                    int wz = (int) fz;

                    if (block == BlockType.WATER) {
                        byte level = getLiquidLevel(x, y, z);
                        if (level == 0) continue;

                        int[] uv = block.getAtlasUV();
                        float u0 = uv[0] * ts + pad;
                        float v0 = uv[1] * ts + pad;
                        float u1 = (uv[0] + 1) * ts - pad;
                        float v1 = (uv[1] + 1) * ts - pad;

                        // Smooth water surface height averaging
                        java.util.function.BiFunction<Integer, Integer, Float> cornerH = (cx, cz) -> {
                            float total = 0.0f;
                            int count = 0;
                            for (int ddx = -1; ddx <= 0; ddx++) {
                                for (int ddz = -1; ddz <= 0; ddz++) {
                                    int bx = cx + ddx;
                                    int bz = cz + ddz;
                                    if (world.getBlock(bx, wy + 1, bz) == BlockType.WATER) {
                                        return 1.0f;
                                    }
                                    BlockType b = world.getBlock(bx, wy, bz);
                                    if (b == BlockType.WATER) {
                                        byte l = world.getLiquidLevel(bx, wy, bz);
                                        total += waterRenderHeight(l);
                                        count++;
                                    }
                                }
                            }
                            return count > 0 ? (total / count) : 0.88f;
                        };

                        float h00 = cornerH.apply(wx, wz);
                        float h10 = cornerH.apply(wx + 1, wz);
                        float h11 = cornerH.apply(wx + 1, wz + 1);
                        float h01 = cornerH.apply(wx, wz + 1);

                        // Top water face
                        BlockType neighborTop = world.getBlock(wx, wy + 1, wz);
                        if (shouldDrawFace.apply(block, neighborTop)) {
                            vWa.add(fx, fy + h00, fz);
                            vWa.add(fx, fy + h01, fz + 1.0f);
                            vWa.add(fx + 1.0f, fy + h11, fz + 1.0f);
                            vWa.add(fx, fy + h00, fz);
                            vWa.add(fx + 1.0f, fy + h11, fz + 1.0f);
                            vWa.add(fx + 1.0f, fy + h10, fz);

                            tWa.add(u0, v0); tWa.add(u0, v1); tWa.add(u1, v1);
                            tWa.add(u0, v0); tWa.add(u1, v1); tWa.add(u1, v0);

                            for (int k = 0; k < 6; k++) nWa.add(0.0f, 1.0f, 0.0f);

                            byte lB = world.getLight(wx, wy + 1, wz);
                            byte ll01 = world.getLight(wx - 1, wy + 1, wz);
                            byte ll10 = world.getLight(wx, wy + 1, wz - 1);
                            byte ll21 = world.getLight(wx + 1, wy + 1, wz);
                            byte ll12 = world.getLight(wx, wy + 1, wz + 1);

                            float light00 = calcVertexLight.apply(calcLightF.apply(lB), calcLightF.apply(ll01)).apply(calcLightF.apply(ll10), calcLightF.apply(world.getLight(wx - 1, wy + 1, wz - 1)));
                            float light10 = calcVertexLight.apply(calcLightF.apply(lB), calcLightF.apply(ll21)).apply(calcLightF.apply(ll10), calcLightF.apply(world.getLight(wx + 1, wy + 1, wz - 1)));
                            float light01 = calcVertexLight.apply(calcLightF.apply(lB), calcLightF.apply(ll01)).apply(calcLightF.apply(ll12), calcLightF.apply(world.getLight(wx - 1, wy + 1, wz + 1)));
                            float light11 = calcVertexLight.apply(calcLightF.apply(lB), calcLightF.apply(ll21)).apply(calcLightF.apply(ll12), calcLightF.apply(world.getLight(wx + 1, wy + 1, wz + 1)));

                            byte c00 = (byte) (255.0f * light00);
                            byte c10 = (byte) (255.0f * light10);
                            byte c01 = (byte) (255.0f * light01);
                            byte c11 = (byte) (255.0f * light11);

                            cWa.add(c00, c00, c00, WATER_VERTEX_ALPHA);
                            cWa.add(c01, c01, c01, WATER_VERTEX_ALPHA);
                            cWa.add(c11, c11, c11, WATER_VERTEX_ALPHA);
                            cWa.add(c00, c00, c00, WATER_VERTEX_ALPHA);
                            cWa.add(c11, c11, c11, WATER_VERTEX_ALPHA);
                            cWa.add(c10, c10, c10, WATER_VERTEX_ALPHA);
                        }

                        // Bottom water face
                        BlockType neighborBottom = wy > 0 ? world.getBlock(wx, wy - 1, wz) : BlockType.BEDROCK;
                        if (shouldDrawFace.apply(block, neighborBottom)) {
                            vWa.add(fx, fy, fz);
                            vWa.add(fx + 1.0f, fy, fz + 1.0f);
                            vWa.add(fx, fy, fz + 1.0f);
                            vWa.add(fx, fy, fz);
                            vWa.add(fx + 1.0f, fy, fz);
                            vWa.add(fx + 1.0f, fy, fz + 1.0f);

                            tWa.add(u0, v0); tWa.add(u1, v1); tWa.add(u0, v1);
                            tWa.add(u0, v0); tWa.add(u1, v0); tWa.add(u1, v1);

                            for (int k = 0; k < 6; k++) nWa.add(0.0f, -1.0f, 0.0f);

                            byte shade = (byte) (255.0f * calcLightF.apply(world.getLight(wx, wy - 1, wz)) * 0.5f);
                            if ((shade & 0xFF) < 35) shade = 35;

                            for (int k = 0; k < 6; k++) cWa.add(shade, shade, shade, WATER_VERTEX_ALPHA);
                        }

                        // Water sides (Z+, Z-, X+, X-)
                        int[][] neighbors = {
                            {wx, wy, wz + 1, 0, 0, 1},
                            {wx, wy, wz - 1, 0, 0, -1},
                            {wx + 1, wy, wz, 1, 0, 0},
                            {wx - 1, wy, wz, -1, 0, 0}
                        };
                        float[] shades = {0.6f, 0.6f, 0.8f, 0.8f};

                        for (int i = 0; i < 4; i++) {
                            BlockType neighbor = world.getBlock(neighbors[i][0], neighbors[i][1], neighbors[i][2]);
                            if (neighbor != BlockType.WATER) {
                                float sMul = shades[i];
                                int nx = neighbors[i][3];
                                int nz = neighbors[i][5];

                                if (i == 0) { // Z+
                                    vWa.add(fx, fy, fz + 1.0f);
                                    vWa.add(fx + 1.0f, fy, fz + 1.0f);
                                    vWa.add(fx + 1.0f, fy + h11, fz + 1.0f);
                                    vWa.add(fx, fy, fz + 1.0f);
                                    vWa.add(fx + 1.0f, fy + h11, fz + 1.0f);
                                    vWa.add(fx, fy + h01, fz + 1.0f);
                                } else if (i == 1) { // Z-
                                    vWa.add(fx + 1.0f, fy, fz);
                                    vWa.add(fx, fy, fz);
                                    vWa.add(fx, fy + h00, fz);
                                    vWa.add(fx + 1.0f, fy, fz);
                                    vWa.add(fx, fy + h00, fz);
                                    vWa.add(fx + 1.0f, fy + h10, fz);
                                } else if (i == 2) { // X+
                                    vWa.add(fx + 1.0f, fy, fz + 1.0f);
                                    vWa.add(fx + 1.0f, fy, fz);
                                    vWa.add(fx + 1.0f, fy + h10, fz);
                                    vWa.add(fx + 1.0f, fy, fz + 1.0f);
                                    vWa.add(fx + 1.0f, fy + h10, fz);
                                    vWa.add(fx + 1.0f, fy + h11, fz + 1.0f);
                                } else { // X-
                                    vWa.add(fx, fy, fz);
                                    vWa.add(fx, fy, fz + 1.0f);
                                    vWa.add(fx, fy + h01, fz + 1.0f);
                                    vWa.add(fx, fy, fz);
                                    vWa.add(fx, fy + h01, fz + 1.0f);
                                    vWa.add(fx, fy + h00, fz);
                                }

                                tWa.add(u0, v1); tWa.add(u1, v1); tWa.add(u1, v0);
                                tWa.add(u0, v1); tWa.add(u1, v0); tWa.add(u0, v0);

                                for (int k = 0; k < 6; k++) nWa.add((float) nx, 0.0f, (float) nz);

                                byte lB2 = world.getLight(neighbors[i][0], neighbors[i][1], neighbors[i][2]);
                                byte shade = (byte) (255.0f * calcLightF.apply(lB2) * sMul);
                                if ((shade & 0xFF) < 35) shade = 35;

                                for (int k = 0; k < 6; k++) cWa.add(shade, shade, shade, WATER_VERTEX_ALPHA);
                            }
                        }
                        continue;
                    }

                    // STANDARD BLOCK MESHING (OPAQUE/TRANSPARENT)
                    int[] uv = block.getAtlasUV();
                    int tx = uv[0];
                    int ty = uv[1];

                    float u0 = tx * ts + pad;
                    float v0 = ty * ts + pad;
                    float u1 = (tx + 1) * ts - pad;
                    float v1 = (ty + 1) * ts - pad;

                    float blockTop = (block == BlockType.SNOW_LAYER) ? 0.125f : ((block == BlockType.TORCH) ? 0.625f : 1.0f);

                    // Top Face
                    BlockType neighborTop = y < HEIGHT - 1 ? getBlock(x, y + 1, z) : BlockType.AIR;
                    if (shouldDrawFace.apply(block, neighborTop)) {
                        float tu0 = u0, tv0 = v0, tu1 = u1, tv1 = v1;
                        int[] topUV = block.getAtlasUVTop();
                        if (topUV[0] != tx || topUV[1] != ty) {
                            tu0 = topUV[0] * ts + pad;
                            tv0 = topUV[1] * ts + pad;
                            tu1 = (topUV[0] + 1) * ts - pad;
                            tv1 = (topUV[1] + 1) * ts - pad;
                        }

                        boolean l_00 = isSolid.apply(world.getBlock(wx - 1, wy + 1, wz - 1));
                        boolean l_10 = isSolid.apply(world.getBlock(wx, wy + 1, wz - 1));
                        boolean l_20 = isSolid.apply(world.getBlock(wx + 1, wy + 1, wz - 1));
                        boolean l_01 = isSolid.apply(world.getBlock(wx - 1, wy + 1, wz));
                        boolean l_21 = isSolid.apply(world.getBlock(wx + 1, wy + 1, wz));
                        boolean l_02 = isSolid.apply(world.getBlock(wx - 1, wy + 1, wz + 1));
                        boolean l_12 = isSolid.apply(world.getBlock(wx, wy + 1, wz + 1));
                        boolean l_22 = isSolid.apply(world.getBlock(wx + 1, wy + 1, wz + 1));

                        float ao00 = calcAO.apply(l_01, l_10).apply(l_00);
                        float ao10 = calcAO.apply(l_21, l_10).apply(l_20);
                        float ao01 = calcAO.apply(l_01, l_12).apply(l_02);
                        float ao11 = calcAO.apply(l_21, l_12).apply(l_22);

                        vOp.add(fx, fy + blockTop, fz);
                        vOp.add(fx, fy + blockTop, fz + 1.0f);
                        vOp.add(fx + 1.0f, fy + blockTop, fz + 1.0f);
                        vOp.add(fx, fy + blockTop, fz);
                        vOp.add(fx + 1.0f, fy + blockTop, fz + 1.0f);
                        vOp.add(fx + 1.0f, fy + blockTop, fz);

                        tOp.add(tu0, tv0); tOp.add(tu0, tv1); tOp.add(tu1, tv1);
                        tOp.add(tu0, tv0); tOp.add(tu1, tv1); tOp.add(tu1, tv0);

                        byte lB = world.getLight(wx, wy + 1, wz);
                        byte ll01 = world.getLight(wx - 1, wy + 1, wz);
                        byte ll10 = world.getLight(wx, wy + 1, wz - 1);
                        byte ll21 = world.getLight(wx + 1, wy + 1, wz);
                        byte ll12 = world.getLight(wx, wy + 1, wz + 1);

                        float light00 = calcVertexLight.apply(calcLightF.apply(lB), calcLightF.apply(ll01)).apply(calcLightF.apply(ll10), calcLightF.apply(world.getLight(wx - 1, wy + 1, wz - 1)));
                        float light10 = calcVertexLight.apply(calcLightF.apply(lB), calcLightF.apply(ll21)).apply(calcLightF.apply(ll10), calcLightF.apply(world.getLight(wx + 1, wy + 1, wz - 1)));
                        float light01 = calcVertexLight.apply(calcLightF.apply(lB), calcLightF.apply(ll01)).apply(calcLightF.apply(ll12), calcLightF.apply(world.getLight(wx - 1, wy + 1, wz + 1)));
                        float light11 = calcVertexLight.apply(calcLightF.apply(lB), calcLightF.apply(ll21)).apply(calcLightF.apply(ll12), calcLightF.apply(world.getLight(wx + 1, wy + 1, wz + 1)));

                        for (int k = 0; k < 6; k++) nOp.add(0.0f, 1.0f, 0.0f);

                        byte c00 = (byte) Math.max(35, 255.0f * light00 * ao00);
                        byte c10 = (byte) Math.max(35, 255.0f * light10 * ao10);
                        byte c01 = (byte) Math.max(35, 255.0f * light01 * ao01);
                        byte c11 = (byte) Math.max(35, 255.0f * light11 * ao11);

                        cOp.add(c00, c00, c00, (byte) 255);
                        cOp.add(c01, c01, c01, (byte) 255);
                        cOp.add(c11, c11, c11, (byte) 255);
                        cOp.add(c00, c00, c00, (byte) 255);
                        cOp.add(c11, c11, c11, (byte) 255);
                        cOp.add(c10, c10, c10, (byte) 255);
                    }

                    // Bottom Face
                    BlockType neighborBottom = y > 0 ? getBlock(x, y - 1, z) : BlockType.AIR;
                    if (shouldDrawFace.apply(block, neighborBottom)) {
                        float bu0 = u0, bv0 = v0, bu1 = u1, bv1 = v1;
                        int[] botUV = block.getAtlasUVBottom();
                        if (botUV[0] != tx || botUV[1] != ty) {
                            bu0 = botUV[0] * ts + pad;
                            bv0 = botUV[1] * ts + pad;
                            bu1 = (botUV[0] + 1) * ts - pad;
                            bv1 = (botUV[1] + 1) * ts - pad;
                        }

                        vOp.add(fx, fy, fz);
                        vOp.add(fx + 1.0f, fy, fz + 1.0f);
                        vOp.add(fx, fy, fz + 1.0f);
                        vOp.add(fx, fy, fz);
                        vOp.add(fx + 1.0f, fy, fz);
                        vOp.add(fx + 1.0f, fy, fz + 1.0f);

                        tOp.add(bu0, bv0); tOp.add(bu1, bv1); tOp.add(bu0, bv1);
                        tOp.add(bu0, bv0); tOp.add(bu1, bv0); tOp.add(bu1, bv1);

                        for (int k = 0; k < 6; k++) nOp.add(0.0f, -1.0f, 0.0f);

                        byte shade = (byte) Math.max(35, 255.0f * calcLightF.apply(world.getLight(wx, wy - 1, wz)) * 0.5f);
                        for (int k = 0; k < 6; k++) cOp.add(shade, shade, shade, (byte) 255);
                    }

                    // Side Faces (Z+, Z-, X+, X-)
                    int[][] sides = {
                        {0, 0, 1, 0},   // Z+
                        {0, 0, -1, 1},  // Z-
                        {1, 0, 0, 2},   // X+
                        {-1, 0, 0, 3}   // X-
                    };
                    float[] shades = {0.6f, 0.6f, 0.8f, 0.8f};

                    for (int sIdx = 0; sIdx < 4; sIdx++) {
                        int nx = sides[sIdx][0];
                        int nz = sides[sIdx][2];
                        int sideFace = sides[sIdx][3];

                        BlockType neighbor;
                        if (sideFace == 0) { // Z+
                            neighbor = z < DEPTH - 1 ? getBlock(x, y, z + 1) : world.getBlock(wx, wy, wz + 1);
                        } else if (sideFace == 1) { // Z-
                            neighbor = z > 0 ? getBlock(x, y, z - 1) : world.getBlock(wx, wy, wz - 1);
                        } else if (sideFace == 2) { // X+
                            neighbor = x < WIDTH - 1 ? getBlock(x + 1, y, z) : world.getBlock(wx + 1, wy, wz);
                        } else { // X-
                            neighbor = x > 0 ? getBlock(x - 1, y, z) : world.getBlock(wx - 1, wy, wz);
                        }

                        if (shouldDrawFace.apply(block, neighbor)) {
                            float su0 = u0, sv0 = v0, su1 = u1, sv1 = v1;
                            int[] sideUV = block.getAtlasUVSide();
                            if (sideUV[0] != tx || sideUV[1] != ty) {
                                su0 = sideUV[0] * ts + pad;
                                sv0 = sideUV[1] * ts + pad;
                                su1 = (sideUV[0] + 1) * ts - pad;
                                sv1 = (sideUV[1] + 1) * ts - pad;
                            }

                            if (sideFace == 0) { // Z+
                                vOp.add(fx, fy, fz + 1.0f);
                                vOp.add(fx + 1.0f, fy, fz + 1.0f);
                                vOp.add(fx + 1.0f, fy + blockTop, fz + 1.0f);
                                vOp.add(fx, fy, fz + 1.0f);
                                vOp.add(fx + 1.0f, fy + blockTop, fz + 1.0f);
                                vOp.add(fx, fy + blockTop, fz + 1.0f);
                            } else if (sideFace == 1) { // Z-
                                vOp.add(fx + 1.0f, fy, fz);
                                vOp.add(fx, fy, fz);
                                vOp.add(fx, fy + blockTop, fz);
                                vOp.add(fx + 1.0f, fy, fz);
                                vOp.add(fx, fy + blockTop, fz);
                                vOp.add(fx + 1.0f, fy + blockTop, fz);
                            } else if (sideFace == 2) { // X+
                                vOp.add(fx + 1.0f, fy, fz + 1.0f);
                                vOp.add(fx + 1.0f, fy, fz);
                                vOp.add(fx + 1.0f, fy + blockTop, fz);
                                vOp.add(fx + 1.0f, fy, fz + 1.0f);
                                vOp.add(fx + 1.0f, fy + blockTop, fz);
                                vOp.add(fx + 1.0f, fy + blockTop, fz + 1.0f);
                            } else { // X-
                                vOp.add(fx, fy, fz);
                                vOp.add(fx, fy, fz + 1.0f);
                                vOp.add(fx, fy + blockTop, fz + 1.0f);
                                vOp.add(fx, fy, fz);
                                vOp.add(fx, fy + blockTop, fz + 1.0f);
                                vOp.add(fx, fy + blockTop, fz);
                            }

                            tOp.add(su0, sv1); tOp.add(su1, sv1); tOp.add(su1, sv0);
                            tOp.add(su0, sv1); tOp.add(su1, sv0); tOp.add(su0, sv0);

                            for (int k = 0; k < 6; k++) nOp.add((float) nx, 0.0f, (float) nz);

                            byte l1 = world.getLight(wx + nx, wy, wz + nz);
                            byte l2 = world.getLight(wx + nx, wy + 1, wz + nz);
                            byte lMax = (byte) Math.max(l1 & 0xFF, l2 & 0xFF);

                            byte shade = (byte) Math.max(35, 255.0f * calcLightF.apply(lMax) * shades[sIdx]);
                            for (int k = 0; k < 6; k++) cOp.add(shade, shade, shade, (byte) 255);
                        }
                    }
                }
            }
        }

        return new MeshData[] {
            new MeshData(vOp.toArray(), tOp.toArray(), nOp.toArray(), cOp.toArray()),
            new MeshData(new float[0], new float[0], new float[0], new byte[0]), // Empty transparent mesh
            new MeshData(vWa.toArray(), tWa.toArray(), nWa.toArray(), cWa.toArray())
        };
    }

    public void uploadMesh(MeshData opaque, MeshData transparent, MeshData water) {
        if (meshOpaque != null) meshOpaque.cleanup();
        meshOpaque = (opaque.v.length == 0) ? null : new Mesh(opaque.v, opaque.t, opaque.n, opaque.c);

        if (meshTransparent != null) meshTransparent.cleanup();
        meshTransparent = null; // Unused in this version

        if (meshWater != null) meshWater.cleanup();
        meshWater = (water.v.length == 0) ? null : new Mesh(water.v, water.t, water.n, water.c);

        meshingInProgress = false;
        dirty = false;
    }

    public void cleanup() {
        if (meshOpaque != null) { meshOpaque.cleanup(); meshOpaque = null; }
        if (meshTransparent != null) { meshTransparent.cleanup(); meshTransparent = null; }
        if (meshWater != null) { meshWater.cleanup(); meshWater = null; }
    }
}
