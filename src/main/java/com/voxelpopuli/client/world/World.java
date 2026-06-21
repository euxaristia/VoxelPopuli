package com.voxelpopuli.client.world;

import com.voxelpopuli.client.block.BlockType;
import com.voxelpopuli.client.noise.Noise;
import com.voxelpopuli.client.renderer.Mesh;
import com.voxelpopuli.client.renderer.Shader;
import com.voxelpopuli.client.renderer.Texture2D;
import com.voxelpopuli.client.renderer.TextureGenerator;
import org.joml.Matrix4f;
import org.joml.Vector3f;
import org.joml.Vector4f;
import org.lwjgl.opengl.GL11;

import java.util.*;
import java.util.concurrent.*;

public class World {
    public static final int VIEW_DISTANCE = 35;
    public static final int POOL_WIDTH = VIEW_DISTANCE * 2 + 1;
    public static final int CHUNK_POOL_SIZE = POOL_WIDTH * POOL_WIDTH;
    public static final float CLOUD_HEIGHT = 200.0f;

    public static record Detonation(Vector3f pos, BlockType blockType) {}

    public final Chunk[] chunks = new Chunk[CHUNK_POOL_SIZE];
    public Texture2D atlas;
    public Mesh cloudModel;
    public Mesh starModel;
    public final Vector3f lastCloudPos = new Vector3f(-999999f, -999999f, -999999f);
    public float lastCloudUpdateTime = -999.0f;
    public final ConcurrentHashMap<BlockPos, BlockType> edits = new ConcurrentHashMap<>();
    public int lastPcx = -999999;
    public int lastPcz = -999999;
    public int dirtyCount = 0;

    public final List<ActiveExplosive> explosives = new CopyOnWriteArrayList<>();
    public final List<Particle> particles = new CopyOnWriteArrayList<>();
    public final List<Detonation> detonations = new CopyOnWriteArrayList<>();
    
    public final Mesh cubeMesh;
    public boolean isLoading = true;
    public int loadingRadius = 0;
    public int chunksGeneratedCount = 0;
    
    public final Set<BlockPos> activeWater = ConcurrentHashMap.newKeySet();
    public final Set<BlockPos> activeFalling = ConcurrentHashMap.newKeySet();
    public float waterTickTimer = 0.0f;
    
    public final List<Integer> visibleChunks = new ArrayList<>();
    public int meshingInFlight = 0;

    private final ExecutorService meshingExecutor = Executors.newFixedThreadPool(
        Math.max(1, Runtime.getRuntime().availableProcessors() - 1),
        r -> {
            Thread t = new Thread(r);
            t.setDaemon(true);
            t.setName("VoxelPopuli-Mesher");
            return t;
        }
    );

    public World() {
        cubeMesh = createCubeMesh();
    }

    private static Mesh createCubeMesh() {
        float[] v = new float[] {
            0.0f, 0.0f, 0.0f, 0.0f, 0.0f, 1.0f, 1.0f, 0.0f, 1.0f, 0.0f, 0.0f, 0.0f, 1.0f, 0.0f, 1.0f, 1.0f, 0.0f, 0.0f, // bottom
            0.0f, 1.0f, 0.0f, 0.0f, 1.0f, 1.0f, 1.0f, 1.0f, 1.0f, 0.0f, 1.0f, 0.0f, 1.0f, 1.0f, 1.0f, 1.0f, 1.0f, 0.0f, // top
            0.0f, 0.0f, 0.0f, 0.0f, 1.0f, 0.0f, 1.0f, 1.0f, 0.0f, 0.0f, 0.0f, 0.0f, 1.0f, 1.0f, 0.0f, 1.0f, 0.0f, 0.0f, // front
            0.0f, 0.0f, 1.0f, 0.0f, 1.0f, 1.0f, 1.0f, 1.0f, 1.0f, 0.0f, 0.0f, 1.0f, 1.0f, 1.0f, 1.0f, 1.0f, 0.0f, 1.0f, // back
            0.0f, 0.0f, 0.0f, 0.0f, 1.0f, 0.0f, 0.0f, 1.0f, 1.0f, 0.0f, 0.0f, 0.0f, 0.0f, 1.0f, 1.0f, 0.0f, 0.0f, 1.0f, // left
            1.0f, 0.0f, 0.0f, 1.0f, 1.0f, 0.0f, 1.0f, 1.0f, 1.0f, 1.0f, 0.0f, 0.0f, 1.0f, 1.0f, 1.0f, 1.0f, 0.0f, 1.0f  // right
        };
        float[] t = new float[v.length / 3 * 2];
        float[] n = new float[v.length];
        byte[] c = new byte[v.length / 3 * 4];
        Arrays.fill(c, (byte) 255);
        return new Mesh(v, t, n, c);
    }

    public void renderExplosives(Shader shader, Matrix4f mvp, float currentTime) {
        int locModel = shader.getUniformLocation("uModel");
        int locDiff = shader.getUniformLocation("colDiffuse");

        for (ActiveExplosive e : explosives) {
            boolean flash = ((int) (currentTime * 4.0f)) % 2 == 0;
            Vector4f diffuse = flash ? new Vector4f(4.0f, 4.0f, 4.0f, 1.0f) : new Vector4f(1.0f, 1.0f, 1.0f, 1.0f);
            shader.setVec4(locDiff, diffuse);

            Matrix4f model = new Matrix4f().translation(e.position);
            shader.setMat4(locModel, model);
            cubeMesh.draw();
        }
        shader.setVec4(locDiff, new Vector4f(1.0f, 1.0f, 1.0f, 1.0f));
    }

