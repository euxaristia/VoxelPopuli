#ifndef WORLD_H
#define WORLD_H

#include "chunk.h"

#define VIEW_DISTANCE 5
#define CHUNK_POOL_SIZE ((VIEW_DISTANCE * 2 + 1) * (VIEW_DISTANCE * 2 + 1))

typedef struct {
    bool hit;
    int x, y, z;
    int nx, ny, nz;
    BlockType block;
} RaycastResult;

typedef struct {
    Chunk chunks[CHUNK_POOL_SIZE];
    Texture2D atlas;
} World;

void World_Init(World *world);
void World_Update(World *world, Vector3 playerPos);
void World_Render(World *world);
void World_Unload(World *world);

BlockType World_GetBlock(World *world, int x, int y, int z);
void World_SetBlock(World *world, int x, int y, int z, BlockType block);
RaycastResult World_Raycast(World *world, Vector3 origin, Vector3 direction, float maxDistance);
Chunk* World_GetChunk(World *world, int cx, int cz);

#endif
