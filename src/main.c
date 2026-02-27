#include "noise.h"
#include "raylib.h"
#include "raymath.h"
#include "world.h"
#include <errno.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>

#define INVENTORY_BLOCK_COUNT 10
#define SAVE_DIR "save"
#define WORLD_SAVE_PATH SAVE_DIR "/world_edits.bin"
#define PLAYER_SAVE_PATH SAVE_DIR "/player_state.bin"
#define WINDOW_SAVE_PATH SAVE_DIR "/window_state.bin"
#define PLAYER_SAVE_MAGIC 0x31505356u
#define PLAYER_SAVE_VERSION 2u
#define WINDOW_SAVE_MAGIC 0x31575356u
#define WINDOW_SAVE_VERSION 1u
#define PLAYER_MAX_HEALTH 20
#define PLAYER_MAX_AIR_SECONDS 15.0f
#define PLAYER_DROWN_DAMAGE_INTERVAL 1.0f
#define PLAYER_FALL_DAMAGE_THRESHOLD 3.0f
#define PLAYER_RESPAWN_INVULN_SECONDS 1.0f
#define PLAYER_VOID_Y -20.0f
#define DEFAULT_SCREEN_WIDTH 1280
#define DEFAULT_SCREEN_HEIGHT 720

typedef struct {
  Vector3 position, velocity;
  Vector3 respawnPosition;
  bool grounded;
  BlockType selectedBlock;
  int selectedSlot;
  bool inventoryOpen;
  int health;
  float airSeconds;
  float fallDistance;
  float damageCooldown;
  float drownTickTimer;
  int blockCounts[INVENTORY_BLOCK_COUNT];
} Player;

BlockType inventoryBlocks[INVENTORY_BLOCK_COUNT] = {
    BLOCK_GRASS, BLOCK_DIRT,  BLOCK_STONE,  BLOCK_OAK_LOG, BLOCK_OAK_LEAVES,
    BLOCK_SAND,  BLOCK_WATER, BLOCK_GRAVEL, BLOCK_BEDROCK, BLOCK_COAL_ORE};

typedef struct {
  uint32_t magic;
  uint32_t version;
  float playerPos[3];
  float playerVelocity[3];
  int32_t selectedSlot;
  int32_t blockCounts[INVENTORY_BLOCK_COUNT];
  int32_t hotbarBlocks[INVENTORY_BLOCK_COUNT];
} PlayerSaveDataV1;

typedef struct {
  uint32_t magic;
  uint32_t version;
  float playerPos[3];
  float playerVelocity[3];
  int32_t selectedSlot;
  int32_t blockCounts[INVENTORY_BLOCK_COUNT];
  int32_t hotbarBlocks[INVENTORY_BLOCK_COUNT];
  int32_t health;
  float airSeconds;
  float respawnPos[3];
} PlayerSaveDataV2;

typedef struct {
  uint32_t magic;
  uint32_t version;
  int32_t width;
  int32_t height;
  uint8_t maximized;
  uint8_t reserved[3];
} WindowSaveData;

int GetInventoryIndex(BlockType block) {
  for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++)
    if (inventoryBlocks[i] == block)
      return i;
  return -1;
}

Color GetBlockUIColour(BlockType block) {
  if (block == BLOCK_GRASS)
    return GREEN;
  if (block == BLOCK_DIRT)
    return BROWN;
  if (block == BLOCK_STONE)
    return GRAY;
  if (block == BLOCK_OAK_LOG)
    return DARKBROWN;
  if (block == BLOCK_OAK_LEAVES)
    return DARKGREEN;
  if (block == BLOCK_SAND)
    return BEIGE;
  if (block == BLOCK_WATER)
    return BLUE;
  if (block == BLOCK_GRAVEL)
    return LIGHTGRAY;
  if (block == BLOCK_BEDROCK)
    return DARKGRAY;
  if (block == BLOCK_COAL_ORE)
    return BLACK;
  return WHITE;
}

void DrawHeartIcon(int x, int y, int fillState) {
  // 0 = transparent, 1 = outline, 2 = fill.
  static const unsigned char sprite[8][9] = {
      {0, 1, 1, 0, 0, 0, 1, 1, 0}, {1, 2, 2, 1, 0, 1, 2, 2, 1},
      {1, 2, 2, 2, 1, 2, 2, 2, 1}, {1, 2, 2, 2, 2, 2, 2, 2, 1},
      {0, 1, 2, 2, 2, 2, 2, 1, 0}, {0, 0, 1, 2, 2, 2, 1, 0, 0},
      {0, 0, 0, 1, 2, 1, 0, 0, 0}, {0, 0, 0, 0, 1, 0, 0, 0, 0}};
  const int scale = 2;
  const int width = 9, height = 8;
  Color border = (Color){28, 10, 10, 255};
  Color full = (Color){224, 42, 42, 255};
  Color empty = (Color){72, 32, 32, 255};

  for (int py = 0; py < height; py++) {
    for (int px = 0; px < width; px++) {
      unsigned char cell = sprite[py][px];
      if (cell == 0)
        continue;
      bool filledSide = (fillState >= 2) || (fillState == 1 && px <= 4);
      Color pixel = (cell == 1) ? border : (filledSide ? full : empty);
      DrawRectangle(x + px * scale, y + py * scale, scale, scale, pixel);
    }
  }
}