    public void renderParticles(Shader shader, Matrix4f mvp) {
        int locModel = shader.getUniformLocation("uModel");
        int locDiff = shader.getUniformLocation("colDiffuse");

        for (Particle p : particles) {
            float alpha = Math.max(0.0f, Math.min(1.0f, p.life / p.maxLife));
            shader.setVec4(locDiff, new Vector4f(1.0f, 1.0f, 1.0f, alpha));

            Matrix4f model = new Matrix4f().translation(p.position).scale(p.scale * alpha);
            shader.setMat4(locModel, model);
            cubeMesh.draw();
        }
    }

    public int getPoolIndex(int cx, int cz) {
        int ix = Math.floorMod(cx, POOL_WIDTH);
        int iz = Math.floorMod(cz, POOL_WIDTH);
        return ix + iz * POOL_WIDTH;
    }

    public Chunk getChunk(int cx, int cz) {
        int index = getPoolIndex(cx, cz);
        Chunk chunk = chunks[index];
        if (chunk != null && chunk.x == cx && chunk.z == cz) {
            return chunk;
        }
        return null;
    }

    public BlockType getBlock(int x, int y, int z) {
        if (y < 0 || y >= Chunk.HEIGHT) {
            return BlockType.AIR;
        }

        int cx = Math.floorDiv(x, Chunk.WIDTH);
        int cz = Math.floorDiv(z, Chunk.DEPTH);
        Chunk chunk = getChunk(cx, cz);
        if (chunk != null) {
            int bx = Math.floorMod(x, Chunk.WIDTH);
            int bz = Math.floorMod(z, Chunk.DEPTH);
            return chunk.getBlock(bx, y, bz);
        }

        BlockType b = edits.get(new BlockPos(x, y, z));
        return (b != null) ? b : BlockType.AIR;
    }

    public void setBlock(int x, int y, int z, BlockType block) {
        if (y < 0 || y >= Chunk.HEIGHT) {
            return;
        }

        edits.put(new BlockPos(x, y, z), block);

        int cx = Math.floorDiv(x, Chunk.WIDTH);
        int cz = Math.floorDiv(z, Chunk.DEPTH);
        int bx = Math.floorMod(x, Chunk.WIDTH);
        int bz = Math.floorMod(z, Chunk.DEPTH);

        Chunk chunk = getChunk(cx, cz);
        if (chunk != null) {
            chunk.setBlock(bx, y, bz, block);
            if (!chunk.dirty) {
                chunk.dirty = true;
                dirtyCount++;
            }

            if (block == BlockType.WATER) {
                activeWater.add(new BlockPos(x, y, z));
            } else if (block == BlockType.SAND || block == BlockType.GRAVEL) {
                activeFalling.add(new BlockPos(x, y, z));
            }

            // Neighbors might now flow or fall
            int[][] neighbors = {
                {x, y + 1, z}, {x, y - 1, z},
                {x + 1, y, z}, {x - 1, y, z},
                {x, y, z + 1}, {x, y, z - 1}
            };
            for (int[] n : neighbors) {
                BlockType nb = getBlock(n[0], n[1], n[2]);
                if (nb == BlockType.WATER) {
                    activeWater.add(new BlockPos(n[0], n[1], n[2]));
                } else if (nb == BlockType.SAND || nb == BlockType.GRAVEL) {
                    activeFalling.add(new BlockPos(n[0], n[1], n[2]));
                }
            }

            List<int[]> dirtyNeighbors = new ArrayList<>();
            if (bx == 0) dirtyNeighbors.add(new int[]{cx - 1, cz});
            if (bx == Chunk.WIDTH - 1) dirtyNeighbors.add(new int[]{cx + 1, cz});
            if (bz == 0) dirtyNeighbors.add(new int[]{cx, cz - 1});
            if (bz == Chunk.DEPTH - 1) dirtyNeighbors.add(new int[]{cx, cz + 1});

            for (int[] n : dirtyNeighbors) {
                Chunk nc = getChunk(n[0], n[1]);
                if (nc != null && !nc.dirty) {
                    nc.dirty = true;
                    dirtyCount++;
                }
            }
        }
    }

    public void applyEditsToChunk(int cx, int cz) {
        int chunkStartX = cx * Chunk.WIDTH;
        int chunkStartZ = cz * Chunk.DEPTH;
        int chunkEndX = chunkStartX + Chunk.WIDTH;
        int chunkEndZ = chunkStartZ + Chunk.DEPTH;

        Chunk chunk = getChunk(cx, cz);
        if (chunk != null) {
            for (Map.Entry<BlockPos, BlockType> entry : edits.entrySet()) {
                BlockPos pos = entry.getKey();
                int ex = pos.x();
                int ey = pos.y();
                int ez = pos.z();
                if (ex >= chunkStartX && ex < chunkEndX && ez >= chunkStartZ && ez < chunkEndZ) {
                    int bx = ex - chunkStartX;
                    int bz = ez - chunkStartZ;
                    chunk.setBlock(bx, ey, bz, entry.getValue());
                    chunk.dirty = true;
                }
            }
        }
    }

    public void generateAtlas() {
        byte[] data = TextureGenerator.generateAtlasData();
        atlas = new Texture2D(data, 256, 256);
    }

