package com.voxelpopuli.client.world;

import com.voxelpopuli.client.block.BlockType;
import org.joml.Vector3f;
import java.util.*;

public class Explosion {
    public static void explode(World world, int x, int y, int z, int initialRadius, boolean isNuke) {
        float totalRadius = initialRadius;
        int nukeCount = 1;

        if (isNuke) {
            // Exponential Multiplication: Check adjacent blocks for more Nukes
            List<int[]> queue = new ArrayList<>();
            queue.add(new int[]{x, y, z});
            Set<String> checked = new HashSet<>();
            checked.add(x + "," + y + "," + z);

            while (!queue.isEmpty()) {
                int[] curr = queue.remove(queue.size() - 1);
                int qx = curr[0], qy = curr[1], qz = curr[2];
                int[][] neighbors = {
                    {1, 0, 0}, {-1, 0, 0},
                    {0, 1, 0}, {0, -1, 0},
                    {0, 0, 1}, {0, 0, -1}
                };
                for (int[] d : neighbors) {
                    int nx = qx + d[0];
                    int ny = qy + d[1];
                    int nz = qz + d[2];
                    String key = nx + "," + ny + "," + nz;
                    if (!checked.contains(key) && world.getBlock(nx, ny, nz) == BlockType.NUKE) {
                        world.setBlock(nx, ny, nz, BlockType.AIR);
                        nukeCount += 1;
                        queue.add(new int[]{nx, ny, nz});
                        checked.add(key);
                    }
                }
            }

            // Radius = BASE * 1.1^(nuke_count - 1) up to a tighter cap
            totalRadius = initialRadius * (float) Math.pow(1.1f, nukeCount - 1);
            if (totalRadius > 45.0f) {
                totalRadius = 45.0f;
            }
        }

        int rInt = (int) totalRadius;
        float r2 = totalRadius * totalRadius;

        // Perform destruction
        int lastCx = Integer.MAX_VALUE;
        int lastCz = Integer.MAX_VALUE;

        for (int dx = -rInt; dx <= rInt; dx++) {
            for (int dy = -rInt; dy <= rInt; dy++) {
                for (int dz = -rInt; dz <= rInt; dz++) {
                    float distSq = dx * dx + dy * dy + dz * dz;
                    if (distSq <= r2 + 0.5f) {
                        int bx = x + dx;
                        int by = y + dy;
                        int bz = z + dz;

                        BlockType b = world.getBlock(bx, by, bz);
                        if (b != BlockType.AIR && b != BlockType.BEDROCK) {
                            world.setBlock(bx, by, bz, BlockType.AIR);
                            int cx = Math.floorDiv(bx, 16);
                            int cz = Math.floorDiv(bz, 16);
                            if (lastCx != cx || lastCz != cz) {
                                lastCx = cx;
                                lastCz = cz;
                            }
                        }
                    }
                }
            }
        }

        // Spawn Particles (Shockwave/Flash)
        // 1. Core Shockwave
        world.particles.add(new Particle(
            new Vector3f(x + 0.1f, y + 0.1f, z + 0.1f),
            new Vector3f(0, 0, 0),
            0.5f,
            0.5f,
            totalRadius * 2.0f
        ));

        // 2. Flying debris/smoke
        int particleCount = isNuke ? 30 : 10;
        Random rng = new Random();
        for (int p = 0; p < particleCount; p++) {
            float vx = -30.0f + rng.nextFloat() * 60.0f;
            float vy = 5.0f + rng.nextFloat() * 35.0f;
            float vz = -30.0f + rng.nextFloat() * 60.0f;
            float life = 1.0f + rng.nextFloat() * 2.0f;
            float scale = 0.2f + rng.nextFloat() * 1.3f;
            world.particles.add(new Particle(
                new Vector3f(x + 0.5f, y + 0.5f, z + 0.5f),
                new Vector3f(vx, vy, vz),
                life,
                3.0f,
                scale
            ));
        }
    }
}
