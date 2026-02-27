#include "world.h"
#include "rlgl.h"
#include "noise.h"
#include <math.h>
#include <stdlib.h>
#include <stdio.h>
#include <stdint.h>

#define WATER_TILE_X 13
#define WATER_TILE_Y 12
#define TILE_SIZE 16
#define WORLD_SAVE_MAGIC 0x31505756u
#define WORLD_SAVE_VERSION 1u

typedef struct {
    uint32_t magic;
    uint32_t version;
    int32_t count;
} WorldSaveHeader;

static int World_FindEditIndex(World *world, int x, int y, int z) {
    for (int i = 0; i < world->editCount; i++) {
        WorldEdit *edit = &world->edits[i];
        if (edit->x == x && edit->y == y && edit->z == z) return i;
    }
    return -1;
}

static bool World_EnsureEditCapacity(World *world, int minCapacity) {
    if (world->editCapacity >= minCapacity) return true;
    int newCapacity = (world->editCapacity > 0) ? world->editCapacity : 256;
    while (newCapacity < minCapacity) newCapacity *= 2;
    WorldEdit *newEdits = (WorldEdit *)realloc(world->edits, (size_t)newCapacity * sizeof(WorldEdit));
    if (!newEdits) return false;
    world->edits = newEdits;
    world->editCapacity = newCapacity;
    return true;
}

static bool World_AddOrUpdateEdit(World *world, int x, int y, int z, BlockType block) {
    int index = World_FindEditIndex(world, x, y, z);
    if (index >= 0) {
        world->edits[index].block = (unsigned char)block;
        return true;
    }
    if (!World_EnsureEditCapacity(world, world->editCount + 1)) return false;
    WorldEdit *edit = &world->edits[world->editCount++];
    edit->x = x;
    edit->y = y;
    edit->z = z;
    edit->block = (unsigned char)block;
    return true;
}

static bool World_ApplyEditsToChunk(World *world, Chunk *chunk) {
    bool changed = false;
    int minX = chunk->x * CHUNK_WIDTH;
    int minZ = chunk->z * CHUNK_DEPTH;
    int maxX = minX + CHUNK_WIDTH;
    int maxZ = minZ + CHUNK_DEPTH;

    for (int i = 0; i < world->editCount; i++) {
        WorldEdit *edit = &world->edits[i];
        if (edit->x < minX || edit->x >= maxX || edit->z < minZ || edit->z >= maxZ) continue;
        if (edit->y < 0 || edit->y >= CHUNK_HEIGHT) continue;

        int bx = edit->x - minX;
        int bz = edit->z - minZ;
        BlockType target = (BlockType)edit->block;
        if (chunk->blocks[bx][edit->y][bz] != target) {
            chunk->blocks[bx][edit->y][bz] = target;
            changed = true;
        }
    }

    if (changed) chunk->dirty = true;
    return changed;
}

static unsigned char ClampToByte(int value) {
    if (value < 0) return 0;
    if (value > 255) return 255;
    return (unsigned char)value;
}

static float Saturate(float value) {
    if (value < 0.0f) return 0.0f;
    if (value > 1.0f) return 1.0f;
    return value;
}

static float Hash01(int x, int y, unsigned int seed) {
    unsigned int h = (unsigned int)x * 374761393u + (unsigned int)y * 668265263u + seed * 2246822519u;
    h = (h ^ (h >> 13u)) * 1274126177u;
    h ^= h >> 16u;
    return (float)(h & 0x00FFFFFFu) / 16777215.0f;
}