void DrawBubbleIcon(int x, int y, bool filled) {
  Color fill =
      filled ? (Color){169, 216, 255, 230} : (Color){76, 101, 130, 210};
  Color highlight =
      filled ? (Color){235, 248, 255, 220} : (Color){120, 144, 170, 190};
  Color border = (Color){26, 45, 66, 230};
  DrawCircle(x, y, 6.0f, fill);
  DrawCircle(x - 2, y - 2, 2.0f, highlight);
  DrawCircleLines(x, y, 6.0f, border);
}

int ClampInt(int value, int minValue, int maxValue) {
  if (value < minValue)
    return minValue;
  if (value > maxValue)
    return maxValue;
  return value;
}

bool SaveWindowState(const char *path) {
  FILE *file = fopen(path, "wb");
  if (!file)
    return false;

  WindowSaveData data = {0};
  data.magic = WINDOW_SAVE_MAGIC;
  data.version = WINDOW_SAVE_VERSION;
  data.width = GetScreenWidth();
  data.height = GetScreenHeight();
  data.maximized = IsWindowState(FLAG_WINDOW_MAXIMIZED) ? 1u : 0u;

  bool ok = fwrite(&data, sizeof(data), 1, file) == 1;
  fclose(file);
  return ok;
}

bool LoadWindowState(const char *path) {
  FILE *file = fopen(path, "rb");
  if (!file)
    return false;

  WindowSaveData data = {0};
  bool ok = fread(&data, sizeof(data), 1, file) == 1;
  fclose(file);
  if (!ok)
    return false;
  if (data.magic != WINDOW_SAVE_MAGIC || data.version != WINDOW_SAVE_VERSION)
    return false;

  int width = ClampInt(data.width, 960, 8192);
  int height = ClampInt(data.height, 540, 8192);
  SetWindowSize(width, height);
  if (data.maximized)
    MaximizeWindow();
  return true;
}

void LoadInventoryFromSave(Player *player, BlockType *hotbar,
                           const int32_t *counts, const int32_t *hotbarBlocks) {
  for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
    int count = counts[i];
    if (count < 0)
      count = 0;
    player->blockCounts[i] = count;

    int hb = hotbarBlocks[i];
    BlockType candidate =
        (hb >= BLOCK_AIR && hb <= BLOCK_BEDROCK) ? (BlockType)hb : BLOCK_AIR;
    int idx = GetInventoryIndex(candidate);
    if (candidate != BLOCK_AIR && (idx < 0 || player->blockCounts[idx] <= 0))
      candidate = BLOCK_AIR;
    hotbar[i] = candidate;
  }
}

void DamagePlayer(Player *player, int amount) {
  if (amount <= 0 || player->health <= 0)
    return;
  if (player->damageCooldown > 0.0f)
    return;
  player->health -= amount;
  if (player->health < 0)
    player->health = 0;
  player->damageCooldown = 0.45f;
}

void TryAutoAssignHotbar(BlockType *hotbar, BlockType block) {
  if (block == BLOCK_AIR)
    return;
  for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++)
    if (hotbar[i] == block)
      return;
  for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
    if (hotbar[i] == BLOCK_AIR) {
      hotbar[i] = block;
      return;
    }
  }
}

void CleanupHotbarSlots(Player *player, BlockType *hotbar) {
  for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
    if (hotbar[i] == BLOCK_AIR)
      continue;
    int idx = GetInventoryIndex(hotbar[i]);
    if (idx < 0 || player->blockCounts[idx] <= 0)
      hotbar[i] = BLOCK_AIR;
  }
  player->selectedBlock = hotbar[player->selectedSlot];
}

bool EnsureSaveDir(void) {
  if (mkdir(SAVE_DIR, 0755) == 0)
    return true;
  return errno == EEXIST;
}

void InitDefaultPlayer(Player *player, BlockType *hotbar) {
  memset(player, 0, sizeof(*player));
  player->position = (Vector3){32.5f, 164.0f, 32.5f};
  player->respawnPosition = player->position;
  player->selectedSlot = 0;
  player->inventoryOpen = false;
  player->health = PLAYER_MAX_HEALTH;
  player->airSeconds = PLAYER_MAX_AIR_SECONDS;
  player->fallDistance = 0.0f;
  player->damageCooldown = 0.0f;
  player->drownTickTimer = PLAYER_DROWN_DAMAGE_INTERVAL;
  for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
    player->blockCounts[i] = 0;
    hotbar[i] = BLOCK_AIR;
  }
  player->selectedBlock = hotbar[player->selectedSlot];
}

