#include "raylib.h"
#include "raymath.h"
#include "world.h"
#include "noise.h"
#include <stdlib.h>
#include <math.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <errno.h>
#include <sys/stat.h>
#include <sys/types.h>

#define INVENTORY_BLOCK_COUNT 9
#define SAVE_DIR "save"
#define WORLD_SAVE_PATH SAVE_DIR "/world_edits.bin"
#define PLAYER_SAVE_PATH SAVE_DIR "/player_state.bin"
#define PLAYER_SAVE_MAGIC 0x31505356u
#define PLAYER_SAVE_VERSION 1u

typedef struct {
    Vector3 position, velocity;
    bool grounded;
    BlockType selectedBlock;
    int selectedSlot;
    bool inventoryOpen;
    int blockCounts[INVENTORY_BLOCK_COUNT];
} Player;

BlockType inventoryBlocks[INVENTORY_BLOCK_COUNT] = {
    BLOCK_GRASS, BLOCK_DIRT, BLOCK_STONE, BLOCK_OAK_LOG, BLOCK_OAK_LEAVES,
    BLOCK_SAND, BLOCK_WATER, BLOCK_GRAVEL, BLOCK_BEDROCK
};

typedef struct {
    uint32_t magic;
    uint32_t version;
    float playerPos[3];
    float playerVelocity[3];
    int32_t selectedSlot;
    int32_t blockCounts[INVENTORY_BLOCK_COUNT];
    int32_t hotbarBlocks[INVENTORY_BLOCK_COUNT];
} PlayerSaveData;

int GetInventoryIndex(BlockType block) {
    for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) if (inventoryBlocks[i] == block) return i;
    return -1;
}

Color GetBlockUIColour(BlockType block) {
    if (block == BLOCK_GRASS) return GREEN;
    if (block == BLOCK_DIRT) return BROWN;
    if (block == BLOCK_STONE) return GRAY;
    if (block == BLOCK_OAK_LOG) return DARKBROWN;
    if (block == BLOCK_OAK_LEAVES) return DARKGREEN;
    if (block == BLOCK_SAND) return BEIGE;
    if (block == BLOCK_WATER) return BLUE;
    if (block == BLOCK_GRAVEL) return LIGHTGRAY;
    if (block == BLOCK_BEDROCK) return DARKGRAY;
    return WHITE;
}

void TryAutoAssignHotbar(BlockType *hotbar, BlockType block) {
    if (block == BLOCK_AIR) return;
    for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) if (hotbar[i] == block) return;
    for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
        if (hotbar[i] == BLOCK_AIR) {
            hotbar[i] = block;
            return;
        }
    }
}

void CleanupHotbarSlots(Player *player, BlockType *hotbar) {
    for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
        if (hotbar[i] == BLOCK_AIR) continue;
        int idx = GetInventoryIndex(hotbar[i]);
        if (idx < 0 || player->blockCounts[idx] <= 0) hotbar[i] = BLOCK_AIR;
    }
    player->selectedBlock = hotbar[player->selectedSlot];
}

bool EnsureSaveDir(void) {
    if (mkdir(SAVE_DIR, 0755) == 0) return true;
    return errno == EEXIST;
}

void InitDefaultPlayer(Player *player, BlockType *hotbar) {
    memset(player, 0, sizeof(*player));
    player->position = (Vector3){ 32.5f, 100.0f, 32.5f };
    player->selectedSlot = 0;
    player->inventoryOpen = false;
    for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
        player->blockCounts[i] = 0;
        hotbar[i] = BLOCK_AIR;
    }
    player->selectedBlock = hotbar[player->selectedSlot];
}

bool SavePlayerState(Player *player, BlockType *hotbar, const char *path) {
    FILE *file = fopen(path, "wb");
    if (!file) return false;

    PlayerSaveData data = { 0 };
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

    bool ok = fwrite(&data, sizeof(data), 1, file) == 1;
    fclose(file);
    return ok;
}

