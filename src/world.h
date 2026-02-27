#ifndef WORLD_H
#define WORLD_H

#include "chunk.h"

#define VIEW_DISTANCE 24
#define POOL_WIDTH (VIEW_DISTANCE * 2 + 1)
#define CHUNK_POOL_SIZE (POOL_WIDTH * POOL_WIDTH)

typedef struct {
    float planes[6][4];
} Frustum;

typedef struct {
    bool hit;
    int x, y, z;
    int nx, ny, nz;
    BlockType block;
} RaycastResult;

typedef struct {
    int x, y, z;
    unsigned char block;
} WorldEdit;

typedef struct {
    Chunk chunks[CHUNK_POOL_SIZE];
    Texture2D atlas;
    int waterAnimFrame;
    WorldEdit *edits;
    int editCount;
    int editCapacity;
    bool suppressEditRecording;
} World;

void World_Init(World *world);
void World_Update(World *world, Vector3 playerPos);
void World_Render(World *world, Frustum frustum);
void World_RenderClouds(World *world, Vector3 playerPos, float time, Frustum frustum);
void World_Unload(World *world);
bool World_SaveEdits(World *world, const char *path);
bool World_LoadEdits(World *world, const char *path);

BlockType World_GetBlock(World *world, int x, int y, int z);
void World_SetBlock(World *world, int x, int y, int z, BlockType block);
RaycastResult World_Raycast(World *world, Vector3 origin, Vector3 direction, float maxDistance);
Chunk* World_GetChunk(World *world, int cx, int cz);

#endif