    public RaycastResult raycast(Vector3f origin, Vector3f direction, float maxDistance) {
        Vector3f dir = new Vector3f(direction).normalize();
        float t = 0.0f;
        int ix = (int) Math.floor(origin.x);
        int iy = (int) Math.floor(origin.y);
        int iz = (int) Math.floor(origin.z);

        int stepX = (int) Math.signum(dir.x);
        int stepY = (int) Math.signum(dir.y);
        int stepZ = (int) Math.signum(dir.z);

        float tMaxX = (dir.x != 0.0f) ? ((ix + Math.max(0, stepX)) - origin.x) / dir.x : Float.POSITIVE_INFINITY;
        float tMaxY = (dir.y != 0.0f) ? ((iy + Math.max(0, stepY)) - origin.y) / dir.y : Float.POSITIVE_INFINITY;
        float tMaxZ = (dir.z != 0.0f) ? ((iz + Math.max(0, stepZ)) - origin.z) / dir.z : Float.POSITIVE_INFINITY;

        float tDeltaX = (dir.x != 0.0f) ? Math.abs(1.0f / dir.x) : Float.POSITIVE_INFINITY;
        float tDeltaY = (dir.y != 0.0f) ? Math.abs(1.0f / dir.y) : Float.POSITIVE_INFINITY;
        float tDeltaZ = (dir.z != 0.0f) ? Math.abs(1.0f / dir.z) : Float.POSITIVE_INFINITY;

        int nx = 0;
        int ny = 0;
        int nz = 0;

        while (t < maxDistance) {
            BlockType block = getBlock(ix, iy, iz);
            if (block != BlockType.AIR && block != BlockType.WATER) {
                return new RaycastResult(true, ix, iy, iz, nx, ny, nz, block);
            }
            if (tMaxX < tMaxY) {
                if (tMaxX < tMaxZ) {
                    ix += stepX;
                    t = tMaxX;
                    tMaxX += tDeltaX;
                    nx = -stepX;
                    ny = 0;
                    nz = 0;
                } else {
                    iz += stepZ;
                    t = tMaxZ;
                    tMaxZ += tDeltaZ;
                    nx = 0;
                    ny = 0;
                    nz = -stepZ;
                }
            } else {
                if (tMaxY < tMaxZ) {
                    iy += stepY;
                    t = tMaxY;
                    tMaxY += tDeltaY;
                    nx = 0;
                    ny = -stepY;
                    nz = 0;
                } else {
                    iz += stepZ;
                    t = tMaxZ;
                    tMaxZ += tDeltaZ;
                    nx = 0;
                    ny = 0;
                    nz = -stepZ;
                }
            }
        }

        return new RaycastResult(false, 0, 0, 0, 0, 0, 0, BlockType.AIR);
    }