bool SavePlayerState(Player *player, BlockType *hotbar, const char *path) {
  FILE *file = fopen(path, "wb");
  if (!file)
    return false;

  PlayerSaveDataV2 data = {0};
  data.magic = PLAYER_SAVE_MAGIC;
  data.version = PLAYER_SAVE_VERSION;
  data.playerPos[0] = player->position.x;
  data.playerPos[1] = player->position.y;
  data.playerPos[2] = player->position.z;
  data.playerVelocity[0] = player->velocity.x;
  data.playerVelocity[1] = player->velocity.y;
  data.playerVelocity[2] = player->velocity.z;
  data.selectedSlot = player->selectedSlot;
  for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
    data.blockCounts[i] = player->blockCounts[i];
    data.hotbarBlocks[i] = (int32_t)hotbar[i];
  }
  data.health = player->health;
  data.airSeconds = player->airSeconds;
  data.respawnPos[0] = player->respawnPosition.x;
  data.respawnPos[1] = player->respawnPosition.y;
  data.respawnPos[2] = player->respawnPosition.z;

  bool ok = fwrite(&data, sizeof(data), 1, file) == 1;
  fclose(file);
  return ok;
}

bool LoadPlayerState(Player *player, BlockType *hotbar, const char *path) {
  FILE *file = fopen(path, "rb");
  if (!file)
    return false;

  uint32_t magic = 0;
  uint32_t version = 0;
  if (fread(&magic, sizeof(magic), 1, file) != 1 ||
      fread(&version, sizeof(version), 1, file) != 1) {
    fclose(file);
    return false;
  }
  if (magic != PLAYER_SAVE_MAGIC) {
    fclose(file);
    return false;
  }
  rewind(file);

  if (version == 1u) {
    PlayerSaveDataV1 data = {0};
    bool ok = fread(&data, sizeof(data), 1, file) == 1;
    fclose(file);
    if (!ok || data.magic != PLAYER_SAVE_MAGIC || data.version != 1u)
      return false;

    player->position =
        (Vector3){data.playerPos[0], data.playerPos[1], data.playerPos[2]};
    player->velocity = (Vector3){data.playerVelocity[0], data.playerVelocity[1],
                                 data.playerVelocity[2]};
    player->respawnPosition = player->position;
    player->selectedSlot = data.selectedSlot;
    if (player->selectedSlot < 0 ||
        player->selectedSlot >= INVENTORY_BLOCK_COUNT)
      player->selectedSlot = 0;
    player->grounded = false;
    player->inventoryOpen = false;
    player->health = PLAYER_MAX_HEALTH;
    player->airSeconds = PLAYER_MAX_AIR_SECONDS;
    player->fallDistance = 0.0f;
    player->damageCooldown = 0.0f;
    player->drownTickTimer = PLAYER_DROWN_DAMAGE_INTERVAL;
    LoadInventoryFromSave(player, hotbar, data.blockCounts, data.hotbarBlocks);
    player->selectedBlock = hotbar[player->selectedSlot];
    return true;
  }

  if (version == PLAYER_SAVE_VERSION) {
    PlayerSaveDataV2 data = {0};
    bool ok = fread(&data, sizeof(data), 1, file) == 1;
    fclose(file);
    if (!ok || data.magic != PLAYER_SAVE_MAGIC ||
        data.version != PLAYER_SAVE_VERSION)
      return false;

    player->position =
        (Vector3){data.playerPos[0], data.playerPos[1], data.playerPos[2]};
    player->velocity = (Vector3){data.playerVelocity[0], data.playerVelocity[1],
                                 data.playerVelocity[2]};
    player->respawnPosition =
        (Vector3){data.respawnPos[0], data.respawnPos[1], data.respawnPos[2]};
    player->selectedSlot = data.selectedSlot;
    if (player->selectedSlot < 0 ||
        player->selectedSlot >= INVENTORY_BLOCK_COUNT)
      player->selectedSlot = 0;
    player->grounded = false;
    player->inventoryOpen = false;
    player->health = ClampInt(data.health, 1, PLAYER_MAX_HEALTH);
    player->airSeconds = data.airSeconds;
    if (player->airSeconds < 0.0f)
      player->airSeconds = 0.0f;
    if (player->airSeconds > PLAYER_MAX_AIR_SECONDS)
      player->airSeconds = PLAYER_MAX_AIR_SECONDS;
    player->fallDistance = 0.0f;
    player->damageCooldown = 0.0f;
    player->drownTickTimer = PLAYER_DROWN_DAMAGE_INTERVAL;
    LoadInventoryFromSave(player, hotbar, data.blockCounts, data.hotbarBlocks);
    player->selectedBlock = hotbar[player->selectedSlot];
    return true;
  }

  fclose(file);
  return false;
}