bool LoadPlayerState(Player *player, BlockType *hotbar, const char *path) {
    FILE *file = fopen(path, "rb");
    if (!file) return false;

    PlayerSaveData data = { 0 };
    bool ok = fread(&data, sizeof(data), 1, file) == 1;
    fclose(file);
    if (!ok) return false;
    if (data.magic != PLAYER_SAVE_MAGIC || data.version != PLAYER_SAVE_VERSION) return false;

    player->position = (Vector3){ data.playerPos[0], data.playerPos[1], data.playerPos[2] };
    player->velocity = (Vector3){ data.playerVelocity[0], data.playerVelocity[1], data.playerVelocity[2] };
    player->selectedSlot = data.selectedSlot;
    if (player->selectedSlot < 0 || player->selectedSlot >= INVENTORY_BLOCK_COUNT) player->selectedSlot = 0;
    player->grounded = false;
    player->inventoryOpen = false;

    for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
        int count = data.blockCounts[i];
        if (count < 0) count = 0;
        player->blockCounts[i] = count;

        int hb = data.hotbarBlocks[i];
        BlockType candidate = (hb >= BLOCK_AIR && hb <= BLOCK_BEDROCK) ? (BlockType)hb : BLOCK_AIR;
        int idx = GetInventoryIndex(candidate);
        if (candidate != BLOCK_AIR && (idx < 0 || player->blockCounts[idx] <= 0)) candidate = BLOCK_AIR;
        hotbar[i] = candidate;
    }

    player->selectedBlock = hotbar[player->selectedSlot];
    return true;
}

bool IsPointInBlock(World *world, Vector3 p) {
    BlockType b = World_GetBlock(world, (int)floor(p.x), (int)floor(p.y), (int)floor(p.z));
    return b != BLOCK_AIR && b != BLOCK_WATER;
}

bool CheckCollision(World *world, Vector3 pos) {
    float w = 0.22f, h = 1.75f; // Slightly slimmer for smoother transitions
    for (float x = -w; x <= w; x += w * 2)
        for (float z = -w; z <= w; z += w * 2)
            for (float y = 0.1f; y <= h; y += h / 2.0f)
                if (IsPointInBlock(world, (Vector3){ pos.x + x, pos.y + y, pos.z + z })) return true;
    return false;
}

