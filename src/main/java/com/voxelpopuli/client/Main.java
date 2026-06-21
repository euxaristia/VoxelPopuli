package com.voxelpopuli.client;

import com.voxelpopuli.client.block.BlockType;
import com.voxelpopuli.client.entity.Player;
import com.voxelpopuli.client.item.ItemStack;
import com.voxelpopuli.client.renderer.*;
import com.voxelpopuli.client.world.*;
import org.joml.Matrix4f;
import org.joml.Vector2f;
import org.joml.Vector3f;
import org.joml.Vector4f;
import org.lwjgl.BufferUtils;
import org.lwjgl.glfw.*;
import org.lwjgl.opengl.*;
import org.lwjgl.system.*;

import java.nio.DoubleBuffer;
import java.nio.IntBuffer;

public class Main {
    private static final int RENDER_WIDTH = 750;
    private static final int RENDER_HEIGHT = 422;

    private enum GameState { LOADING, PLAYING, PAUSED }

    private long window;
    private WindowState windowState;
    private GameState gameState = GameState.LOADING;

    private World world;
    private Player player;
    private Camera camera;
    private MiningState miningState;

    private Shader terrainShader;
    private Shader waterShader;
    private Shader flatShader;
    private Shader uiShader;
    private Shader textureShader;
    private Shader textureUiShader;
    private Shader colorShader;

    private RenderTexture2D renderTarget;
    private Texture2D fontTexture;

    private boolean isFullscreen = false;
    private final int[] winPos = new int[2];
    private final int[] winSize = new int[2];

    private final Vector2f cameraAngle = new Vector2f((float) Math.PI, 0.0f);
    private double lastCursorX = 0;
    private double lastCursorY = 0;

    private boolean leftMouseHeld = false;
    private boolean craftingTableOpen = false;

    // Gamepad state
    private float prevGpLt = -1.0f;
    private boolean prevGpDpadRight = false;
    private boolean prevGpDpadLeft = false;
    private boolean prevGpRb = false;
    private boolean prevGpLb = false;

    // FPS counter
    private int fpsFrames = 0;
    private double fpsLastTime = 0;

    private Mesh quadMesh;
    private GameLoop gameLoop;

    public static void main(String[] args) {
        new Main().run();
    }

    public void run() {
        windowState = WindowState.load();

        GLFWErrorCallback.createPrint(System.err).set();

        if (!GLFW.glfwInit()) {
            throw new IllegalStateException("Unable to initialize GLFW");
        }

        GLFW.glfwWindowHint(GLFW.GLFW_CONTEXT_VERSION_MAJOR, 3);
        GLFW.glfwWindowHint(GLFW.GLFW_CONTEXT_VERSION_MINOR, 3);
        GLFW.glfwWindowHint(GLFW.GLFW_OPENGL_PROFILE, GLFW.GLFW_OPENGL_CORE_PROFILE);
        GLFW.glfwWindowHint(GLFW.GLFW_AUTO_ICONIFY, GLFW.GLFW_FALSE);

        String os = System.getProperty("os.name").toLowerCase();
        if (os.contains("mac")) {
            GLFW.glfwWindowHint(GLFW.GLFW_OPENGL_FORWARD_COMPAT, GLFW.GLFW_TRUE);
        }

        window = GLFW.glfwCreateWindow(windowState.width, windowState.height, "VoxelPopuli Java", 0, 0);
        if (window == 0) {
            throw new RuntimeException("Failed to create the GLFW window");
        }

        GLFW.glfwMakeContextCurrent(window);
        GLFW.glfwSetWindowPos(window, windowState.x, windowState.y);
        if (windowState.maximized) {
            GLFW.glfwMaximizeWindow(window);
        }

        // Set input modes and callbacks
        GLFW.glfwSetInputMode(window, GLFW.GLFW_CURSOR, GLFW.GLFW_CURSOR_DISABLED);

        GLFW.glfwSetFramebufferSizeCallback(window, (win, w, h) -> {
            GL11.glViewport(0, 0, w, h);
        });

        GLFW.glfwSetKeyCallback(window, (win, key, scancode, action, mods) -> {
            onKeyEvent(key, action, mods);
        });

        GLFW.glfwSetCursorPosCallback(window, (win, xpos, ypos) -> {
            onCursorPosEvent(xpos, ypos);
        });

        GLFW.glfwSetMouseButtonCallback(window, (win, button, action, mods) -> {
            onMouseButtonEvent(button, action, mods);
        });

        GLFW.glfwSetScrollCallback(window, (win, xoffset, yoffset) -> {
            onScrollEvent(xoffset, yoffset);
        });

        GL.createCapabilities();

        GL11.glEnable(GL11.GL_DEPTH_TEST);
        GL11.glDepthFunc(GL11.GL_LEQUAL);
        GL11.glEnable(GL11.GL_CULL_FACE);
        GL11.glCullFace(GL11.GL_BACK);
        GL11.glEnable(GL11.GL_BLEND);
        GL11.glBlendFunc(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA);

        // Initialize world, shaders, textures
        world = new World();
        world.generateAtlas();
        world.initCelestial();

        player = new Player();
        player.position.set(32.5f, 150.0f, 32.5f);
        player.invSlots[0] = new ItemStack(BlockType.NUKE, 64);
        player.invSlots[1] = new ItemStack(BlockType.FLINT_AND_STEEL, 1);

        camera = new Camera();
        miningState = new MiningState();

        terrainShader = new Shader(Shader.loadSource("assets/shaders/ps1.vs"), Shader.loadSource("assets/shaders/ps1.fs"));
        waterShader = new Shader(Shader.loadSource("assets/shaders/water.vs"), Shader.loadSource("assets/shaders/water.fs"));
        flatShader = new Shader(Shader.loadSource("assets/shaders/flat.vs"), Shader.loadSource("assets/shaders/flat.fs"));
        uiShader = new Shader(Shader.loadSource("assets/shaders/ui.vs"), Shader.loadSource("assets/shaders/ui.fs"));
        textureShader = new Shader(Shader.loadSource("assets/shaders/texture.vs"), Shader.loadSource("assets/shaders/texture.fs"));
        textureUiShader = new Shader(Shader.loadSource("assets/shaders/ui_texture.vs"), Shader.loadSource("assets/shaders/ui_texture.fs"));
        colorShader = new Shader(Shader.loadSource("assets/shaders/texture.vs"), Shader.loadSource("assets/shaders/color.fs"));

        renderTarget = new RenderTexture2D(RENDER_WIDTH, RENDER_HEIGHT);
        fontTexture = Texture2D.fromFile("assets/font.png");

        // Init Quad Mesh
        float[] qv = new float[] {
            -1.0f, -1.0f, 0.0f,
             1.0f, -1.0f, 0.0f,
             1.0f,  1.0f, 0.0f,
            -1.0f, -1.0f, 0.0f,
             1.0f,  1.0f, 0.0f,
            -1.0f,  1.0f, 0.0f
        };
        float[] qt = new float[] {
            0.0f, 0.0f,
            1.0f, 0.0f,
            1.0f, 1.0f,
            0.0f, 0.0f,
            1.0f, 1.0f,
            0.0f, 1.0f
        };
        quadMesh = new Mesh(qv, qt, null, null);

        // Initial cursor pos
        try (MemoryStack stack = MemoryStack.stackPush()) {
            DoubleBuffer x = stack.mallocDouble(1);
            DoubleBuffer y = stack.mallocDouble(1);
            GLFW.glfwGetCursorPos(window, x, y);
            lastCursorX = x.get(0);
            lastCursorY = y.get(0);
        }

        fpsLastTime = GLFW.glfwGetTime();

        // Start Game Loop
        gameLoop = new GameLoop(new GameLoop.GameCallback() {
            @Override
            public void onUpdate(float dt) {
                GLFW.glfwPollEvents();
                if (GLFW.glfwWindowShouldClose(window)) {
                    gameLoop.stop();
                    return;
                }
                Main.this.onUpdate(dt);
            }

            @Override
            public void onRender(float alpha) {
                // Update FPS window title
                fpsFrames++;
                double current = GLFW.glfwGetTime();
                double elapsed = current - fpsLastTime;
                if (elapsed >= 1.0) {
                    float fps = (float) (fpsFrames / elapsed);
                    fpsFrames = 0;
                    fpsLastTime = current;
                    GLFW.glfwSetWindowTitle(window, String.format("VoxelPopuli Java - %.0f FPS", fps));
                }

                Main.this.onRender(alpha);
            }
        });

        gameLoop.start();

        // Cleanup resources
        quadMesh.cleanup();
        renderTarget.cleanup();
        fontTexture.cleanup();
        terrainShader.cleanup();
        waterShader.cleanup();
        flatShader.cleanup();
        uiShader.cleanup();
        textureShader.cleanup();
        textureUiShader.cleanup();
        colorShader.cleanup();
        world.cleanup();

        // Save window state
        try (MemoryStack stack = MemoryStack.stackPush()) {
            IntBuffer xw = stack.mallocInt(1);
            IntBuffer yw = stack.mallocInt(1);
            GLFW.glfwGetWindowPos(window, xw, yw);
            windowState.x = xw.get(0);
            windowState.y = yw.get(0);

            IntBuffer ww = stack.mallocInt(1);
            IntBuffer hw = stack.mallocInt(1);
            GLFW.glfwGetWindowSize(window, ww, hw);
            windowState.width = ww.get(0);
            windowState.height = hw.get(0);
        }
        windowState.maximized = GLFW.glfwGetWindowAttrib(window, GLFW.GLFW_MAXIMIZED) == GLFW.GLFW_TRUE;
        windowState.save();

        Callbacks.glfwFreeCallbacks(window);
        GLFW.glfwDestroyWindow(window);
        GLFW.glfwTerminate();
    }

