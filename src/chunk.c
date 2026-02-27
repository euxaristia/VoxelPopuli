#include "chunk.h"
#include "noise.h"
#include "world.h"
#include <stdlib.h>
#include <string.h>
#include <math.h>

#define WATER_VERTEX_ALPHA 208

void Chunk_Init(Chunk *chunk, int x, int z) {
    chunk->x = x; chunk->z = z;
    memset(chunk->blocks, 0, sizeof(chunk->blocks));
    chunk->dirty = false;
    chunk->meshOpaque = (Mesh){ 0 }; chunk->modelOpaque = (Model){ 0 };
    chunk->meshTransparent = (Mesh){ 0 }; chunk->modelTransparent = (Model){ 0 };
}

void Chunk_Generate(Chunk *chunk) {
    // PASS 1: Terrain
    for (int x = 0; x < CHUNK_WIDTH; x++) {
        for (int z = 0; z < CHUNK_DEPTH; z++) {
            float worldX = (float)(chunk->x * CHUNK_WIDTH + x);
            float worldZ = (float)(chunk->z * CHUNK_DEPTH + z);
            float continental = Perlin2D(worldX, worldZ, 0.005f, 3);
            float baseH = continental * 40.0f + 64.0f;
            float detail = Perlin2D(worldX, worldZ, 0.03f, 4);
            float hillF = (continental > 0.2f) ? (continental - 0.2f) * 2.0f : 0.0f;
            int height = (int)(baseH + detail * 15.0f * (1.0f + hillF));

            for (int y = 0; y < CHUNK_HEIGHT; y++) {
                BlockType b = BLOCK_AIR;
                if (y == 0) b = BLOCK_BEDROCK;
                else if (y < height - 4) b = BLOCK_STONE;
                else if (y < height) b = (height < 62) ? BLOCK_SAND : BLOCK_DIRT;
                else if (y == height) b = (height < 62) ? BLOCK_SAND : BLOCK_GRASS;
                else if (y < 60) b = BLOCK_WATER;
                chunk->blocks[x][y][z] = b;
            }
        }
    }

    // PASS 2: Trees
    for (int x = 0; x < CHUNK_WIDTH; x++) {
        for (int z = 0; z < CHUNK_DEPTH; z++) {
            float worldX = (float)(chunk->x * CHUNK_WIDTH + x);
            float worldZ = (float)(chunk->z * CHUNK_DEPTH + z);
            
            // Re-calculate height for tree placement
            float continental = Perlin2D(worldX, worldZ, 0.005f, 3);
            float baseH = continental * 40.0f + 64.0f;
            float detail = Perlin2D(worldX, worldZ, 0.03f, 4);
            float hillF = (continental > 0.2f) ? (continental - 0.2f) * 2.0f : 0.0f;
            int height = (int)(baseH + detail * 15.0f * (1.0f + hillF));

            if (height >= 62 && x > 2 && x < CHUNK_WIDTH - 3 && z > 2 && z < CHUNK_DEPTH - 3) {
                int seed = (int)(worldX * 73.1f + worldZ * 91.7f);
                if (seed % 85 == 0) {
                    int treeH = 4 + (seed % 3);
                    
                    // Only place tree if there's grass
                    if (chunk->blocks[x][height][z] == BLOCK_GRASS) {
                        for (int h = 1; h <= treeH; h++) chunk->blocks[x][height + h][z] = BLOCK_OAK_LOG;
                        for (int ly = -2; ly <= 1; ly++) {
                            int rad = (ly < 0) ? 2 : 1;
                            for (int lx = -rad; lx <= rad; lx++) {
                                for (int lz = -rad; lz <= rad; lz++) {
                                    if (ly < 0 && abs(lx) == rad && abs(lz) == rad && (seed + lx + lz) % 3 == 0) continue; 
                                    if (ly == 1 && (abs(lx) + abs(lz) > 1)) continue;
                                    
                                    int ly_idx = height + treeH + ly + 1;
                                    if (ly_idx >= 0 && ly_idx < CHUNK_HEIGHT) {
                                        if (chunk->blocks[x + lx][ly_idx][z + lz] == BLOCK_AIR)
                                            chunk->blocks[x + lx][ly_idx][z + lz] = BLOCK_OAK_LEAVES;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

static bool ShouldDrawFace(BlockType current, BlockType neighbor) {
    if (neighbor == BLOCK_AIR) return true;
    if (neighbor == BLOCK_WATER && current != BLOCK_WATER) return true;
    if (neighbor == BLOCK_OAK_LEAVES && current != BLOCK_OAK_LEAVES) return true;
    return false;
}

static float *vOpaque = NULL, *nOpaque = NULL, *tOpaque = NULL;
static unsigned char *cOpaque = NULL;
static float *vTrans = NULL, *nTrans = NULL, *tTrans = NULL;
static unsigned char *cTrans = NULL;

static void EnsureBuffers() {
    if (vOpaque != NULL) return;
    int maxF = CHUNK_WIDTH * CHUNK_HEIGHT * CHUNK_DEPTH * 6;
    vOpaque = (float *)malloc(maxF * 18 * sizeof(float));
    nOpaque = (float *)malloc(maxF * 18 * sizeof(float));
    tOpaque = (float *)malloc(maxF * 12 * sizeof(float));
    cOpaque = (unsigned char *)malloc(maxF * 24 * sizeof(unsigned char));
    vTrans = (float *)malloc(maxF * 18 * sizeof(float));
    nTrans = (float *)malloc(maxF * 18 * sizeof(float));
    tTrans = (float *)malloc(maxF * 12 * sizeof(float));
    cTrans = (unsigned char *)malloc(maxF * 24 * sizeof(unsigned char));
}

void Chunk_BuildMesh(Chunk *chunk, void *pWorld) {
    World *world = (World *)pWorld; EnsureBuffers();
    if (chunk->modelOpaque.meshCount > 0) UnloadModel(chunk->modelOpaque);
    if (chunk->modelTransparent.meshCount > 0) UnloadModel(chunk->modelTransparent);
    chunk->modelOpaque = chunk->modelTransparent = (Model){ 0 };
    chunk->meshOpaque = chunk->meshTransparent = (Mesh){ 0 };

    Chunk *neighbors[4] = {
        World_GetChunk(world, chunk->x, chunk->z+1),
        World_GetChunk(world, chunk->x, chunk->z-1),
        World_GetChunk(world, chunk->x+1, chunk->z),
        World_GetChunk(world, chunk->x-1, chunk->z)
    };

    int vCOp = 0, vCTr = 0;
    float ts = 1.0f / 16.0f, pad = 0.001f;

    for (int x = 0; x < CHUNK_WIDTH; x++) {
        for (int y = 0; y < CHUNK_HEIGHT; y++) {
            for (int z = 0; z < CHUNK_DEPTH; z++) {
                BlockType block = chunk->blocks[x][y][z];
                if (block == BLOCK_AIR) continue;

                bool isT = (block == BLOCK_WATER || block == BLOCK_OAK_LEAVES);
                float *vB = isT ? vTrans : vOpaque;
                float *nB = isT ? nTrans : nOpaque;
                float *tB = isT ? tTrans : tOpaque;
                unsigned char *cB = isT ? cTrans : cOpaque;
                int *vc = isT ? &vCTr : &vCOp;

                float fx = (float)x, fy = (float)y, fz = (float)z;
                
                int tx = 0, ty = 0;
                if (block == BLOCK_STONE) tx = 1; else if (block == BLOCK_DIRT) tx = 2;
                else if (block == BLOCK_GRASS) tx = 3; else if (block == BLOCK_BEDROCK) { tx = 1; ty = 1; }
                else if (block == BLOCK_WATER) { tx = 13; ty = 12; } else if (block == BLOCK_OAK_LOG) tx = 4;
                else if (block == BLOCK_OAK_LEAVES) tx = 5; else if (block == BLOCK_SAND) tx = 6;
                else if (block == BLOCK_GRAVEL) tx = 7;

                float u0 = tx*ts+pad, v0 = ty*ts+pad, u1 = (tx+1)*ts-pad, v1 = (ty+1)*ts-pad;
                float wOff = (block == BLOCK_WATER) ? 0.1f : 0.0f;

                // Top
                if (ShouldDrawFace(block, (y == CHUNK_HEIGHT-1) ? BLOCK_AIR : chunk->blocks[x][y+1][z])) {
                    float tu0=u0, tv0=v0, tu1=u1, tv1=v1;
                    if(block==BLOCK_GRASS){ tu0=0; tv0=0; tu1=ts-pad; tv1=ts-pad; }
                    else if(block==BLOCK_OAK_LOG){ tu0=5*ts+pad; tv0=ts+pad; tu1=6*ts-pad; tv1=2*ts-pad; }
                    float v[] = { fx, fy+1-wOff, fz,  fx, fy+1-wOff, fz+1,  fx+1, fy+1-wOff, fz+1, 
                                  fx, fy+1-wOff, fz,  fx+1, fy+1-wOff, fz+1,  fx+1, fy+1-wOff, fz };
                    float t[] = { tu0, tv0, tu0, tv1, tu1, tv1, tu0, tv0, tu1, tv1, tu1, tv0 };
                    memcpy(&vB[*vc*3], v, 72); memcpy(&tB[*vc*2], t, 48);
                    for(int i=0; i<6; i++){ int idx=*vc+i; nB[idx*3]=0; nB[idx*3+1]=1; nB[idx*3+2]=0; cB[idx*4]=cB[idx*4+1]=cB[idx*4+2]=255; cB[idx*4+3]=(block==BLOCK_WATER)?WATER_VERTEX_ALPHA:255; }
                    *vc += 6;
                }
                // Bottom
                if (ShouldDrawFace(block, (y == 0) ? BLOCK_BEDROCK : chunk->blocks[x][y-1][z])) {
                    float tu0=u0, tv0=v0, tu1=u1, tv1=v1;
                    if(block==BLOCK_GRASS){ tu0=2*ts+pad; tv0=0; tu1=3*ts-pad; tv1=ts-pad; }
                    else if(block==BLOCK_OAK_LOG){ tu0=5*ts+pad; tv0=ts+pad; tu1=6*ts-pad; tv1=2*ts-pad; }
                    float v[] = { fx, fy, fz,  fx+1, fy, fz+1,  fx, fy, fz+1, 
                                  fx, fy, fz,  fx+1, fy, fz,    fx+1, fy, fz+1 };
                    float t[] = { tu0, tv0, tu1, tv1, tu0, tv1, tu0, tv0, tu1, tv0, tu1, tv1 };
                    memcpy(&vB[*vc*3], v, 72); memcpy(&tB[*vc*2], t, 48);
                    for(int i=0; i<6; i++){ int idx=*vc+i; nB[idx*3]=0; nB[idx*3+1]=-1; nB[idx*3+2]=0; cB[idx*4]=cB[idx*4+1]=cB[idx*4+2]=120; cB[idx*4+3]=255; }
                    *vc += 6;
                }
                // Sides
                int snx[] = {0, 0, 1, -1};
                int snz[] = {1, -1, 0, 0};
                for(int s=0; s<4; s++) {
                    int nx = x + snx[s], nz = z + snz[s];
                    BlockType neighbor = BLOCK_AIR;
                    if (nx >= 0 && nx < CHUNK_WIDTH && nz >= 0 && nz < CHUNK_DEPTH) {
                        neighbor = chunk->blocks[nx][y][nz];
                    } else {
                        Chunk *nc = neighbors[s];
                        if (nc) {
                            int bx = (nx + CHUNK_WIDTH) % CHUNK_WIDTH;
                            int bz = (nz + CHUNK_DEPTH) % CHUNK_DEPTH;
                            neighbor = nc->blocks[bx][y][bz];
                        }
                    }

                    if (ShouldDrawFace(block, neighbor)) {
                        bool merged = false;
                        if (y > 0 && chunk->blocks[x][y-1][z] == block) {
                            BlockType nBelow = BLOCK_AIR;
                            if (nx >= 0 && nx < CHUNK_WIDTH && nz >= 0 && nz < CHUNK_DEPTH) nBelow = chunk->blocks[nx][y-1][nz];
                            else if (neighbors[s]) nBelow = neighbors[s]->blocks[(nx + CHUNK_WIDTH) % CHUNK_WIDTH][y-1][(nz + CHUNK_DEPTH) % CHUNK_DEPTH];
                            if (ShouldDrawFace(block, nBelow)) merged = true;
                        }

                        if (!merged) {
                            int h = 1;
                            while (y + h < CHUNK_HEIGHT && chunk->blocks[x][y+h][z] == block) {
                                BlockType nAbove = BLOCK_AIR;
                                if (nx >= 0 && nx < CHUNK_WIDTH && nz >= 0 && nz < CHUNK_DEPTH) nAbove = chunk->blocks[nx][y+h][nz];
                                else if (neighbors[s]) nAbove = neighbors[s]->blocks[(nx + CHUNK_WIDTH) % CHUNK_WIDTH][y+h][(nz + CHUNK_DEPTH) % CHUNK_DEPTH];
                                if (ShouldDrawFace(block, nAbove)) h++; else break;
                            }
                            float fh = (float)h;
                            float sv[18];
                            if(s==0){ float f[]={fx,fy,fz+1, fx+1,fy,fz+1, fx+1,fy+fh-wOff,fz+1, fx,fy,fz+1, fx+1,fy+fh-wOff,fz+1, fx,fy+fh-wOff,fz+1}; memcpy(sv,f,72); }
                            else if(s==1){ float b[]={fx+1,fy,fz, fx,fy,fz, fx,fy+fh-wOff,fz, fx+1,fy,fz, fx,fy+fh-wOff,fz, fx+1,fy+fh-wOff,fz}; memcpy(sv,b,72); }
                            else if(s==2){ float r[]={fx+1,fy,fz+1, fx+1,fy,fz, fx+1,fy+fh-wOff,fz, fx+1,fy,fz+1, fx+1,fy+fh-wOff,fz, fx+1,fy+fh-wOff,fz+1}; memcpy(sv,r,72); }
                            else { float l[]={fx,fy,fz, fx,fy,fz+1, fx,fy+fh-wOff,fz+1, fx,fy,fz, fx,fy+fh-wOff,fz+1, fx,fy+fh-wOff,fz}; memcpy(sv,l,72); }
                            float st[] = { u0,v1*fh, u1,v1*fh, u1,v0, u0,v1*fh, u1,v0, u0,v0 };
                            memcpy(&vB[*vc*3], sv, 72); memcpy(&tB[*vc*2], st, 48);
                            for(int i=0; i<6; i++){ int idx=*vc+i; nB[idx*3]=snx[s]; nB[idx*3+1]=0; nB[idx*3+2]=snz[s]; float sh=(s<2)?0.6f:0.8f; cB[idx*4]=cB[idx*4+1]=cB[idx*4+2]=(unsigned char)(255*sh); cB[idx*4+3]=(block==BLOCK_WATER)?WATER_VERTEX_ALPHA:255; }
                            *vc += 6;
                        }
                    }
                }
            }
        }
    }

    if (vCOp > 0) {
        chunk->meshOpaque.vertexCount = vCOp; chunk->meshOpaque.triangleCount = vCOp/3;
        chunk->meshOpaque.vertices = (float *)malloc(vCOp*12); chunk->meshOpaque.normals = (float *)malloc(vCOp*12);
        chunk->meshOpaque.texcoords = (float *)malloc(vCOp*8); chunk->meshOpaque.colors = (unsigned char *)malloc(vCOp*4);
        memcpy(chunk->meshOpaque.vertices, vOpaque, vCOp*12); memcpy(chunk->meshOpaque.normals, nOpaque, vCOp*12);
        memcpy(chunk->meshOpaque.texcoords, tOpaque, vCOp*8); memcpy(chunk->meshOpaque.colors, cOpaque, vCOp*4);
        UploadMesh(&chunk->meshOpaque, false); chunk->modelOpaque = LoadModelFromMesh(chunk->meshOpaque);
    }
    if (vCTr > 0) {
        chunk->meshTransparent.vertexCount = vCTr; chunk->meshTransparent.triangleCount = vCTr/3;
        chunk->meshTransparent.vertices = (float *)malloc(vCTr*3*sizeof(float));
        chunk->meshTransparent.normals = (float *)malloc(vCTr*3*sizeof(float));
        chunk->meshTransparent.texcoords = (float *)malloc(vCTr*2*sizeof(float));
        chunk->meshTransparent.colors = (unsigned char *)malloc(vCTr*4*sizeof(unsigned char));
        memcpy(chunk->meshTransparent.vertices, vTrans, vCTr*12); memcpy(chunk->meshTransparent.normals, nTrans, vCTr*12);
        memcpy(chunk->meshTransparent.texcoords, tTrans, vCTr*8); memcpy(chunk->meshTransparent.colors, cTrans, vCTr*4);
        UploadMesh(&chunk->meshTransparent, false); chunk->modelTransparent = LoadModelFromMesh(chunk->meshTransparent);
    }
    chunk->dirty = false;
}

void Chunk_RenderOpaque(Chunk *chunk) { if (chunk->modelOpaque.meshCount > 0) DrawModel(chunk->modelOpaque, (Vector3){(float)chunk->x*CHUNK_WIDTH,0,(float)chunk->z*CHUNK_DEPTH}, 1.0f, WHITE); }
void Chunk_RenderTransparent(Chunk *chunk) { if (chunk->modelTransparent.meshCount > 0) DrawModel(chunk->modelTransparent, (Vector3){(float)chunk->x*CHUNK_WIDTH,0,(float)chunk->z*CHUNK_DEPTH}, 1.0f, WHITE); }
void Chunk_Unload(Chunk *chunk) {
    if (chunk->modelOpaque.meshCount > 0) UnloadModel(chunk->modelOpaque);
    if (chunk->modelTransparent.meshCount > 0) UnloadModel(chunk->modelTransparent);
    chunk->meshOpaque = chunk->meshTransparent = (Mesh){ 0 };
    chunk->modelOpaque = chunk->modelTransparent = (Model){ 0 };
}