    public void update(Vector3f playerPos, float dt) {
        int pcx = (int) Math.floor(playerPos.x / Chunk.WIDTH);
        int pcz = (int) Math.floor(playerPos.z / Chunk.DEPTH);

        // --- Spiral Chunk Generation ---
        if (pcx != lastPcx || pcz != lastPcz) {
            if (isLoading) {
                int generatedThisFrame = 0;
                while (loadingRadius <= VIEW_DISTANCE && generatedThisFrame < 64) {
                    int r = loadingRadius;
                    for (int x = pcx - r; x <= pcx + r; x++) {
                        for (int z = pcz - r; z <= pcz + r; z++) {
                            if (x > pcx - r && x < pcx + r && z > pcz - r && z < pcz + r) {
                                continue;
                            }
                            int index = getPoolIndex(x, z);
                            Chunk oldChunk = chunks[index];
                            boolean shouldReplace = (oldChunk == null || oldChunk.x != x || oldChunk.z != z);
                            if (shouldReplace) {
                                Chunk chunk = new Chunk(x, z);
                                chunk.generate();
                                chunks[index] = chunk;
                                applyEditsToChunk(x, z);
                                chunk.dirty = true;
                                dirtyCount++;
                                chunksGeneratedCount++;
                                generatedThisFrame++;
                                if (generatedThisFrame >= 64) {
                                    break;
                                }
                            }
                        }
                        if (generatedThisFrame >= 64) {
                            break;
                        }
                    }
                    if (generatedThisFrame < 64) {
                        loadingRadius++;
                    }
                }
                if (loadingRadius > VIEW_DISTANCE) {
                    isLoading = false;
                    lastPcx = pcx;
                    lastPcz = pcz;
                }
            } else {
                // NORMAL MODE: Non-blocking spiral
                int generatedThisFrame = 0;
                for (int r = 0; r <= VIEW_DISTANCE; r++) {
                    for (int x = pcx - r; x <= pcx + r; x++) {
                        for (int z = pcz - r; z <= pcz + r; z++) {
                            if (x > pcx - r && x < pcx + r && z > pcz - r && z < pcz + r) {
                                continue;
                            }
                            int index = getPoolIndex(x, z);
                            Chunk oldChunk = chunks[index];
                            boolean shouldReplace = (oldChunk == null || oldChunk.x != x || oldChunk.z != z);
                            if (shouldReplace) {
                                Chunk chunk = new Chunk(x, z);
                                chunk.generate();
                                chunks[index] = chunk;
                                applyEditsToChunk(x, z);

                                chunk.dirty = true;
                                dirtyCount++;

                                int[][] neighbors = {
                                    {x - 1, z}, {x + 1, z}, {x, z - 1}, {x, z + 1}
                                };
                                for (int[] n : neighbors) {
                                    Chunk nc = getChunk(n[0], n[1]);
                                    if (nc != null && !nc.dirty) {
                                        nc.dirty = true;
                                        dirtyCount++;
                                    }
                                }

                                generatedThisFrame++;
                                if (generatedThisFrame >= 8) {
                                    break;
                                }
                            }
                        }
                        if (generatedThisFrame >= 8) {
                            break;
                        }
                    }
                    if (generatedThisFrame >= 8) {
                        break;
                    }
                }
                if (generatedThisFrame < 8) {
                    lastPcx = pcx;
                    lastPcz = pcz;
                }
            }
        }

        // --- Prioritized Meshing ---
        if (meshingInFlight > 0) {
            List<Map.Entry<Integer, Float>> readyIndices = new ArrayList<>();
            for (int i = 0; i < CHUNK_POOL_SIZE; i++) {
                Chunk chunk = chunks[i];
                if (chunk != null && chunk.meshingInProgress && chunk.pendingOpaque != null) {
                    float dx = chunk.x * Chunk.WIDTH + 8.0f - playerPos.x;
                    float dz = chunk.z * Chunk.DEPTH + 8.0f - playerPos.z;
                    float distSq = dx * dx + dz * dz;
                    readyIndices.add(new AbstractMap.SimpleEntry<>(i, distSq));
                }
            }
            readyIndices.sort(Map.Entry.comparingByValue());
            for (Map.Entry<Integer, Float> entry : readyIndices) {
                int index = entry.getKey();
                Chunk chunk = chunks[index];
                if (chunk != null && chunk.pendingOpaque != null) {
                    chunk.uploadMesh(chunk.pendingOpaque, chunk.pendingTransparent, chunk.pendingWater);
                    chunk.pendingOpaque = null;
                    chunk.pendingTransparent = null;
                    chunk.pendingWater = null;
                    meshingInFlight--;
                }
            }
        }

        // Scan for dirty chunks and dispatch to background
        if (dirtyCount > 0 && detonations.isEmpty()) {
            List<Map.Entry<Integer, Float>> dirtyIndices = new ArrayList<>();
            for (int i = 0; i < CHUNK_POOL_SIZE; i++) {
                Chunk chunk = chunks[i];
                if (chunk != null && chunk.dirty && !chunk.meshingInProgress) {
                    float dx = chunk.x * Chunk.WIDTH + 8.0f - playerPos.x;
                    float dz = chunk.z * Chunk.DEPTH + 8.0f - playerPos.z;
                    float distSq = dx * dx + dz * dz;
                    dirtyIndices.add(new AbstractMap.SimpleEntry<>(i, distSq));
                }
            }
            dirtyIndices.sort(Map.Entry.comparingByValue());

            int dispatchesThisFrame = 0;
            for (Map.Entry<Integer, Float> entry : dirtyIndices) {
                int index = entry.getKey();
                Chunk chunk = chunks[index];
                if (chunk != null) {
                    chunk.dirty = false;
                    chunk.meshingInProgress = true;
                    dirtyCount--;
                    meshingInFlight++;

                    meshingExecutor.submit(() -> {
                        try {
                            chunk.calculateLighting();
                            Chunk.MeshData[] data = chunk.calculateMeshData(World.this);
                            chunk.pendingOpaque = data[0];
                            chunk.pendingTransparent = data[1];
                            chunk.pendingWater = data[2];
                        } catch (Exception ex) {
                            ex.printStackTrace();
                        }
                    });

                    dispatchesThisFrame++;
                    if (dispatchesThisFrame >= 16) {
                        break;
                    }
                }
            }
        }

        // --- Explosive Ticking ---
        for (int i = 0; i < explosives.size(); ) {
            ActiveExplosive e = explosives.get(i);
            e.fuse -= dt;
            if (e.fuse <= 0.0f) {
                explosives.remove(i);
                detonations.add(new Detonation(e.position, e.blockType));
            } else {
                i++;
            }
        }

        // --- Process Detonations ---
        if (!detonations.isEmpty()) {
            List<Detonation> dets = new ArrayList<>(detonations);
            detonations.clear();
            for (Detonation d : dets) {
                int radius = (d.blockType == BlockType.NUKE) ? 20 : 4;
                boolean isNuke = (d.blockType == BlockType.NUKE);
                Explosion.explode(this, (int) d.pos.x, (int) d.pos.y, (int) d.pos.z, radius, isNuke);
            }
        }

        // --- Particle Ticking ---
        for (int i = 0; i < particles.size(); ) {
            Particle p = particles.get(i);
            p.position.add(p.velocity.x * dt, p.velocity.y * dt, p.velocity.z * dt);
            p.life -= dt;
            if (p.life <= 0.0f) {
                particles.remove(i);
            } else {
                i++;
            }
        }

        // --- Physics Simulation ---
        updateWater(dt);
        updateFallingBlocks();
    }

    public byte getLiquidLevel(int x, int y, int z) {
        if (y < 0 || y >= Chunk.HEIGHT) {
            return 0;
        }
        int cx = Math.floorDiv(x, Chunk.WIDTH);
        int cz = Math.floorDiv(z, Chunk.DEPTH);
        int bx = Math.floorMod(x, Chunk.WIDTH);
        int bz = Math.floorMod(z, Chunk.DEPTH);

        Chunk chunk = getChunk(cx, cz);
        if (chunk != null) {
            return chunk.getLiquidLevel(bx, y, bz);
        }
        return 0;
    }

