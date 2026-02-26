#include "raylib.h"
#include "raymath.h"
#include "world.h"
#include "noise.h"
#include <stdlib.h>
#include <math.h>

typedef struct {
    Vector3 position; // Feet position
    Vector3 velocity;
    bool grounded;
    BlockType selectedBlock;
    int selectedSlot;
    bool inventoryOpen;
} Player;

BlockType inventoryBlocks[] = { 
    BLOCK_GRASS, BLOCK_DIRT, BLOCK_STONE, BLOCK_OAK_LOG, 
    BLOCK_OAK_LEAVES, BLOCK_SAND, BLOCK_WATER, BLOCK_GRAVEL, BLOCK_BEDROCK 
};

bool IsPointInBlock(World *world, Vector3 p) {
    return World_GetBlock(world, (int)floor(p.x), (int)floor(p.y), (int)floor(p.z)) != BLOCK_AIR;
}

bool CheckCollision(World *world, Vector3 pos) {
    float w = 0.25f; 
    float h = 1.75f;
    for (float x = -w; x <= w; x += w * 2) {
        for (float z = -w; z <= w; z += w * 2) {
            for (float y = 0.1f; y <= h; y += h / 2.0f) {
                if (IsPointInBlock(world, (Vector3){ pos.x + x, pos.y + y, pos.z + z })) return true;
            }
        }
    }
    return false;
}

