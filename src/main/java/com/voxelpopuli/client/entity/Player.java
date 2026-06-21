package com.voxelpopuli.client.entity;

import com.voxelpopuli.client.block.BlockType;
import com.voxelpopuli.client.item.ItemStack;
import com.voxelpopuli.client.item.ToolProperties;
import com.voxelpopuli.client.item.Crafting;
import com.voxelpopuli.client.world.World;
import com.voxelpopuli.client.world.Chunk;
import org.joml.Vector3f;

import java.util.Arrays;

public class Player {
    public final Vector3f position = new Vector3f(0.0f, 130.0f, 0.0f);
    public final Vector3f velocity = new Vector3f(0.0f, 0.0f, 0.0f);
    public boolean grounded = false;
    public float airSeconds = 15.0f;
    public boolean inventoryOpen = false;
    public int selectedSlot = 0; // hotbar index 0 to 8
    public int health = 20; // 10 hearts
    public boolean flying = false;
    public double lastSpaceRelease = -999.0;
    public boolean spaceWasPressed = false;

    // Slots layout:
    // 0..=8 (9 slots): Hotbar
    // 9..=35 (27 slots): Main inventory
    // 36..=39 (4 slots): Armor slots (visual only)
    // 40..=43 (4 slots): 2x2 crafting input
    // 44 (1 slot): 2x2 crafting output
    public final ItemStack[] invSlots = new ItemStack[45];
    public ItemStack cursor = null; // item currently on mouse cursor

    // Crafting table slots:
    // 0..=8 (9 slots): 3x3 crafting input
    // 9 (1 slot): 3x3 crafting output
    public final ItemStack[] craftTableSlots = new ItemStack[10];

    public Player() {
        // Initialize inventory to starting items if desired, or keep empty
    }

    public static int stackMax(BlockType b) {
        return b.getMaxStackSize();
    }

    public void invAdd(BlockType block, int amt) {
        int sm = stackMax(block);
        for (int pass = 0; pass < 2; pass++) {
            // Check hotbar first (0-8), then main inventory (9-35)
            for (int i = 0; i < 36; i++) {
                int slotIndex = (i < 9) ? i : i; // 0..8 then 9..35
                if (amt <= 0) return;

                ItemStack s = invSlots[slotIndex];
                if (pass == 0) {
                    if (s != null && s.block == block && s.count < sm) {
                        int add = Math.min(sm - s.count, amt);
                        invSlots[slotIndex] = new ItemStack(block, s.count + add, s.durability);
                        amt -= add;
                    }
                } else {
                    if (s == null) {
                        int add = Math.min(sm, amt);
                        invSlots[slotIndex] = new ItemStack(block, add);
                        amt -= add;
                    }
                }
            }
        }
    }

    public void invAddTool(ItemStack tool) {
        for (int i = 0; i < 36; i++) {
            if (invSlots[i] == null) {
                invSlots[i] = tool.clone();
                return;
            }
        }
    }

