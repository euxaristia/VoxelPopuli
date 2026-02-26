#ifndef CHUNK_H
#define CHUNK_H

#include "raylib.h"
#include "block.h"

#define CHUNK_WIDTH 16
#define CHUNK_HEIGHT 256
#define CHUNK_DEPTH 16

typedef struct {
    BlockType blocks[CHUNK_WIDTH][CHUNK_HEIGHT][CHUNK_DEPTH];
    Mesh mesh;
    Model model;
    bool dirty;
    int x, z; // Chunk coordinates
} Chunk;

void Chunk_Init(Chunk *chunk, int x, int z);
void Chunk_Generate(Chunk *chunk);
void Chunk_BuildMesh(Chunk *chunk, void *world);
void Chunk_Render(Chunk *chunk);
void Chunk_Unload(Chunk *chunk);

#endif