int main(void) {
    const int screenWidth = 1280;
    const int screenHeight = 720;
    InitWindow(screenWidth, screenHeight, "VoxelPopuli - Minecraft 1.0 Clone");
    InitNoise();
    World *world = (World *)malloc(sizeof(World));
    World_Init(world);

    Player player = { 0 };
    player.position = (Vector3){ 32.5f, 100.0f, 32.5f };
    player.selectedSlot = 0;
    player.selectedBlock = BLOCK_GRASS;
    player.inventoryOpen = false;

    for (int i=0; i<100; i++) World_Update(world, player.position);
    for (int y = CHUNK_HEIGHT - 1; y >= 0; y--) {
        if (World_GetBlock(world, (int)player.position.x, y, (int)player.position.z) != BLOCK_AIR) {
            player.position.y = (float)y + 1.1f;
            break;
        }
    }

    Camera3D camera = { 0 };
    camera.up = (Vector3){ 0.0f, 1.0f, 0.0f };
    camera.fovy = 75.0f;
    camera.projection = CAMERA_PERSPECTIVE;
    Vector2 cameraAngle = { PI, 0 };

    BlockType hotbar[9];
    for(int i=0; i<9; i++) hotbar[i] = (i < 5) ? inventoryBlocks[i] : BLOCK_AIR;
    player.selectedBlock = hotbar[0];

    DisableCursor();
    SetTargetFPS(60);

    while (!WindowShouldClose()) {
        float dt = GetFrameTime();
        if (dt > 0.1f) dt = 0.1f;

        if (IsKeyPressed(KEY_E)) {
            player.inventoryOpen = !player.inventoryOpen;
            if (player.inventoryOpen) EnableCursor();
            else DisableCursor();
        }

        Vector3 forward = { sinf(cameraAngle.x), 0, cosf(cameraAngle.x) };
        Vector3 right = { cosf(cameraAngle.x), 0, -sinf(cameraAngle.x) };

        if (!player.inventoryOpen) {
            for (int i = 0; i < 9; i++) {
                if (IsKeyPressed(KEY_ONE + i)) {
                    player.selectedSlot = i;
                    player.selectedBlock = hotbar[i];
                }
            }

            Vector2 mouseDelta = GetMouseDelta();
            cameraAngle.x -= mouseDelta.x * 0.003f; // Changed back to -= to fix user reported flip
            cameraAngle.y -= mouseDelta.y * 0.003f;
            if (cameraAngle.y > PI/2 - 0.01f) cameraAngle.y = PI/2 - 0.01f;
            if (cameraAngle.y < -PI/2 + 0.01f) cameraAngle.y = -PI/2 + 0.01f;

            bool inWater = World_GetBlock(world, (int)floor(player.position.x), (int)floor(player.position.y + 0.5f), (int)floor(player.position.z)) == BLOCK_WATER;

            Vector3 moveDir = { 0 };
            if (IsKeyDown(KEY_W)) moveDir = Vector3Add(moveDir, forward);
            if (IsKeyDown(KEY_S)) moveDir = Vector3Subtract(moveDir, forward);
            if (IsKeyDown(KEY_A)) moveDir = Vector3Add(moveDir, right);
            if (IsKeyDown(KEY_D)) moveDir = Vector3Subtract(moveDir, right);

            float speed = IsKeyDown(KEY_LEFT_CONTROL) ? 8.5f : 4.5f;
            if (inWater) speed *= 0.6f;
            if (Vector3Length(moveDir) > 0.1f) moveDir = Vector3Scale(Vector3Normalize(moveDir), speed);

            player.velocity.x = moveDir.x;
            player.velocity.z = moveDir.z;
            float gravity = inWater ? 8.0f : 28.0f;
            player.velocity.y -= gravity * dt;

            if (player.grounded && IsKeyPressed(KEY_SPACE)) {
                player.velocity.y = inWater ? 4.0f : 9.0f;
                player.grounded = false;
            } else if (inWater && IsKeyDown(KEY_SPACE)) {
                player.velocity.y = 3.0f;
            }

            player.position.y += player.velocity.y * dt;
            if (CheckCollision(world, player.position)) {
                if (player.velocity.y < 0) player.grounded = true;
                player.position.y -= player.velocity.y * dt;
                player.velocity.y = 0;
            } else if (player.velocity.y != 0) player.grounded = false;

            player.position.x += player.velocity.x * dt;
            if (CheckCollision(world, player.position)) player.position.x -= player.velocity.x * dt;

            player.position.z += player.velocity.z * dt;
            if (CheckCollision(world, player.position)) player.position.z -= player.velocity.z * dt;

            if (IsMouseButtonPressed(MOUSE_LEFT_BUTTON)) {
                Ray ray = { camera.position, Vector3Normalize(Vector3Subtract(camera.target, camera.position)) };
                RaycastResult res = World_Raycast(world, ray.position, ray.direction, 5.0f);
                if (res.hit) World_SetBlock(world, res.x, res.y, res.z, BLOCK_AIR);
            }
            if (IsMouseButtonPressed(MOUSE_RIGHT_BUTTON) && player.selectedBlock != BLOCK_AIR) {
                Ray ray = { camera.position, Vector3Normalize(Vector3Subtract(camera.target, camera.position)) };
                RaycastResult res = World_Raycast(world, ray.position, ray.direction, 5.0f);
                if (res.hit) {
                    int px = res.x + res.nx, py = res.y + res.ny, pz = res.z + res.nz;
                    if (!( (int)floor(player.position.x) == px && (int)floor(player.position.y) == py && (int)floor(player.position.z) == pz ) &&
                        !( (int)floor(player.position.x) == px && (int)floor(player.position.y+1) == py && (int)floor(player.position.z) == pz )) {
                        World_SetBlock(world, px, py, pz, player.selectedBlock);
                    }
                }
            }
        }

        World_Update(world, player.position);
        bool headInWater = World_GetBlock(world, (int)floor(camera.position.x), (int)floor(camera.position.y), (int)floor(camera.position.z)) == BLOCK_WATER;

        camera.position = (Vector3){ player.position.x, player.position.y + 1.6f, player.position.z };
        camera.target = Vector3Add(camera.position, (Vector3){
            cosf(cameraAngle.y) * sinf(cameraAngle.x),
            sinf(cameraAngle.y),
            cosf(cameraAngle.y) * cosf(cameraAngle.x)
        });

        BeginDrawing();
            ClearBackground(SKYBLUE);
            BeginMode3D(camera);
                World_Render(world);
                if (!player.inventoryOpen) {
                    Ray ray = { camera.position, Vector3Normalize(Vector3Subtract(camera.target, camera.position)) };
                    RaycastResult res = World_Raycast(world, ray.position, ray.direction, 5.0f);
                    if (res.hit) DrawCubeWires((Vector3){(float)res.x + 0.5f, (float)res.y + 0.5f, (float)res.z + 0.5f}, 1.01f, 1.01f, 1.01f, BLACK);
                }
            EndMode3D();

            if (headInWater) DrawRectangle(0, 0, screenWidth, screenHeight, (Color){ 0, 121, 241, 150 });
            DrawRectangleGradientV(0, 0, screenWidth, 100, Fade(BLACK, 0.3f), BLANK);
            DrawRectangleGradientV(0, screenHeight - 100, screenWidth, 100, BLANK, Fade(BLACK, 0.3f));

            int hbSize = 60, hbPadding = 10;
            int hbTotalWidth = (hbSize + hbPadding) * 9 - hbPadding;
            int hbX = (screenWidth - hbTotalWidth) / 2;
            int hbY = screenHeight - hbSize - 20;

            for (int i = 0; i < 9; i++) {
                Rectangle rect = { (float)hbX + i * (hbSize + hbPadding), (float)hbY, (float)hbSize, (float)hbSize };
                DrawRectangleRec(rect, player.selectedSlot == i ? Fade(WHITE, 0.6f) : Fade(BLACK, 0.4f));
                DrawRectangleLinesEx(rect, 2, player.selectedSlot == i ? YELLOW : WHITE);
                if (hotbar[i] != BLOCK_AIR) {
                    Color c = GREEN;
                    if (hotbar[i] == BLOCK_DIRT) c = BROWN;
                    else if (hotbar[i] == BLOCK_STONE) c = GRAY;
                    else if (hotbar[i] == BLOCK_OAK_LOG) c = DARKBROWN;
                    else if (hotbar[i] == BLOCK_OAK_LEAVES) c = DARKGREEN;
                    else if (hotbar[i] == BLOCK_SAND) c = BEIGE;
                    else if (hotbar[i] == BLOCK_WATER) c = BLUE;
                    else if (hotbar[i] == BLOCK_GRAVEL) c = LIGHTGRAY;
                    else if (hotbar[i] == BLOCK_BEDROCK) c = DARKGRAY;
                    DrawRectangle(rect.x + 10, rect.y + 10, rect.width - 20, rect.height - 20, c);
                }
                DrawText(TextFormat("%d", i+1), rect.x + 5, rect.y + 5, 10, WHITE);
            }

            if (player.inventoryOpen) {
                DrawRectangle(0, 0, screenWidth, screenHeight, Fade(BLACK, 0.6f));
                DrawText("INVENTORY", screenWidth/2 - 60, 100, 20, WHITE);
                int invX = screenWidth / 2 - 200;
                int invY = 200;
                for (int i = 0; i < 9; i++) {
                    Rectangle rect = { (float)invX + (i%5) * 80, (float)invY + (i/5) * 80, 70, 70 };
                    DrawRectangleRec(rect, Fade(WHITE, 0.2f));
                    Color c = GREEN;
                    if (inventoryBlocks[i] == BLOCK_DIRT) c = BROWN;
                    else if (inventoryBlocks[i] == BLOCK_STONE) c = GRAY;
                    else if (inventoryBlocks[i] == BLOCK_OAK_LOG) c = DARKBROWN;
                    else if (inventoryBlocks[i] == BLOCK_OAK_LEAVES) c = DARKGREEN;
                    else if (inventoryBlocks[i] == BLOCK_SAND) c = BEIGE;
                    else if (inventoryBlocks[i] == BLOCK_WATER) c = BLUE;
                    else if (inventoryBlocks[i] == BLOCK_GRAVEL) c = LIGHTGRAY;
                    else if (inventoryBlocks[i] == BLOCK_BEDROCK) c = DARKGRAY;
                    DrawRectangle(rect.x + 10, rect.y + 10, rect.width - 20, rect.height - 20, c);
                    if (CheckCollisionPointRec(GetMousePosition(), rect)) {
                        DrawRectangleLinesEx(rect, 2, YELLOW);
                        if (IsMouseButtonPressed(MOUSE_LEFT_BUTTON)) {
                            hotbar[player.selectedSlot] = inventoryBlocks[i];
                            player.selectedBlock = hotbar[player.selectedSlot];
                            player.inventoryOpen = false;
                            DisableCursor();
                        }
                    }
                }
            }
            if (!player.inventoryOpen) DrawCircle(screenWidth/2, screenHeight/2, 2, WHITE);
            DrawText(TextFormat("FPS: %d", GetFPS()), 20, 20, 20, GREEN);
        EndDrawing();
    }
    World_Unload(world);
    free(world);
    CloseWindow();
    return 0;
}