bool IsPointInBlock(World *world, Vector3 p) {
  BlockType b =
      World_GetBlock(world, (int)floor(p.x), (int)floor(p.y), (int)floor(p.z));
  return b != BLOCK_AIR && b != BLOCK_WATER;
}

bool CheckCollision(World *world, Vector3 pos) {
  float w = 0.22f, h = 1.75f; // Slightly slimmer for smoother transitions
  for (float x = -w; x <= w; x += w * 2)
    for (float z = -w; z <= w; z += w * 2)
      for (float y = 0.1f; y <= h; y += h / 2.0f)
        if (IsPointInBlock(world, (Vector3){pos.x + x, pos.y + y, pos.z + z}))
          return true;
  return false;
}

void RespawnPlayer(Player *player, World *world) {
  int sx = (int)floor(player->respawnPosition.x);
  int sz = (int)floor(player->respawnPosition.z);
  float safeY = player->respawnPosition.y;
  for (int y = CHUNK_HEIGHT - 1; y >= 0; y--) {
    BlockType b = World_GetBlock(world, sx, y, sz);
    if (b != BLOCK_AIR && b != BLOCK_WATER) {
      safeY = (float)y + 1.1f;
      break;
    }
  }

  player->position =
      (Vector3){player->respawnPosition.x, safeY, player->respawnPosition.z};
  player->velocity = (Vector3){0};
  player->grounded = false;
  player->inventoryOpen = false;
  player->health = PLAYER_MAX_HEALTH;
  player->airSeconds = PLAYER_MAX_AIR_SECONDS;
  player->fallDistance = 0.0f;
  player->damageCooldown = PLAYER_RESPAWN_INVULN_SECONDS;
  player->drownTickTimer = PLAYER_DROWN_DAMAGE_INTERVAL;

  for (int i = 0; i < 32 && CheckCollision(world, player->position); i++)
    player->position.y += 1.0f;
}

Frustum ExtractFrustum(Camera3D camera) {
  Matrix modelview = GetCameraMatrix(camera);
  Matrix projection = MatrixPerspective(
      camera.fovy * DEG2RAD, (float)GetScreenWidth() / (float)GetScreenHeight(),
      0.1f, 1000.0f);
  Matrix viewProj = MatrixMultiply(modelview, projection);

  Frustum f;
  // Left
  f.planes[0][0] = viewProj.m3 + viewProj.m0;
  f.planes[0][1] = viewProj.m7 + viewProj.m4;
  f.planes[0][2] = viewProj.m11 + viewProj.m8;
  f.planes[0][3] = viewProj.m15 + viewProj.m12;
  // Right
  f.planes[1][0] = viewProj.m3 - viewProj.m0;
  f.planes[1][1] = viewProj.m7 - viewProj.m4;
  f.planes[1][2] = viewProj.m11 - viewProj.m8;
  f.planes[1][3] = viewProj.m15 - viewProj.m12;
  // Bottom
  f.planes[2][0] = viewProj.m3 + viewProj.m1;
  f.planes[2][1] = viewProj.m7 + viewProj.m5;
  f.planes[2][2] = viewProj.m11 + viewProj.m9;
  f.planes[2][3] = viewProj.m15 + viewProj.m13;
  // Top
  f.planes[3][0] = viewProj.m3 - viewProj.m1;
  f.planes[3][1] = viewProj.m7 - viewProj.m5;
  f.planes[3][2] = viewProj.m11 - viewProj.m9;
  f.planes[3][3] = viewProj.m15 - viewProj.m13;
  // Near
  f.planes[4][0] = viewProj.m3 + viewProj.m2;
  f.planes[4][1] = viewProj.m7 + viewProj.m6;
  f.planes[4][2] = viewProj.m11 + viewProj.m10;
  f.planes[4][3] = viewProj.m15 + viewProj.m14;
  // Far
  f.planes[5][0] = viewProj.m3 - viewProj.m2;
  f.planes[5][1] = viewProj.m7 - viewProj.m6;
  f.planes[5][2] = viewProj.m11 - viewProj.m10;
  f.planes[5][3] = viewProj.m15 - viewProj.m14;

  for (int i = 0; i < 6; i++) {
    float t = sqrtf(f.planes[i][0] * f.planes[i][0] +
                    f.planes[i][1] * f.planes[i][1] +
                    f.planes[i][2] * f.planes[i][2]);
    f.planes[i][0] /= t;
    f.planes[i][1] /= t;
    f.planes[i][2] /= t;
    f.planes[i][3] /= t;
  }
  return f;
}

