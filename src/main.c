#include <errno.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <time.h>

#include "noise.h"
#include "raylib.h"
#include "raymath.h"
#include "world.h"
#include "ps1.h"
#include "rlgl.h"

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

static const BlockType inventoryBlocks[INVENTORY_BLOCK_COUNT] = {
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

static int GetInventoryIndex(BlockType block) {
  for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++)
    if (inventoryBlocks[i] == block) return i;
  return -1;
}

static Color GetBlockUIColour(BlockType block) {
  if (block == BLOCK_GRASS) return GREEN;
  if (block == BLOCK_DIRT) return BROWN;
  if (block == BLOCK_STONE) return GRAY;
  if (block == BLOCK_OAK_LOG) return DARKBROWN;
  if (block == BLOCK_OAK_LEAVES) return DARKGREEN;
  if (block == BLOCK_SAND) return BEIGE;
  if (block == BLOCK_WATER) return BLUE;
  if (block == BLOCK_GRAVEL) return LIGHTGRAY;
  if (block == BLOCK_BEDROCK) return DARKGRAY;
  if (block == BLOCK_COAL_ORE) return BLACK;
  return WHITE;
}

static void DrawHeartIcon(int x, int y, int fillState) {
  static const unsigned char sprite[8][9] = {
      {0, 1, 1, 0, 0, 0, 1, 1, 0}, {1, 2, 2, 1, 0, 1, 2, 2, 1},
      {1, 2, 2, 2, 1, 2, 2, 2, 1}, {1, 2, 2, 2, 2, 2, 2, 2, 1},
      {0, 1, 2, 2, 2, 2, 2, 1, 0}, {0, 0, 1, 2, 2, 2, 1, 0, 0},
      {0, 0, 0, 1, 2, 1, 0, 0, 0}, {0, 0, 0, 0, 1, 0, 0, 0, 0}};
  const int scale = 2;
  for (int py = 0; py < 8; py++) {
    for (int px = 0; px < 9; px++) {
      unsigned char cell = sprite[py][px]; if (cell == 0) continue;
      bool filledSide = (fillState >= 2) || (fillState == 1 && px <= 4);
      Color pixel = (cell == 1) ? (Color){28, 10, 10, 255} : (filledSide ? (Color){224, 42, 42, 255} : (Color){72, 32, 32, 255});
      DrawRectangle(x + px * scale, y + py * scale, scale, scale, pixel);
    }
  }
}

static void DrawBubbleIcon(int x, int y, bool filled) {
  Color fill = filled ? (Color){169, 216, 255, 230} : (Color){76, 101, 130, 210};
  Color highlight = filled ? (Color){235, 248, 255, 220} : (Color){120, 144, 170, 190};
  Color border = (Color){26, 45, 66, 230};
  DrawCircle(x, y, 6.0f, fill);
  DrawCircle(x - 2, y - 2, 2.0f, highlight);
  DrawCircleLines(x, y, 6.0f, border);
}

static bool SaveWindowState(const char *path) {
  FILE *file = fopen(path, "wb"); if (!file) return false;
  WindowSaveData data = {WINDOW_SAVE_MAGIC, WINDOW_SAVE_VERSION, GetScreenWidth(), GetScreenHeight(), (uint8_t)(IsWindowState(FLAG_WINDOW_MAXIMIZED) ? 1u : 0u), {0}};
  bool ok = fwrite(&data, sizeof(data), 1, file) == 1; fclose(file); return ok;
}

static bool LoadWindowState(const char *path) {
  FILE *file = fopen(path, "rb"); if (!file) return false;
  WindowSaveData data = {0}; bool ok = fread(&data, sizeof(data), 1, file) == 1; fclose(file);
  if (!ok || data.magic != WINDOW_SAVE_MAGIC || data.version != WINDOW_SAVE_VERSION) return false;
  SetWindowSize(data.width < 960 ? 960 : data.width, data.height < 540 ? 540 : data.height);
  if (data.maximized) MaximizeWindow(); return true;
}

static void CleanupHotbarSlots(Player *player, BlockType *hotbar) {
  for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
    if (hotbar[i] == BLOCK_AIR) continue;
    int idx = GetInventoryIndex(hotbar[i]);
    if (idx < 0 || player->blockCounts[idx] <= 0) hotbar[i] = BLOCK_AIR;
  }
  player->selectedBlock = hotbar[player->selectedSlot];
}