    public void invClick(int slot, boolean right, boolean shift) {
        if (slot == 44) {
            // Crafting output: pick up only (no placing)
            if (!right && cursor == null && invSlots[44] != null) {
                cursor = invSlots[44].clone();
                invSlots[44] = null;
                // Consume 1 item from each ingredient
                for (int i = 40; i < 44; i++) {
                    if (invSlots[i] != null) {
                        invSlots[i].count -= 1;
                        if (invSlots[i].count == 0) {
                            invSlots[i] = null;
                        }
                    }
                }
                updateCraftOutput2x2();
            }
            return;
        }

        // Armor slots swap
        if (slot >= 36 && slot < 40) {
            if (!shift) {
                ItemStack temp = invSlots[slot];
                invSlots[slot] = cursor;
                cursor = temp;
            }
            return;
        }

        if (shift) {
            ItemStack s = invSlots[slot];
            if (s != null) {
                invSlots[slot] = null;
                int sm = stackMax(s.block);
                int a = (slot < 9) ? 9 : 0;
                int b = (slot < 9) ? 36 : 9;

                int rem = s.count;
                for (int i = a; i < b; i++) {
                    if (rem == 0) break;
                    ItemStack d = invSlots[i];
                    if (d != null && d.block == s.block && d.count < sm) {
                        int add = Math.min(sm - d.count, rem);
                        invSlots[i] = new ItemStack(s.block, d.count + add, d.durability);
                        rem -= add;
                    }
                }

                for (int i = a; i < b; i++) {
                    if (rem == 0) break;
                    if (invSlots[i] == null) {
                        int n = Math.min(sm, rem);
                        invSlots[i] = new ItemStack(s.block, n, s.durability);
                        rem -= n;
                    }
                }

                if (rem > 0) {
                    invSlots[slot] = new ItemStack(s.block, rem, s.durability);
                }
            }

            if (slot >= 40 && slot < 44) {
                updateCraftOutput2x2();
            }
            return;
        }

        if (right) {
            if (cursor == null) {
                ItemStack s = invSlots[slot];
                if (s != null) {
                    int half = (s.count + 1) / 2;
                    cursor = new ItemStack(s.block, half, s.durability);
                    int left = s.count - half;
                    invSlots[slot] = (left > 0) ? new ItemStack(s.block, left, s.durability) : null;
                }
            } else {
                int sm = stackMax(cursor.block);
                ItemStack d = invSlots[slot];
                boolean ok = (d == null || (d.block == cursor.block && d.count < sm));
                if (ok) {
                    if (d == null) {
                        invSlots[slot] = new ItemStack(cursor.block, 1, cursor.durability);
                    } else {
                        invSlots[slot] = new ItemStack(d.block, d.count + 1, d.durability);
                    }
                    cursor.count -= 1;
                    if (cursor.count == 0) {
                        cursor = null;
                    }
                }
            }
        } else {
            // Left click swap / merge
            ItemStack h = cursor;
            ItemStack d = invSlots[slot];

            if (h == null) {
                cursor = invSlots[slot];
                invSlots[slot] = null;
            } else if (d == null) {
                invSlots[slot] = h;
                cursor = null;
            } else if (h.block == d.block) {
                int sm = stackMax(d.block);
                int add = Math.min(sm - d.count, h.count);
                invSlots[slot] = new ItemStack(d.block, d.count + add, d.durability);
                h.count -= add;
                cursor = (h.count > 0) ? h : null;
            } else {
                ItemStack temp = invSlots[slot];
                invSlots[slot] = cursor;
                cursor = temp;
            }
        }

        if (slot >= 40 && slot < 44) {
            updateCraftOutput2x2();
        }
    }

    public void updateCraftOutput2x2() {
        ItemStack[] grid = new ItemStack[4];
        System.arraycopy(invSlots, 40, grid, 0, 4);
        Crafting.Recipe r = Crafting.findRecipe(grid, 2, 2);
        if (r != null) {
            invSlots[44] = new ItemStack(r.output, r.outputCount);
        } else {
            invSlots[44] = null;
        }
    }

    public void updateCraftOutput3x3() {
        ItemStack[] grid = new ItemStack[9];
        System.arraycopy(craftTableSlots, 0, grid, 0, 9);
        Crafting.Recipe r = Crafting.findRecipe(grid, 3, 3);
        if (r != null) {
            craftTableSlots[9] = new ItemStack(r.output, r.outputCount);
        } else {
            craftTableSlots[9] = null;
        }
    }