    private void onKeyEvent(int key, int action, int mods) {
        if (gameState == GameState.LOADING) return;
        boolean shift = (mods & GLFW.GLFW_MOD_SHIFT) != 0;

        if (key == GLFW.GLFW_KEY_ESCAPE && action == GLFW.GLFW_PRESS) {
            if (gameState == GameState.PAUSED) {
                gameState = GameState.PLAYING;
                GLFW.glfwSetInputMode(window, GLFW.GLFW_CURSOR, GLFW.GLFW_CURSOR_DISABLED);
            } else if (gameState == GameState.PLAYING) {
                if (craftingTableOpen) {
                    player.ctClose();
                    craftingTableOpen = false;
                    player.inventoryOpen = false;
                    if (player.cursor != null) {
                        player.invAdd(player.cursor.block, player.cursor.count);
                        player.cursor = null;
                    }
                    GLFW.glfwSetInputMode(window, GLFW.GLFW_CURSOR, GLFW.GLFW_CURSOR_DISABLED);
                } else if (player.inventoryOpen) {
                    player.inventoryOpen = false;
                    if (player.cursor != null) {
                        player.invAdd(player.cursor.block, player.cursor.count);
                        player.cursor = null;
                    }
                    GLFW.glfwSetInputMode(window, GLFW.GLFW_CURSOR, GLFW.GLFW_CURSOR_DISABLED);
                } else {
                    gameState = GameState.PAUSED;
                    GLFW.glfwSetInputMode(window, GLFW.GLFW_CURSOR, GLFW.GLFW_CURSOR_NORMAL);
                }
            }
        } else if (key == GLFW.GLFW_KEY_E && action == GLFW.GLFW_PRESS) {
            if (gameState != GameState.PLAYING) return;
            if (craftingTableOpen) {
                player.ctClose();
                craftingTableOpen = false;
                player.inventoryOpen = false;
                if (player.cursor != null) {
                    player.invAdd(player.cursor.block, player.cursor.count);
                    player.cursor = null;
                }
                GLFW.glfwSetInputMode(window, GLFW.GLFW_CURSOR, GLFW.GLFW_CURSOR_DISABLED);
            } else {
                player.inventoryOpen = !player.inventoryOpen;
                if (player.inventoryOpen) {
                    GLFW.glfwSetInputMode(window, GLFW.GLFW_CURSOR, GLFW.GLFW_CURSOR_NORMAL);
                } else {
                    if (player.cursor != null) {
                        player.invAdd(player.cursor.block, player.cursor.count);
                        player.cursor = null;
                    }
                    GLFW.glfwSetInputMode(window, GLFW.GLFW_CURSOR, GLFW.GLFW_CURSOR_DISABLED);
                }
            }
        } else if (key == GLFW.GLFW_KEY_F11 && action == GLFW.GLFW_PRESS) {
            isFullscreen = !isFullscreen;
            if (isFullscreen) {
                try (MemoryStack stack = MemoryStack.stackPush()) {
                    IntBuffer x = stack.mallocInt(1);
                    IntBuffer y = stack.mallocInt(1);
                    GLFW.glfwGetWindowPos(window, x, y);
                    winPos[0] = x.get(0);
                    winPos[1] = y.get(0);

                    IntBuffer w = stack.mallocInt(1);
                    IntBuffer h = stack.mallocInt(1);
                    GLFW.glfwGetWindowSize(window, w, h);
                    winSize[0] = w.get(0);
                    winSize[1] = h.get(0);
                }

                long monitor = GLFW.glfwGetPrimaryMonitor();
                if (monitor != 0) {
                    GLFWVidMode vidMode = GLFW.glfwGetVideoMode(monitor);
                    if (vidMode != null) {
                        GLFW.glfwSetWindowMonitor(window, monitor, 0, 0, vidMode.width(), vidMode.height(), vidMode.refreshRate());
                    }
                }
            } else {
                GLFW.glfwSetWindowMonitor(window, 0, winPos[0], winPos[1], winSize[0], winSize[1], GLFW.GLFW_DONT_CARE);
            }
        } else if (action == GLFW.GLFW_PRESS && key >= GLFW.GLFW_KEY_1 && key <= GLFW.GLFW_KEY_9) {
            player.selectedSlot = key - GLFW.GLFW_KEY_1;
        } else if (key == GLFW.GLFW_KEY_Q && action == GLFW.GLFW_PRESS) {
            if (player.inventoryOpen) {
                player.cursor = null;
            } else {
                player.invSlots[player.selectedSlot] = null;
            }
        }
    }

    private void onCursorPosEvent(double xpos, double ypos) {
        if (gameState == GameState.PLAYING && !player.inventoryOpen) {
            float dx = (float) (xpos - lastCursorX);
            float dy = (float) (ypos - lastCursorY);
            cameraAngle.x -= dx * 0.003f;
            cameraAngle.y -= dy * 0.003f;
            cameraAngle.y = Math.clamp(cameraAngle.y, -1.56f, 1.56f);
        }
        lastCursorX = xpos;
        lastCursorY = ypos;
    }

