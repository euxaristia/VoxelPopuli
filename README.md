# VoxelPopuli 🧱

A lightweight Minecraft-inspired voxel sandbox written in Rust with `wgpu` and GPU compute meshing.

## Highlights ✨
- **wgpu Renderer**: Modern GPU-accelerated rendering pipeline with WebGPU standard support
- **Survival Mechanics**: Health, Hunger/Food, Saturation, Armor Defense, Oxygen, and Starvation
- **Mob System**: Natural spawning & AI for Villagers, Iron Golems, Zombies, Skeletons, Creepers, Pigs, Cows, and Sheep
- **Village Generation**: Procedural villages with core structures, pathways, and mob spawns
- **Crafting & Smelting**: 3x3 Crafting Table, Furnace smelting, Chests, Farmland farming, and 100+ items/blocks
- **Explosions & TNT**: Chain-reaction ignition, blast shockwaves, and 3D bounce physics
- **Terrain & Environment**: Procedural biomes, Perlin noise heightmaps, Minecraft 1.0 WorldGenMinable ore veins, dynamic clouds, celestial bodies (Sun, Moon, Stars), and smooth ambient occlusion lighting
- **Java Anvil Export**: Export worlds directly to Minecraft Java Anvil `.mca` format (Y 0..=255)
- **Java World Import**: Load Minecraft Java 1.16-1.17 worlds (Y 0..=255, Anvil `.mca` + NBT) directly into the game
- **Inventory System**: Hotbar and full inventory grid with stack counts, durability bars, procedural item icons, and Bedrock-style linear block placement
- **Combat**: Bow & Arrow ranged projectiles, melee attacks, and Experience Orbs with XP bar
- **Performance Telemetry**: Real-time F3 debug overlay with frame time percentiles (P95/P99), subsystem breakdowns, and latency histogram