static bool EnsureSaveDir(void) { if (mkdir(SAVE_DIR, 0755) == 0) return true; return errno == EEXIST; }

static void InitDefaultPlayer(Player *player, BlockType *hotbar) {
  memset(player, 0, sizeof(*player));
  player->position = (Vector3){32.5f, 164.0f, 32.5f};
  player->respawnPosition = player->position;
  player->health = PLAYER_MAX_HEALTH; player->airSeconds = PLAYER_MAX_AIR_SECONDS;
  player->drownTickTimer = PLAYER_DROWN_DAMAGE_INTERVAL;
  for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) { player->blockCounts[i] = 0; hotbar[i] = BLOCK_AIR; }
}

static bool SavePlayerState(const Player *player, const BlockType *hotbar, const char *path) {
  FILE *file = fopen(path, "wb"); if (!file) return false;
  PlayerSaveDataV2 data = {PLAYER_SAVE_MAGIC, PLAYER_SAVE_VERSION, {player->position.x, player->position.y, player->position.z}, {player->velocity.x, player->velocity.y, player->velocity.z}, player->selectedSlot};
  for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) { data.blockCounts[i] = player->blockCounts[i]; data.hotbarBlocks[i] = (int32_t)hotbar[i]; }
  data.health = player->health; data.airSeconds = player->airSeconds; data.respawnPos[0] = player->respawnPosition.x; data.respawnPos[1] = player->respawnPosition.y; data.respawnPos[2] = player->respawnPosition.z;
  bool ok = fwrite(&data, sizeof(data), 1, file) == 1; fclose(file); return ok;
}

static bool LoadPlayerState(Player *player, BlockType *hotbar, const char *path) {
  FILE *file = fopen(path, "rb"); if (!file) return false;
  uint32_t magic = 0, version = 0; fread(&magic, sizeof(magic), 1, file); fread(&version, sizeof(version), 1, file);
  if (magic != PLAYER_SAVE_MAGIC) { fclose(file); return false; }
  rewind(file);
  if (version == PLAYER_SAVE_VERSION) {
    PlayerSaveDataV2 data = {0}; fread(&data, sizeof(data), 1, file);
    player->position = (Vector3){data.playerPos[0], data.playerPos[1], data.playerPos[2]};
    player->velocity = (Vector3){data.playerVelocity[0], data.playerVelocity[1], data.playerVelocity[2]};
    player->respawnPosition = (Vector3){data.respawnPos[0], data.respawnPos[1], data.respawnPos[2]};
    player->selectedSlot = data.selectedSlot; player->health = data.health; player->airSeconds = data.airSeconds;
    for (int i=0; i<INVENTORY_BLOCK_COUNT; i++) { player->blockCounts[i] = data.blockCounts[i]; hotbar[i] = (BlockType)data.hotbarBlocks[i]; }
    fclose(file); return true;
  }
  fclose(file); return false;
}

static bool IsPointInBlock(World *world, Vector3 p) {
  BlockType b = World_GetBlock(world, (int)floor(p.x), (int)floor(p.y), (int)floor(p.z));
  return b != BLOCK_AIR && b != BLOCK_WATER;
}

static bool CheckCollision(World *world, Vector3 pos) {
  float w = 0.22f, h = 1.75f;
  for (float x = -w; x <= w; x += w * 2) {
    for (float z = -w; z <= w; z += w * 2) {
      for (int yi = 0; yi <= 2; yi++) {
        float y = 0.1f + yi * ((h - 0.1f) / 2.0f);
        if (IsPointInBlock(world, (Vector3){pos.x + x, pos.y + y, pos.z + z})) return true;
      }
    }
  }
  return false;
}

static void RespawnPlayer(Player *player, World *world) {
  int sx = (int)floor(player->respawnPosition.x), sz = (int)floor(player->respawnPosition.z);
  float safeY = player->respawnPosition.y;
  for (int y = CHUNK_HEIGHT - 1; y >= 0; y--) {
    BlockType b = World_GetBlock(world, sx, y, sz);
    if (b != BLOCK_AIR && b != BLOCK_WATER) { safeY = (float)y + 1.8f; break; }
  }
  player->position = (Vector3){player->respawnPosition.x, safeY, player->respawnPosition.z};
  player->velocity = (Vector3){0}; player->grounded = false; player->health = PLAYER_MAX_HEALTH;
  player->airSeconds = PLAYER_MAX_AIR_SECONDS; player->fallDistance = 0.0f;
  for (int i = 0; i < 32 && (CheckCollision(world, player->position) || World_GetBlock(world, (int)floor(player->position.x), (int)floor(player->position.y), (int)floor(player->position.z)) == BLOCK_WATER); i++) player->position.y += 1.0f;
}