static void BuildWaterTile(Color *pixels, float time) {
    float invSize = 1.0f / (float)TILE_SIZE;
    float driftX = time * 0.16f;
    float driftY = -time * 0.11f;

    for (int y = 0; y < TILE_SIZE; y++) {
        for (int x = 0; x < TILE_SIZE; x++) {
            float u = ((float)x + 0.5f) * invSize;
            float v = ((float)y + 0.5f) * invSize;

            float warpX = Perlin2D((u + driftX) * 3.4f + 7.2f, (v + driftY) * 3.4f - 2.9f, 1.0f, 2);
            float warpY = Perlin2D((u - driftY) * 3.1f - 4.8f, (v + driftX) * 3.1f + 11.3f, 1.0f, 2);
            float du = u + warpX * 0.09f;
            float dv = v + warpY * 0.09f;

            float body = Perlin2D((du + driftX) * 6.0f + 19.1f, (dv + driftY) * 6.0f + 3.7f, 1.0f, 3);
            float detail = Perlin2D((du - driftY * 0.7f) * 11.0f - 8.4f, (dv + driftX * 0.7f) * 11.0f + 14.2f, 1.0f, 2);
            float foam = Perlin2D((u + driftX * 1.4f) * 14.0f + 27.1f, (v + driftY * 1.2f) * 14.0f - 31.4f, 1.0f, 1);
            float jitter = (Hash01(x, y, 97u) - 0.5f) * 0.08f;

            float mix = Saturate(0.5f + body * 0.42f + detail * 0.20f + jitter);

            int r = (int)(24.0f + mix * 38.0f);
            int g = (int)(92.0f + mix * 58.0f);
            int b = (int)(205.0f + mix * 48.0f);
            int a = 228;

            if (detail < -0.35f) {
                r -= 10;
                g -= 8;
                b -= 5;
            }
            if (foam > 0.48f) {
                r += 8;
                g += 12;
                b += 16;
            }

            Color c = {
                ClampToByte(r),
                ClampToByte(g),
                ClampToByte(b),
                ClampToByte(a)
            };
            pixels[y * TILE_SIZE + x] = c;
        }
    }
}

static void World_UpdateWaterAtlas(World *world, float time) {
    int frame = (int)floorf(time * 10.0f);
    if (frame == world->waterAnimFrame) return;

    Color pixels[TILE_SIZE * TILE_SIZE];
    BuildWaterTile(pixels, time);
    Rectangle rect = {
        (float)(WATER_TILE_X * TILE_SIZE),
        (float)(WATER_TILE_Y * TILE_SIZE),
        (float)TILE_SIZE,
        (float)TILE_SIZE
    };
    UpdateTextureRec(world->atlas, rect, pixels);
    world->waterAnimFrame = frame;
}

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
    if (chunk->x != cx || chunk->z != cz) return;
    int bx = x - (cx * CHUNK_WIDTH);
    int bz = z - (cz * CHUNK_DEPTH);
    chunk->blocks[bx][y][bz] = block;
    chunk->dirty = true;
    if (bx == 0) { Chunk *n = World_GetChunk(world, cx-1, cz); if(n) n->dirty = true; }
    if (bx == CHUNK_WIDTH-1) { Chunk *n = World_GetChunk(world, cx+1, cz); if(n) n->dirty = true; }
    if (bz == 0) { Chunk *n = World_GetChunk(world, cx, cz-1); if(n) n->dirty = true; }
    if (bz == CHUNK_DEPTH-1) { Chunk *n = World_GetChunk(world, cx, cz+1); if(n) n->dirty = true; }
    if (!world->suppressEditRecording) World_AddOrUpdateEdit(world, x, y, z, block);
}