    public void ctClick(int ctSlot, boolean right) {
        if (ctSlot == 9) {
            // Output slot: pick up only
            if (!right && cursor == null && craftTableSlots[9] != null) {
                cursor = craftTableSlots[9].clone();
                craftTableSlots[9] = null;
                for (int i = 0; i < 9; i++) {
                    if (craftTableSlots[i] != null) {
                        craftTableSlots[i].count -= 1;
                        if (craftTableSlots[i].count == 0) {
                            craftTableSlots[i] = null;
                        }
                    }
                }
                updateCraftOutput3x3();
            }
            return;
        }

        // Map ctSlot to actual slot reference in inventory/grid arrays
        int arraySelector; // 0 = craftTableSlots, 1 = invSlots
        int arrayIndex;

        if (ctSlot <= 8) {
            arraySelector = 0;
            arrayIndex = ctSlot;
        } else if (ctSlot >= 100 && ctSlot <= 126) {
            arraySelector = 1;
            arrayIndex = ctSlot - 100 + 9;
        } else if (ctSlot >= 127 && ctSlot <= 135) {
            arraySelector = 1;
            arrayIndex = ctSlot - 127;
        } else {
            return;
        }

        ItemStack[] activeArray = (arraySelector == 0) ? craftTableSlots : invSlots;

        if (right) {
            if (cursor == null) {
                ItemStack s = activeArray[arrayIndex];
                if (s != null) {
                    int half = (s.count + 1) / 2;
                    cursor = new ItemStack(s.block, half, s.durability);
                    int left = s.count - half;
                    activeArray[arrayIndex] = (left > 0) ? new ItemStack(s.block, left, s.durability) : null;
                }
            } else {
                int sm = stackMax(cursor.block);
                ItemStack d = activeArray[arrayIndex];
                boolean ok = (d == null || (d.block == cursor.block && d.count < sm));
                if (ok) {
                    if (d == null) {
                        activeArray[arrayIndex] = new ItemStack(cursor.block, 1, cursor.durability);
                    } else {
                        activeArray[arrayIndex] = new ItemStack(d.block, d.count + 1, d.durability);
                    }
                    cursor.count -= 1;
                    if (cursor.count == 0) {
                        cursor = null;
                    }
                }
            }
        } else {
            ItemStack h = cursor;
            ItemStack d = activeArray[arrayIndex];

            if (h == null) {
                cursor = activeArray[arrayIndex];
                activeArray[arrayIndex] = null;
            } else if (d == null) {
                activeArray[arrayIndex] = h;
                cursor = null;
            } else if (h.block == d.block) {
                int sm = stackMax(d.block);
                int add = Math.min(sm - d.count, h.count);
                activeArray[arrayIndex] = new ItemStack(d.block, d.count + add, d.durability);
                h.count -= add;
                cursor = (h.count > 0) ? h : null;
            } else {
                ItemStack temp = activeArray[arrayIndex];
                activeArray[arrayIndex] = cursor;
                cursor = temp;
            }
        }

        if (ctSlot <= 8) {
            updateCraftOutput3x3();
        }
    }

    public void ctClose() {
        for (int i = 0; i < 9; i++) {
            ItemStack s = craftTableSlots[i];
            if (s != null) {
                craftTableSlots[i] = null;
                invAdd(s.block, s.count);
            }
        }
        craftTableSlots[9] = null;
    }

    public static boolean isPointInBlock(World world, Vector3f p) {
        BlockType b = world.getBlock((int) Math.floor(p.x), (int) Math.floor(p.y), (int) Math.floor(p.z));
        return b != BlockType.AIR && b != BlockType.WATER;
    }

    public static boolean checkCollision(World world, Vector3f pos) {
        float w = 0.22f;
        float h = 1.75f;
        for (float xOff : new float[]{-w, w}) {
            for (float zOff : new float[]{-w, w}) {
                for (int yi = 0; yi <= 2; yi++) {
                    float y = 0.1f + yi * ((h - 0.1f) / 2.0f);
                    if (isPointInBlock(world, new Vector3f(pos.x + xOff, pos.y + y, pos.z + zOff))) {
                        return true;
                    }
                }
            }
        }
        return false;
    }

    public boolean intersectsBlock(int bx, int by, int bz) {
        float w = 0.3f;
        Vector3f playerMin = new Vector3f(position.x - w, position.y, position.z - w);
        Vector3f playerMax = new Vector3f(position.x + w, position.y + 1.8f, position.z + w);
        Vector3f blockMin = new Vector3f(bx, by, bz);
        Vector3f blockMax = new Vector3f(bx + 1, by + 1, bz + 1);

        return playerMax.x > blockMin.x
            && playerMin.x < blockMax.x
            && playerMax.y > blockMin.y
            && playerMin.y < blockMax.y
            && playerMax.z > blockMin.z
            && playerMin.z < blockMax.z;
    }