    public void setLiquidLevel(int x, int y, int z, byte level) {
        if (y < 0 || y >= Chunk.HEIGHT) {
            return;
        }
        int cx = Math.floorDiv(x, Chunk.WIDTH);
        int cz = Math.floorDiv(z, Chunk.DEPTH);
        int bx = Math.floorMod(x, Chunk.WIDTH);
        int bz = Math.floorMod(z, Chunk.DEPTH);

        Chunk chunk = getChunk(cx, cz);
        if (chunk != null) {
            byte oldLevel = chunk.getLiquidLevel(bx, y, bz);
            if (oldLevel == level) {
                return;
            }
            chunk.setLiquidLevel(bx, y, bz, level);
            if (level > 0) {
                if (chunk.getBlock(bx, y, bz) != BlockType.WATER) {
                    chunk.setBlock(bx, y, bz, BlockType.WATER);
                }
            } else {
                if (chunk.getBlock(bx, y, bz) == BlockType.WATER) {
                    chunk.setBlock(bx, y, bz, BlockType.AIR);
                }
            }
            if (!chunk.dirty) {
                chunk.dirty = true;
                dirtyCount++;
            }
        }
    }

    private void scheduleWaterNeighbors(int x, int y, int z) {
        int[][] neighbors = {
            {1, 0, 0}, {-1, 0, 0},
            {0, 1, 0}, {0, -1, 0},
            {0, 0, 1}, {0, 0, -1}
        };
        for (int[] d : neighbors) {
            int nx = x + d[0];
            int ny = y + d[1];
            int nz = z + d[2];
            if (getBlock(nx, ny, nz) == BlockType.WATER) {
                activeWater.add(new BlockPos(nx, ny, nz));
            }
        }
    }

    private byte waterLevelFromDecay(byte decay, boolean falling) {
        if (falling) {
            return (byte) (Chunk.WATER_FALL_BIT + decay + 1);
        } else {
            return (byte) (decay + 1);
        }
    }

    private int calcFlowDecay(int x, int y, int z) {
        int minDecay = 1000;
        int[][] dirs2D = {{1, 0}, {-1, 0}, {0, 1}, {0, -1}};
        for (int[] d : dirs2D) {
            byte nl = getLiquidLevel(x + d[0], y, z + d[1]);
            if (nl == 0) {
                continue;
            }
            int nd = (Chunk.waterIsSource(nl) || Chunk.waterIsFalling(nl)) ? 0 : Chunk.waterDecay(nl);
            if (nd < minDecay) {
                minDecay = nd;
            }
        }
        if (getLiquidLevel(x, y + 1, z) > 0) {
            minDecay = 0;
        }
        if (minDecay >= 1000) {
            return -1;
        }
        int newDecay = minDecay + 1;
        return (newDecay > 7) ? -1 : newDecay;
    }

    private boolean[] getFlowDirections(int x, int y, int z) {
        int[][] dirs = {{1, 0}, {-1, 0}, {0, 1}, {0, -1}};
        int[] costs = new int[4];
        Arrays.fill(costs, 1000);
        for (int i = 0; i < 4; i++) {
            int nx = x + dirs[i][0];
            int nz = z + dirs[i][1];
            BlockType nb = getBlock(nx, y, nz);
            if (nb.isSolid()) {
                continue;
            }
            if (nb == BlockType.WATER && Chunk.waterIsSource(getLiquidLevel(nx, y, nz))) {
                continue;
            }

            BlockType below = getBlock(nx, y - 1, nz);
            if (y > 0 && (below == BlockType.AIR || (below == BlockType.WATER && !Chunk.waterIsSource(getLiquidLevel(nx, y - 1, nz))))) {
                costs[i] = 0;
            } else {
                costs[i] = findDropCost(nx, y, nz, 1, i);
            }
        }

        int minCost = 1000;
        for (int c : costs) {
            if (c < minCost) minCost = c;
        }

        boolean[] result = new boolean[4];
        if (minCost >= 1000) {
            for (int i = 0; i < 4; i++) {
                result[i] = !getBlock(x + dirs[i][0], y, z + dirs[i][1]).isSolid();
            }
            return result;
        }

        for (int i = 0; i < 4; i++) {
            result[i] = (costs[i] == minCost);
        }
        return result;
    }

    private int findDropCost(int x, int y, int z, int depth, int fromDir) {
        if (depth > 4) {
            return 1000;
        }
        int[] opposite = {1, 0, 3, 2};
        int[][] dirs = {{1, 0}, {-1, 0}, {0, 1}, {0, -1}};
        int minCost = 1000;
        for (int i = 0; i < 4; i++) {
            if (i == opposite[fromDir]) {
                continue;
            }
            int nx = x + dirs[i][0];
            int nz = z + dirs[i][1];
            BlockType nb = getBlock(nx, y, nz);
            if (nb.isSolid()) {
                continue;
            }
            if (nb == BlockType.WATER && Chunk.waterIsSource(getLiquidLevel(nx, y, nz))) {
                continue;
            }

            BlockType below = getBlock(nx, y - 1, nz);
            if (y > 0 && (below == BlockType.AIR || (below == BlockType.WATER && !Chunk.waterIsSource(getLiquidLevel(nx, y - 1, nz))))) {
                return depth;
            }
            int cost = findDropCost(nx, y, nz, depth + 1, i);
            if (cost < minCost) {
                minCost = cost;
            }
        }
        return minCost;
    }