static Frustum ExtractFrustum(void) {
  // Must use current rlgl matrices
  Matrix matView = rlGetMatrixModelview();
  Matrix matProj = rlGetMatrixProjection();
  Matrix viewProj = MatrixMultiply(matView, matProj);
  Frustum f;
  f.planes[0][0]=viewProj.m3+viewProj.m0; f.planes[0][1]=viewProj.m7+viewProj.m4; f.planes[0][2]=viewProj.m11+viewProj.m8; f.planes[0][3]=viewProj.m15+viewProj.m12;
  f.planes[1][0]=viewProj.m3-viewProj.m0; f.planes[1][1]=viewProj.m7-viewProj.m4; f.planes[1][2]=viewProj.m11-viewProj.m8; f.planes[1][3]=viewProj.m15-viewProj.m12;
  f.planes[2][0]=viewProj.m3+viewProj.m1; f.planes[2][1]=viewProj.m7+viewProj.m5; f.planes[2][2]=viewProj.m11+viewProj.m9; f.planes[2][3]=viewProj.m15+viewProj.m13;
  f.planes[3][0]=viewProj.m3-viewProj.m1; f.planes[3][1]=viewProj.m7-viewProj.m5; f.planes[3][2]=viewProj.m11-viewProj.m9; f.planes[3][3]=viewProj.m15-viewProj.m13;
  f.planes[4][0]=viewProj.m3+viewProj.m2; f.planes[4][1]=viewProj.m7+viewProj.m6; f.planes[4][2]=viewProj.m11+viewProj.m10; f.planes[4][3]=viewProj.m15+viewProj.m14;
  f.planes[5][0]=viewProj.m3-viewProj.m2; f.planes[5][1]=viewProj.m7-viewProj.m6; f.planes[5][2]=viewProj.m11-viewProj.m10; f.planes[5][3]=viewProj.m15-viewProj.m14;
  for (int i=0; i<6; i++) {
    float t = sqrtf(f.planes[i][0]*f.planes[i][0] + f.planes[i][1]*f.planes[i][1] + f.planes[i][2]*f.planes[i][2]);
    f.planes[i][0]/=t; f.planes[i][1]/=t; f.planes[i][2]/=t; f.planes[i][3]/=t;
  }
  return f;
}

#ifdef PS1_RENDERER
static void UpdatePS1Target(RenderTexture2D *target, int *currentW, int *currentH) {
    int sw = GetScreenWidth(), sh = GetScreenHeight(), th = 480, tw = (int)((float)th * (float)sw/(float)sh);
    if (tw != *currentW || th != *currentH) { UnloadRenderTexture(*target); *target = LoadRenderTexture(tw, th); SetTextureFilter(target->texture, TEXTURE_FILTER_POINT); *currentW = tw; *currentH = th; }
}
#endif

static void DrawSun(Vector3 playerPos, float time) {
  float sunDist = 450.0f, sunSize = 80.0f, angle = (time / 120.0f) * 2.0f * PI;
  Vector3 sunDir = { 0, sinf(angle), cosf(angle) };
  Vector3 sunPos = Vector3Add(playerPos, Vector3Scale(sunDir, sunDist));
  rlDrawRenderBatchActive(); rlPushMatrix();
  rlTranslatef(sunPos.x, sunPos.y, sunPos.z);
  rlRotatef(RAD2DEG * atan2f(playerPos.x - sunPos.x, playerPos.z - sunPos.z), 0, 1, 0);
  rlRotatef(RAD2DEG * asinf((sunPos.y - playerPos.y) / sunDist), 1, 0, 0);
  rlDisableDepthTest(); rlDisableBackfaceCulling(); rlSetTexture(rlGetTextureIdDefault());
  rlBegin(RL_QUADS); rlColor4ub(255, 255, 255, 255);
  rlVertex3f(-sunSize, -sunSize, 0); rlVertex3f(sunSize, -sunSize, 0);
  rlVertex3f(sunSize, sunSize, 0); rlVertex3f(-sunSize, sunSize, 0);
  rlEnd();
  rlSetTexture(0); rlEnableBackfaceCulling(); rlEnableDepthTest();
  rlPopMatrix(); rlDrawRenderBatchActive();
}

