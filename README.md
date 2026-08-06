# VoxelPopuli 🧱

A lightweight Minecraft-inspired voxel sandbox written in Rust with `wgpu` and GPU compute meshing.

## Highlights ✨
- **wgpu Renderer**: Modern GPU-accelerated rendering pipeline with WebGPU standard support
- **Survival Mechanics**: Health, Hunger/Food, Saturation, Armor Defense, Oxygen, and Starvation
- **Mob System**: Natural spawning & AI for Villagers, Iron Golems, Zombies, Skeletons, Creepers, Pigs, Cows, and Sheep
- **Village Generation**: Procedural villages with core structures, pathways, and mob spawns
- **Crafting & Smelting**: 3x3 Crafting Table, Furnace smelting, Chests, Farmland farming, and 100+ items/blocks
- **Explosions & TNT**: Chain-reaction ignition, blast shockwaves, and 3D bounce physics
- **Terrain & Environment**: Procedural biomes, Perlin noise heightmaps, dynamic clouds, celestial bodies (Sun, Moon, Stars), and smooth ambient occlusion lighting
- **Java Anvil Exporter**: Export worlds directly to Minecraft Java Anvil `.mca` format (Y 0..255)
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

Export generated terrain to a classic Java Anvil world folder (`16×16×256` chunks, Y `0..255`):

```bash
cargo run --release -- --export-java17 ./java17-world --export-radius 4 --seed 12345
```

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
- `src/main.rs`: Entry point, windowing, player physics, survival status, UI & HUD rendering.
- `src/renderer.rs`: `wgpu` render pipeline abstractions, shaders, textures, and compute meshing.
- `src/world.rs`: World state, chunk streaming, background meshing, cloud rendering, mobs, and explosion physics.
- `src/chunk.rs`: Procedural terrain generation, biome heightmaps, and block state meshing.
- `src/block.rs`: 100+ `BlockType` variants (blocks, materials, tools, food, armor, combat).
- `src/item.rs`: Centralized block properties, tool stats, food properties, armor defense, and atlas UV mappings.
- `src/crafting.rs`: 3x3 shaped/shapeless crafting database, furnace smelting, and fuel burn times.
- `src/mob.rs`: Mob entities, dimensions, wander AI, loot drops, and models.
- `src/village.rs`: Procedural village layout generation and structure stamping.
- `src/mining.rs`: Block mining progress and crack stage rendering.
- `src/explosion.rs`: TNT ignition, blast shockwaves, and chain-reaction physics.
- `src/java_compat.rs`: Anvil MCA world format export & NBT serialization.
- `src/profiler.rs`: Real-time frame profiler & performance telemetry.

## License 📜
Released into the **Public Domain** (The Unlicense). See [LICENSE](./LICENSE).

