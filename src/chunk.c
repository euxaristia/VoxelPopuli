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
            
            // 1. Continentalness / Base Height
            float continental = Perlin2D(worldX, worldZ, 0.005f, 3);
            float baseHeight = continental * 40.0f + 64.0f;
            
            // 2. Detail Noise (Hills/Valleys)
            float detail = Perlin2D(worldX, worldZ, 0.03f, 4);
            float hillFactor = (continental > 0.2f) ? (continental - 0.2f) * 2.0f : 0.0f;
            int height = (int)(baseHeight + detail * 15.0f * (1.0f + hillFactor));

            // 3. Fill Blocks
            for (int y = 0; y < CHUNK_HEIGHT; y++) {
                BlockType b = BLOCK_AIR;
                if (y == 0) b = BLOCK_BEDROCK;
                else if (y < height - 4) b = BLOCK_STONE;
                else if (y < height) {
                    if (height < 62) b = BLOCK_SAND; // Beach
                    else b = BLOCK_DIRT;
                }
                else if (y == height) {
                    if (height < 62) b = BLOCK_SAND;
                    else b = BLOCK_GRASS;
                }
                else if (y < 60) b = BLOCK_WATER;
                
                chunk->blocks[x][y][z] = b;
            }

            // 4. Tree Generation (More authentic pattern)
            if (height >= 62 && x > 2 && x < CHUNK_WIDTH - 3 && z > 2 && z < CHUNK_DEPTH - 3) {
                int seed = (int)(worldX * 73.1f + worldZ * 91.7f);
                if (seed % 80 == 0) {
                    int treeHeight = 4 + (seed % 3);
                    
                    // Trunk
                    for (int h = 1; h <= treeHeight; h++) {
                        chunk->blocks[x][height + h][z] = BLOCK_OAK_LOG;
                    }
                    
                    // Leaf Canopy (Minecraft 1.0 Oak Pattern)
                    for (int ly = -2; ly <= 1; ly++) {
                        int radius = (ly < 0) ? 2 : (ly == 0 ? 1 : 1);
                        if (ly == 1) radius = 1;
                        
                        for (int lx = -radius; lx <= radius; lx++) {
                            for (int lz = -radius; lz <= radius; lz++) {
                                // Cut corners for standard leaves
                                if (ly < 0 && abs(lx) == radius && abs(lz) == radius) {
                                    if ((seed + lx + lz) % 3 == 0) continue; 
                                }
                                
                                // Top layer is a small cross
                                if (ly == 1 && (abs(lx) + abs(lz) > 1)) continue;

                                if (chunk->blocks[x + lx][height + treeHeight + ly + 1][z + lz] == BLOCK_AIR) {
                                    chunk->blocks[x + lx][height + treeHeight + ly + 1][z + lz] = BLOCK_OAK_LEAVES;
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
    return block == BLOCK_AIR || block == BLOCK_OAK_LEAVES;
}

static bool ShouldDrawFace(BlockType current, BlockType neighbor) {
    if (neighbor == BLOCK_AIR) return true;
    if (neighbor == BLOCK_WATER && current != BLOCK_WATER) return true;
    if (neighbor == BLOCK_OAK_LEAVES && current != BLOCK_OAK_LEAVES) return true;
    return false;
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
    float ts = 1.0f / 16.0f; // Texture step
    float pad = 0.001f;      // UV Padding

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

                int tx = 0, ty = 0;
                if (block == BLOCK_STONE) { tx = 1; ty = 0; }
                else if (block == BLOCK_DIRT) { tx = 2; ty = 0; }
                else if (block == BLOCK_GRASS) { tx = 3; ty = 0; }
                else if (block == BLOCK_BEDROCK) { tx = 1; ty = 1; }
                else if (block == BLOCK_WATER) { tx = 13; ty = 12; }
                else if (block == BLOCK_OAK_LOG) { tx = 4; ty = 0; }
                else if (block == BLOCK_OAK_LEAVES) { tx = 5; ty = 0; }
                else if (block == BLOCK_SAND) { tx = 6; ty = 0; }
                else if (block == BLOCK_GRAVEL) { tx = 7; ty = 0; }

                float u0 = tx * ts + pad, v0 = ty * ts + pad;
                float u1 = (tx + 1) * ts - pad, v1 = (ty + 1) * ts - pad;

                float topShade = 1.0f;
                float sideShade = 0.8f;
                float frontShade = 0.6f;

                // Top Face
                BlockType above = (y == CHUNK_HEIGHT - 1) ? BLOCK_AIR : chunk->blocks[x][y+1][z];
                if (ShouldDrawFace(block, above)) {
                    float tu0 = u0, tv0 = v0, tu1 = u1, tv1 = v1;
                    if (block == BLOCK_GRASS) { tu0 = 0*ts+pad; tv0 = 0*ts+pad; tu1 = 1*ts-pad; tv1 = 1*ts-pad; }
                    else if (block == BLOCK_OAK_LOG) { tu0 = 5*ts+pad; tv0 = 1*ts+pad; tu1 = 6*ts-pad; tv1 = 2*ts-pad; }
                    
                    float v[] = { fx, fy+1, fz,  fx, fy+1, fz+1,  fx+1, fy+1, fz+1,
                                  fx, fy+1, fz,  fx+1, fy+1, fz+1,  fx+1, fy+1, fz };
                    float u[] = { tu0, tv0,  tu0, tv1,  tu1, tv1,
                                  tu0, tv0,  tu1, tv1,  tu1, tv0 };
                    memcpy(&vertices[vCount * 3], v, sizeof(v));
                    memcpy(&texcoords[vCount * 2], u, sizeof(u));
                    for(int i=0; i<6; i++) {
                        normals[(vCount+i)*3] = 0; normals[(vCount+i)*3+1] = 1; normals[(vCount+i)*3+2] = 0;
                        colors[(vCount+i)*4] = colors[(vCount+i)*4+1] = colors[(vCount+i)*4+2] = (unsigned char)(255 * topShade); 
                        colors[(vCount+i)*4+3] = (block == BLOCK_WATER) ? 180 : 255;
                    }
                    vCount += 6;
                }
                // Bottom Face
                BlockType below = (y == 0) ? BLOCK_BEDROCK : chunk->blocks[x][y-1][z];
                if (ShouldDrawFace(block, below)) {
                    float tu0 = u0, tv0 = v0, tu1 = u1, tv1 = v1;
                    if (block == BLOCK_GRASS) { tu0 = 2*ts+pad; tv0 = 0*ts+pad; tu1 = 3*ts-pad; tv1 = 1*ts-pad; }
                    else if (block == BLOCK_OAK_LOG) { tu0 = 5*ts+pad; tv0 = 1*ts+pad; tu1 = 6*ts-pad; tv1 = 2*ts-pad; }
                    
                    float v[] = { fx, fy, fz,  fx+1, fy, fz+1,  fx, fy, fz+1,
                                  fx, fy, fz,  fx+1, fy, fz,    fx+1, fy, fz+1 };
                    float u[] = { tu0, tv0,  tu1, tv1,  tu0, tv1,
                                  tu0, tv0,  tu1, tv0,  tu1, tv1 };
                    memcpy(&vertices[vCount * 3], v, sizeof(v));
                    memcpy(&texcoords[vCount * 2], u, sizeof(u));
                    for(int i=0; i<6; i++) {
                        normals[(vCount+i)*3] = 0; normals[(vCount+i)*3+1] = -1; normals[(vCount+i)*3+2] = 0;
                        colors[(vCount+i)*4] = colors[(vCount+i)*4+1] = colors[(vCount+i)*4+2] = 120; colors[(vCount+i)*4+3] = 255;
                    }
                    vCount += 6;
                }
                // Front
                BlockType front = World_GetBlock(world, wx, y, wz + 1);
                if (ShouldDrawFace(block, front)) {
                    float v[] = { fx, fy, fz+1,  fx+1, fy, fz+1,  fx+1, fy+1, fz+1,
                                  fx, fy, fz+1,  fx+1, fy+1, fz+1,  fx, fy+1, fz+1 };
                    float u[] = { u0, v0,  u1, v0,  u1, v1,
                                  u0, v0,  u1, v1,  u0, v1 };
                    memcpy(&vertices[vCount * 3], v, sizeof(v));
                    memcpy(&texcoords[vCount * 2], u, sizeof(u));
                    for(int i=0; i<6; i++) {
                        normals[(vCount+i)*3] = 0; normals[(vCount+i)*3+1] = 0; normals[(vCount+i)*3+2] = 1;
                        colors[(vCount+i)*4] = colors[(vCount+i)*4+1] = colors[(vCount+i)*4+2] = (unsigned char)(255 * frontShade); 
                        colors[(vCount+i)*4+3] = (block == BLOCK_WATER) ? 180 : 255;
                    }
                    vCount += 6;
                }
                // Back
                BlockType back = World_GetBlock(world, wx, y, wz - 1);
                if (ShouldDrawFace(block, back)) {
                    float v[] = { fx, fy, fz,  fx+1, fy+1, fz,  fx+1, fy, fz,
                                  fx, fy, fz,  fx, fy+1, fz,    fx+1, fy+1, fz };
                    float u[] = { u1, v0,  u0, v1,  u0, v0,
                                  u1, v0,  u1, v1,  u0, v1 };
                    memcpy(&vertices[vCount * 3], v, sizeof(v));
                    memcpy(&texcoords[vCount * 2], u, sizeof(u));
                    for(int i=0; i<6; i++) {
                        normals[(vCount+i)*3] = 0; normals[(vCount+i)*3+1] = 0; normals[(vCount+i)*3+2] = -1;
                        colors[(vCount+i)*4] = colors[(vCount+i)*4+1] = colors[(vCount+i)*4+2] = (unsigned char)(255 * frontShade); 
                        colors[(vCount+i)*4+3] = (block == BLOCK_WATER) ? 180 : 255;
                    }
                    vCount += 6;
                }
                // Right
                BlockType right = World_GetBlock(world, wx + 1, y, wz);
                if (ShouldDrawFace(block, right)) {
                    float v[] = { fx+1, fy, fz,  fx+1, fy+1, fz+1,  fx+1, fy, fz+1,
                                  fx+1, fy, fz,  fx+1, fy+1, fz,    fx+1, fy+1, fz+1 };
                    float u[] = { u0, v0,  u1, v1,  u1, v0,
                                  u0, v0,  u0, v1,  u1, v1 };
                    memcpy(&vertices[vCount * 3], v, sizeof(v));
                    memcpy(&texcoords[vCount * 2], u, sizeof(u));
                    for(int i=0; i<6; i++) {
                        normals[(vCount+i)*3] = 1; normals[(vCount+i)*3+1] = 0; normals[(vCount+i)*3+2] = 0;
                        colors[(vCount+i)*4] = colors[(vCount+i)*4+1] = colors[(vCount+i)*4+2] = (unsigned char)(255 * sideShade); 
                        colors[(vCount+i)*4+3] = (block == BLOCK_WATER) ? 180 : 255;
                    }
                    vCount += 6;
                }
                // Left
                BlockType left = World_GetBlock(world, wx - 1, y, wz);
                if (ShouldDrawFace(block, left)) {
                    float v[] = { fx, fy, fz,  fx, fy, fz+1,  fx, fy+1, fz+1,
                                  fx, fy, fz,  fx, fy+1, fz+1,  fx, fy+1, fz };
                    float u[] = { u1, v0,  u0, v0,  u0, v1,
                                  u1, v0,  u0, v1,  u1, v1 };
                    memcpy(&vertices[vCount * 3], v, sizeof(v));
                    memcpy(&texcoords[vCount * 2], u, sizeof(u));
                    for(int i=0; i<6; i++) {
                        normals[(vCount+i)*3] = -1; normals[(vCount+i)*3+1] = 0; normals[(vCount+i)*3+2] = 0;
                        colors[(vCount+i)*4] = colors[(vCount+i)*4+1] = colors[(vCount+i)*4+2] = (unsigned char)(255 * sideShade); 
                        colors[(vCount+i)*4+3] = (block == BLOCK_WATER) ? 180 : 255;
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
