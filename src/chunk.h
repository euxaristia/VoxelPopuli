#ifndef CHUNK_H
#define CHUNK_H

#include "block.h"
#include "raylib.h"

#define CHUNK_WIDTH 16
#define CHUNK_HEIGHT 256
#define CHUNK_DEPTH 16

typedef struct {
  BlockType blocks[CHUNK_WIDTH][CHUNK_HEIGHT][CHUNK_DEPTH];
  Mesh meshOpaque;
  Model modelOpaque;
  Mesh meshTransparent;
  Model modelTransparent;
  bool dirty;
  int x, z;
  int minY;
  int maxY;
} Chunk;

void Chunk_Init(Chunk *chunk, int x, int z);
void Chunk_Generate(Chunk *chunk);
void Chunk_BuildMesh(Chunk *chunk, void *world);
void Chunk_RenderOpaque(Chunk *chunk);
void Chunk_RenderTransparent(Chunk *chunk);
void Chunk_Unload(Chunk *chunk);

#endif