    public void update(World world, Vector3f moveInput, float dt, boolean isSprinting, boolean isJumping, boolean isSneaking, double currentTime) {
        if (inventoryOpen) {
            return;
        }

        // Double-tap space for flight toggle
        boolean spaceJustPressed = isJumping && !spaceWasPressed;
        if (spaceJustPressed && !grounded) {
            double timeSinceLast = currentTime - lastSpaceRelease;
            if (timeSinceLast < 0.35) {
                flying = !flying;
                velocity.y = 0.0f;
            }
        }
        if (!isJumping && spaceWasPressed) {
            lastSpaceRelease = currentTime;
        }
        spaceWasPressed = isJumping;

        if (grounded && flying) {
            flying = false;
        }

        boolean waistInW = world.getBlock((int) Math.floor(position.x), (int) Math.floor(position.y + 0.9f), (int) Math.floor(position.z)) == BlockType.WATER;
        boolean feetInW = world.getBlock((int) Math.floor(position.x), (int) Math.floor(position.y + 0.1f), (int) Math.floor(position.z)) == BlockType.WATER;
        boolean headInW = world.getBlock((int) Math.floor(position.x), (int) Math.floor(position.y + 1.6f), (int) Math.floor(position.z)) == BlockType.WATER;
        boolean inWater = waistInW || feetInW;

        Vector3f mv = new Vector3f(moveInput);
        if (mv.lengthSquared() > 0.1f) {
            mv.normalize();
            float speed;
            if (flying) {
                speed = 10.92f;
            } else if (inWater) {
                speed = 2.0f;
            } else if (isSprinting) {
                speed = 5.612f;
            } else {
                speed = 4.317f;
            }
            mv.mul(speed);
        }

        if (flying) {
            float flyDrag = (float) Math.pow(0.09f, dt);
            velocity.x = velocity.x * (1.0f - flyDrag) + mv.x * flyDrag;
            velocity.z = velocity.z * (1.0f - flyDrag) + mv.z * flyDrag;

            float flySpeed = 7.8f;
            if (isJumping) {
                velocity.y = flySpeed;
            } else if (isSneaking) {
                velocity.y = -flySpeed;
            } else {
                velocity.y *= (float) Math.pow(0.6f, dt * 20.0f);
            }
        } else {
            float friction = inWater ? 0.8f : (grounded ? 0.6f : 0.98f);
            velocity.x = velocity.x * (1.0f - friction * dt * 20.0f) + mv.x * friction * dt * 20.0f;
            velocity.z = velocity.z * (1.0f - friction * dt * 20.0f) + mv.z * friction * dt * 20.0f;

            if (inWater) {
                if (isJumping) {
                    velocity.y += 0.04f * 20.0f;
                    if (velocity.y > 2.0f) velocity.y = 2.0f;
                } else {
                    velocity.y -= 0.02f * 20.0f;
                    if (velocity.y < -2.0f) velocity.y = -2.0f;
                }
            } else {
                velocity.y -= 32.0f * dt; // gravity
                float fallDrag = (float) Math.pow(0.98f, dt * 20.0f);
                velocity.y *= fallDrag;
                if (isJumping && grounded) {
                    velocity.y = 8.4f;
                    grounded = false;
                }
            }
        }

        // Revert movement axis-by-axis on block collisions
        float dy = velocity.y * dt;
        position.y += dy;
        if (checkCollision(world, position)) {
            if (velocity.y <= 0.0f) {
                grounded = true;
            }
            position.y -= dy;
            velocity.y = 0.0f;
        } else if (velocity.y != 0.0f) {
            grounded = false;
        }

        float dx = velocity.x * dt;
        position.x += dx;
        if (checkCollision(world, position)) {
            position.x -= dx;
        }

        float dz = velocity.z * dt;
        position.z += dz;
        if (checkCollision(world, position)) {
            position.z -= dz;
        }

        if (headInW) {
            airSeconds = Math.max(0.0f, airSeconds - dt);
        } else {
            airSeconds = Math.min(15.0f, airSeconds + dt * 3.0f);
        }
    }
}
