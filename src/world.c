#include "world.h"
#include <math.h>
#include <stdlib.h>

Chunk* World_GetChunk(World *world, int cx, int cz) {
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) {
        if (world->chunks[i].x == cx && world->chunks[i].z == cz) return &world->chunks[i];
    }
    return NULL;
}

BlockType World_GetBlock(World *world, int x, int y, int z) {
    if (y < 0 || y >= CHUNK_HEIGHT) return BLOCK_AIR;
    int cx = (int)floor((float)x / CHUNK_WIDTH);
    int cz = (int)floor((float)z / CHUNK_DEPTH);
    Chunk *chunk = World_GetChunk(world, cx, cz);
    if (!chunk) return BLOCK_AIR;
    int bx = x - (cx * CHUNK_WIDTH);
    int bz = z - (cz * CHUNK_DEPTH);
    return chunk->blocks[bx][y][bz];
}

void World_SetBlock(World *world, int x, int y, int z, BlockType block) {
    if (y < 0 || y >= CHUNK_HEIGHT) return;
    int cx = (int)floor((float)x / CHUNK_WIDTH);
    int cz = (int)floor((float)z / CHUNK_DEPTH);
    Chunk *chunk = World_GetChunk(world, cx, cz);
    if (!chunk) return;
    int bx = x - (cx * CHUNK_WIDTH);
    int bz = z - (cz * CHUNK_DEPTH);
    chunk->blocks[bx][y][bz] = block;
    chunk->dirty = true;
}

RaycastResult World_Raycast(World *world, Vector3 origin, Vector3 direction, float maxDistance) {
    RaycastResult result = { 0 };
    Vector3 pos = origin;
    float step = 0.05f;
    Vector3 lastPos = origin;
    for (float d = 0; d < maxDistance; d += step) {
        pos.x += direction.x * step;
        pos.y += direction.y * step;
        pos.z += direction.z * step;
        int ix = (int)floor(pos.x);
        int iy = (int)floor(pos.y);
        int iz = (int)floor(pos.z);
        if (iy >= 0 && iy < CHUNK_HEIGHT) {
            BlockType b = World_GetBlock(world, ix, iy, iz);
            if (b != BLOCK_AIR && b != BLOCK_WATER) {
                result.hit = true;
                result.x = ix; result.y = iy; result.z = iz;
                result.block = b;
                int lx = (int)floor(lastPos.x);
                int ly = (int)floor(lastPos.y);
                int lz = (int)floor(lastPos.z);
                result.nx = lx - ix;
                result.ny = ly - iy;
                result.nz = lz - iz;
                return result;
            }
        }
        lastPos = pos;
    }
    return result;
}

void DrawPixelatedBlock(Image *img, int tx, int ty, Color baseColor, float noiseAmount) {
    for (int y = 0; y < 16; y++) {
        for (int x = 0; x < 16; x++) {
            float n = (float)rand() / (float)RAND_MAX * noiseAmount - noiseAmount/2.0f;
            Color c = baseColor;
            c.r = (unsigned char)fmax(0, fmin(255, c.r + n * 255));
            c.g = (unsigned char)fmax(0, fmin(255, c.g + n * 255));
            c.b = (unsigned char)fmax(0, fmin(255, c.b + n * 255));
            // Add a slight border effect
            if (x == 0 || y == 0 || x == 15 || y == 15) {
                c.r *= 0.9f; c.g *= 0.9f; c.b *= 0.9f;
            }
            ImageDrawPixel(img, tx * 16 + x, ty * 16 + y, c);
        }
    }
}

void World_Init(World *world) {
    Image img = GenImageColor(256, 256, BLANK);
    
    // Generate noisy textures for each block type
    DrawPixelatedBlock(&img, 0, 0, (Color){110, 180, 80, 255}, 0.2f);  // Grass Top
    DrawPixelatedBlock(&img, 1, 0, (Color){140, 140, 140, 255}, 0.15f); // Stone
    DrawPixelatedBlock(&img, 2, 0, (Color){130, 100, 70, 255}, 0.2f);  // Dirt
    DrawPixelatedBlock(&img, 3, 0, (Color){120, 160, 70, 255}, 0.3f);  // Grass Side
    DrawPixelatedBlock(&img, 4, 0, (Color){100, 80, 50, 255}, 0.15f);  // Log side
    DrawPixelatedBlock(&img, 5, 0, (Color){40, 100, 40, 255}, 0.4f);   // Leaves
    DrawPixelatedBlock(&img, 1, 1, (Color){60, 60, 60, 255}, 0.2f);    // Bedrock
    DrawPixelatedBlock(&img, 5, 1, (Color){110, 90, 60, 255}, 0.1f);   // Log top
    DrawPixelatedBlock(&img, 13, 12, (Color){40, 100, 200, 200}, 0.1f); // Water
    
    // Sand & Gravel (using some of the empty slots)
    DrawPixelatedBlock(&img, 6, 0, (Color){220, 210, 160, 255}, 0.15f); // Sand
    DrawPixelatedBlock(&img, 7, 0, (Color){120, 120, 130, 255}, 0.2f);  // Gravel

    world->atlas = LoadTextureFromImage(img);
    UnloadImage(img);

    for (int i = 0; i < CHUNK_POOL_SIZE; i++) {
        Chunk_Init(&world->chunks[i], 999999, 999999);
    }
}

void World_Update(World *world, Vector3 playerPos) {
    int pcx = (int)floor(playerPos.x / CHUNK_WIDTH);
    int pcz = (int)floor(playerPos.z / CHUNK_DEPTH);
    int rebuiltThisFrame = 0;
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) {
        int dx = abs(world->chunks[i].x - pcx);
        int dz = abs(world->chunks[i].z - pcz);
        if (dx > VIEW_DISTANCE || dz > VIEW_DISTANCE) {
            bool found = false;
            for (int x = pcx - VIEW_DISTANCE; x <= pcx + VIEW_DISTANCE; x++) {
                for (int z = pcz - VIEW_DISTANCE; z <= pcz + VIEW_DISTANCE; z++) {
                    if (World_GetChunk(world, x, z) == NULL) {
                        Chunk_Unload(&world->chunks[i]);
                        Chunk_Init(&world->chunks[i], x, z);
                        Chunk_Generate(&world->chunks[i]);
                        found = true; break;
                    }
                }
                if (found) break;
            }
        }
    }
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) {
        if (world->chunks[i].dirty) {
            Chunk_BuildMesh(&world->chunks[i], world);
            if (world->chunks[i].model.meshCount > 0)
                world->chunks[i].model.materials[0].maps[MATERIAL_MAP_DIFFUSE].texture = world->atlas;
            rebuiltThisFrame++;
            if (rebuiltThisFrame >= 2) break;
        }
    }
}

void World_Render(World *world) {
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) {
        if (world->chunks[i].x != 999999) Chunk_Render(&world->chunks[i]);
    }
}

void World_Unload(World *world) {
    UnloadTexture(world->atlas);
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) Chunk_Unload(&world->chunks[i]);
}
