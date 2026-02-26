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

void World_Init(World *world) {
    Image img = GenImageChecked(256, 256, 16, 16, GREEN, DARKGREEN);
    ImageDrawRectangle(&img, 16, 0, 16, 16, GRAY);         // Stone
    ImageDrawRectangle(&img, 32, 0, 16, 16, BROWN);        // Dirt
    ImageDrawRectangle(&img, 48, 0, 16, 16, LIME);         // Grass Side
    ImageDrawRectangle(&img, 0, 0, 16, 16, GREEN);         // Grass Top
    ImageDrawRectangle(&img, 16, 16, 16, 16, BLACK);       // Bedrock
    ImageDrawRectangle(&img, 64, 0, 16, 16, (Color){101, 67, 33, 255}); // Log side
    ImageDrawRectangle(&img, 80, 16, 16, 16, (Color){80, 50, 20, 255}); // Log top
    ImageDrawRectangle(&img, 80, 0, 16, 16, (Color){0, 100, 0, 255});   // Leaves
    ImageDrawRectangle(&img, 13 * 16, 12 * 16, 16, 16, (Color){0, 121, 241, 255}); // Water
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