    public void updateWater(float dt) {
        waterTickTimer += dt;
        if (waterTickTimer < 0.25f) {
            return;
        }
        waterTickTimer -= 0.25f;

        if (activeWater.isEmpty()) {
            return;
        }

        List<BlockPos> toProcess = new ArrayList<>(activeWater);
        activeWater.clear();

        int processed = 0;
        for (BlockPos pos : toProcess) {
            int x = pos.x();
            int y = pos.y();
            int z = pos.z();

            byte level = getLiquidLevel(x, y, z);
            if (level == 0) {
                continue;
            }

            boolean isSrc = Chunk.waterIsSource(level);

            // 1. Infinite water source
            int sourceCount = 0;
            int[][] dirs2D = {{1, 0}, {-1, 0}, {0, 1}, {0, -1}};
            for (int[] d : dirs2D) {
                if (Chunk.waterIsSource(getLiquidLevel(x + d[0], y, z + d[1]))) {
                    sourceCount++;
                }
            }

            if (!isSrc && sourceCount >= 2) {
                BlockType below = getBlock(x, y - 1, z);
                if (below.isSolid() || below == BlockType.WATER) {
                    setLiquidLevel(x, y, z, Chunk.WATER_SOURCE);
                    scheduleWaterNeighbors(x, y, z);
                    processed++;
                }
            }

            // 2. Recalculate flow decay
            level = getLiquidLevel(x, y, z);
            isSrc = Chunk.waterIsSource(level);
            if (!isSrc) {
                int newDecay = calcFlowDecay(x, y, z);
                if (newDecay < 0) {
                    setLiquidLevel(x, y, z, (byte) 0);
                    scheduleWaterNeighbors(x, y, z);
                    processed++;
                    continue;
                }
                byte newLevel = waterLevelFromDecay((byte) newDecay, false);
                if (newLevel != level) {
                    setLiquidLevel(x, y, z, newLevel);
                }
            }

            // Re-read level
            level = getLiquidLevel(x, y, z);
            if (level == 0) {
                processed++;
                continue;
            }
            byte decay = Chunk.waterDecay(level);
            isSrc = Chunk.waterIsSource(level);

            // 3. Flow downward
            if (y > 0) {
                BlockType below = getBlock(x, y - 1, z);
                if (!below.isSolid()) {
                    byte fallingLevel = waterLevelFromDecay(decay, true);
                    if (below == BlockType.AIR) {
                        setLiquidLevel(x, y - 1, z, fallingLevel);
                        if (isSrc) {
                            setLiquidLevel(x, y, z, (byte) 0);
                            scheduleWaterNeighbors(x, y, z);
                        }
                        activeWater.add(new BlockPos(x, y - 1, z));
                    } else if (below == BlockType.WATER) {
                        byte bl = getLiquidLevel(x, y - 1, z);
                        if (!Chunk.waterIsSource(bl) && !Chunk.waterIsFalling(bl)) {
                            setLiquidLevel(x, y - 1, z, fallingLevel);
                            if (isSrc) {
                                setLiquidLevel(x, y, z, (byte) 0);
                                scheduleWaterNeighbors(x, y, z);
                            }
                            activeWater.add(new BlockPos(x, y - 1, z));
                        }
                    }
                    if (isSrc) {
                        processed++;
                        continue;
                    }
                }
            }

            // 4. Spread horizontally
            if (decay < 7 || isSrc) {
                boolean[] flowDirs = getFlowDirections(x, y, z);
                boolean anyFlow = false;
                for (boolean b : flowDirs) {
                    if (b) anyFlow = true;
                }

                if (isSrc && anyFlow) {
                    if (sourceCount >= 2) {
                        for (int i = 0; i < 4; i++) {
                            if (!flowDirs[i]) continue;
                            int nx = x + dirs2D[i][0];
                            int nz = z + dirs2D[i][1];
                            BlockType nb = getBlock(nx, y, nz);
                            byte nl = getLiquidLevel(nx, y, nz);
                            if (nb == BlockType.AIR || (nb == BlockType.WATER && !Chunk.waterIsSource(nl))) {
                                setLiquidLevel(nx, y, nz, level);
                                activeWater.add(new BlockPos(nx, y, nz));
                            }
                        }
                    } else {
                        for (int i = 0; i < 4; i++) {
                            if (!flowDirs[i]) continue;
                            int nx = x + dirs2D[i][0];
                            int nz = z + dirs2D[i][1];
                            BlockType nb = getBlock(nx, y, nz);
                            byte nl = getLiquidLevel(nx, y, nz);
                            if (nb == BlockType.AIR || (nb == BlockType.WATER && !Chunk.waterIsSource(nl))) {
                                setLiquidLevel(nx, y, nz, level);
                                setLiquidLevel(x, y, z, (byte) 0);
                                scheduleWaterNeighbors(x, y, z);
                                activeWater.add(new BlockPos(nx, y, nz));
                                break;
                            }
                        }
                    }
                } else {
                    byte spreadDecay = isSrc ? (byte)1 : (byte)(decay + 1);
                    byte spreadLevel = waterLevelFromDecay(spreadDecay, false);
                    for (int i = 0; i < 4; i++) {
                        if (!flowDirs[i]) continue;
                        int nx = x + dirs2D[i][0];
                        int nz = z + dirs2D[i][1];
                        BlockType nb = getBlock(nx, y, nz);
                        if (nb.isSolid()) continue;
                        if (nb == BlockType.AIR) {
                            setLiquidLevel(nx, y, nz, spreadLevel);
                            activeWater.add(new BlockPos(nx, y, nz));
                        } else if (nb == BlockType.WATER) {
                            byte nl = getLiquidLevel(nx, y, nz);
                            if (!Chunk.waterIsSource(nl) && nl > spreadLevel) {
                                setLiquidLevel(nx, y, nz, spreadLevel);
                                activeWater.add(new BlockPos(nx, y, nz));
                            }
                        }
                    }
                }
            }

            processed++;
            if (processed > 4000) {
                for (int idx = processed; idx < toProcess.size(); idx++) {
                    activeWater.add(toProcess.get(idx));
                }
                break;
            }
        }
    }