    private void onMouseButtonEvent(int button, int action, int mods) {
        if (gameState == GameState.LOADING) return;
        boolean left = (button == GLFW.GLFW_MOUSE_BUTTON_LEFT);
        boolean right = (button == GLFW.GLFW_MOUSE_BUTTON_RIGHT);
        boolean shift = (mods & GLFW.GLFW_MOD_SHIFT) != 0;

        try (MemoryStack stack = MemoryStack.stackPush()) {
            IntBuffer fbW = stack.mallocInt(1);
            IntBuffer fbH = stack.mallocInt(1);
            GLFW.glfwGetFramebufferSize(window, fbW, fbH);
            float sw = fbW.get(0);
            float sh = fbH.get(0);

            float px = sw / 2.0f - 190.0f;
            float py = sh / 2.0f - 170.0f;
            float mx = (float) lastCursorX;
            float my = (float) lastCursorY;

            if (craftingTableOpen && action == GLFW.GLFW_PRESS) {
                Integer ctSlot = ctSlotAtPos(mx, my, px, py);
                if (ctSlot != null) {
                    player.ctClick(ctSlot, right && !left);
                }
            } else if (player.inventoryOpen && action == GLFW.GLFW_PRESS) {
                Integer slot = slotAtPos(mx, my, px, py);
                if (slot != null) {
                    player.invClick(slot, right && !left, shift);
                }
            } else if (gameState == GameState.PAUSED && action == GLFW.GLFW_PRESS && left) {
                float btnW = 340.0f;
                float btnH = 44.0f;
                float btnX = 100.0f;
                float btnYStart = sh / 2.0f - 100.0f;

                for (int i = 0; i < 4; i++) {
                    float y = btnYStart + i * (btnH + 12.0f);
                    if (mx >= btnX && mx <= btnX + btnW && my >= y && my <= y + btnH) {
                        if (i == 0) {
                            gameState = GameState.PLAYING;
                            GLFW.glfwSetInputMode(window, GLFW.GLFW_CURSOR, GLFW.GLFW_CURSOR_DISABLED);
                        } else if (i == 3) {
                            GLFW.glfwSetWindowShouldClose(window, true);
                        }
                    }
                }
            } else if (gameState == GameState.PLAYING && !player.inventoryOpen) {
                if (left) {
                    leftMouseHeld = (action == GLFW.GLFW_PRESS);
                    if (action == GLFW.GLFW_RELEASE) {
                        miningState.reset();
                    }
                }
                if (right && action == GLFW.GLFW_PRESS) {
                    Vector3f eyePos = camera.position;
                    Vector3f lookDir = camera.getLookDirection();
                    RaycastResult res = world.raycast(eyePos, lookDir, 8.0f);
                    if (res.hit) {
                        BlockType targetBlock = world.getBlock(res.x, res.y, res.z);
                        if (targetBlock == BlockType.CRAFTING_TABLE) {
                            craftingTableOpen = true;
                            player.inventoryOpen = true;
                            GLFW.glfwSetInputMode(window, GLFW.GLFW_CURSOR, GLFW.GLFW_CURSOR_NORMAL);
                            return;
                        }

                        ItemStack s = player.invSlots[player.selectedSlot];
                        if (s != null) {
                            if (s.block == BlockType.FLINT_AND_STEEL) {
                                BlockType target = world.getBlock(res.x, res.y, res.z);
                                if (target == BlockType.TNT) {
                                    world.setBlock(res.x, res.y, res.z, BlockType.AIR);
                                    world.explosives.add(new ActiveExplosive(new Vector3f(res.x, res.y, res.z), 4.0f, BlockType.TNT));
                                    return;
                                } else if (target == BlockType.NUKE) {
                                    world.setBlock(res.x, res.y, res.z, BlockType.AIR);
                                    world.explosives.add(new ActiveExplosive(new Vector3f(res.x, res.y, res.z), 4.0f, BlockType.NUKE));
                                    return;
                                }
                            }

                            if (!s.block.isItem()) {
                                int nx = res.x + res.nx;
                                int ny = res.y + res.ny;
                                int nz = res.z + res.nz;
                                if (world.getBlock(nx, ny, nz) == BlockType.AIR && !player.intersectsBlock(nx, ny, nz)) {
                                    world.setBlock(nx, ny, nz, s.block);
                                    s.count -= 1;
                                    if (s.count == 0) {
                                        player.invSlots[player.selectedSlot] = null;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    private void onScrollEvent(double xoffset, double yoffset) {
        if (gameState == GameState.PLAYING && !player.inventoryOpen) {
            if (yoffset > 0) {
                player.selectedSlot = (player.selectedSlot + 8) % 9;
            } else if (yoffset < 0) {
                player.selectedSlot = (player.selectedSlot + 1) % 9;
            }
        }
    }

    private void onUpdate(float dt) {
        double currentTime = GLFW.glfwGetTime();

        float gpLx = 0.0f;
        float gpLy = 0.0f;
        boolean gpJump = false;
        boolean gpSneak = false;
        boolean gpSprint = false;

        // Poll Gamepad
        try (GLFWGamepadState gs = GLFWGamepadState.malloc()) {
            for (int jid = GLFW.GLFW_JOYSTICK_1; jid <= GLFW.GLFW_JOYSTICK_16; jid++) {
                if (GLFW.glfwJoystickPresent(jid) && GLFW.glfwJoystickIsGamepad(jid)) {
                    if (GLFW.glfwGetGamepadState(jid, gs)) {
                        float DEADZONE = 0.15f;
                        float LOOK_SPEED = 3.0f;

                        // Camera Look
                        if (gameState == GameState.PLAYING && !player.inventoryOpen) {
                            float rx = gs.axes(GLFW.GLFW_GAMEPAD_AXIS_RIGHT_X);
                            float ry = gs.axes(GLFW.GLFW_GAMEPAD_AXIS_RIGHT_Y);
                            rx = (Math.abs(rx) > DEADZONE) ? rx : 0.0f;
                            ry = (Math.abs(ry) > DEADZONE) ? ry : 0.0f;
                            cameraAngle.x -= rx * LOOK_SPEED * dt;
                            cameraAngle.y -= ry * LOOK_SPEED * dt;
                            cameraAngle.y = Math.clamp(cameraAngle.y, -1.56f, 1.56f);
                        }

                        // Left stick
                        float lx = gs.axes(GLFW.GLFW_GAMEPAD_AXIS_LEFT_X);
                        float ly = gs.axes(GLFW.GLFW_GAMEPAD_AXIS_LEFT_Y);
                        gpLx = (Math.abs(lx) > DEADZONE) ? lx : 0.0f;
                        gpLy = (Math.abs(ly) > DEADZONE) ? ly : 0.0f;

                        gpJump = (gs.buttons(GLFW.GLFW_GAMEPAD_BUTTON_A) == GLFW.GLFW_PRESS);
                        gpSneak = (gs.buttons(GLFW.GLFW_GAMEPAD_BUTTON_B) == GLFW.GLFW_PRESS);
                        gpSprint = (gs.buttons(GLFW.GLFW_GAMEPAD_BUTTON_LEFT_THUMB) == GLFW.GLFW_PRESS);

                        float rt = gs.axes(GLFW.GLFW_GAMEPAD_AXIS_RIGHT_TRIGGER);
                        float lt = gs.axes(GLFW.GLFW_GAMEPAD_AXIS_LEFT_TRIGGER);

                        if (!player.inventoryOpen) {
                            boolean gpBreakHeld = rt > 0.5f;
                            if (!gpBreakHeld && !leftMouseHeld) {
                                miningState.reset();
                            }

                            boolean gpPlace = lt > 0.5f && prevGpLt <= 0.5f;
                            if (gpPlace) {
                                Vector3f gpEye = camera.position;
                                Vector3f gpLook = camera.getLookDirection();
                                RaycastResult res = world.raycast(gpEye, gpLook, 8.0f);
                                if (res.hit) {
                                    BlockType targetBlock = world.getBlock(res.x, res.y, res.z);
                                    if (targetBlock == BlockType.CRAFTING_TABLE) {
                                        craftingTableOpen = true;
                                        player.inventoryOpen = true;
                                        GLFW.glfwSetInputMode(window, GLFW.GLFW_CURSOR, GLFW.GLFW_CURSOR_NORMAL);
                                    } else {
                                        ItemStack s = player.invSlots[player.selectedSlot];
                                        if (s != null) {
                                            if (s.block == BlockType.FLINT_AND_STEEL) {
                                                BlockType target = world.getBlock(res.x, res.y, res.z);
                                                if (target == BlockType.TNT) {
                                                    world.setBlock(res.x, res.y, res.z, BlockType.AIR);
                                                    world.explosives.add(new ActiveExplosive(new Vector3f(res.x, res.y, res.z), 4.0f, BlockType.TNT));
                                                } else if (target == BlockType.NUKE) {
                                                    world.setBlock(res.x, res.y, res.z, BlockType.AIR);
                                                    world.explosives.add(new ActiveExplosive(new Vector3f(res.x, res.y, res.z), 4.0f, BlockType.NUKE));
                                                }
                                            } else if (!s.block.isItem()) {
                                                int nx = res.x + res.nx;
                                                int ny = res.y + res.ny;
                                                int nz = res.z + res.nz;
                                                if (world.getBlock(nx, ny, nz) == BlockType.AIR && !player.intersectsBlock(nx, ny, nz)) {
                                                    world.setBlock(nx, ny, nz, s.block);
                                                    s.count -= 1;
                                                    if (s.count == 0) {
                                                        player.invSlots[player.selectedSlot] = null;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        prevGpLt = lt;

                        boolean dpadR = (gs.buttons(GLFW.GLFW_GAMEPAD_BUTTON_DPAD_RIGHT) == GLFW.GLFW_PRESS);
                        boolean dpadL = (gs.buttons(GLFW.GLFW_GAMEPAD_BUTTON_DPAD_LEFT) == GLFW.GLFW_PRESS);
                        boolean rb = (gs.buttons(GLFW.GLFW_GAMEPAD_BUTTON_RIGHT_BUMPER) == GLFW.GLFW_PRESS);
                        boolean lb = (gs.buttons(GLFW.GLFW_GAMEPAD_BUTTON_LEFT_BUMPER) == GLFW.GLFW_PRESS);

                        if ((dpadR && !prevGpDpadRight) || (rb && !prevGpRb)) {
                            player.selectedSlot = (player.selectedSlot + 1) % 9;
                        }
                        if ((dpadL && !prevGpDpadLeft) || (lb && !prevGpLb)) {
                            player.selectedSlot = (player.selectedSlot + 8) % 9;
                        }

                        prevGpDpadRight = dpadR;
                        prevGpDpadLeft = dpadL;
                        prevGpRb = rb;
                        prevGpLb = lb;

                        break; // Process one gamepad
                    }
                }
            }
        }

        Vector3f moveDir = new Vector3f(0.0f, 0.0f, 0.0f);
        if (gameState == GameState.PLAYING && !player.inventoryOpen) {
            Vector3f forward = camera.getForward();
            Vector3f right = camera.getRight();
            if (GLFW.glfwGetKey(window, GLFW.GLFW_KEY_W) == GLFW.GLFW_PRESS) {
                moveDir.add(forward);
            }
            if (GLFW.glfwGetKey(window, GLFW.GLFW_KEY_S) == GLFW.GLFW_PRESS) {
                moveDir.sub(forward);
            }
            if (GLFW.glfwGetKey(window, GLFW.GLFW_KEY_A) == GLFW.GLFW_PRESS) {
                moveDir.sub(right);
            }
            if (GLFW.glfwGetKey(window, GLFW.GLFW_KEY_D) == GLFW.GLFW_PRESS) {
                moveDir.add(right);
            }

            // Gamepad stick movement
            moveDir.add(new Vector3f(right).mul(gpLx));
            moveDir.add(new Vector3f(forward).mul(-gpLy));
        }

        boolean isSprinting = (GLFW.glfwGetKey(window, GLFW.GLFW_KEY_LEFT_CONTROL) == GLFW.GLFW_PRESS) || gpSprint;
        boolean isJumping = (GLFW.glfwGetKey(window, GLFW.GLFW_KEY_SPACE) == GLFW.GLFW_PRESS) || gpJump;
        boolean isSneaking = (GLFW.glfwGetKey(window, GLFW.GLFW_KEY_LEFT_SHIFT) == GLFW.GLFW_PRESS) || gpSneak;

        camera.position.set(player.position).add(0.0f, 1.6f, 0.0f);
        camera.yaw = cameraAngle.x;
        camera.pitch = cameraAngle.y;

        if (gameState == GameState.PLAYING) {
            player.update(world, moveDir, dt, isSprinting, isJumping, isSneaking, currentTime);
            world.update(player.position, dt);

            // Mining check
            boolean gpRtHeld = false;
            try (GLFWGamepadState gs = GLFWGamepadState.malloc()) {
                for (int jid = GLFW.GLFW_JOYSTICK_1; jid <= GLFW.GLFW_JOYSTICK_16; jid++) {
                    if (GLFW.glfwJoystickPresent(jid) && GLFW.glfwJoystickIsGamepad(jid)) {
                        if (GLFW.glfwGetGamepadState(jid, gs)) {
                            if (gs.axes(GLFW.GLFW_GAMEPAD_AXIS_RIGHT_TRIGGER) > 0.5f) {
                                gpRtHeld = true;
                            }
                            break;
                        }
                    }
                }
            }

            if ((leftMouseHeld || gpRtHeld) && !player.inventoryOpen) {
                ItemStack heldItem = player.invSlots[player.selectedSlot];
                BlockType heldType = (heldItem != null) ? heldItem.block : BlockType.AIR;
                Vector3f eyePos = camera.position;
                Vector3f lookDir = camera.getLookDirection();
                MinedBlock mined = miningState.update(world, eyePos, lookDir, heldType, dt);
                if (mined != null) {
                    if (mined.drop() != BlockType.AIR && mined.dropCount() > 0) {
                        player.invAdd(mined.drop(), mined.dropCount());
                    }
                    world.setBlock(mined.x(), mined.y(), mined.z(), BlockType.AIR);

                    // Tool durability decay
                    ItemStack tool = player.invSlots[player.selectedSlot];
                    if (tool != null && tool.block.isTool() && tool.durability != null) {
                        tool.durability = Math.max(0, tool.durability - 1);
                        if (tool.durability == 0) {
                            player.invSlots[player.selectedSlot] = null;
                        }
                    }
                }
            } else {
                miningState.reset();
            }
        } else if (gameState == GameState.LOADING) {
            world.update(new Vector3f(32.5f, 0.0f, 32.5f), (float) currentTime);
            if (!world.isLoading) {
                float spawnY = 150.0f;
                for (int y = 255; y >= 0; y--) {
                    if (world.getBlock(32, y, 32) != BlockType.AIR) {
                        spawnY = y + 2.0f;
                        break;
                    }
                }
                player.position.set(32.5f, spawnY, 32.5f);
                gameState = GameState.PLAYING;
            }
        }
    }

    private void onRender(float alpha) {
        double currentTime = GLFW.glfwGetTime();

        if (gameState == GameState.LOADING) {
            try (MemoryStack stack = MemoryStack.stackPush()) {
                IntBuffer w = stack.mallocInt(1);
                IntBuffer h = stack.mallocInt(1);
                GLFW.glfwGetFramebufferSize(window, w, h);
                int winSw = w.get(0);
                int winSh = h.get(0);
                float sw = winSw;
                float sh = winSh;

                GL11.glViewport(0, 0, winSw, winSh);
                GL11.glClearColor(0.117f, 0.117f, 0.117f, 1.0f);
                GL11.glClear(GL11.GL_COLOR_BUFFER_BIT | GL11.GL_DEPTH_BUFFER_BIT);

                GL11.glDisable(GL11.GL_DEPTH_TEST);
                GL11.glDisable(GL11.GL_CULL_FACE);
                GL11.glEnable(GL11.GL_BLEND);
                GL11.glBlendFunc(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA);

                // Spinner
                float spinX = sw / 2.0f;
                float spinY = sh / 2.0f + 10.0f;
                float spinRadius = 16.0f;
                for (int i = 0; i < 8; i++) {
                    float angle = (i / 8.0f) * 2.0f * (float) Math.PI;
                    int tOffset = ((int) (currentTime * 8.0f)) % 8;
                    int brightness;
                    if (i == tOffset) brightness = 255;
                    else if (i == (tOffset + 7) % 8) brightness = 180;
                    else if (i == (tOffset + 6) % 8) brightness = 100;
                    else brightness = 40;

                    float sx = spinX + (float) Math.cos(angle) * spinRadius;
                    float sy = spinY + (float) Math.sin(angle) * spinRadius;
                    drawRect(uiShader, sx - 2.0f, sy - 2.0f, 4.0f, 4.0f, new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) brightness}, sw, sh);
                }

                // Progress Bar
                float barW = Math.min(sw * 0.45f, 500.0f);
                float barH = 4.0f;
                float barX = (sw - barW) / 2.0f;
                float barY = sh / 2.0f + 60.0f;

                drawRect(uiShader, barX - 1.0f, barY - 1.0f, barW + 2.0f, barH + 2.0f, new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) 255}, sw, sh);
                drawRect(uiShader, barX, barY, barW, barH, new byte[]{(byte) 30, (byte) 30, (byte) 30, (byte) 255}, sw, sh);

                float totalChunks = World.CHUNK_POOL_SIZE;
                float progress = Math.clamp(world.chunksGeneratedCount / totalChunks, 0.0f, 1.0f);
                drawRect(uiShader, barX, barY, barW * progress, barH, new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) 255}, sw, sh);

                int pct = (int) (progress * 100.0f);
                String pctStr = pct + "%";
                drawText(fontTexture, pctStr, sw / 2.0f - 15.0f, barY + 15.0f, 18.0f, textureUiShader, sw, sh);

                GL11.glEnable(GL11.GL_DEPTH_TEST);
            }
            GLFW.glfwSwapBuffers(window);
            return;
        }

        Vector3f eyePos = camera.position;
        Vector3f lookDir = camera.getLookDirection();

        float sunAngle = (float) (currentTime / 1200.0) * 2.0f * (float) Math.PI;
        float sunY = (float) Math.sin(sunAngle);
        Vector3f sunDir = new Vector3f(0.0f, sunY, (float) Math.cos(sunAngle));
        Vector4f skyC;
        if (sunY > 0.3f) {
            skyC = new Vector4f(135.0f / 255.0f, 206.0f / 255.0f, 235.0f / 255.0f, 1.0f);
        } else if (sunY > -0.2f) {
            float t = Math.clamp((sunY + 0.2f) / 0.5f, 0.0f, 1.0f);
            if (t < 0.5f) {
                float u = t * 2.0f;
                skyC = new Vector4f(
                    (255.0f * u + 135.0f * (1.0f - u)) / 255.0f,
                    (160.0f * u + 100.0f * (1.0f - u)) / 255.0f,
                    (180.0f * u + 150.0f * (1.0f - u)) / 255.0f,
                    1.0f
                );
            } else {
                float u = (t - 0.5f) * 2.0f;
                skyC = new Vector4f(
                    (135.0f * u + 255.0f * (1.0f - u)) / 255.0f,
                    (206.0f * u + 160.0f * (1.0f - u)) / 255.0f,
                    (235.0f * u + 180.0f * (1.0f - u)) / 255.0f,
                    1.0f
                );
            }
        } else if (sunY > -0.5f) {
            float t = (sunY + 0.5f) / 0.3f;
            skyC = new Vector4f(
                (30.0f * t + 10.0f * (1.0f - t)) / 255.0f,
                (30.0f * t + 10.0f * (1.0f - t)) / 255.0f,
                (80.0f * t + 50.0f * (1.0f - t)) / 255.0f,
                1.0f
            );
        } else {
            skyC = new Vector4f(5.0f / 255.0f, 5.0f / 255.0f, 15.0f / 255.0f, 1.0f);
        }

        renderTarget.bind();
        GL11.glClearColor(skyC.x, skyC.y, skyC.z, 1.0f);
        GL11.glClear(GL11.GL_COLOR_BUFFER_BIT | GL11.GL_DEPTH_BUFFER_BIT);

        float aspect = (float) RENDER_WIDTH / RENDER_HEIGHT;
        Matrix4f projection = camera.getProjectionMatrix(75.0f, aspect, 0.1f, 1000.0f);
        Matrix4f view = camera.getViewMatrix();
        Matrix4f mvp = new Matrix4f(projection).mul(view);

        world.renderStars(player.position, (float) currentTime, flatShader, mvp);
        drawSunMoon(flatShader, player.position, (float) currentTime, mvp);

        terrainShader.bind();
        terrainShader.setMat4(terrainShader.getUniformLocation("uMVP"), mvp);
        terrainShader.setMat4(terrainShader.getUniformLocation("uModel"), new Matrix4f());
        terrainShader.setFloat(terrainShader.getUniformLocation("uTime"), (float) currentTime);
        terrainShader.setVec2(terrainShader.getUniformLocation("uResolution"), new Vector2f(RENDER_WIDTH, RENDER_HEIGHT));
        terrainShader.setVec3(terrainShader.getUniformLocation("sunDir"), sunDir);
        terrainShader.setVec4(terrainShader.getUniformLocation("colDiffuse"), new Vector4f(1.0f, 1.0f, 1.0f, 1.0f));
        terrainShader.setVec3(terrainShader.getUniformLocation("viewPos"), eyePos);
        terrainShader.setVec4(terrainShader.getUniformLocation("skyCol"), skyC);

        Frustum frustum = Frustum.fromMatrix(mvp);
        world.computeVisibleChunks(frustum);
        world.renderOpaque(frustum);

        // Crack Overlay
        if (miningState.target != null && miningState.getCrackStage() != null) {
            int cx = miningState.target.x();
            int cy = miningState.target.y();
            int cz = miningState.target.z();
            int stage = miningState.getCrackStage();

            float ts = 1.0f / 16.0f;
            float u = stage * ts;
            float v = 3.0f * ts;
            float e = 0.002f;
            float x0 = cx - e;
            float y0 = cy - e;
            float z0 = cz - e;
            float x1 = cx + 1.0f + e;
            float y1 = cy + 1.0f + e;
            float z1 = cz + 1.0f + e;

            float[] verts = new float[] {
                x0,y0,z0, x1,y0,z0, x1,y1,z0, x1,y1,z0, x0,y1,z0, x0,y0,z0,
                x0,y0,z1, x1,y0,z1, x1,y1,z1, x1,y1,z1, x0,y1,z1, x0,y0,z1,
                x0,y0,z0, x0,y0,z1, x0,y1,z1, x0,y1,z1, x0,y1,z0, x0,y0,z0,
                x1,y0,z0, x1,y0,z1, x1,y1,z1, x1,y1,z1, x1,y1,z0, x1,y0,z0,
                x0,y0,z0, x1,y0,z0, x1,y0,z1, x1,y0,z1, x0,y0,z1, x0,y0,z0,
                x0,y1,z0, x1,y1,z0, x1,y1,z1, x1,y1,z1, x0,y1,z1, x0,y1,z0
            };

            float u1 = u + ts;
            float v1 = v + ts;
            float[] uvs = new float[72];
            for (int i = 0; i < 6; i++) {
                uvs[i * 12]     = u;  uvs[i * 12 + 1] = v1;
                uvs[i * 12 + 2] = u1; uvs[i * 12 + 3] = v1;
                uvs[i * 12 + 4] = u1; uvs[i * 12 + 5] = v;
                uvs[i * 12 + 6] = u1; uvs[i * 12 + 7] = v;
                uvs[i * 12 + 8] = u;  uvs[i * 12 + 9] = v;
                uvs[i * 12 + 10] = u; uvs[i * 12 + 11] = v1;
            }

            if (world.atlas != null) {
                world.atlas.bind(0);
            }
            GL11.glEnable(GL11.GL_BLEND);
            GL11.glBlendFunc(GL11.GL_DST_COLOR, GL11.GL_ZERO);
            GL11.glDepthMask(false);
            GL11.glEnable(GL11.GL_POLYGON_OFFSET_FILL);
            GL11.glPolygonOffset(-1.0f, -1.0f);

            Mesh crackMesh = new Mesh(verts, uvs, null, null);
            crackMesh.draw();
            crackMesh.cleanup();

            GL11.glDisable(GL11.GL_POLYGON_OFFSET_FILL);
            GL11.glDepthMask(true);
            GL11.glBlendFunc(GL11.GL_SRC_ALPHA, GL11.GL_ONE_MINUS_SRC_ALPHA);
            GL11.glDisable(GL11.GL_BLEND);
        }

        world.renderExplosives(terrainShader, mvp, (float) currentTime);
        world.renderParticles(terrainShader, mvp);

        boolean aboveClouds = eyePos.y > World.CLOUD_HEIGHT;
        if (!aboveClouds) {
            world.renderClouds(flatShader, mvp);
        }

        terrainShader.bind();
        world.renderTransparent(frustum);

        waterShader.bind();
        waterShader.setMat4(waterShader.getUniformLocation("uMVP"), mvp);
        waterShader.setMat4(waterShader.getUniformLocation("uModel"), new Matrix4f());
        waterShader.setFloat(waterShader.getUniformLocation("uTime"), (float) currentTime);
        waterShader.setVec3(waterShader.getUniformLocation("sunDir"), sunDir);
        waterShader.setVec3(waterShader.getUniformLocation("viewPos"), eyePos);
        waterShader.setVec4(waterShader.getUniformLocation("skyCol"), skyC);
        world.renderWater(frustum);

        if (aboveClouds) {
            world.renderClouds(flatShader, mvp);
        }

        renderTarget.unbind();

        // Upscale
        try (MemoryStack stack = MemoryStack.stackPush()) {
            IntBuffer w = stack.mallocInt(1);
            IntBuffer h = stack.mallocInt(1);
            GLFW.glfwGetFramebufferSize(window, w, h);
            int winWidth = w.get(0);
            int winHeight = h.get(0);
            GL11.glViewport(0, 0, winWidth, winHeight);

            GL11.glClearColor(0.0f, 0.0f, 0.0f, 1.0f);
            GL11.glClear(GL11.GL_COLOR_BUFFER_BIT | GL11.GL_DEPTH_BUFFER_BIT);

            textureShader.bind();
            drawTextureQuad(renderTarget.getTexture(), quadMesh);

            // Underwater Overlay
            BlockType camBlock = world.getBlock((int) Math.floor(eyePos.x), (int) Math.floor(eyePos.y), (int) Math.floor(eyePos.z));
            if (camBlock == BlockType.WATER) {
                GL11.glEnable(GL11.GL_BLEND);
                GL11.glDisable(GL11.GL_DEPTH_TEST);
                drawScreenQuad(colorShader, new Vector4f(0.05f, 0.05f, 0.2f, 0.55f), quadMesh);
                GL11.glEnable(GL11.GL_DEPTH_TEST);
            }

            GL11.glDisable(GL11.GL_DEPTH_TEST);
            GL11.glEnable(GL11.GL_BLEND);
            GL11.glDisable(GL11.GL_CULL_FACE);

            float sw = winWidth;
            float sh = winHeight;

            uiShader.bind();
            uiShader.setVec2(uiShader.getUniformLocation("uScreenSize"), new Vector2f(sw, sh));

            // Hotbar HUD
            float hbx = (sw - (9.0f * 40.0f - 4.0f)) / 2.0f;
            for (int i = 0; i < 9; i++) {
                float rx = hbx + i * 40.0f;
                float ry = sh - 46.0f;
                byte[] bg = (player.selectedSlot == i) ? new byte[]{(byte) 198, (byte) 198, (byte) 198, (byte) 255} : new byte[]{(byte) 85, (byte) 85, (byte) 85, (byte) 220};
                drawRect(uiShader, rx, ry, 36.0f, 36.0f, bg, sw, sh);

                drawRect(uiShader, rx, ry, 36.0f, 1.0f, new byte[]{0, 0, 0, (byte) 255}, sw, sh);
                drawRect(uiShader, rx, ry + 35.0f, 36.0f, 1.0f, new byte[]{0, 0, 0, (byte) 255}, sw, sh);
                drawRect(uiShader, rx, ry, 1.0f, 36.0f, new byte[]{0, 0, 0, (byte) 255}, sw, sh);
                drawRect(uiShader, rx + 35.0f, ry, 1.0f, 36.0f, new byte[]{0, 0, 0, (byte) 255}, sw, sh);

                if (player.selectedSlot == i) {
                    drawRect(uiShader, rx - 2.0f, ry - 2.0f, 40.0f, 2.0f, new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) 255}, sw, sh);
                    drawRect(uiShader, rx - 2.0f, ry + 36.0f, 40.0f, 2.0f, new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) 255}, sw, sh);
                    drawRect(uiShader, rx - 2.0f, ry - 2.0f, 2.0f, 40.0f, new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) 255}, sw, sh);
                    drawRect(uiShader, rx + 36.0f, ry - 2.0f, 2.0f, 40.0f, new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) 255}, sw, sh);
                }

                ItemStack s = player.invSlots[i];
                if (s != null && world.atlas != null) {
                    drawItemIcon(world.atlas, textureUiShader, s.block, rx + 4.0f, ry + 4.0f, 28.0f, 28.0f, sw, sh);
                }
            }

            // Crosshair
            if (!player.inventoryOpen) {
                drawRect(uiShader, sw / 2.0f - 2.0f, sh / 2.0f - 2.0f, 4.0f, 4.0f, new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) 255}, sw, sh);
            }

            // Crafting Table Overlay
            if (craftingTableOpen) {
                drawScreenQuad(colorShader, new Vector4f(0.0f, 0.0f, 0.0f, 0.6f), quadMesh);

                GL11.glDisable(GL11.GL_DEPTH_TEST);
                GL11.glEnable(GL11.GL_BLEND);
                GL11.glDisable(GL11.GL_CULL_FACE);

                uiShader.bind();
                uiShader.setVec2(uiShader.getUniformLocation("uScreenSize"), new Vector2f(sw, sh));

                float px = sw / 2.0f - 190.0f;
                float py = sh / 2.0f - 170.0f;
                float pw = 380.0f;
                float ph = 340.0f;

                drawRect(uiShader, px - 2.0f, py - 2.0f, pw + 4.0f, ph + 4.0f, new byte[]{55, 55, 55, (byte) 255}, sw, sh);
                drawRect(uiShader, px, py, pw, ph, new byte[]{(byte) 139, 120, 100, (byte) 255}, sw, sh);
                drawRect(uiShader, px + 2.0f, py + 2.0f, pw - 4.0f, ph - 4.0f, new byte[]{(byte) 198, (byte) 182, (byte) 161, (byte) 255}, sw, sh);

                drawRect(uiShader, px + 6.0f, py + 158.0f, pw - 12.0f, 2.0f, new byte[]{85, 75, 65, (byte) 255}, sw, sh);
                drawRect(uiShader, px + 6.0f, py + 160.0f, pw - 12.0f, 1.0f, new byte[]{(byte) 220, (byte) 200, (byte) 180, (byte) 255}, sw, sh);

                drawRect(uiShader, px + 16.0f, py + 4.0f, 130.0f, 12.0f, new byte[]{85, 75, 65, 120}, sw, sh);

                float ss = 36.0f;

                // 3x3 Grid
                for (int i = 0; i < 9; i++) {
                    float[] r = ctSlotRect(i, px, py);
                    drawSlot(uiShader, r[0], r[1], ss, null, false, sw, sh);
                    ItemStack s = player.craftTableSlots[i];
                    if (s != null && world.atlas != null) {
                        drawItemIcon(world.atlas, textureUiShader, s.block, r[0] + 4.0f, r[1] + 4.0f, 28.0f, 28.0f, sw, sh);
                    }
                }

                // Arrow
                drawRect(uiShader, px + 150.0f, py + 56.0f, 40.0f, 4.0f, new byte[]{85, 75, 65, (byte) 255}, sw, sh);
                drawRect(uiShader, px + 180.0f, py + 48.0f, 4.0f, 20.0f, new byte[]{85, 75, 65, (byte) 255}, sw, sh);

                // Output slot
                {
                    float[] r = ctSlotRect(9, px, py);
                    drawRect(uiShader, r[0] - 2.0f, r[1] - 2.0f, ss + 4.0f, ss + 4.0f, new byte[]{55, 55, 55, (byte) 255}, sw, sh);
                    drawRect(uiShader, r[0], r[1], ss, ss, new byte[]{(byte) 165, (byte) 155, (byte) 140, (byte) 255}, sw, sh);
                    ItemStack s = player.craftTableSlots[9];
                    if (s != null && world.atlas != null) {
                        drawItemIcon(world.atlas, textureUiShader, s.block, r[0] + 4.0f, r[1] + 4.0f, 28.0f, 28.0f, sw, sh);
                    }
                }

                // Main inventory slots 9-35
                for (int i = 0; i < 27; i++) {
                    float[] r = ctSlotRect(100 + i, px, py);
                    drawSlot(uiShader, r[0], r[1], ss, null, false, sw, sh);
                    ItemStack s = player.invSlots[9 + i];
                    if (s != null && world.atlas != null) {
                        drawItemIcon(world.atlas, textureUiShader, s.block, r[0] + 4.0f, r[1] + 4.0f, 28.0f, 28.0f, sw, sh);
                    }
                }

                // Hotbar slots 0-8
                for (int i = 0; i < 9; i++) {
                    float[] r = ctSlotRect(127 + i, px, py);
                    drawSlot(uiShader, r[0], r[1], ss, null, false, sw, sh);
                    ItemStack s = player.invSlots[i];
                    if (s != null && world.atlas != null) {
                        drawItemIcon(world.atlas, textureUiShader, s.block, r[0] + 4.0f, r[1] + 4.0f, 28.0f, 28.0f, sw, sh);
                    }
                }

                // Cursor item
                float mx = (float) lastCursorX;
                float my = (float) lastCursorY;
                if (player.cursor != null && world.atlas != null) {
                    drawItemIcon(world.atlas, textureUiShader, player.cursor.block, mx - 14.0f, my - 14.0f, 28.0f, 28.0f, sw, sh);
                }
            } else if (player.inventoryOpen) {
                // Render Inventory overlay
                drawScreenQuad(colorShader, new Vector4f(0.0f, 0.0f, 0.0f, 0.6f), quadMesh);

                GL11.glDisable(GL11.GL_DEPTH_TEST);
                GL11.glEnable(GL11.GL_BLEND);
                GL11.glDisable(GL11.GL_CULL_FACE);

                uiShader.bind();
                uiShader.setVec2(uiShader.getUniformLocation("uScreenSize"), new Vector2f(sw, sh));

                float px = sw / 2.0f - 190.0f;
                float py = sh / 2.0f - 170.0f;
                float pw = 380.0f;
                float ph = 340.0f;

                drawRect(uiShader, px - 2.0f, py - 2.0f, pw + 4.0f, ph + 4.0f, new byte[]{55, 55, 55, (byte) 255}, sw, sh);
                drawRect(uiShader, px, py, pw, ph, new byte[]{(byte) 139, 120, 100, (byte) 255}, sw, sh);
                drawRect(uiShader, px + 2.0f, py + 2.0f, pw - 4.0f, ph - 4.0f, new byte[]{(byte) 198, (byte) 182, (byte) 161, (byte) 255}, sw, sh);

                drawRect(uiShader, px + 6.0f, py + 158.0f, pw - 12.0f, 2.0f, new byte[]{85, 75, 65, (byte) 255}, sw, sh);
                drawRect(uiShader, px + 6.0f, py + 160.0f, pw - 12.0f, 1.0f, new byte[]{(byte) 220, (byte) 200, (byte) 180, (byte) 255}, sw, sh);

                float plx = px + 50.0f;
                float ply = py + 10.0f;
                drawRect(uiShader, plx, ply, 90.0f, 140.0f, new byte[]{85, 75, 65, 120}, sw, sh);

                drawRect(uiShader, plx + 20.0f, ply + 4.0f, 50.0f, 50.0f, new byte[]{(byte) 140, 95, 65, (byte) 255}, sw, sh);
                drawRect(uiShader, plx + 24.0f, ply + 14.0f, 14.0f, 10.0f, new byte[]{50, 50, 50, (byte) 180}, sw, sh);
                drawRect(uiShader, plx + 52.0f, ply + 14.0f, 14.0f, 10.0f, new byte[]{50, 50, 50, (byte) 180}, sw, sh);
                drawRect(uiShader, plx + 22.0f, ply + 56.0f, 46.0f, 40.0f, new byte[]{65, 95, (byte) 170, (byte) 255}, sw, sh);
                drawRect(uiShader, plx + 6.0f, ply + 56.0f, 14.0f, 38.0f, new byte[]{65, 95, (byte) 170, (byte) 255}, sw, sh);
                drawRect(uiShader, plx + 70.0f, ply + 56.0f, 14.0f, 38.0f, new byte[]{65, 95, (byte) 170, (byte) 255}, sw, sh);
                drawRect(uiShader, plx + 22.0f, ply + 98.0f, 20.0f, 40.0f, new byte[]{50, 60, (byte) 140, (byte) 255}, sw, sh);
                drawRect(uiShader, plx + 48.0f, ply + 98.0f, 20.0f, 40.0f, new byte[]{50, 60, (byte) 140, (byte) 255}, sw, sh);

                byte[][] armorColors = new byte[][] {
                    {(byte) 120, (byte) 140, (byte) 200, (byte) 180},
                    {(byte) 120, (byte) 140, (byte) 200, (byte) 180},
                    {(byte) 120, (byte) 140, (byte) 200, (byte) 180},
                    {(byte) 120, (byte) 140, (byte) 200, (byte) 180}
                };
                for (int i = 0; i < 4; i++) {
                    float[] r = slotRect(36 + i, px, py);
                    drawRect(uiShader, r[0], r[1], r[2], r[3], new byte[]{100, 88, 78, (byte) 255}, sw, sh);
                    drawRect(uiShader, r[0] + 2.0f, r[1] + 2.0f, r[2] - 4.0f, r[3] - 4.0f, new byte[]{75, 65, 58, (byte) 180}, sw, sh);
                    drawRect(uiShader, r[0] + 8.0f, r[1] + 8.0f, 20.0f, 20.0f, armorColors[i], sw, sh);
                    ItemStack s = player.invSlots[36 + i];
                    if (s != null && world.atlas != null) {
                        drawItemIcon(world.atlas, textureUiShader, s.block, r[0] + 4.0f, r[1] + 4.0f, r[2] - 8.0f, r[3] - 8.0f, sw, sh);
                    }
                }

                for (int i = 40; i < 44; i++) {
                    float[] r = slotRect(i, px, py);
                    float mx = (float) lastCursorX;
                    float my = (float) lastCursorY;
                    boolean hov = mx >= r[0] && mx < r[0] + r[2] && my >= r[1] && my < r[1] + r[3];
                    byte[] bg = hov ? new byte[]{(byte) 130, (byte) 115, (byte) 100, (byte) 255} : new byte[]{(byte) 100, (byte) 88, (byte) 78, (byte) 255};
                    drawRect(uiShader, r[0], r[1], r[2], r[3], bg, sw, sh);
                    drawRect(uiShader, r[0] + 2.0f, r[1] + 2.0f, r[2] - 4.0f, r[3] - 4.0f, new byte[]{75, 65, 58, (byte) 200}, sw, sh);
                    ItemStack s = player.invSlots[i];
                    if (s != null && world.atlas != null) {
                        drawItemIcon(world.atlas, textureUiShader, s.block, r[0] + 4.0f, r[1] + 4.0f, r[2] - 8.0f, r[3] - 8.0f, sw, sh);
                    }
                }

                drawRect(uiShader, px + 279.0f, py + 45.0f, 18.0f, 8.0f, new byte[]{85, 75, 65, (byte) 255}, sw, sh);
                drawRect(uiShader, px + 291.0f, py + 38.0f, 6.0f, 22.0f, new byte[]{85, 75, 65, (byte) 255}, sw, sh);

                {
                    float[] r = slotRect(44, px, py);
                    float mx = (float) lastCursorX;
                    float my = (float) lastCursorY;
                    boolean hov = mx >= r[0] && mx < r[0] + r[2] && my >= r[1] && my < r[1] + r[3];
                    byte[] bg = hov ? new byte[]{(byte) 145, (byte) 130, (byte) 110, (byte) 255} : new byte[]{(byte) 110, (byte) 98, (byte) 85, (byte) 255};
                    drawRect(uiShader, r[0] - 2.0f, r[1] - 2.0f, r[2] + 4.0f, r[3] + 4.0f, new byte[]{55, 45, 35, (byte) 255}, sw, sh);
                    drawRect(uiShader, r[0], r[1], r[2], r[3], bg, sw, sh);
                    ItemStack s = player.invSlots[44];
                    if (s != null && world.atlas != null) {
                        drawItemIcon(world.atlas, textureUiShader, s.block, r[0] + 4.0f, r[1] + 4.0f, r[2] - 8.0f, r[3] - 8.0f, sw, sh);
                    }
                }

                float mx = (float) lastCursorX;
                float my = (float) lastCursorY;
                for (int i = 9; i < 36; i++) {
                    float[] r = slotRect(i, px, py);
                    boolean hov = mx >= r[0] && mx < r[0] + r[2] && my >= r[1] && my < r[1] + r[3];
                    drawSlot(uiShader, r[0], r[1], r[2], player.invSlots[i], hov, sw, sh);
                }

                for (int i = 0; i < 9; i++) {
                    float[] r = slotRect(i, px, py);
                    boolean hov = mx >= r[0] && mx < r[0] + r[2] && my >= r[1] && my < r[1] + r[3];
                    if (player.selectedSlot == i) {
                        drawRect(uiShader, r[0] - 2.0f, r[1] - 2.0f, r[2] + 4.0f, r[3] + 4.0f, new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) 180}, sw, sh);
                    }
                    drawSlot(uiShader, r[0], r[1], r[2], player.invSlots[i], hov, sw, sh);
                }

                if (player.cursor != null && world.atlas != null) {
                    drawItemIcon(world.atlas, textureUiShader, player.cursor.block, mx - 14.0f, my - 14.0f, 28.0f, 28.0f, sw, sh);
                    int dots = (int) Math.clamp(Math.ceil(player.cursor.count / 8.0f), 1.0f, 8.0f);
                    float dsz = 4.0f;
                    float dgap = 2.0f;
                    float dw = dots * (dsz + dgap) - dgap;
                    for (int d = 0; d < dots; d++) {
                        drawRect(uiShader, mx - 14.0f + 28.0f - dw + d * (dsz + dgap), my + 8.0f, dsz, dsz, new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) 255}, sw, sh);
                    }
                }
            }

            // Health & Oxygen HUD
            if (!player.inventoryOpen) {
                float heartSize = 24.0f;
                float bubbleSize = 20.0f;
                float spacing = 4.0f;
                float wOff = (10.0f * (heartSize + spacing)) / 2.0f;

                float hxStart = sw / 2.0f - wOff - 80.0f;
                float hy = sh - 80.0f;
                for (int i = 0; i < 10; i++) {
                    float rx = hxStart + i * (heartSize + spacing);
                    int heartVal = Math.clamp(player.health - i * 2, 0, 2);
                    drawHeart(uiShader, rx, hy, heartSize, sw, sh, heartVal == 0);
                }

                boolean headInW = world.getBlock((int) Math.floor(eyePos.x), (int) Math.floor(eyePos.y + 0.6f), (int) Math.floor(eyePos.z)) == BlockType.WATER;
                if (headInW || player.airSeconds < 15.0f) {
                    float oxStart = sw / 2.0f + 80.0f;
                    int bubblesLeft = (int) Math.ceil(player.airSeconds / 1.5f);
                    for (int i = 0; i < 10; i++) {
                        float rx = oxStart + i * (bubbleSize + spacing);
                        drawBubble(uiShader, rx, hy, bubbleSize, sw, sh, i >= bubblesLeft);
                    }
                }
            }

            if (gameState == GameState.PAUSED) {
                drawPauseMenu(uiShader, textureUiShader, fontTexture, sw, sh);
            }

            GL11.glEnable(GL11.GL_DEPTH_TEST);
            GL11.glEnable(GL11.GL_CULL_FACE);
        }

        GLFW.glfwSwapBuffers(window);
    }

    private void drawSlot(Shader shader, float sx, float sy, float ss, ItemStack item, boolean hov, float sw, float sh) {
        byte[] bg = (hov && item != null) ? new byte[]{(byte) 145, (byte) 130, (byte) 110, (byte) 255} : new byte[]{(byte) 100, (byte) 88, (byte) 78, (byte) 255};
        drawRect(shader, sx, sy, ss, ss, bg, sw, sh);
        drawRect(shader, sx + 2.0f, sy + 2.0f, ss - 4.0f, ss - 4.0f, new byte[]{(byte) 75, (byte) 65, (byte) 55, (byte) 200}, sw, sh);
        if (item != null && world.atlas != null) {
            drawItemIcon(world.atlas, textureUiShader, item.block, sx + 4.0f, sy + 4.0f, ss - 8.0f, ss - 8.0f, sw, sh);

            int dots = (int) Math.clamp(Math.ceil(item.count / 8.0f), 1.0f, 8.0f);
            float dsz = 4.0f;
            float dgap = 2.0f;
            float dw = dots * (dsz + dgap) - dgap;
            float dx0 = sx + ss - dw - 3.0f;
            float dy0 = sy + ss - dsz - 3.0f;
            for (int d = 0; d < dots; d++) {
                drawRect(shader, dx0 + d * (dsz + dgap), dy0, dsz, dsz, new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) 200}, sw, sh);
            }
        }
    }

    public static void drawRect(Shader shader, float x, float y, float w, float h, byte[] color, float screenWidth, float screenHeight) {
        float[] v = new float[] {
            x, y, 0.0f,
            x + w, y, 0.0f,
            x + w, y + h, 0.0f,
            x, y, 0.0f,
            x + w, y + h, 0.0f,
            x, y + h, 0.0f
        };
        byte[] c = new byte[24];
        for (int i = 0; i < 6; i++) {
            c[i * 4]     = color[0];
            c[i * 4 + 1] = color[1];
            c[i * 4 + 2] = color[2];
            c[i * 4 + 3] = color[3];
        }
        Mesh mesh = new Mesh(v, null, null, c);
        shader.bind();
        shader.setVec2(shader.getUniformLocation("uScreenSize"), new Vector2f(screenWidth, screenHeight));
        mesh.draw();
        mesh.cleanup();
    }

    public static void drawTextureUiUv(Texture2D texture, float x, float y, float w, float h, float u, float v, float uw, float vh, Shader shader, float screenWidth, float screenHeight) {
        float[] verts = new float[] {
            x, y, 0.0f,
            x + w, y, 0.0f,
            x + w, y + h, 0.0f,
            x, y, 0.0f,
            x + w, y + h, 0.0f,
            x, y + h, 0.0f
        };
        float[] texs = new float[] {
            u, v,
            u + uw, v,
            u + uw, v + vh,
            u, v,
            u + uw, v + vh,
            u, v + vh
        };
        Mesh mesh = new Mesh(verts, texs, null, null);
        shader.bind();
        shader.setVec2(shader.getUniformLocation("uScreenSize"), new Vector2f(screenWidth, screenHeight));
        texture.bind(0);
        mesh.draw();
        mesh.cleanup();
    }

    public static void drawText(Texture2D texture, String text, float x, float y, float size, Shader shader, float sw, float sh) {
        float curX = x;
        for (int idx = 0; idx < text.length(); idx++) {
            char c = text.charAt(idx);
            int i = c;
            if (i < 32 || i >= 128) {
                continue;
            }
            int index = i - 32;
            float col = (index % 16);
            float row = (index / 16);
            float uw = 1.0f / 16.0f;
            float vh = 1.0f / 8.0f;
            drawTextureUiUv(texture, curX, y, size, size, col * uw, row * vh, uw, vh, shader, sw, sh);
            curX += size * 0.55f;
        }
    }

    public static void drawTextureQuad(Texture2D texture, Mesh quadMesh) {
        texture.bind(0);
        quadMesh.draw();
    }

    public static void drawScreenQuad(Shader shader, Vector4f color, Mesh quadMesh) {
        shader.bind();
        shader.setVec4(shader.getUniformLocation("uColor"), color);
        GL11.glDisable(GL11.GL_DEPTH_TEST);
        quadMesh.draw();
        GL11.glEnable(GL11.GL_DEPTH_TEST);
    }

    public static void drawSunMoon(Shader shader, Vector3f playerPos, float time, Matrix4f mvp) {
        float dist = 450.0f;
        float size = 60.0f;
        float angle = (time / 1200.0f) * 2.0f * (float) Math.PI;
        float sunY = (float) Math.sin(angle);
        Vector3f sunDir = new Vector3f(0.0f, sunY, (float) Math.cos(angle));
        Vector3f sunPos = new Vector3f(playerPos).add(new Vector3f(sunDir).mul(dist));

        float moonAngle = angle + (float) Math.PI;
        Vector3f moonDir = new Vector3f(0.0f, (float) Math.sin(moonAngle), (float) Math.cos(moonAngle));
        Vector3f moonPos = new Vector3f(playerPos).add(new Vector3f(moonDir).mul(dist));

        float sunriseMult = (sunY > -0.3f && sunY < 0.3f) ? 0.6f : 1.0f;
        byte[] sunColor = new byte[] {
            (byte) (255.0f * sunriseMult),
            (byte) (200.0f * sunriseMult),
            (byte) (150.0f * sunriseMult),
            (byte) 255
        };
        byte[] moonColor = new byte[] {
            (byte) 220, (byte) 225, (byte) 255, (byte) 255
        };

        Mesh sunMesh = createCelestialMesh(playerPos, sunPos, sunColor, size);
        Mesh moonMesh = createCelestialMesh(playerPos, moonPos, moonColor, size * 0.8f);

        shader.bind();
        shader.setMat4(shader.getUniformLocation("uMVP"), mvp);

        GL11.glDisable(GL11.GL_DEPTH_TEST);
        GL11.glDisable(GL11.GL_CULL_FACE);

        shader.setInt(shader.getUniformLocation("uBodyType"), 1); // 1 = Sun
        sunMesh.draw();

        shader.setInt(shader.getUniformLocation("uBodyType"), 2); // 2 = Moon
        moonMesh.draw();

        shader.setInt(shader.getUniformLocation("uBodyType"), 0);

        GL11.glEnable(GL11.GL_CULL_FACE);
        GL11.glEnable(GL11.GL_DEPTH_TEST);

        sunMesh.cleanup();
        moonMesh.cleanup();
    }

    private static Mesh createCelestialMesh(Vector3f playerPos, Vector3f pos, byte[] color, float qSize) {
        Vector3f look = new Vector3f(playerPos).sub(pos).normalize();
        Vector3f r = new Vector3f(look).cross(new Vector3f(0.0f, 1.0f, 0.0f)).normalize();
        Vector3f u = new Vector3f(r).cross(look).normalize();

        Vector3f v0 = new Vector3f(pos).add(new Vector3f(r).mul(-qSize)).add(new Vector3f(u).mul(-qSize));
        Vector3f v1 = new Vector3f(pos).add(new Vector3f(r).mul(qSize)).add(new Vector3f(u).mul(-qSize));
        Vector3f v2 = new Vector3f(pos).add(new Vector3f(r).mul(qSize)).add(new Vector3f(u).mul(qSize));
        Vector3f v3 = new Vector3f(pos).add(new Vector3f(r).mul(-qSize)).add(new Vector3f(u).mul(qSize));

        float[] v = new float[] {
            v0.x, v0.y, v0.z,
            v1.x, v1.y, v1.z,
            v2.x, v2.y, v2.z,
            v0.x, v0.y, v0.z,
            v2.x, v2.y, v2.z,
            v3.x, v3.y, v3.z
        };

        byte[] c = new byte[24];
        for (int i = 0; i < 6; i++) {
            c[i * 4]     = color[0];
            c[i * 4 + 1] = color[1];
            c[i * 4 + 2] = color[2];
            c[i * 4 + 3] = color[3];
        }

        float[] t = new float[] {
            0.0f, 0.0f,
            1.0f, 0.0f,
            1.0f, 1.0f,
            0.0f, 0.0f,
            1.0f, 1.0f,
            0.0f, 1.0f
        };

        return new Mesh(v, t, null, c);
    }

    public static void drawItemIcon(Texture2D atlas, Shader texShader, BlockType block, float x, float y, float w, float h, float sw, float sh) {
        int[] uv = block.getAtlasUV();
        float ts = 1.0f / 16.0f;
        float u = uv[0] * ts;
        float v = uv[1] * ts;
        drawTextureUiUv(atlas, x, y, w, h, u, v, ts, ts, texShader, sw, sh);
    }

    public static void drawHeart(Shader shader, float x, float y, float size, float screenWidth, float screenHeight, boolean empty) {
        byte[] color = empty ? new byte[]{(byte) 40, (byte) 40, (byte) 40, (byte) 200} : new byte[]{(byte) 220, (byte) 20, (byte) 20, (byte) 255};
        byte[] border = new byte[]{(byte) 20, (byte) 20, (byte) 20, (byte) 255};

        for (int px = 0; px < 9; px++) {
            for (int py = 0; py < 9; py++) {
                boolean fill = false;
                if (py == 0) {
                    fill = (px >= 1 && px <= 3) || (px >= 5 && px <= 7);
                } else if (py >= 1 && py <= 3) {
                    fill = (px >= 0 && px <= 8);
                } else if (py == 4) {
                    fill = (px >= 1 && px <= 7);
                } else if (py == 5) {
                    fill = (px >= 2 && px <= 6);
                } else if (py == 6) {
                    fill = (px >= 3 && px <= 5);
                } else if (py == 7) {
                    fill = (px == 4);
                }

                if (fill) {
                    float bx = x + (px / 9.0f) * size;
                    float by = y + (py / 9.0f) * size;
                    float bw = size / 9.0f;

                    boolean isBorder = false;
                    if (py == 0) {
                        isBorder = (px >= 1 && px <= 3) || (px >= 5 && px <= 7);
                    } else if (py == 1) {
                        isBorder = (px == 0 || px == 4 || px == 8);
                    } else if (py >= 2 && py <= 3) {
                        isBorder = (px == 0 || px == 8);
                    } else if (py == 4) {
                        isBorder = (px == 1 || px == 7);
                    } else if (py == 5) {
                        isBorder = (px == 2 || px == 6);
                    } else if (py == 6) {
                        isBorder = (px == 3 || px == 5);
                    } else if (py == 7) {
                        isBorder = (px == 4);
                    }

                    boolean isHighlight = !empty && ((px == 1 && py == 1) || (px == 2 && py == 1) || (px == 1 && py == 2));

                    byte[] pColor = isBorder ? border : (isHighlight ? new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) 255} : color);
                    drawRect(shader, bx, by, bw, bw, pColor, screenWidth, screenHeight);
                }
            }
        }
    }

    public static void drawBubble(Shader shader, float x, float y, float size, float screenWidth, float screenHeight, boolean empty) {
        byte[] color = empty ? new byte[]{(byte) 20, (byte) 20, (byte) 40, (byte) 150} : new byte[]{(byte) 60, (byte) 150, (byte) 255, (byte) 200};
        byte[] border = new byte[]{(byte) 20, (byte) 20, (byte) 40, (byte) 255};

        for (int px = 0; px < 8; px++) {
            for (int py = 0; py < 8; py++) {
                float distSq = (float) (Math.pow(px - 3.5f, 2) + Math.pow(py - 3.5f, 2));
                if (distSq <= 16.0f) {
                    float bx = x + (px / 8.0f) * size;
                    float by = y + (py / 8.0f) * size;
                    float bw = size / 8.0f;

                    boolean isBorder = distSq > 9.0f;
                    byte[] pColor = isBorder ? border : ((px == 2 && py == 2 && !empty) ? new byte[]{(byte) 255, (byte) 255, (byte) 255, (byte) 255} : color);
                    drawRect(shader, bx, by, bw, bw, pColor, screenWidth, screenHeight);
                }
            }
        }
    }

    public static void drawPauseMenu(Shader uiShader, Shader texShader, Texture2D fontTex, float sw, float sh) {
        drawRect(uiShader, 0.0f, 0.0f, sw, sh, new byte[]{0, 0, 0, (byte) 160}, sw, sh);

        float btnW = 340.0f;
        float btnH = 44.0f;
        float btnX = 100.0f;
        float btnYStart = sh / 2.0f - 100.0f;

        String[] labels = new String[]{"Resume Game", "Settings", "Marketplace", "Save & Quit"};
        for (int i = 0; i < labels.length; i++) {
            float y = btnYStart + i * (btnH + 12.0f);
            drawRect(uiShader, btnX - 2.0f, y - 2.0f, btnW + 4.0f, btnH + 4.0f, new byte[]{20, 20, 20, (byte) 255}, sw, sh);
            drawRect(uiShader, btnX, y, btnW, btnH, new byte[]{(byte) 198, (byte) 198, (byte) 198, (byte) 255}, sw, sh);
            drawRect(uiShader, btnX + 2.0f, y + 2.0f, btnW - 4.0f, btnH - 4.0f, new byte[]{110, 110, 110, (byte) 255}, sw, sh);

            float textSz = 18.0f;
            float textX = btnX + (btnW - labels[i].length() * textSz * 0.55f) / 2.0f;
            drawText(fontTex, labels[i], textX, y + 13.0f, textSz, texShader, sw, sh);
        }
    }

    public static float[] slotRect(int slot, float px, float py) {
        float ss = 36.0f;
        float st = 38.0f;
        if (slot >= 0 && slot <= 8) {
            return new float[]{px + 10.0f + slot * st, py + 294.0f, ss, ss};
        } else if (slot >= 9 && slot <= 35) {
            int i = slot - 9;
            return new float[]{
                px + 10.0f + (i % 9) * st,
                py + 168.0f + (i / 9) * st,
                ss,
                ss
            };
        } else if (slot >= 36 && slot <= 39) {
            return new float[]{px + 10.0f, py + 10.0f + (slot - 36) * st, ss, ss};
        } else if (slot >= 40 && slot <= 43) {
            int i = slot - 40;
            return new float[]{
                px + 195.0f + (i % 2) * st,
                py + 28.0f + (i / 2) * st,
                ss,
                ss
            };
        } else if (slot == 44) {
            return new float[]{px + 304.0f, py + 42.0f, ss, ss};
        }
        return new float[]{0.0f, 0.0f, 0.0f, 0.0f};
    }

    public static Integer slotAtPos(float mx, float my, float px, float py) {
        for (int s = 0; s < 45; s++) {
            if (s >= 36 && s <= 39) continue;
            float[] r = slotRect(s, px, py);
            if (mx >= r[0] && mx < r[0] + r[2] && my >= r[1] && my < r[1] + r[3]) {
                return s;
            }
        }
        return null;
    }

    public static float[] ctSlotRect(int slot, float px, float py) {
        float ss = 36.0f;
        float st = 38.0f;
        if (slot >= 0 && slot <= 8) {
            int col = slot % 3;
            int row = slot / 3;
            return new float[]{
                px + 16.0f + col * st,
                py + 18.0f + row * st,
                ss,
                ss
            };
        } else if (slot == 9) {
            return new float[]{px + 200.0f, py + 52.0f, ss, ss};
        } else if (slot >= 100 && slot <= 126) {
            int i = slot - 100;
            return new float[]{
                px + 10.0f + (i % 9) * st,
                py + 168.0f + (i / 9) * st,
                ss,
                ss
            };
        } else if (slot >= 127 && slot <= 135) {
            int i = slot - 127;
            return new float[]{px + 10.0f + i * st, py + 294.0f, ss, ss};
        }
        return new float[]{0.0f, 0.0f, 0.0f, 0.0f};
    }

    public static Integer ctSlotAtPos(float mx, float my, float px, float py) {
        for (int s = 0; s <= 9; s++) {
            float[] r = ctSlotRect(s, px, py);
            if (mx >= r[0] && mx < r[0] + r[2] && my >= r[1] && my < r[1] + r[3]) {
                return s;
            }
        }
        for (int s = 100; s <= 126; s++) {
            float[] r = ctSlotRect(s, px, py);
            if (mx >= r[0] && mx < r[0] + r[2] && my >= r[1] && my < r[1] + r[3]) {
                return s;
            }
        }
        for (int s = 127; s <= 135; s++) {
            float[] r = ctSlotRect(s, px, py);
            if (mx >= r[0] && mx < r[0] + r[2] && my >= r[1] && my < r[1] + r[3]) {
                return s;
            }
        }
        return null;
    }

    public static class WindowState {
        public int width = 1280;
        public int height = 720;
        public int x = 100;
        public int y = 100;
        public boolean maximized = false;

        public static WindowState load() {
            WindowState state = new WindowState();
            java.io.File file = new java.io.File("window.cfg");
            if (file.exists()) {
                try (java.io.BufferedReader br = new java.io.BufferedReader(new java.io.FileReader(file))) {
                    String line;
                    while ((line = br.readLine()) != null) {
                        String[] parts = line.split(":");
                        if (parts.length == 2) {
                            String key = parts[0].trim();
                            String value = parts[1].trim();
                            switch (key) {
                                case "width" -> state.width = Integer.parseInt(value);
                                case "height" -> state.height = Integer.parseInt(value);
                                case "x" -> state.x = Integer.parseInt(value);
                                case "y" -> state.y = Integer.parseInt(value);
                                case "maximized" -> state.maximized = Boolean.parseBoolean(value);
                            }
                        }
                    }
                } catch (Exception e) {
                    // ignore
                }
            }
            return state;
        }

        public void save() {
            try (java.io.PrintWriter pw = new java.io.PrintWriter(new java.io.FileWriter("window.cfg"))) {
                pw.println("width:" + width);
                pw.println("height:" + height);
                pw.println("x:" + x);
                pw.println("y:" + y);
                pw.println("maximized:" + maximized);
            } catch (Exception e) {
                // ignore
            }
        }
    }
}