## Stack 🛠️
- [Rust](https://www.rust-lang.org/) (2024 Edition)
- [wgpu](https://wgpu.rs/) (WebGPU-native cross-platform graphics API)
- [GLFW](https://www.glfw.org/) (Windowing & Input via `glfw-rs`)
- [glam](https://github.com/bit_shifter/glam-rs) (Linear Algebra)
- [mimalloc](https://github.com/purplecabbage/mimalloc-rust) (High-performance global allocator)

## Build ⚙️

Dependencies: standard C compiler and GPU drivers supporting Vulkan/DirectX 12/Metal.

```bash
cargo build --release
```

## Run 🚀

Always run in release mode for maximum chunk-streaming and meshing performance:

```bash
cargo run --release
```

New worlds start in survival with an empty inventory. Gather logs, craft planks
and a crafting table, make wooden tools, mine cobblestone, then build a furnace
to turn ore into ingots and raw meat into cooked food.

The game resumes `world.vps` automatically and saves on exit. To keep a separate
survival world, use the same save path each time:

```bash
cargo run --release -- --save survival.vps
```

If an older build placed you in a dark cave at startup, add `--reset-spawn` once
to move to the surface above your saved location. This keeps inventory and world
edits; the new position is saved on exit:

```bash
cargo run --release -- --save survival.vps --reset-spawn
```

For the original starter kit and double-tap flight controls in a separate world:

```bash
cargo run --release -- --sandbox --save sandbox.vps
```

Existing saves retain their inventory and original flight controls. Save version
2 also stores chest contents, furnace fuel/progress, dropped items, cursor and
crafting-table items, hunger exhaustion, and time of day. Version 1 saves remain
readable; older game binaries cannot read version 2 saves. An unreadable save
stops startup rather than being replaced with a new world. `--seed` starts a new
world, so use a different `--save` path to keep your current world.

Chests hold 27 stacks. Furnaces have input, fuel, and output slots; one item takes
10 seconds to smelt, and one coal burns for 80 seconds. They continue cooking
while closed during play; pausing freezes the simulation. Shift-click transfers
stacks and right-click splits them. Broken containers release their contents.
Items that do not fit in your inventory remain on the ground and are saved.
Beds set your respawn point and advance night to morning. Death currently keeps
your inventory and reloads terrain around your spawn before resuming.

## Export Minecraft Java World 🌍

Export generated terrain to a classic Java Anvil world folder (`16x16x256` chunks, Y `0..255`):

```bash
cargo run --release -- --export-java17 ./my-java-world --export-radius 4 --seed 12345
```

## Import Minecraft Java World 📥

Load a real Minecraft Java edition world save into VoxelPopuli:

```bash
cargo run --release -- --import-world path/to/saves/MyWorld
```

The importer reads Anvil `.mca` region files, decompresses NBT chunk data, unpacks packed block states, and maps Java block palettes to VoxelPopuli block types.

## Controls 🎮
- `W A S D`: Move
- `Mouse`: Look
- `Space`: Jump / Swim up (Double-tap in sandbox worlds for Flight)
- `Left Ctrl`: Sprint (requires Hunger > 6)
- `Shift`: Sneak / Swim down
- `Left Click`: Break block / Mine / Attack
- `Right Click`: Place block / Eat food / Equip armor / Open crafting tables, chests, and furnaces / Use beds & TNT
- `Q` / `Ctrl+Q`: Drop one item / Drop the selected stack (or cursor item in an open inventory)
- `1 - 9` / `Scroll`: Select hotbar slot
- `E`: Open Inventory / Crafting menu
- `F3`: Toggle debug HUD & performance telemetry overlay
- `Esc`: Pause menu / Settings

## Project Layout 📁
- `src/main.rs`: Entry point, windowing, game loop, input handling, and pause menu.
- `src/renderer.rs`: `wgpu` render pipeline abstractions, shaders, textures, and compute meshing.
- `src/world.rs`: World state, chunk streaming, background meshing, cloud rendering, mobs, and explosion physics.
- `src/chunk.rs`: Procedural terrain generation, biome heightmaps, Minecraft 1.0 ore vein algorithm, and block state meshing.
- `src/block.rs`: 100+ `BlockType` variants (blocks, materials, tools, food, armor, combat).
- `src/item.rs`: Centralized block properties, tool stats, food properties, armor defense, and atlas UV mappings.
- `src/inventory.rs`: Inventory grid, hotbar management, stack counts, durability tracking, and Bedrock-style linear placement lock.
- `src/container.rs`: Persistent chest slots, furnace simulation, and container transfers.
- `src/container_ui.rs`: Chest and furnace screens.
- `src/save.rs`: Versioned saves and atomic replacement of saved progress.
- `src/crafting.rs`: 3x3 shaped/shapeless crafting database, furnace smelting, and fuel burn times.
- `src/mob.rs`: Mob entities, dimensions, wander AI, loot drops, and models.
- `src/village.rs`: Procedural village layout generation and structure stamping.
- `src/mining.rs`: Block mining progress and crack stage rendering.
- `src/explosion.rs`: TNT ignition, blast shockwaves, and chain-reaction physics.
- `src/java_compat.rs`: Anvil MCA world format export/import, NBT serialization/deserialization, and block palette mapping.
- `src/atlas.rs`: Procedural texture atlas generation with per-block and per-item pixel art.
- `src/hud.rs`: HUD rendering for health, hunger, armor, oxygen, XP bar, and hotbar.
- `src/player.rs`: Player physics, collision detection, and movement.
- `src/noise.rs`: Perlin noise implementation for terrain generation.
- `src/profiler.rs`: Real-time frame profiler & performance telemetry.

## Survival verification

```bash
cargo test --locked --offline
cargo build --release --locked --offline
cargo run --release --locked --offline -- --smoke-test-ui
```

The smoke test renders the production chest and furnace screens through wgpu
in a hidden test window, writes screenshots to `target/test-artifacts/`, and
exits without opening or changing a world save. See
[the survival playtest checklist](docs/survival-playtest.md) for interactive checks.

## Lighting and first-person rendering

Fancy graphics uses linear-color lighting, ACES tone mapping, directional sun
and moon light, and subtle distance haze. Water has a muted blue-green tint,
view-dependent sky reflection, and animated normals that leave block edges in
place. Empty clicks punch, holding attack repeats the swing, and the arm extends
past the bottom of the viewport throughout the animation.

The renderer takes inspiration from Microsoft's
[Vibrant Visuals lighting](https://learn.microsoft.com/en-us/minecraft/creator/documents/vibrantvisuals/lightingcustomization?view=minecraft-bedrock-stable),
[atmosphere](https://learn.microsoft.com/en-us/minecraft/creator/documents/vibrantvisuals/atmosphericscustomization?view=minecraft-bedrock-stable),
and [water](https://learn.microsoft.com/en-us/minecraft/creator/documents/vibrantvisuals/watercustomization?view=minecraft-bedrock-stable)
documentation. It remains a custom implementation: shadow maps, screen-space
reflections, volumetric light shafts, and particle-concentration water settings
are not implemented. Haze is an analytic approximation; water reflection samples
the sky color rather than nearby geometry.

```bash
cargo run --release --locked --offline -- --smoke-test-hand
cargo run --release --locked --offline -- --smoke-test-lighting
```

These hidden-window tests leave saves unchanged and capture the actual presented
frames under `target/test-artifacts/`. They exercise press, release, and held
attack in Fancy and Fast modes; noon, sunset, midnight, and sunrise; and GPU
probes for color conversion, distance haze, cave visibility, hidden sun glare,
cloud tint, and stars being occluded by terrain.

## License 📜
Released into the **Public Domain** (The Unlicense). See [LICENSE](./LICENSE).