    public void updateFallingBlocks() {
        if (activeFalling.isEmpty()) {
            return;
        }
        List<BlockPos> toProcess = new ArrayList<>(activeFalling);
        activeFalling.clear();

        int processed = 0;
        for (BlockPos pos : toProcess) {
            int x = pos.x();
            int y = pos.y();
            int z = pos.z();

            BlockType b = getBlock(x, y, z);
            if (b != BlockType.SAND && b != BlockType.GRAVEL) {
                continue;
            }

            if (y > 0) {
                BlockType target = getBlock(x, y - 1, z);
                if (target == BlockType.AIR || target == BlockType.WATER) {
                    setBlock(x, y, z, BlockType.AIR);
                    setBlock(x, y - 1, z, b);
                }
            }

            processed++;
            if (processed > 50000) {
                for (int idx = processed; idx < toProcess.size(); idx++) {
                    activeFalling.add(toProcess.get(idx));
                }
                break;
            }
        }
    }

    public void initCelestial() {
        List<Float> v = new ArrayList<>();
        List<Byte> c = new ArrayList<>();
        Random rng = new Random();
        for (int i = 0; i < 300; i++) {
            float x = (-1.0f + rng.nextFloat() * 2.0f) * 400.0f;
            float y = 50.0f + rng.nextFloat() * 200.0f;
            float z = (-1.0f + rng.nextFloat() * 2.0f) * 400.0f;
            float q = 0.5f;

            v.add(x - q); v.add(y); v.add(z);
            v.add(x + q); v.add(y); v.add(z);
            v.add(x + q); v.add(y + q); v.add(z);

            v.add(x - q); v.add(y); v.add(z);
            v.add(x + q); v.add(y + q); v.add(z);
            v.add(x - q); v.add(y + q); v.add(z);

            for (int k = 0; k < 6; k++) {
                c.add((byte) 255); c.add((byte) 255); c.add((byte) 255); c.add((byte) 255);
            }
        }

        float[] vArr = new float[v.size()];
        for (int i = 0; i < v.size(); i++) vArr[i] = v.get(i);
        byte[] cArr = new byte[c.size()];
        for (int i = 0; i < c.size(); i++) cArr[i] = c.get(i);

        starModel = new Mesh(vArr, new float[vArr.length / 3 * 2], new float[vArr.length], cArr);
    }

    public void updateClouds(Vector3f playerPos, float time) {
        float dist = lastCloudPos.distance(playerPos);
        boolean timePassed = (time - lastCloudUpdateTime) > 30.0f;
        if (dist < 128.0f && !timePassed && cloudModel != null) {
            return;
        }

        cloudModel = null;
        lastCloudPos.set(playerPos);
        lastCloudUpdateTime = time;

        float cloudHeight = CLOUD_HEIGHT;
        float cloudSize = 16.0f;
        int range = 64;
        int startX = (int) Math.floor(playerPos.x / cloudSize) - range;
        int startZ = (int) Math.floor(playerPos.z / cloudSize) - range;

        List<Float> v = new ArrayList<>();
        List<Byte> c = new ArrayList<>();
        List<Float> n = new ArrayList<>();
        byte[] cloudColor = {(byte) 225, (byte) 233, (byte) 240, (byte) 150};

        for (int x = startX; x < startX + range * 2; x++) {
            for (int z = startZ; z < startZ + range * 2; z++) {
                float drift = time * 0.0005f;
                float base = Noise.perlin2d(x * 0.18f + drift, z * 0.18f + 41.0f, 0.5f, 2);
                float breakup = Noise.perlin2d(x * 0.48f + drift, z * 0.48f + 7.0f, 0.5f, 1);
                if (base > 0.10f && breakup > -0.08f) {
                    float px = x * cloudSize;
                    float py = cloudHeight;
                    float pz = z * cloudSize;
                    float sx = cloudSize;
                    float sy = 1.1f;
                    float sz = cloudSize;

                    float[] cube = {
                        px, py + sy, pz,
                        px, py + sy, pz + sz,
                        px + sx, py + sy, pz + sz,
                        px, py + sy, pz,
                        px + sx, py + sy, pz + sz,
                        px + sx, py + sy, pz,

                        px, py, pz,
                        px + sx, py, pz + sz,
                        px, py, pz + sz,
                        px, py, pz,
                        px + sx, py, pz,
                        px + sx, py, pz + sz,

                        px, py, pz + sz,
                        px + sx, py, pz + sz,
                        px + sx, py + sy, pz + sz,
                        px, py, pz + sz,
                        px + sx, py + sy, pz + sz,
                        px, py + sy, pz + sz,

                        px + sx, py, pz,
                        px, py, pz,
                        px, py + sy, pz,
                        px + sx, py, pz,
                        px, py + sy, pz,
                        px + sx, py + sy, pz,

                        px, py, pz,
                        px, py, pz + sz,
                        px, py + sy, pz + sz,
                        px, py, pz,
                        px, py + sy, pz + sz,
                        px, py + sy, pz,

                        px + sx, py, pz + sz,
                        px + sx, py, pz,
                        px + sx, py + sy, pz,
                        px + sx, py, pz + sz,
                        px + sx, py + sy, pz,
                        px + sx, py + sy, pz + sz
                    };

                    for (float f : cube) v.add(f);
                    for (int k = 0; k < 36; k++) {
                        for (byte b : cloudColor) c.add(b);
                        n.add(0.0f); n.add(1.0f); n.add(0.0f);
                    }
                }
            }
        }

        if (!v.isEmpty()) {
            float[] vArr = new float[v.size()];
            for (int i = 0; i < v.size(); i++) vArr[i] = v.get(i);
            float[] nArr = new float[n.size()];
            for (int i = 0; i < n.size(); i++) nArr[i] = n.get(i);
            byte[] cArr = new byte[c.size()];
            for (int i = 0; i < c.size(); i++) cArr[i] = c.get(i);

            cloudModel = new Mesh(vArr, new float[vArr.length / 3 * 2], nArr, cArr);
        }
    }