RaycastResult World_Raycast(World *world, Vector3 origin, Vector3 direction, float maxDistance) {
    RaycastResult result = { 0 };
    Vector3 pos = origin; float step = 0.05f; Vector3 lastPos = origin;
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
            c.r = (unsigned char)fmax(0, fmin(255, (int)c.r + n * 255));
            c.g = (unsigned char)fmax(0, fmin(255, (int)c.g + n * 255));
            c.b = (unsigned char)fmax(0, fmin(255, (int)c.b + n * 255));
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
    Color waterPixels[TILE_SIZE * TILE_SIZE];
    BuildWaterTile(waterPixels, 0.0f);
    for (int y = 0; y < TILE_SIZE; y++) {
        for (int x = 0; x < TILE_SIZE; x++) {
            ImageDrawPixel(&img, WATER_TILE_X * TILE_SIZE + x, WATER_TILE_Y * TILE_SIZE + y, waterPixels[y * TILE_SIZE + x]);
        }
    }
    DrawPixelatedBlock(&img, 6, 0, (Color){220, 210, 160, 255}, 0.15f);
    DrawPixelatedBlock(&img, 7, 0, (Color){120, 120, 130, 255}, 0.2f);
    world->atlas = LoadTextureFromImage(img);
    UnloadImage(img);
    world->waterAnimFrame = -1;
    world->edits = NULL;
    world->editCount = 0;
    world->editCapacity = 0;
    world->suppressEditRecording = false;
    World_UpdateWaterAtlas(world, 0.0f);
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) Chunk_Init(&world->chunks[i], 999999, 999999);
}

void World_Update(World *world, Vector3 playerPos) {
    World_UpdateWaterAtlas(world, (float)GetTime());
    int pcx = (int)floor(playerPos.x / CHUNK_WIDTH);
    int pcz = (int)floor(playerPos.z / CHUNK_DEPTH);
    for (int x = pcx - VIEW_DISTANCE; x <= pcx + VIEW_DISTANCE; x++) {
        for (int z = pcz - VIEW_DISTANCE; z <= pcz + VIEW_DISTANCE; z++) {
            int ix = ((x % POOL_WIDTH) + POOL_WIDTH) % POOL_WIDTH;
            int iz = ((z % POOL_WIDTH) + POOL_WIDTH) % POOL_WIDTH;
            int index = ix + iz * POOL_WIDTH;
            Chunk *c = &world->chunks[index];
            if (c->x != x || c->z != z) {
                Chunk_Unload(c); Chunk_Init(c, x, z); Chunk_Generate(c);
                World_ApplyEditsToChunk(world, c);
                Chunk *n;
                if ((n = World_GetChunk(world, x-1, z))) n->dirty = true;
                if ((n = World_GetChunk(world, x+1, z))) n->dirty = true;
                if ((n = World_GetChunk(world, x, z-1))) n->dirty = true;
                if ((n = World_GetChunk(world, x, z+1))) n->dirty = true;
            }
        }
    }
    int buildsThisFrame = 0;
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) {
        if (world->chunks[i].dirty) {
            Chunk_BuildMesh(&world->chunks[i], world);
            if (world->chunks[i].modelOpaque.meshCount > 0) world->chunks[i].modelOpaque.materials[0].maps[MATERIAL_MAP_DIFFUSE].texture = world->atlas;
            if (world->chunks[i].modelTransparent.meshCount > 0) world->chunks[i].modelTransparent.materials[0].maps[MATERIAL_MAP_DIFFUSE].texture = world->atlas;
            if (++buildsThisFrame >= 4) break; 
        }
    }
}

