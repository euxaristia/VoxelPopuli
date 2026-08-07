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
- **Java Anvil Export**: Export worlds directly to Minecraft Java Anvil `.mca` format (Y 0..255)
- **Java World Import**: Load real Minecraft Java edition world saves (Anvil `.mca` + NBT) directly into the game
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
- `Space`: Jump / Swim up (Double-tap in creative for Flight)
- `Left Ctrl`: Sprint (requires Hunger > 6)
- `Shift`: Sneak / Swim down
- `Left Click`: Break block / Mine / Attack
- `Right Click`: Place block / Eat food / Equip armor / Interact with Crafting Table & TNT
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

## License 📜
Released into the **Public Domain** (The Unlicense). See [LICENSE](./LICENSE).