int main(void) {
  SetConfigFlags(FLAG_WINDOW_RESIZABLE);
  InitWindow(DEFAULT_SCREEN_WIDTH, DEFAULT_SCREEN_HEIGHT,
             "VoxelPopuli - Minecraft 1.0 Clone");
  InitNoise();
  SetWindowMinSize(960, 540);
  LoadWindowState(WINDOW_SAVE_PATH);
  World *world = (World *)malloc(sizeof(World));
  World_Init(world);
  Player player;
  BlockType hotbar[INVENTORY_BLOCK_COUNT];
  InitDefaultPlayer(&player, hotbar);

  if (!EnsureSaveDir())
    TraceLog(LOG_WARNING, "Could not ensure save directory '%s'", SAVE_DIR);
  if (!World_LoadEdits(world, WORLD_SAVE_PATH))
    TraceLog(LOG_WARNING, "Could not load world edits from '%s'",
             WORLD_SAVE_PATH);
  bool loadedPlayer = LoadPlayerState(&player, hotbar, PLAYER_SAVE_PATH);

  for (int i = 0; i < 100; i++)
    World_Update(world, player.position);
  if (!loadedPlayer) {
    int sx = (int)floor(player.position.x);
    int sz = (int)floor(player.position.z);
    for (int y = CHUNK_HEIGHT - 1; y >= 0; y--)
      if (World_GetBlock(world, sx, y, sz) != BLOCK_AIR) {
        player.position.y = (float)y + 1.1f;
        break;
      }
    player.respawnPosition = player.position;
  }
  if (CheckCollision(world, player.position)) {
    for (int i = 0; i < 32 && CheckCollision(world, player.position); i++)
      player.position.y += 1.0f;
  }
  if (!loadedPlayer)
    player.respawnPosition = player.position;
  CleanupHotbarSlots(&player, hotbar);

  Camera3D camera = {(Vector3){0}, (Vector3){0}, (Vector3){0, 1, 0}, 75.0f, 0};
  Vector2 cameraAngle = {PI, 0};
  player.selectedBlock = hotbar[player.selectedSlot];
  int ignoreMouseDeltaFrames = 0;
  float fpsTimer = 0.0f;

  DisableCursor();
  SetTargetFPS(GetMonitorRefreshRate(GetCurrentMonitor()));

  while (!WindowShouldClose()) {
    float dt = GetFrameTime();
    if (dt > 0.05f)
      dt = 0.05f;

    fpsTimer += GetFrameTime();
    if (fpsTimer >= 1.0f) {
      printf("FPS: %d\n", GetFPS());
      fpsTimer = 0.0f;
    }

    if (player.damageCooldown > 0.0f) {
      player.damageCooldown -= dt;
      if (player.damageCooldown < 0.0f)
        player.damageCooldown = 0.0f;
    }
    if (IsKeyPressed(KEY_E)) {
      player.inventoryOpen = !player.inventoryOpen;
      if (player.inventoryOpen) {
        EnableCursor();
      } else {
        DisableCursor();
        SetMousePosition(GetScreenWidth() / 2, GetScreenHeight() / 2);
        ignoreMouseDeltaFrames = 2;
      }
    }

    if (!player.inventoryOpen) {
      if (IsWindowResized()) {
        SetMousePosition(GetScreenWidth() / 2, GetScreenHeight() / 2);
        ignoreMouseDeltaFrames = 2;
      }
      Vector2 md = {0};
      if (!IsWindowFocused() || ignoreMouseDeltaFrames > 0) {
        if (ignoreMouseDeltaFrames > 0)
          ignoreMouseDeltaFrames--;
        (void)GetMouseDelta();
      } else {
        md = GetMouseDelta();
      }
      cameraAngle.x -= md.x * 0.003f;
      cameraAngle.y -= md.y * 0.003f;
      if (cameraAngle.y > 1.56f)
        cameraAngle.y = 1.56f;
      if (cameraAngle.y < -1.56f)
        cameraAngle.y = -1.56f;
    }

    for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
      if (IsKeyPressed(KEY_ONE + i)) {
        player.selectedSlot = i;
        player.selectedBlock = hotbar[i];
      }
    }
    if (!player.inventoryOpen) {
      int wheelSteps = (int)GetMouseWheelMove();
      if (wheelSteps != 0) {
        int slot = player.selectedSlot - wheelSteps;
        while (slot < 0)
          slot += INVENTORY_BLOCK_COUNT;
        while (slot >= INVENTORY_BLOCK_COUNT)
          slot -= INVENTORY_BLOCK_COUNT;
        player.selectedSlot = slot;
        player.selectedBlock = hotbar[player.selectedSlot];
      }
    }

    camera.position = (Vector3){player.position.x, player.position.y + 1.6f,
                                player.position.z};
    Vector3 forwardDir = {cosf(cameraAngle.y) * sinf(cameraAngle.x),
                          sinf(cameraAngle.y),
                          cosf(cameraAngle.y) * cosf(cameraAngle.x)};
    camera.target = Vector3Add(camera.position, forwardDir);

    // MATHEMATICALLY DERIVED VECTORS
    Vector3 forward = {sinf(cameraAngle.x), 0, cosf(cameraAngle.x)};
    Vector3 right =
        Vector3CrossProduct(forward, (Vector3){0, 1, 0}); // True Right vector

    if (!player.inventoryOpen) {
      bool bodyInWater =
          World_GetBlock(world, (int)floor(player.position.x),
                         (int)floor(player.position.y + 0.9f),
                         (int)floor(player.position.z)) == BLOCK_WATER;

      Vector3 moveDir = {0};
      if (IsKeyDown(KEY_W))
        moveDir = Vector3Add(moveDir, forward);
      if (IsKeyDown(KEY_S))
        moveDir = Vector3Subtract(moveDir, forward);
      if (IsKeyDown(KEY_A))
        moveDir = Vector3Subtract(moveDir, right);
      if (IsKeyDown(KEY_D))
        moveDir = Vector3Add(moveDir, right);

      float speed = IsKeyDown(KEY_LEFT_CONTROL) ? 8.5f : 4.5f;
      if (bodyInWater)
        speed *= 0.75f;
      if (Vector3Length(moveDir) > 0.1f)
        moveDir = Vector3Scale(Vector3Normalize(moveDir), speed);
      player.velocity.x = moveDir.x;
      player.velocity.z = moveDir.z;
      float gravity = bodyInWater ? 14.0f : 28.0f;
      player.velocity.y -= gravity * dt;
      if (player.velocity.y < -35.0f)
        player.velocity.y = -35.0f; // Terminal velocity to prevent clipping

      if (IsKeyPressed(KEY_SPACE)) {
        if (player.grounded) {
          player.velocity.y = 9.0f;
          player.grounded = false;
        } else if (bodyInWater) {
          player.velocity.y = 9.8f;
        }
      } else if (bodyInWater && IsKeyDown(KEY_SPACE)) {
        player.velocity.y += 35.0f * dt;
        if (player.velocity.y > 5.6f)
          player.velocity.y = 5.6f;
      }

      float prevY = player.position.y;
      bool landed = false;
      int ySteps = (int)ceilf(fabsf(player.velocity.y * dt) / 0.1f);
      if (ySteps == 0)
        ySteps = 1;
      float stepY = (player.velocity.y * dt) / ySteps;
      for (int i = 0; i < ySteps; i++) {
        player.position.y += stepY;
        if (CheckCollision(world, player.position)) {
          if (player.velocity.y < 0.0f) {
            player.grounded = true;
            landed = true;
          }
          player.position.y -= stepY;
          player.velocity.y = 0.0f;
          break;
        } else if (player.velocity.y != 0.0f) {
          player.grounded = false;
        }
      }

      int xSteps = (int)ceilf(fabsf(player.velocity.x * dt) / 0.1f);
      if (xSteps == 0)
        xSteps = 1;
      float stepX = (player.velocity.x * dt) / xSteps;
      for (int i = 0; i < xSteps; i++) {
        player.position.x += stepX;
        if (CheckCollision(world, player.position)) {
          player.position.x -= stepX;
          player.velocity.x = 0.0f;
          break;
        }
      }

      int zSteps = (int)ceilf(fabsf(player.velocity.z * dt) / 0.1f);
      if (zSteps == 0)
        zSteps = 1;
      float stepZ = (player.velocity.z * dt) / zSteps;
      for (int i = 0; i < zSteps; i++) {
        player.position.z += stepZ;
        if (CheckCollision(world, player.position)) {
          player.position.z -= stepZ;
          player.velocity.z = 0.0f;
          break;
        }
      }

      if (CheckCollision(world, player.position))
        player.position.y += 0.1f;

      bool bodyInWaterNow =
          World_GetBlock(world, (int)floor(player.position.x),
                         (int)floor(player.position.y + 0.9f),
                         (int)floor(player.position.z)) == BLOCK_WATER;
      bool headInWaterNow =
          World_GetBlock(world, (int)floor(player.position.x),
                         (int)floor(player.position.y + 1.62f),
                         (int)floor(player.position.z)) == BLOCK_WATER;

      if (!bodyInWaterNow && !player.grounded && player.position.y < prevY)
        player.fallDistance += (prevY - player.position.y);
      if (landed) {
        int fallDamage =
            (int)floorf(player.fallDistance - PLAYER_FALL_DAMAGE_THRESHOLD);
        if (fallDamage > 0)
          DamagePlayer(&player, fallDamage);
        player.fallDistance = 0.0f;
      }
      if (bodyInWaterNow || player.grounded)
        player.fallDistance = 0.0f;

      if (headInWaterNow) {
        player.airSeconds -= dt;
        if (player.airSeconds <= 0.0f) {
          player.airSeconds = 0.0f;
          player.drownTickTimer -= dt;
          if (player.drownTickTimer <= 0.0f) {
            DamagePlayer(&player, 1);
            player.drownTickTimer = PLAYER_DROWN_DAMAGE_INTERVAL;
          }
        } else {
          player.drownTickTimer = PLAYER_DROWN_DAMAGE_INTERVAL;
        }
      } else {
        player.airSeconds += dt * 3.0f;
        if (player.airSeconds > PLAYER_MAX_AIR_SECONDS)
          player.airSeconds = PLAYER_MAX_AIR_SECONDS;
        player.drownTickTimer = PLAYER_DROWN_DAMAGE_INTERVAL;
      }

      if (player.position.y < PLAYER_VOID_Y)
        DamagePlayer(&player, PLAYER_MAX_HEALTH);
      if (player.health <= 0) {
        RespawnPlayer(&player, world);
        DisableCursor();
      }

      if (IsMouseButtonPressed(MOUSE_LEFT_BUTTON)) {
        Ray ray = {camera.position, Vector3Normalize(Vector3Subtract(
                                        camera.target, camera.position))};
        RaycastResult res =
            World_Raycast(world, ray.position, ray.direction, 5.0f);
        if (res.hit && res.block != BLOCK_BEDROCK) {
          int harvested = GetInventoryIndex(res.block);
          if (harvested >= 0) {
            player.blockCounts[harvested]++;
            TryAutoAssignHotbar(hotbar, res.block);
          }
          World_SetBlock(world, res.x, res.y, res.z, BLOCK_AIR);
        }
      }
      int selectedIndex = GetInventoryIndex(player.selectedBlock);
      if (IsMouseButtonPressed(MOUSE_RIGHT_BUTTON) && selectedIndex >= 0 &&
          player.blockCounts[selectedIndex] > 0) {
        Ray ray = {camera.position, Vector3Normalize(Vector3Subtract(
                                        camera.target, camera.position))};
        RaycastResult res =
            World_Raycast(world, ray.position, ray.direction, 5.0f);
        if (res.hit) {
          int px = res.x + res.nx, py = res.y + res.ny, pz = res.z + res.nz;
          if (!CheckCollision(world, (Vector3){(float)px + 0.5f, (float)py,
                                               (float)pz + 0.5f})) {
            BlockType existing = World_GetBlock(world, px, py, pz);
            if (existing == BLOCK_AIR || existing == BLOCK_WATER) {
              World_SetBlock(world, px, py, pz, player.selectedBlock);
              if (World_GetBlock(world, px, py, pz) == player.selectedBlock)
                player.blockCounts[selectedIndex]--;
            }
          }
        }
      }
    }
    CleanupHotbarSlots(&player, hotbar);

    World_Update(world, player.position);
    int screenWidth = GetScreenWidth();
    int screenHeight = GetScreenHeight();
    int vignetteHeight = 100;
    if (vignetteHeight > screenHeight / 2)
      vignetteHeight = screenHeight / 2;
    int hotbarWidth = 620;
    int hotbarY = screenHeight - 80;
    int hbX = (screenWidth - hotbarWidth) / 2;
    int heartsX = hbX;
    int heartsY = hotbarY - 28;
    int inventoryStartX = screenWidth / 2 - 200;
    int inventoryStartY = screenHeight / 2 - 160;
    BeginDrawing();
    ClearBackground(SKYBLUE);
    BeginMode3D(camera);
    Frustum frustum = ExtractFrustum(camera);
    World_Render(world, frustum);
    World_RenderClouds(world, player.position, (float)GetTime(), frustum);
    if (!player.inventoryOpen) {
      Ray ray = {camera.position, Vector3Normalize(Vector3Subtract(
                                      camera.target, camera.position))};
      RaycastResult res =
          World_Raycast(world, ray.position, ray.direction, 5.0f);
      if (res.hit)
        DrawCubeWires((Vector3){(float)res.x + 0.5f, (float)res.y + 0.5f,
                                (float)res.z + 0.5f},
                      1.01f, 1.01f, 1.01f, BLACK);
    }
    EndMode3D();

    bool cameraInWater =
        World_GetBlock(world, (int)floor(camera.position.x),
                       (int)floor(camera.position.y),
                       (int)floor(camera.position.z)) == BLOCK_WATER;
    if (cameraInWater)
      DrawRectangle(0, 0, screenWidth, screenHeight, (Color){0, 121, 241, 150});
    DrawRectangleGradientV(0, 0, screenWidth, vignetteHeight, Fade(BLACK, 0.3f),
                           BLANK);
    DrawRectangleGradientV(0, screenHeight - vignetteHeight, screenWidth,
                           vignetteHeight, BLANK, Fade(BLACK, 0.3f));

    for (int i = 0; i < PLAYER_MAX_HEALTH / 2; i++) {
      int units = player.health - i * 2;
      int fill = (units >= 2) ? 2 : (units == 1 ? 1 : 0);
      DrawHeartIcon(heartsX + i * 20, heartsY, fill);
    }
    if (cameraInWater || player.airSeconds < PLAYER_MAX_AIR_SECONDS) {
      int bubbles =
          (int)ceilf((player.airSeconds / PLAYER_MAX_AIR_SECONDS) * 10.0f);
      if (bubbles < 0)
        bubbles = 0;
      if (bubbles > 10)
        bubbles = 10;
      for (int i = 0; i < 10; i++)
        DrawBubbleIcon(heartsX + i * 18 + 6, heartsY - 12, i < bubbles);
    }
    for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
      Rectangle rect = {(float)hbX + i * 70, (float)hotbarY, 60, 60};
      DrawRectangleRec(rect, player.selectedSlot == i ? Fade(WHITE, 0.6f)
                                                      : Fade(BLACK, 0.4f));
      DrawRectangleLinesEx(rect, 2, player.selectedSlot == i ? YELLOW : WHITE);
      if (hotbar[i] != BLOCK_AIR) {
        int idx = GetInventoryIndex(hotbar[i]);
        int count = (idx >= 0) ? player.blockCounts[idx] : 0;
        DrawRectangle(rect.x + 10, rect.y + 10, 40, 40,
                      GetBlockUIColour(hotbar[i]));
        DrawText(TextFormat("%d", count), (int)rect.x + 4, (int)rect.y + 38, 18,
                 (count > 0) ? WHITE : LIGHTGRAY);
      }
    }

    if (player.inventoryOpen) {
      DrawRectangle(0, 0, screenWidth, screenHeight, Fade(BLACK, 0.6f));
      const char *inventoryTitle = "Inventory";
      int titleSize = 30;
      DrawText(inventoryTitle,
               (screenWidth - MeasureText(inventoryTitle, titleSize)) / 2,
               inventoryStartY - 60, titleSize, WHITE);
      const char *inventoryHint = "Collect blocks by mining. Click item to "
                                  "assign selected hotbar slot.";
      int hintSize = 20;
      DrawText(inventoryHint,
               (screenWidth - MeasureText(inventoryHint, hintSize)) / 2,
               inventoryStartY - 24, hintSize, LIGHTGRAY);
      for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
        Rectangle r = {(float)inventoryStartX + (i % 5) * 80,
                       (float)inventoryStartY + (i / 5) * 80, 70, 70};
        DrawRectangleRec(r, Fade(WHITE, 0.2f));
        DrawRectangle(r.x + 15, r.y + 12, 40, 40,
                      GetBlockUIColour(inventoryBlocks[i]));
        DrawText(TextFormat("%d", player.blockCounts[i]), (int)r.x + 6,
                 (int)r.y + 48, 18, WHITE);
        if (CheckCollisionPointRec(GetMousePosition(), r)) {
          DrawRectangleLinesEx(r, 2, YELLOW);
          if (IsMouseButtonPressed(MOUSE_LEFT_BUTTON) &&
              player.blockCounts[i] > 0) {
            hotbar[player.selectedSlot] = inventoryBlocks[i];
            player.selectedBlock = hotbar[player.selectedSlot];
          }
        } else
          DrawRectangleLinesEx(r, 2, Fade(WHITE, 0.6f));
      }
    }
    if (!player.inventoryOpen)
      DrawCircle(screenWidth / 2, screenHeight / 2, 2, WHITE);
    DrawText(TextFormat("FPS: %d", GetFPS()), 20, 20, 20, GREEN);
    EndDrawing();
  }
  if (!EnsureSaveDir())
    TraceLog(LOG_WARNING, "Could not ensure save directory '%s'", SAVE_DIR);
  if (!SaveWindowState(WINDOW_SAVE_PATH))
    TraceLog(LOG_WARNING, "Could not save window state to '%s'",
             WINDOW_SAVE_PATH);
  if (!World_SaveEdits(world, WORLD_SAVE_PATH))
    TraceLog(LOG_WARNING, "Could not save world edits to '%s'",
             WORLD_SAVE_PATH);
  if (!SavePlayerState(&player, hotbar, PLAYER_SAVE_PATH))
    TraceLog(LOG_WARNING, "Could not save player state to '%s'",
             PLAYER_SAVE_PATH);
  World_Unload(world);
  free(world);
  CloseWindow();
  return 0;
}
