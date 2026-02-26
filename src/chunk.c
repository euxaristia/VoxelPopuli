#include "chunk.h"
#include "noise.h"
#include <stdlib.h>
#include <string.h>

void Chunk_Init(Chunk *chunk, int x, int z) {
    chunk->x = x;
    chunk->z = z;
    memset(chunk->blocks, 0, sizeof(chunk->blocks));
    chunk->dirty = true;
    chunk->mesh = (Mesh){ 0 };
    chunk->model = (Model){ 0 };
}

#include "world.h"

void Chunk_Generate(Chunk *chunk) {
    for (int x = 0; x < CHUNK_WIDTH; x++) {
        for (int z = 0; z < CHUNK_DEPTH; z++) {
            float worldX = (float)(chunk->x * CHUNK_WIDTH + x);
            float worldZ = (float)(chunk->z * CHUNK_DEPTH + z);
            
            float noise = Perlin2D(worldX, worldZ, 0.05f, 4);
            int height = (int)(noise * 20.0f) + 64;

            for (int y = 0; y < CHUNK_HEIGHT; y++) {
                if (y == 0) chunk->blocks[x][y][z] = BLOCK_BEDROCK;
                else if (y < height - 3) chunk->blocks[x][y][z] = BLOCK_STONE;
                else if (y < height) chunk->blocks[x][y][z] = BLOCK_DIRT;
                else if (y == height) chunk->blocks[x][y][z] = BLOCK_GRASS;
                else if (y < 60) chunk->blocks[x][y][z] = BLOCK_WATER;
                else chunk->blocks[x][y][z] = BLOCK_AIR;
            }

            // Simple tree generation (only on grass above water)
            if (height >= 60) {
                // Use a deterministic "random" based on world coordinates
                int seed = (int)(worldX * 123.45f + worldZ * 678.90f);
                if (seed % 100 == 0) {
                    // Check if we have space (within chunk boundaries for simplicity)
                    if (x > 2 && x < CHUNK_WIDTH - 2 && z > 2 && z < CHUNK_DEPTH - 2) {
                        // Log
                        for (int h = 1; h <= 5; h++) {
                            chunk->blocks[x][height + h][z] = BLOCK_OAK_LOG;
                        }
                        // Leaves
                        for (int lx = -2; lx <= 2; lx++) {
                            for (int lz = -2; lz <= 2; lz++) {
                                for (int ly = 0; ly <= 2; ly++) {
                                    if (chunk->blocks[x + lx][height + 4 + ly][z + lz] == BLOCK_AIR)
                                        chunk->blocks[x + lx][height + 4 + ly][z + lz] = BLOCK_OAK_LEAVES;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

static bool IsTransparent(BlockType block) {
    return block == BLOCK_AIR || block == BLOCK_WATER || block == BLOCK_OAK_LEAVES;
}

void Chunk_BuildMesh(Chunk *chunk, void *pWorld) {
    World *world = (World *)pWorld;
    // If there's an existing model, unload it. This will also unload its mesh.
    if (chunk->model.meshCount > 0) {
        UnloadModel(chunk->model);
        chunk->mesh = (Mesh){ 0 };
        chunk->model = (Model){ 0 };
    }

    int maxFaces = CHUNK_WIDTH * CHUNK_HEIGHT * CHUNK_DEPTH * 6;
    float *vertices = (float *)malloc(maxFaces * 6 * 3 * sizeof(float));
    float *normals = (float *)malloc(maxFaces * 6 * 3 * sizeof(float));
    float *texcoords = (float *)malloc(maxFaces * 6 * 2 * sizeof(float));
    unsigned char *colors = (unsigned char *)malloc(maxFaces * 6 * 4 * sizeof(unsigned char));

    if (!vertices || !normals || !texcoords || !colors) {
        if (vertices) free(vertices);
        if (normals) free(normals);
        if (texcoords) free(texcoords);
        if (colors) free(colors);
        return;
    }

    int vCount = 0;
    float ts = 1.0f / 16.0f; // Texture step (16x16 atlas)

    for (int x = 0; x < CHUNK_WIDTH; x++) {
        for (int y = 0; y < CHUNK_HEIGHT; y++) {
            for (int z = 0; z < CHUNK_DEPTH; z++) {
                BlockType block = chunk->blocks[x][y][z];
                if (block == BLOCK_AIR) continue;

                int wx = chunk->x * CHUNK_WIDTH + x;
                int wz = chunk->z * CHUNK_DEPTH + z;

                float fx = (float)x;
                float fy = (float)y;
                float fz = (float)z;

                // Simple UV mapping based on block type
                int tx = 0, ty = 0;
                if (block == BLOCK_STONE) { tx = 1; ty = 0; }
                else if (block == BLOCK_DIRT) { tx = 2; ty = 0; }
                else if (block == BLOCK_GRASS) { tx = 3; ty = 0; }
                else if (block == BLOCK_BEDROCK) { tx = 1; ty = 1; }
                else if (block == BLOCK_WATER) { tx = 13; ty = 12; }
                else if (block == BLOCK_OAK_LOG) { tx = 4; ty = 0; }
                else if (block == BLOCK_OAK_LEAVES) { tx = 5; ty = 0; }

                // Top Face
                BlockType above = (y == CHUNK_HEIGHT - 1) ? BLOCK_AIR : chunk->blocks[x][y+1][z];
                if (IsTransparent(above)) {
                    int ttx = tx, tty = ty;
                    if (block == BLOCK_GRASS) { ttx = 0; tty = 0; }
                    else if (block == BLOCK_OAK_LOG) { ttx = 5; tty = 1; }
                    
                    float v[] = { fx, fy+1, fz,  fx, fy+1, fz+1,  fx+1, fy+1, fz+1,
                                  fx, fy+1, fz,  fx+1, fy+1, fz+1,  fx+1, fy+1, fz };
                    float u[] = { ttx*ts, tty*ts,  ttx*ts, (tty+1)*ts,  (ttx+1)*ts, (tty+1)*ts,
                                  ttx*ts, tty*ts,  (ttx+1)*ts, (tty+1)*ts,  (ttx+1)*ts, tty*ts };
                    memcpy(&vertices[vCount * 3], v, sizeof(v));
                    memcpy(&texcoords[vCount * 2], u, sizeof(u));
                    for(int i=0; i<6; i++) {
                        normals[(vCount+i)*3] = 0; normals[(vCount+i)*3+1] = 1; normals[(vCount+i)*3+2] = 0;
                        colors[(vCount+i)*4] = 255; colors[(vCount+i)*4+1] = 255; colors[(vCount+i)*4+2] = 255; colors[(vCount+i)*4+3] = 255;
                        if (block == BLOCK_WATER) colors[(vCount+i)*4+3] = 180;
                    }
                    vCount += 6;
                }
                // Bottom Face
                BlockType below = (y == 0) ? BLOCK_BEDROCK : chunk->blocks[x][y-1][z];
                if (IsTransparent(below)) {
                    int ttx = tx, tty = ty;
                    if (block == BLOCK_GRASS) { ttx = 2; tty = 0; }
                    else if (block == BLOCK_OAK_LOG) { ttx = 5; tty = 1; }
                    
                    float v[] = { fx, fy, fz,  fx+1, fy, fz+1,  fx, fy, fz+1,
                                  fx, fy, fz,  fx+1, fy, fz,    fx+1, fy, fz+1 };
                    float u[] = { ttx*ts, tty*ts,  (ttx+1)*ts, (tty+1)*ts,  ttx*ts, (tty+1)*ts,
                                  ttx*ts, tty*ts,  (ttx+1)*ts, tty*ts,  (ttx+1)*ts, (tty+1)*ts };
                    memcpy(&vertices[vCount * 3], v, sizeof(v));
                    memcpy(&texcoords[vCount * 2], u, sizeof(u));
                    for(int i=0; i<6; i++) {
                        normals[(vCount+i)*3] = 0; normals[(vCount+i)*3+1] = -1; normals[(vCount+i)*3+2] = 0;
                        colors[(vCount+i)*4] = 255; colors[(vCount+i)*4+1] = 255; colors[(vCount+i)*4+2] = 255; colors[(vCount+i)*4+3] = 255;
                        if (block == BLOCK_WATER) colors[(vCount+i)*4+3] = 180;
                    }
                    vCount += 6;
                }
                // Side faces
                // Front
                BlockType front = World_GetBlock(world, wx, y, wz + 1);
                if (IsTransparent(front)) {
                    float v[] = { fx, fy, fz+1,  fx+1, fy, fz+1,  fx+1, fy+1, fz+1,
                                  fx, fy, fz+1,  fx+1, fy+1, fz+1,  fx, fy+1, fz+1 };
                    float u[] = { tx*ts, ty*ts,  (tx+1)*ts, ty*ts,  (tx+1)*ts, (ty+1)*ts,
                                  tx*ts, ty*ts,  (tx+1)*ts, (ty+1)*ts,  tx*ts, (ty+1)*ts };
                    memcpy(&vertices[vCount * 3], v, sizeof(v));
                    memcpy(&texcoords[vCount * 2], u, sizeof(u));
                    for(int i=0; i<6; i++) {
                        normals[(vCount+i)*3] = 0; normals[(vCount+i)*3+1] = 0; normals[(vCount+i)*3+2] = 1;
                        colors[(vCount+i)*4] = 255; colors[(vCount+i)*4+1] = 255; colors[(vCount+i)*4+2] = 255; colors[(vCount+i)*4+3] = 255;
                        if (block == BLOCK_WATER) colors[(vCount+i)*4+3] = 180;
                    }
                    vCount += 6;
                }
                // Back
                BlockType back = World_GetBlock(world, wx, y, wz - 1);
                if (IsTransparent(back)) {
                    float v[] = { fx, fy, fz,  fx+1, fy+1, fz,  fx+1, fy, fz,
                                  fx, fy, fz,  fx, fy+1, fz,    fx+1, fy+1, fz };
                    float u[] = { (tx+1)*ts, ty*ts,  tx*ts, (ty+1)*ts,  tx*ts, ty*ts,
                                  (tx+1)*ts, ty*ts,  (tx+1)*ts, (ty+1)*ts,  tx*ts, (ty+1)*ts };
                    memcpy(&vertices[vCount * 3], v, sizeof(v));
                    memcpy(&texcoords[vCount * 2], u, sizeof(u));
                    for(int i=0; i<6; i++) {
                        normals[(vCount+i)*3] = 0; normals[(vCount+i)*3+1] = 0; normals[(vCount+i)*3+2] = -1;
                        colors[(vCount+i)*4] = 255; colors[(vCount+i)*4+1] = 255; colors[(vCount+i)*4+2] = 255; colors[(vCount+i)*4+3] = 255;
                        if (block == BLOCK_WATER) colors[(vCount+i)*4+3] = 180;
                    }
                    vCount += 6;
                }
                // Right
                BlockType right = World_GetBlock(world, wx + 1, y, wz);
                if (IsTransparent(right)) {
                    float v[] = { fx+1, fy, fz,  fx+1, fy+1, fz+1,  fx+1, fy, fz+1,
                                  fx+1, fy, fz,  fx+1, fy+1, fz,    fx+1, fy+1, fz+1 };
                    float u[] = { tx*ts, ty*ts,  (tx+1)*ts, (ty+1)*ts,  (tx+1)*ts, ty*ts,
                                  tx*ts, ty*ts,  tx*ts, (ty+1)*ts,  (tx+1)*ts, (ty+1)*ts };
                    memcpy(&vertices[vCount * 3], v, sizeof(v));
                    memcpy(&texcoords[vCount * 2], u, sizeof(u));
                    for(int i=0; i<6; i++) {
                        normals[(vCount+i)*3] = 1; normals[(vCount+i)*3+1] = 0; normals[(vCount+i)*3+2] = 0;
                        colors[(vCount+i)*4] = 255; colors[(vCount+i)*4+1] = 255; colors[(vCount+i)*4+2] = 255; colors[(vCount+i)*4+3] = 255;
                        if (block == BLOCK_WATER) colors[(vCount+i)*4+3] = 180;
                    }
                    vCount += 6;
                }
                // Left
                BlockType left = World_GetBlock(world, wx - 1, y, wz);
                if (IsTransparent(left)) {
                    float v[] = { fx, fy, fz,  fx, fy, fz+1,  fx, fy+1, fz+1,
                                  fx, fy, fz,  fx, fy+1, fz+1,  fx, fy+1, fz };
                    float u[] = { (tx+1)*ts, ty*ts,  tx*ts, ty*ts,  tx*ts, (ty+1)*ts,
                                  (tx+1)*ts, ty*ts,  tx*ts, (ty+1)*ts,  (tx+1)*ts, (ty+1)*ts };
                    memcpy(&vertices[vCount * 3], v, sizeof(v));
                    memcpy(&texcoords[vCount * 2], u, sizeof(u));
                    for(int i=0; i<6; i++) {
                        normals[(vCount+i)*3] = -1; normals[(vCount+i)*3+1] = 0; normals[(vCount+i)*3+2] = 0;
                        colors[(vCount+i)*4] = 255; colors[(vCount+i)*4+1] = 255; colors[(vCount+i)*4+2] = 255; colors[(vCount+i)*4+3] = 255;
                        if (block == BLOCK_WATER) colors[(vCount+i)*4+3] = 180;
                    }
                    vCount += 6;
                }
            }
        }
    }

    if (vCount > 0) {
        chunk->mesh.vertexCount = vCount;
        chunk->mesh.triangleCount = vCount / 3;
        chunk->mesh.vertices = (float *)malloc(vCount * 3 * sizeof(float));
        chunk->mesh.normals = (float *)malloc(vCount * 3 * sizeof(float));
        chunk->mesh.texcoords = (float *)malloc(vCount * 2 * sizeof(float));
        chunk->mesh.colors = (unsigned char *)malloc(vCount * 4 * sizeof(unsigned char));
        
        memcpy(chunk->mesh.vertices, vertices, vCount * 3 * sizeof(float));
        memcpy(chunk->mesh.normals, normals, vCount * 3 * sizeof(float));
        memcpy(chunk->mesh.texcoords, texcoords, vCount * 2 * sizeof(float));
        memcpy(chunk->mesh.colors, colors, vCount * 4 * sizeof(unsigned char));

        UploadMesh(&chunk->mesh, false);
        chunk->model = LoadModelFromMesh(chunk->mesh);
    }
    
    chunk->dirty = false;

    free(vertices);
    free(normals);
    free(texcoords);
    free(colors);
}

void Chunk_Render(Chunk *chunk) {
    Vector3 pos = { (float)chunk->x * CHUNK_WIDTH, 0, (float)chunk->z * CHUNK_DEPTH };
    DrawModel(chunk->model, pos, 1.0f, WHITE);
}

void Chunk_Unload(Chunk *chunk) {
    UnloadModel(chunk->model);
}
