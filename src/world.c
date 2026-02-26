#include "world.h"
#include <math.h>
#include <stdlib.h>

// O(1) Chunk lookup using modulo-grid mapping
Chunk* World_GetChunk(World *world, int cx, int cz) {
    int ix = ((cx % POOL_WIDTH) + POOL_WIDTH) % POOL_WIDTH;
    int iz = ((cz % POOL_WIDTH) + POOL_WIDTH) % POOL_WIDTH;
    int index = ix + iz * POOL_WIDTH;
    Chunk *c = &world->chunks[index];
    if (c->x == cx && c->z == cz) return c;
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
    int ix = ((cx % POOL_WIDTH) + POOL_WIDTH) % POOL_WIDTH;
    int iz = ((cz % POOL_WIDTH) + POOL_WIDTH) % POOL_WIDTH;
    Chunk *chunk = &world->chunks[ix + iz * POOL_WIDTH];
    
    if (chunk->x != cx || chunk->z != cz) return; // Not the right chunk loaded here

    int bx = x - (cx * CHUNK_WIDTH);
    int bz = z - (cz * CHUNK_DEPTH);
    chunk->blocks[bx][y][bz] = block;
    chunk->dirty = true;
    
    // Mark neighbors dirty
    if (bx == 0) { Chunk *n = World_GetChunk(world, cx-1, cz); if(n) n->dirty = true; }
    if (bx == CHUNK_WIDTH-1) { Chunk *n = World_GetChunk(world, cx+1, cz); if(n) n->dirty = true; }
    if (bz == 0) { Chunk *n = World_GetChunk(world, cx, cz-1); if(n) n->dirty = true; }
    if (bz == CHUNK_DEPTH-1) { Chunk *n = World_GetChunk(world, cx, cz+1); if(n) n->dirty = true; }
}

RaycastResult World_Raycast(World *world, Vector3 origin, Vector3 direction, float maxDistance) {
    RaycastResult result = { 0 };
    Vector3 pos = origin;
    float step = 0.05f;
    Vector3 lastPos = origin;
    for (float d = 0; d < maxDistance; d += step) {
        pos.x += direction.x * step; pos.y += direction.y * step; pos.z += direction.z * step;
        int ix = (int)floor(pos.x); int iy = (int)floor(pos.y); int iz = (int)floor(pos.z);
        if (iy >= 0 && iy < CHUNK_HEIGHT) {
            BlockType b = World_GetBlock(world, ix, iy, iz);
            if (b != BLOCK_AIR && b != BLOCK_WATER) {
                result.hit = true; result.x = ix; result.y = iy; result.z = iz; result.block = b;
                int lx = (int)floor(lastPos.x); int ly = (int)floor(lastPos.y); int lz = (int)floor(lastPos.z);
                result.nx = lx - ix; result.ny = ly - iy; result.nz = lz - iz;
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
            if (x == 0 || y == 0 || x == 15 || y == 15) { c.r *= 0.9f; c.g *= 0.9f; c.b *= 0.9f; }
            ImageDrawPixel(img, tx * 16 + x, ty * 16 + y, c);
        }
    }
}

void World_Init(World *world) {
    Image img = GenImageColor(256, 256, BLANK);
    DrawPixelatedBlock(&img, 0, 0, (Color){110, 180, 80, 255}, 0.2f);
    DrawPixelatedBlock(&img, 1, 0, (Color){140, 140, 140, 255}, 0.15f);
    DrawPixelatedBlock(&img, 2, 0, (Color){130, 100, 70, 255}, 0.2f);
    DrawPixelatedBlock(&img, 3, 0, (Color){120, 160, 70, 255}, 0.3f);
    DrawPixelatedBlock(&img, 4, 0, (Color){100, 80, 50, 255}, 0.15f);
    DrawPixelatedBlock(&img, 5, 0, (Color){40, 100, 40, 255}, 0.4f);
    DrawPixelatedBlock(&img, 1, 1, (Color){60, 60, 60, 255}, 0.2f);
    DrawPixelatedBlock(&img, 5, 1, (Color){110, 90, 60, 255}, 0.1f);
    for (int y = 0; y < 16; y++) for (int x = 0; x < 16; x++) ImageDrawPixel(&img, 13 * 16 + x, 12 * 16 + y, (Color){40, 100, 200, 140});
    DrawPixelatedBlock(&img, 6, 0, (Color){220, 210, 160, 255}, 0.15f);
    DrawPixelatedBlock(&img, 7, 0, (Color){120, 120, 130, 255}, 0.2f);
    world->atlas = LoadTextureFromImage(img);
    UnloadImage(img);
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) Chunk_Init(&world->chunks[i], 999999, 999999);
}

void World_Update(World *world, Vector3 playerPos) {
    int pcx = (int)floor(playerPos.x / CHUNK_WIDTH);
    int pcz = (int)floor(playerPos.z / CHUNK_DEPTH);

    // Efficiently check and recycle chunks within the visible range
    for (int x = pcx - VIEW_DISTANCE; x <= pcx + VIEW_DISTANCE; x++) {
        for (int z = pcz - VIEW_DISTANCE; z <= pcz + VIEW_DISTANCE; z++) {
            int ix = ((x % POOL_WIDTH) + POOL_WIDTH) % POOL_WIDTH;
            int iz = ((z % POOL_WIDTH) + POOL_WIDTH) % POOL_WIDTH;
            int index = ix + iz * POOL_WIDTH;
            Chunk *c = &world->chunks[index];

            if (c->x != x || c->z != z) {
                Chunk_Unload(c);
                Chunk_Init(c, x, z);
                Chunk_Generate(c);
                // Mark neighbors dirty
                Chunk *n;
                if ((n = World_GetChunk(world, x-1, z))) n->dirty = true;
                if ((n = World_GetChunk(world, x+1, z))) n->dirty = true;
                if ((n = World_GetChunk(world, x, z-1))) n->dirty = true;
                if ((n = World_GetChunk(world, x, z+1))) n->dirty = true;
            }
        }
    }

    // Limit rebuilds to 1 per frame at 180Hz to ensure smooth performance
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) {
        if (world->chunks[i].dirty) {
            Chunk_BuildMesh(&world->chunks[i], world);
            if (world->chunks[i].modelOpaque.meshCount > 0)
                world->chunks[i].modelOpaque.materials[0].maps[MATERIAL_MAP_DIFFUSE].texture = world->atlas;
            if (world->chunks[i].modelTransparent.meshCount > 0)
                world->chunks[i].modelTransparent.materials[0].maps[MATERIAL_MAP_DIFFUSE].texture = world->atlas;
            break; // ONLY ONE PER FRAME
        }
    }
}

void World_Render(World *world) {
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) if (world->chunks[i].x != 999999) Chunk_RenderOpaque(&world->chunks[i]);
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) if (world->chunks[i].x != 999999) Chunk_RenderTransparent(&world->chunks[i]);
}

void World_Unload(World *world) {
    UnloadTexture(world->atlas);
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) Chunk_Unload(&world->chunks[i]);
}