int main(void) {
    InitWindow(1280, 720, "VoxelPopuli - Minecraft 1.0 Clone"); InitNoise();
    World *world = (World *)malloc(sizeof(World)); World_Init(world);
    Player player;
    BlockType hotbar[INVENTORY_BLOCK_COUNT];
    InitDefaultPlayer(&player, hotbar);

    if (!EnsureSaveDir()) TraceLog(LOG_WARNING, "Could not ensure save directory '%s'", SAVE_DIR);
    if (!World_LoadEdits(world, WORLD_SAVE_PATH)) TraceLog(LOG_WARNING, "Could not load world edits from '%s'", WORLD_SAVE_PATH);
    bool loadedPlayer = LoadPlayerState(&player, hotbar, PLAYER_SAVE_PATH);

    for (int i=0; i<100; i++) World_Update(world, player.position);
    if (!loadedPlayer) {
        int sx = (int)floor(player.position.x);
        int sz = (int)floor(player.position.z);
        for (int y = CHUNK_HEIGHT-1; y>=0; y--) if(World_GetBlock(world, sx, y, sz) != BLOCK_AIR) { player.position.y = (float)y+1.1f; break; }
    }
    if (CheckCollision(world, player.position)) {
        for (int i = 0; i < 32 && CheckCollision(world, player.position); i++) player.position.y += 1.0f;
    }
    CleanupHotbarSlots(&player, hotbar);

    Camera3D camera = { (Vector3){0}, (Vector3){0}, (Vector3){0,1,0}, 75.0f, 0 };
    Vector2 cameraAngle = { PI, 0 };
    player.selectedBlock = hotbar[player.selectedSlot];

    DisableCursor(); SetTargetFPS(180);

    while (!WindowShouldClose()) {
        float dt = GetFrameTime(); if (dt > 0.05f) dt = 0.05f;
        if (IsKeyPressed(KEY_E)) { player.inventoryOpen = !player.inventoryOpen; if (player.inventoryOpen) EnableCursor(); else DisableCursor(); }

        if (!player.inventoryOpen) {
            Vector2 md = GetMouseDelta();
            cameraAngle.x -= md.x * 0.003f;
            cameraAngle.y -= md.y * 0.003f;
            if (cameraAngle.y > 1.56f) cameraAngle.y = 1.56f; if (cameraAngle.y < -1.56f) cameraAngle.y = -1.56f;
        }

        for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
            if (IsKeyPressed(KEY_ONE+i)) {
                player.selectedSlot = i;
                player.selectedBlock = hotbar[i];
            }
        }

        camera.position = (Vector3){ player.position.x, player.position.y + 1.6f, player.position.z };
        Vector3 forwardDir = { cosf(cameraAngle.y)*sinf(cameraAngle.x), sinf(cameraAngle.y), cosf(cameraAngle.y)*cosf(cameraAngle.x) };
        camera.target = Vector3Add(camera.position, forwardDir);

        // MATHEMATICALLY DERIVED VECTORS
        Vector3 forward = { sinf(cameraAngle.x), 0, cosf(cameraAngle.x) };
        Vector3 right = Vector3CrossProduct(forward, (Vector3){0, 1, 0}); // True Right vector

        if (!player.inventoryOpen) {
            bool inWater = World_GetBlock(world, (int)floor(player.position.x), (int)floor(player.position.y + 0.5f), (int)floor(player.position.z)) == BLOCK_WATER;

            Vector3 moveDir = { 0 };
            if (IsKeyDown(KEY_W)) moveDir = Vector3Add(moveDir, forward); 
            if (IsKeyDown(KEY_S)) moveDir = Vector3Subtract(moveDir, forward);
            if (IsKeyDown(KEY_A)) moveDir = Vector3Subtract(moveDir, right); // Standard Left
            if (IsKeyDown(KEY_D)) moveDir = Vector3Add(moveDir, right);      // Standard Right

            float speed = IsKeyDown(KEY_LEFT_CONTROL) ? 8.5f : 4.5f; if (inWater) speed *= 0.75f;
            if (Vector3Length(moveDir) > 0.1f) moveDir = Vector3Scale(Vector3Normalize(moveDir), speed);
            player.velocity.x = moveDir.x; player.velocity.z = moveDir.z;
            float gravity = inWater ? 14.0f : 28.0f; player.velocity.y -= gravity * dt;

            if (IsKeyPressed(KEY_SPACE)) {
                if (player.grounded) { player.velocity.y = 9.0f; player.grounded = false; }
                else if (inWater) { player.velocity.y = 8.5f; } // Purely velocity based leap
            } else if (inWater && IsKeyDown(KEY_SPACE)) {
                player.velocity.y += 35.0f * dt; if (player.velocity.y > 4.5f) player.velocity.y = 4.5f;
            }

            // Continuous Physics Resolution
            player.position.y += player.velocity.y * dt;
            if (CheckCollision(world, player.position)) {
                if (player.velocity.y < 0) player.grounded = true;
                player.position.y -= player.velocity.y * dt; player.velocity.y = 0;
            } else if (player.velocity.y != 0) player.grounded = false;

            player.position.x += player.velocity.x * dt;
            if (CheckCollision(world, player.position)) player.position.x -= player.velocity.x * dt;

            player.position.z += player.velocity.z * dt;
            if (CheckCollision(world, player.position)) player.position.z -= player.velocity.z * dt;

            if (CheckCollision(world, player.position)) player.position.y += 0.1f; // Gentle anti-stuck

            if (IsMouseButtonPressed(MOUSE_LEFT_BUTTON)) {
                Ray ray = { camera.position, Vector3Normalize(Vector3Subtract(camera.target, camera.position)) };
                RaycastResult res = World_Raycast(world, ray.position, ray.direction, 5.0f);
                if (res.hit) {
                    int harvested = GetInventoryIndex(res.block);
                    if (harvested >= 0) {
                        player.blockCounts[harvested]++;
                        TryAutoAssignHotbar(hotbar, res.block);
                    }
                    World_SetBlock(world, res.x, res.y, res.z, BLOCK_AIR);
                }
            }
            int selectedIndex = GetInventoryIndex(player.selectedBlock);
            if (IsMouseButtonPressed(MOUSE_RIGHT_BUTTON) && selectedIndex >= 0 && player.blockCounts[selectedIndex] > 0) {
                Ray ray = { camera.position, Vector3Normalize(Vector3Subtract(camera.target, camera.position)) };
                RaycastResult res = World_Raycast(world, ray.position, ray.direction, 5.0f);
                if (res.hit) {
                    int px = res.x+res.nx, py = res.y+res.ny, pz = res.z+res.nz;
                    if (!CheckCollision(world, (Vector3){(float)px+0.5f, (float)py, (float)pz+0.5f})) {
                        BlockType existing = World_GetBlock(world, px, py, pz);
                        if (existing == BLOCK_AIR || existing == BLOCK_WATER) {
                            World_SetBlock(world, px, py, pz, player.selectedBlock);
                            if (World_GetBlock(world, px, py, pz) == player.selectedBlock) player.blockCounts[selectedIndex]--;
                        }
                    }
                }
            }
        }
        CleanupHotbarSlots(&player, hotbar);

        World_Update(world, player.position);
        BeginDrawing();
            ClearBackground(SKYBLUE);
            BeginMode3D(camera);
                World_Render(world);
                World_RenderClouds(world, player.position, (float)GetTime());
                if (!player.inventoryOpen) {
                    Ray ray = { camera.position, Vector3Normalize(Vector3Subtract(camera.target, camera.position)) };
                    RaycastResult res = World_Raycast(world, ray.position, ray.direction, 5.0f);
                    if (res.hit) DrawCubeWires((Vector3){(float)res.x+0.5f, (float)res.y+0.5f, (float)res.z+0.5f}, 1.01f, 1.01f, 1.01f, BLACK);
                }
            EndMode3D();

            if (World_GetBlock(world, (int)floor(camera.position.x), (int)floor(camera.position.y), (int)floor(camera.position.z)) == BLOCK_WATER) 
                DrawRectangle(0, 0, 1280, 720, (Color){ 0, 121, 241, 150 });
            DrawRectangleGradientV(0, 0, 1280, 100, Fade(BLACK, 0.3f), BLANK);
            DrawRectangleGradientV(0, 620, 1280, 100, BLANK, Fade(BLACK, 0.3f));

            int hbX = (1280 - 620) / 2;
            for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
                Rectangle rect = { (float)hbX + i * 70, 640, 60, 60 };
                DrawRectangleRec(rect, player.selectedSlot == i ? Fade(WHITE, 0.6f) : Fade(BLACK, 0.4f));
                DrawRectangleLinesEx(rect, 2, player.selectedSlot == i ? YELLOW : WHITE);
                if (hotbar[i] != BLOCK_AIR) {
                    int idx = GetInventoryIndex(hotbar[i]);
                    int count = (idx >= 0) ? player.blockCounts[idx] : 0;
                    DrawRectangle(rect.x + 10, rect.y + 10, 40, 40, GetBlockUIColour(hotbar[i]));
                    DrawText(TextFormat("%d", count), (int)rect.x + 4, (int)rect.y + 38, 18, (count > 0) ? WHITE : LIGHTGRAY);
                }
            }

            if (player.inventoryOpen) {
                DrawRectangle(0, 0, 1280, 720, Fade(BLACK, 0.6f));
                DrawText("Inventory", 540, 140, 30, WHITE);
                DrawText("Collect blocks by mining. Click item to assign selected hotbar slot.", 360, 176, 20, LIGHTGRAY);
                for (int i = 0; i < INVENTORY_BLOCK_COUNT; i++) {
                    Rectangle r = { (float)1280/2 - 200 + (i%5)*80, 200 + (i/5)*80, 70, 70 };
                    DrawRectangleRec(r, Fade(WHITE, 0.2f));
                    DrawRectangle(r.x + 15, r.y + 12, 40, 40, GetBlockUIColour(inventoryBlocks[i]));
                    DrawText(TextFormat("%d", player.blockCounts[i]), (int)r.x + 6, (int)r.y + 48, 18, WHITE);
                    if (CheckCollisionPointRec(GetMousePosition(), r)) {
                        DrawRectangleLinesEx(r, 2, YELLOW);
                        if (IsMouseButtonPressed(MOUSE_LEFT_BUTTON) && player.blockCounts[i] > 0) {
                            hotbar[player.selectedSlot] = inventoryBlocks[i];
                            player.selectedBlock = hotbar[player.selectedSlot];
                        }
                    } else DrawRectangleLinesEx(r, 2, Fade(WHITE, 0.6f));
                }
            }
            if (!player.inventoryOpen) DrawCircle(640, 360, 2, WHITE);
            DrawText(TextFormat("FPS: %d", GetFPS()), 20, 20, 20, GREEN);
        EndDrawing();
    }
    if (!EnsureSaveDir()) TraceLog(LOG_WARNING, "Could not ensure save directory '%s'", SAVE_DIR);
    if (!World_SaveEdits(world, WORLD_SAVE_PATH)) TraceLog(LOG_WARNING, "Could not save world edits to '%s'", WORLD_SAVE_PATH);
    if (!SavePlayerState(&player, hotbar, PLAYER_SAVE_PATH)) TraceLog(LOG_WARNING, "Could not save player state to '%s'", PLAYER_SAVE_PATH);
    World_Unload(world); free(world); CloseWindow(); return 0;
}
