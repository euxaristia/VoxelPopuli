#include "raylib.h"
#include "raymath.h"
#include "world.h"
#include "noise.h"
#include <stdlib.h>
#include <math.h>

typedef struct { Vector3 position, velocity; bool grounded; BlockType selectedBlock; int selectedSlot; bool inventoryOpen; } Player;

BlockType inventoryBlocks[] = { BLOCK_GRASS, BLOCK_DIRT, BLOCK_STONE, BLOCK_OAK_LOG, BLOCK_OAK_LEAVES, BLOCK_SAND, BLOCK_WATER, BLOCK_GRAVEL, BLOCK_BEDROCK };

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
    Player player = { (Vector3){ 32.5f, 100.0f, 32.5f }, (Vector3){0}, false, BLOCK_GRASS, 0, false };
    for (int i=0; i<100; i++) World_Update(world, player.position);
    for (int y = CHUNK_HEIGHT-1; y>=0; y--) if(World_GetBlock(world, 32, y, 32) != BLOCK_AIR) { player.position.y = (float)y+1.1f; break; }

    Camera3D camera = { (Vector3){0}, (Vector3){0}, (Vector3){0,1,0}, 75.0f, 0 };
    Vector2 cameraAngle = { PI, 0 };
    BlockType hotbar[9]; for(int i=0; i<9; i++) hotbar[i] = (i < 5) ? inventoryBlocks[i] : BLOCK_AIR;

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

        camera.position = (Vector3){ player.position.x, player.position.y + 1.6f, player.position.z };
        Vector3 forwardDir = { cosf(cameraAngle.y)*sinf(cameraAngle.x), sinf(cameraAngle.y), cosf(cameraAngle.y)*cosf(cameraAngle.x) };
        camera.target = Vector3Add(camera.position, forwardDir);

        // MATHEMATICALLY DERIVED VECTORS
        Vector3 forward = { sinf(cameraAngle.x), 0, cosf(cameraAngle.x) };
        Vector3 right = Vector3CrossProduct(forward, (Vector3){0, 1, 0}); // True Right vector

        if (!player.inventoryOpen) {
            for (int i = 0; i < 9; i++) if (IsKeyPressed(KEY_ONE+i)) { player.selectedSlot = i; player.selectedBlock = hotbar[i]; }

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
                if (res.hit) World_SetBlock(world, res.x, res.y, res.z, BLOCK_AIR);
            }
            if (IsMouseButtonPressed(MOUSE_RIGHT_BUTTON) && player.selectedBlock != BLOCK_AIR) {
                Ray ray = { camera.position, Vector3Normalize(Vector3Subtract(camera.target, camera.position)) };
                RaycastResult res = World_Raycast(world, ray.position, ray.direction, 5.0f);
                if (res.hit) {
                    int px = res.x+res.nx, py = res.y+res.ny, pz = res.z+res.nz;
                    if (!CheckCollision(world, (Vector3){(float)px+0.5f, (float)py, (float)pz+0.5f})) World_SetBlock(world, px, py, pz, player.selectedBlock);
                }
            }
        }

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
            for (int i = 0; i < 9; i++) {
                Rectangle rect = { (float)hbX + i * 70, 640, 60, 60 };
                DrawRectangleRec(rect, player.selectedSlot == i ? Fade(WHITE, 0.6f) : Fade(BLACK, 0.4f));
                DrawRectangleLinesEx(rect, 2, player.selectedSlot == i ? YELLOW : WHITE);
                if (hotbar[i] != BLOCK_AIR) {
                    Color c = GREEN;
                    if (hotbar[i] == BLOCK_DIRT) c = BROWN; else if (hotbar[i] == BLOCK_STONE) c = GRAY;
                    else if (hotbar[i] == BLOCK_OAK_LOG) c = DARKBROWN; else if (hotbar[i] == BLOCK_OAK_LEAVES) c = DARKGREEN;
                    else if (hotbar[i] == BLOCK_SAND) c = BEIGE; else if (hotbar[i] == BLOCK_WATER) c = BLUE;
                    else if (hotbar[i] == BLOCK_GRAVEL) c = LIGHTGRAY; else if (hotbar[i] == BLOCK_BEDROCK) c = DARKGRAY;
                    DrawRectangle(rect.x + 10, rect.y + 10, 40, 40, c);
                }
            }

            if (player.inventoryOpen) {
                DrawRectangle(0, 0, 1280, 720, Fade(BLACK, 0.6f));
                for (int i = 0; i < 9; i++) {
                    Rectangle r = { (float)1280/2 - 200 + (i%5)*80, 200 + (i/5)*80, 70, 70 };
                    DrawRectangleRec(r, Fade(WHITE, 0.2f));
                    if (CheckCollisionPointRec(GetMousePosition(), r)) {
                        DrawRectangleLinesEx(r, 2, YELLOW);
                        if (IsMouseButtonPressed(MOUSE_LEFT_BUTTON)) { hotbar[player.selectedSlot] = inventoryBlocks[i]; player.selectedBlock = hotbar[player.selectedSlot]; player.inventoryOpen = false; DisableCursor(); }
                    }
                }
            }
            if (!player.inventoryOpen) DrawCircle(640, 360, 2, WHITE);
            DrawText(TextFormat("FPS: %d", GetFPS()), 20, 20, 20, GREEN);
        EndDrawing();
    }
    World_Unload(world); free(world); CloseWindow(); return 0;
}