void World_Render(World *world, Frustum frustum) {
    static bool *visibleChunks = NULL;
    if (visibleChunks == NULL) visibleChunks = (bool *)malloc(CHUNK_POOL_SIZE * sizeof(bool));

    for (int i = 0; i < CHUNK_POOL_SIZE; i++) {
        Chunk *c = &world->chunks[i];
        visibleChunks[i] = false;
        if (c->x == 999999) continue;
        
        float x1 = (float)c->x * CHUNK_WIDTH;
        float y1 = 0;
        float z1 = (float)c->z * CHUNK_DEPTH;
        float x2 = x1 + CHUNK_WIDTH;
        float y2 = CHUNK_HEIGHT;
        float z2 = z1 + CHUNK_DEPTH;

        bool inFrustum = true;
        for (int p = 0; p < 6; p++) {
            if (frustum.planes[p][0] * x1 + frustum.planes[p][1] * y1 + frustum.planes[p][2] * z1 + frustum.planes[p][3] > 0) continue;
            if (frustum.planes[p][0] * x2 + frustum.planes[p][1] * y1 + frustum.planes[p][2] * z1 + frustum.planes[p][3] > 0) continue;
            if (frustum.planes[p][0] * x1 + frustum.planes[p][1] * y2 + frustum.planes[p][2] * z1 + frustum.planes[p][3] > 0) continue;
            if (frustum.planes[p][0] * x2 + frustum.planes[p][1] * y2 + frustum.planes[p][2] * z1 + frustum.planes[p][3] > 0) continue;
            if (frustum.planes[p][0] * x1 + frustum.planes[p][1] * y1 + frustum.planes[p][2] * z2 + frustum.planes[p][3] > 0) continue;
            if (frustum.planes[p][0] * x2 + frustum.planes[p][1] * y1 + frustum.planes[p][2] * z2 + frustum.planes[p][3] > 0) continue;
            if (frustum.planes[p][0] * x1 + frustum.planes[p][1] * y2 + frustum.planes[p][2] * z2 + frustum.planes[p][3] > 0) continue;
            if (frustum.planes[p][0] * x2 + frustum.planes[p][1] * y2 + frustum.planes[p][2] * z2 + frustum.planes[p][3] > 0) continue;
            inFrustum = false;
            break;
        }
        visibleChunks[i] = inFrustum;
        if (inFrustum) Chunk_RenderOpaque(c);
    }
    
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) {
        if (visibleChunks[i]) Chunk_RenderTransparent(&world->chunks[i]);
    }
}

void World_RenderClouds(World *world, Vector3 playerPos, float time, Frustum frustum) {
    (void)world;
    float cloudHeight = fmaxf(110.0f, playerPos.y + 24.0f);
    float cloudSize = 8.0f;
    int range = 64;
    int startX = (int)floor(playerPos.x / cloudSize) - range;
    int startZ = (int)floor(playerPos.z / cloudSize) - range;
    
    rlDisableBackfaceCulling();
    for (int x = startX; x < startX + range * 2; x++) {
        for (int z = startZ; z < startZ + range * 2; z++) {
            float x1 = (float)x * cloudSize;
            float z1 = (float)z * cloudSize;
            float x2 = x1 + cloudSize;
            float z2 = z1 + cloudSize;
            float y1 = cloudHeight;
            float y2 = cloudHeight + 1.1f;

            bool inFrustum = true;
            for (int p = 0; p < 6; p++) {
                if (frustum.planes[p][0] * x1 + frustum.planes[p][1] * y1 + frustum.planes[p][2] * z1 + frustum.planes[p][3] > 0) continue;
                if (frustum.planes[p][0] * x2 + frustum.planes[p][1] * y1 + frustum.planes[p][2] * z1 + frustum.planes[p][3] > 0) continue;
                if (frustum.planes[p][0] * x1 + frustum.planes[p][1] * y2 + frustum.planes[p][2] * z1 + frustum.planes[p][3] > 0) continue;
                if (frustum.planes[p][0] * x2 + frustum.planes[p][1] * y2 + frustum.planes[p][2] * z1 + frustum.planes[p][3] > 0) continue;
                if (frustum.planes[p][0] * x1 + frustum.planes[p][1] * y1 + frustum.planes[p][2] * z2 + frustum.planes[p][3] > 0) continue;
                if (frustum.planes[p][0] * x2 + frustum.planes[p][1] * y1 + frustum.planes[p][2] * z2 + frustum.planes[p][3] > 0) continue;
                if (frustum.planes[p][0] * x1 + frustum.planes[p][1] * y2 + frustum.planes[p][2] * z2 + frustum.planes[p][3] > 0) continue;
                if (frustum.planes[p][0] * x2 + frustum.planes[p][1] * y2 + frustum.planes[p][2] * z2 + frustum.planes[p][3] > 0) continue;
                inFrustum = false;
                break;
            }
            if (!inFrustum) continue;

            // Two noise fields keep clouds plentiful while breaking large continuous slabs.
            float base = Perlin2D((float)x * 0.09f + time * 0.003f, (float)z * 0.09f + 41.0f, 0.5f, 2);
            float breakup = Perlin2D((float)x * 0.24f - time * 0.005f, (float)z * 0.24f + 7.0f, 0.5f, 1);
            if (base > 0.10f && breakup > -0.08f) {
                Vector3 pos = { x * cloudSize + cloudSize/2.0f, cloudHeight, z * cloudSize + cloudSize/2.0f };
                DrawCube(pos, cloudSize, 1.1f, cloudSize, (Color){225, 233, 240, 150});
            }
        }
    }
    rlEnableBackfaceCulling();
}