    public void computeVisibleChunks(Frustum frustum) {
        visibleChunks.clear();
        for (int i = 0; i < CHUNK_POOL_SIZE; i++) {
            Chunk chunk = chunks[i];
            if (chunk != null) {
                Vector3f min = new Vector3f(chunk.x * Chunk.WIDTH, 0, chunk.z * Chunk.DEPTH);
                Vector3f max = new Vector3f(min.x + Chunk.WIDTH, Chunk.HEIGHT, min.z + Chunk.DEPTH);
                if (frustum.isBoxVisible(min, max)) {
                    visibleChunks.add(i);
                }
            }
        }
    }

    public void renderOpaque(Frustum frustum) {
        if (atlas != null) {
            atlas.bind(0);
        }
        for (int i : visibleChunks) {
            Chunk chunk = chunks[i];
            if (chunk != null && chunk.meshOpaque != null) {
                chunk.meshOpaque.draw();
            }
        }
    }

    public void renderTransparent(Frustum frustum) {
        GL11.glEnable(GL11.GL_BLEND);
        GL11.glBlendFunc(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA);
        GL11.glEnable(GL11.GL_CULL_FACE);

        for (int i : visibleChunks) {
            Chunk chunk = chunks[i];
            if (chunk != null && chunk.meshTransparent != null) {
                chunk.meshTransparent.draw();
            }
        }
    }

    public void renderWater(Frustum frustum) {
        GL11.glEnable(GL11.GL_BLEND);
        GL11.glBlendFunc(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA);
        GL11.glDepthMask(false);
        GL11.glEnable(GL11.GL_CULL_FACE);

        for (int i : visibleChunks) {
            Chunk chunk = chunks[i];
            if (chunk != null && chunk.meshWater != null) {
                chunk.meshWater.draw();
            }
        }
        GL11.glDepthMask(true);
    }

    public void renderClouds(Shader shader, Matrix4f mvp) {
        if (cloudModel != null) {
            shader.bind();
            shader.setMat4(shader.getUniformLocation("uMVP"), mvp);
            shader.setFloat(shader.getUniformLocation("uIsCircle"), 0f);

            GL11.glDisable(GL11.GL_CULL_FACE);
            GL11.glEnable(GL11.GL_BLEND);
            GL11.glBlendFunc(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA);
            GL11.glDepthMask(false);

            cloudModel.draw();

            GL11.glDepthMask(true);
            GL11.glDisable(GL11.GL_BLEND);
            GL11.glEnable(GL11.GL_CULL_FACE);
        }
    }

    public void renderStars(Vector3f playerPos, float time, Shader shader, Matrix4f mvp) {
        float angle = (time / 1200.0f) * 2.0f * (float) Math.PI;
        float sunY = (float) Math.sin(angle);
        if (sunY > -0.3f) {
            return;
        }
        float starIntensity = Math.max(0.0f, Math.min(1.0f, (1.0f - ((sunY + 0.3f) / 0.3f))));
        int starAlpha = (int)(200.0f * starIntensity);
        if (starAlpha < 30) {
            return;
        }
        if (starModel != null) {
            shader.bind();
            shader.setFloat(shader.getUniformLocation("uIsCircle"), 0f);
            Matrix4f starMvp = new Matrix4f(mvp).translate(playerPos);
            shader.setMat4(shader.getUniformLocation("uMVP"), starMvp);

            GL11.glDisable(GL11.GL_DEPTH_TEST);
            starModel.draw();
            GL11.glEnable(GL11.GL_DEPTH_TEST);
        }
    }

    public byte getLight(int x, int y, int z) {
        if (y < 0 || y >= Chunk.HEIGHT) {
            return 15;
        }
        int cx = Math.floorDiv(x, Chunk.WIDTH);
        int cz = Math.floorDiv(z, Chunk.DEPTH);
        Chunk chunk = getChunk(cx, cz);
        if (chunk != null) {
            int bx = Math.floorMod(x, Chunk.WIDTH);
            int bz = Math.floorMod(z, Chunk.DEPTH);
            return chunk.getLight(bx, y, bz);
        }
        return 15;
    }

    public void cleanup() {
        meshingExecutor.shutdown();
        try {
            meshingExecutor.awaitTermination(2, TimeUnit.SECONDS);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
        }
        if (starModel != null) starModel.cleanup();
        if (cloudModel != null) cloudModel.cleanup();
        if (cubeMesh != null) cubeMesh.cleanup();
        for (Chunk chunk : chunks) {
            if (chunk != null) {
                chunk.cleanup();
            }
        }
    }
}