int main(void) {
  srand((unsigned int)time(NULL)); SetConfigFlags(FLAG_WINDOW_RESIZABLE);
  InitWindow(DEFAULT_SCREEN_WIDTH, DEFAULT_SCREEN_HEIGHT, "VoxelPopuli - Minecraft 1.0 Clone");
  InitNoise(); SetWindowMinSize(960, 540); LoadWindowState(WINDOW_SAVE_PATH);
  World *world = (World *)malloc(sizeof(World)); World_Init(world);
#ifdef PS1_RENDERER
  Shader ps1Shader = LoadShaderFromMemory(ps1_vs, ps1_fs);
  int ps1W = 640, ps1H = 480; RenderTexture2D ps1Target = LoadRenderTexture(ps1W, ps1H);
  SetTextureFilter(ps1Target.texture, TEXTURE_FILTER_POINT);
  float precisionVal = 480.0f; SetShaderValue(ps1Shader, GetShaderLocation(ps1Shader, "uPrecision"), &precisionVal, SHADER_UNIFORM_FLOAT);
  world->ps1Shader = ps1Shader;
#endif
  Player player; BlockType hotbar[INVENTORY_BLOCK_COUNT]; InitDefaultPlayer(&player, hotbar);
  EnsureSaveDir(); World_LoadEdits(world, WORLD_SAVE_PATH); bool loaded = LoadPlayerState(&player, hotbar, PLAYER_SAVE_PATH);
  // Warm up chunks for spawn logic
  for (int i=0; i<100; i++) World_Update(world, player.position);
  if (!loaded) {
    int sx = (int)floor(player.position.x), sz = (int)floor(player.position.z);
    for (int y=CHUNK_HEIGHT-1; y>=0; y--) { if (World_GetBlock(world, sx, y, sz) != BLOCK_AIR) { player.position.y = (float)y + 1.8f; break; } }
    player.respawnPosition = player.position;
  }
  for (int i = 0; i < 32 && CheckCollision(world, player.position); i++) player.position.y += 1.0f;
  CleanupHotbarSlots(&player, hotbar);
  Camera3D camera = {{0}, {0}, {0, 1, 0}, 75.0f, 0}; Vector2 cameraAngle = {PI, 0}; DisableCursor();
  SetTargetFPS(GetMonitorRefreshRate(GetCurrentMonitor()));

  while (!WindowShouldClose()) {
    float dt = fminf(GetFrameTime(), 0.05f);
    if (IsKeyPressed(KEY_E)) { player.inventoryOpen = !player.inventoryOpen; if (player.inventoryOpen) EnableCursor(); else { DisableCursor(); SetMousePosition(GetScreenWidth()/2, GetScreenHeight()/2); } }
    if (!player.inventoryOpen) {
      Vector2 md = GetMouseDelta(); cameraAngle.x -= md.x * 0.003f; cameraAngle.y -= md.y * 0.003f; cameraAngle.y = fmaxf(-1.56f, fminf(1.56f, cameraAngle.y));
      int wheel = (int)GetMouseWheelMove(); if (wheel != 0) { player.selectedSlot = (player.selectedSlot - wheel + INVENTORY_BLOCK_COUNT) % INVENTORY_BLOCK_COUNT; player.selectedBlock = hotbar[player.selectedSlot]; }
    }
    camera.position = (Vector3){player.position.x, player.position.y + 1.6f, player.position.z};
    camera.target = Vector3Add(camera.position, (Vector3){cosf(cameraAngle.y)*sinf(cameraAngle.x), sinf(cameraAngle.y), cosf(cameraAngle.y)*cosf(cameraAngle.x)});
    Vector3 forward = {sinf(cameraAngle.x), 0, cosf(cameraAngle.x)}, right = Vector3CrossProduct(forward, (Vector3){0, 1, 0});
    if (!player.inventoryOpen) {
      bool headInW = World_GetBlock(world, (int)floor(player.position.x), (int)floor(player.position.y + 1.6f), (int)floor(player.position.z)) == BLOCK_WATER;
      bool waistInW = World_GetBlock(world, (int)floor(player.position.x), (int)floor(player.position.y + 0.9f), (int)floor(player.position.z)) == BLOCK_WATER;
      bool feetInW = World_GetBlock(world, (int)floor(player.position.x), (int)floor(player.position.y + 0.1f), (int)floor(player.position.z)) == BLOCK_WATER;
      Vector3 mv = {0}; if (IsKeyDown(KEY_W)) mv = Vector3Add(mv, forward); if (IsKeyDown(KEY_S)) mv = Vector3Subtract(mv, forward); if (IsKeyDown(KEY_A)) mv = Vector3Subtract(mv, right); if (IsKeyDown(KEY_D)) mv = Vector3Add(mv, right);
      if (Vector3Length(mv) > 0.1f) mv = Vector3Scale(Vector3Normalize(mv), (waistInW ? 3.5f : (IsKeyDown(KEY_LEFT_CONTROL) ? 8.5f : 4.5f)));
      player.velocity.x = mv.x; player.velocity.z = mv.z; player.velocity.y -= (waistInW ? 14.0f : 28.0f) * dt;
      if (IsKeyDown(KEY_SPACE)) { 
        if (waistInW) { 
          player.velocity.y = 4.0f; 
        } else if (player.grounded) { 
          player.velocity.y = 9.0f; 
          player.grounded = false; 
        } else if (feetInW) {
          player.velocity.y = 5.0f; 
        }
      }
      float dy = player.velocity.y * dt; player.position.y += dy;
      if (CheckCollision(world, player.position)) { if (player.velocity.y < 0.0f) player.grounded = true; player.position.y -= dy; player.velocity.y = 0.0f; } else if (player.velocity.y != 0.0f) player.grounded = false;
      float dx = player.velocity.x * dt; player.position.x += dx; if (CheckCollision(world, player.position)) player.position.x -= dx;
      float dz = player.velocity.z * dt; player.position.z += dz; if (CheckCollision(world, player.position)) player.position.z -= dz;
      if (headInW) { player.airSeconds -= dt; if (player.airSeconds < 0) player.airSeconds = 0; } else { player.airSeconds += dt * 3.0f; if (player.airSeconds > PLAYER_MAX_AIR_SECONDS) player.airSeconds = PLAYER_MAX_AIR_SECONDS; }
      if (player.position.y < PLAYER_VOID_Y) RespawnPlayer(&player, world);
    }
    World_Update(world, player.position);
    int sw = GetScreenWidth(), sh = GetScreenHeight();
#ifdef PS1_RENDERER
    int ps1W = 640, ps1H = 480; UpdatePS1Target(&ps1Target, &ps1W, &ps1H);
#endif
    BeginDrawing();
#ifdef PS1_RENDERER
    BeginTextureMode(ps1Target);
#endif
    float time = (float)GetTime(), sunAngle = (time / 120.0f) * 2.0f * PI, sunY = sinf(sunAngle);
    Color skyC = SKYBLUE; if (sunY < 0.2f) { float m = fmaxf(0.0f, fminf(1.0f, (0.2f-sunY)*2.5f)); skyC.r=(unsigned char)(SKYBLUE.r*(1-m)+10*m); skyC.g=(unsigned char)(SKYBLUE.g*(1-m)+10*m); skyC.b=(unsigned char)(SKYBLUE.b*(1-m)+30*m); }
    ClearBackground(skyC);
    BeginMode3D(camera);
    Frustum f = ExtractFrustum(); DrawSun(player.position, time);
#ifdef PS1_RENDERER
    BeginShaderMode(ps1Shader);
    Vector3 sD = {0, sinf(sunAngle), cosf(sunAngle)}; Vector4 sCV = ColorNormalize(skyC);
    SetShaderValue(ps1Shader, GetShaderLocation(ps1Shader, "sunDir"), &sD, SHADER_UNIFORM_VEC3);
    SetShaderValue(ps1Shader, GetShaderLocation(ps1Shader, "time"), &time, SHADER_UNIFORM_FLOAT);
    SetShaderValue(ps1Shader, GetShaderLocation(ps1Shader, "viewPos"), &camera.position, SHADER_UNIFORM_VEC3);
    SetShaderValue(ps1Shader, GetShaderLocation(ps1Shader, "skyCol"), &sCV, SHADER_UNIFORM_VEC4);
#endif
    World_Render(world, f);
#ifdef PS1_RENDERER
    EndShaderMode();
#endif
    World_RenderClouds(world, player.position, time, f);
    EndMode3D();
#ifdef PS1_RENDERER
    EndTextureMode(); DrawTexturePro(ps1Target.texture, (Rectangle){0, 0, (float)ps1Target.texture.width, (float)-ps1Target.texture.height}, (Rectangle){0, 0, (float)sw, (float)sh}, (Vector2){0}, 0.0f, WHITE);
#endif
    if (World_GetBlock(world, (int)floor(camera.position.x), (int)floor(camera.position.y), (int)floor(camera.position.z)) == BLOCK_WATER) DrawRectangle(0, 0, sw, sh, (Color){0, 121, 241, 150});
    int hbX = (sw - 400) / 2, heartsY = sh - 80;
    for (int i = 0; i < PLAYER_MAX_HEALTH / 2; i++) DrawHeartIcon(hbX + i * 30, heartsY, (player.health - i * 2 >= 2) ? 2 : (player.health - i * 2 == 1 ? 1 : 0));
    if (player.airSeconds < PLAYER_MAX_AIR_SECONDS || World_GetBlock(world, (int)floor(player.position.x), (int)floor(player.position.y + 1.6f), (int)floor(player.position.z)) == BLOCK_WATER) {
      int bubbles = (int)ceilf((player.airSeconds / PLAYER_MAX_AIR_SECONDS) * 10.0f);
      for (int i = 0; i < 10; i++) DrawBubbleIcon(hbX + i * 20, heartsY - 20, i < bubbles);
    }
    for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
      Rectangle r = {(float)hbX + i * 40, (float)sh - 45, 35, 35}; DrawRectangleRec(r, player.selectedSlot == i ? Fade(WHITE, 0.6f) : Fade(BLACK, 0.4f));
      DrawRectangleLinesEx(r, 2, player.selectedSlot == i ? YELLOW : WHITE);
      if (hotbar[i] != BLOCK_AIR) {
        int idx = GetInventoryIndex(hotbar[i]), count = (idx >= 0) ? player.blockCounts[idx] : 0;
        DrawRectangle((int)(r.x + 5), (int)(r.y + 5), 25, 25, GetBlockUIColour(hotbar[i]));
        DrawText(TextFormat("%d", count), (int)r.x + 2, (int)r.y + 2, 10, WHITE);
      }
    }
    if (player.inventoryOpen) {
      DrawRectangle(0, 0, sw, sh, Fade(BLACK, 0.6f)); DrawText("Inventory", (sw - MeasureText("Inventory", 40)) / 2, sh / 2 - 140, 40, WHITE);
      for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
        Rectangle r = {(float)(sw / 2 - 175) + (i % 5) * 70, (float)(sh / 2 - 70) + (i / 5) * 70, 60, 60};
        DrawRectangleRec(r, Fade(WHITE, 0.2f)); DrawRectangle((int)(r.x + 12), (int)(r.y + 12), 36, 36, GetBlockUIColour(inventoryBlocks[i]));
        DrawText(TextFormat("%d", player.blockCounts[i]), (int)r.x + 4, (int)r.y + 40, 20, WHITE);
        if (CheckCollisionPointRec(GetMousePosition(), r)) {
          DrawRectangleLinesEx(r, 2, YELLOW); if (IsMouseButtonPressed(MOUSE_LEFT_BUTTON) && player.blockCounts[i] > 0) { hotbar[player.selectedSlot] = inventoryBlocks[i]; player.selectedBlock = hotbar[player.selectedSlot]; }
        } else DrawRectangleLinesEx(r, 2, Fade(WHITE, 0.6f));
      }
    }
    if (!player.inventoryOpen) DrawCircle(sw / 2, sh / 2, 2, WHITE);
    DrawText(TextFormat("FPS: %d", GetFPS()), 10, 10, 20, GREEN);
    EndDrawing();
  }
  SaveWindowState(WINDOW_SAVE_PATH); World_SaveEdits(world, WORLD_SAVE_PATH); SavePlayerState(&player, hotbar, PLAYER_SAVE_PATH);
  World_Unload(world); CloseWindow(); return 0;
}