void World_Unload(World *world) {
    UnloadTexture(world->atlas);
    for (int i = 0; i < CHUNK_POOL_SIZE; i++) Chunk_Unload(&world->chunks[i]);
    free(world->edits);
    world->edits = NULL;
    world->editCount = 0;
    world->editCapacity = 0;
}

bool World_SaveEdits(World *world, const char *path) {
    FILE *file = fopen(path, "wb");
    if (!file) return false;

    WorldSaveHeader header = {
        WORLD_SAVE_MAGIC,
        WORLD_SAVE_VERSION,
        (int32_t)world->editCount
    };
    if (fwrite(&header, sizeof(header), 1, file) != 1) {
        fclose(file);
        return false;
    }

    for (int i = 0; i < world->editCount; i++) {
        int32_t x = (int32_t)world->edits[i].x;
        int32_t y = (int32_t)world->edits[i].y;
        int32_t z = (int32_t)world->edits[i].z;
        uint8_t block = (uint8_t)world->edits[i].block;

        if (fwrite(&x, sizeof(x), 1, file) != 1 ||
            fwrite(&y, sizeof(y), 1, file) != 1 ||
            fwrite(&z, sizeof(z), 1, file) != 1 ||
            fwrite(&block, sizeof(block), 1, file) != 1) {
            fclose(file);
            return false;
        }
    }

    fclose(file);
    return true;
}

bool World_LoadEdits(World *world, const char *path) {
    FILE *file = fopen(path, "rb");
    if (!file) return true;

    WorldSaveHeader header = { 0 };
    if (fread(&header, sizeof(header), 1, file) != 1) {
        fclose(file);
        return false;
    }
    if (header.magic != WORLD_SAVE_MAGIC || header.version != WORLD_SAVE_VERSION || header.count < 0 || header.count > 5000000) {
        fclose(file);
        return false;
    }

    world->editCount = 0;
    if (!World_EnsureEditCapacity(world, header.count)) {
        fclose(file);
        return false;
    }

    for (int i = 0; i < header.count; i++) {
        int32_t x = 0, y = 0, z = 0;
        uint8_t block = 0;
        if (fread(&x, sizeof(x), 1, file) != 1 ||
            fread(&y, sizeof(y), 1, file) != 1 ||
            fread(&z, sizeof(z), 1, file) != 1 ||
            fread(&block, sizeof(block), 1, file) != 1) {
            fclose(file);
            return false;
        }
        if (y < 0 || y >= CHUNK_HEIGHT) continue;
        if (block > (uint8_t)BLOCK_BEDROCK) continue;
        World_AddOrUpdateEdit(world, (int)x, (int)y, (int)z, (BlockType)block);
    }

    fclose(file);

    for (int i = 0; i < CHUNK_POOL_SIZE; i++) {
        if (world->chunks[i].x == 999999) continue;
        World_ApplyEditsToChunk(world, &world->chunks[i]);
    }
    return true;
}
