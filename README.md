# VoxelPopuli

A voxel sandbox written in Rust. Procedural terrain, survival gameplay, 77 creature species, and a deferred lighting renderer, all running on wgpu. Plays in the browser via WebAssembly or natively on Windows, Linux, and macOS.

**[Play in browser](https://euxaristia.github.io/VoxelPopuli/)** (WebGPU required)

## Highlights

- **Deferred wgpu renderer** with G-buffer geometry, HDR lighting, ACES tone mapping, ambient occlusion, and distance haze. Directional sun/moonlight, celestial bodies, dynamic clouds, moon phases, and animated water with sky reflection. 12 WGSL shaders.
- **Survival loop**: health, hunger, saturation, armor defense, oxygen, starvation, sprinting exhaustion, beds, and respawn. Gather, craft, smelt, farm, fight.
- **77 Overworld creatures** with original pixel textures, articulated models, habitat spawning, swimming, flight, skeleton archery AI, bee pollination, and a sandbox catalogue. See [creature coverage](docs/creatures.md).
- **Procedural world**: biomes, Perlin noise heightmaps, Minecraft 1.0 ore veins, villages with pathways and mob spawns, caves, trees (oak, birch, mangrove, cherry), and bee habitats.
- **158 block/item types**: blocks, ores, tools, weapons, food, armor, redstone, pistons, doors, beds, farmland, fluids, fire, and TNT.
- **Crafting and containers**: 3x3 crafting table, furnace smelting, chests (27 slots), farming, shift-click transfers, stack splitting, and item drops.
- **Combat**: melee attacks with damage animation, bow and arrow projectiles, experience orbs with XP bar, and creeper/TNT explosions with chain reactions and blast physics.
- **Inventory**: hotbar and full grid with stack counts, durability bars, procedural item icons, and Bedrock-style linear block placement.
- **Bedrock-format saves**: native LevelDB world directory with background chunk streaming and atomic persistence. Legacy `.vps` saves migrate automatically.
- **Java interop**: export to Anvil `.mca` (Y 0..255) and import Java 1.16-1.17 worlds with NBT decoding and block palette mapping.
- **WebAssembly**: same Rust codebase compiled to `wasm32-unknown-unknown`, deployed to GitHub Pages via CI. Browser saves persist in IndexedDB.
- **Performance telemetry**: F3 debug overlay with frame time percentiles (P95/P99), subsystem breakdowns, and latency histogram.
- **Camera modes**: first-person, rear third-person, and front third-person (F5 cycle), with arm/held-item depth pass.

## Stack

- [Rust](https://www.rust-lang.org/) 2024 edition (~46K lines)
- [wgpu](https://wgpu.rs/) (WebGPU-native graphics, Vulkan/DX12/Metal/OpenGL/WebGPU backends)
- [GLFW](https://www.glfw.org/) (desktop windowing via `glfw-rs`; browser uses `web-sys`)
- [glam](https://github.com/bit_shifter/glam-rs) (linear algebra)
- [mimalloc](https://github.com/purplecabbage/mimalloc-rust) (global allocator, desktop only)
- [rayon](https://github.com/rayon-rs/rayon) (parallel chunk meshing, desktop only)
- [bedrock-leveldb](https://crates.io/crates/bedrock-leveldb) (native world storage)

## Build

Standard C compiler and GPU drivers supporting Vulkan, DirectX 12, Metal, or OpenGL.

```bash
cargo build --release
```

Linux needs GLFW/OpenGL system libraries:

```bash
sudo apt-get install libgl1-mesa-dev libx11-dev libxcursor-dev libxinerama-dev libxrandr-dev libxi-dev libwayland-dev wayland-protocols libxkbcommon-dev
```

### WebAssembly

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked
python scripts/build-web.py
python -m http.server 8080 --bind 127.0.0.1 --directory target/web
```

See [browser build, deployment, and save guide](docs/web.md).

## Run

Always use release mode for chunk streaming and meshing performance:

```bash
cargo run --release
```

New worlds start in survival with an empty inventory. Gather logs, craft planks and a crafting table, make wooden tools, mine cobblestone, then build a furnace to turn ore into ingots and raw meat into cooked food.

Desktop builds use a native Bedrock world directory (`world/`) and save player progress on exit. Generated and displaced chunks are persisted while streaming. Storage waits run on workers; outgoing chunks stay loaded until saved. See [chunk streaming and profiling](docs/chunk-streaming.md) and [Bedrock saves and compatibility limits](docs/bedrock.md). Browser builds retain their existing browser save format. To keep a separate survival world:

```bash
cargo run --release -- --save survival
```

If an older build placed you in a dark cave at startup, add `--reset-spawn` once to move to the surface above your saved location. This keeps inventory and world edits; the new position is saved on exit:

```bash
cargo run --release -- --save survival --reset-spawn
```

For the original starter kit and double-tap flight controls in a separate world:

```bash
cargo run --release -- --sandbox --save sandbox
```

Legacy version 1-3 `.vps` saves remain readable and are retained during migration. Current browser saves and desktop supplemental session records use version 7, with 16-bit block/item IDs. This adds 64 KiB to each loaded chunk's block array; the optional GPU voxel pool also uses 64 KiB more per chunk slot. New worlds include seven additional bee habitats and birch, mangrove, and cherry trees. Existing worlds keep their original generator. See [world generation and compatibility](docs/world-generation.md). The default `world.vps` migrates to `world/`; an explicit `--save survival.vps` migrates to `survival.bedrock/`. An unreadable save stops startup. `--seed` and `--import-world` require a new native save directory when one already exists.

Chests hold 27 stacks. Furnaces have input, fuel, and output slots; one item takes 10 seconds to smelt, and one coal burns for 80 seconds. They continue cooking while closed during play; pausing freezes the simulation. Shift-click transfers stacks and right-click splits them. Broken containers release their contents. Items that do not fit in your inventory remain on the ground and are saved. Beds set your respawn point and advance night to morning. Death currently keeps your inventory and reloads terrain around your spawn before resuming.

## Export Minecraft Java world

Export generated terrain to a classic Java Anvil world folder (16x16x256 chunks, Y 0..255):

```bash
cargo run --release -- --export-java17 ./my-java-world --export-radius 4 --seed 12345
```

## Import Minecraft Java world

Load a Minecraft Java edition world save into VoxelPopuli:

```bash
cargo run --release -- --import-world path/to/saves/MyWorld
```

The importer reads Anvil `.mca` region files, decompresses NBT chunk data, unpacks packed block states, and maps Java block palettes to VoxelPopuli block types.

## Controls

| Input | Action |
|---|---|
| W A S D | Move |
| Mouse | Look |
| Space | Jump, swim up (double-tap for flight in sandbox) |
| Left Ctrl / double-tap W | Sprint (requires hunger > 6 in survival) |
| Shift | Sneak, swim down |
| Left click | Break block, mine, attack |
| Right click | Place block, eat, equip armor, open containers, use beds and TNT |
| Q / Ctrl+Q | Drop one item / drop the selected stack |
| 1-9 / Scroll | Select hotbar slot |
| E | Inventory and crafting menu |
| F3 | Debug HUD and performance telemetry |
| F5 | Cycle camera: first-person, rear, front third-person |
| F6 | Creature catalogue (sandbox) |
| Esc | Pause menu and settings |

Sprinting continues while moving forward and stops when forward input is released, you sneak, hit a wall, open a menu, draw a bow, or run low on food. Gamepads use left-stick click or a quick double push forward. Sprinting and jumping consume exhaustion, which drains saturation before hunger; eating replenishes food. See [sprinting and hunger](docs/sprinting.md).

## Project layout

| Module | Purpose |
|---|---|
| `src/main.rs` | Entry point, game loop, input, pause menu |
| `src/renderer.rs` | wgpu pipeline, deferred G-buffer, compute meshing, shaders |
| `src/world.rs` | World state, chunk streaming, clouds, mobs, explosions |
| `src/world/streaming.rs` | Background chunk load/save and mesh workers |
| `src/world/mobs.rs` | Spawn validation, natural encounters, summoning |
| `src/world/bees.rs` | Bee pollination and hive mechanics |
| `src/world/skeletons.rs` | Skeleton archery AI goals |
| `src/chunk.rs` | Terrain generation, biomes, ore veins, meshing |
| `src/block.rs` | 158 `BlockType` variants |
| `src/item.rs` | Block properties, tool stats, food, armor, atlas UVs |
| `src/inventory.rs` | Inventory grid, hotbar, stacks, durability, placement lock |
| `src/container.rs` | Chest slots, furnace simulation, transfers |
| `src/container_ui.rs` | Chest and furnace screens |
| `src/save.rs` | Versioned saves, atomic replacement |
| `src/crafting.rs` | Shaped/shapeless recipes, smelting, fuel burn times |
| `src/mob.rs` | Mob state, dimensions, combat, loot, animation |
| `src/mob_catalog.rs` | 77 species definitions, habitats |
| `src/mob_visuals.rs` | Pixel textures, articulated cuboid models |
| `src/creature_ui.rs` | Sandbox creature catalogue |
| `src/village.rs` | Procedural village layout and structure stamping |
| `src/mining.rs` | Mining progress and crack rendering |
| `src/explosion.rs` | TNT ignition, blast waves, chain reactions |
| `src/combat_animation.rs` | Melee and damage animations |
| `src/skeleton_ai.rs` | Skeleton combat behavior |
| `src/sprint.rs` | Sprint mechanics and exhaustion |
| `src/hand.rs` | First-person arm and held-item rendering |
| `src/camera.rs` | First/third-person camera logic |
| `src/celestial.rs` | Sun, moon, stars, moon phases |
| `src/fire.rs` | Fire spread simulation |
| `src/bee.rs` | Bee state and pollination |
| `src/player.rs` | Player physics, collision, movement |
| `src/hud.rs` | Health, hunger, armor, oxygen, XP bar, hotbar |
| `src/java_compat.rs` | Anvil export/import, NBT, block palette mapping |
| `src/atlas.rs` | Procedural texture atlas, per-block pixel art |
| `src/noise.rs` | Perlin noise |
| `src/profiler.rs` | Frame profiler and telemetry |
| `src/platform.rs` | Platform abstraction (desktop/WASM) |
| `src/web.rs`, `src/web_window.rs` | Browser-specific windowing and event loop |
| `src/smoke.rs` | Smoke-test harness |
| `src/png_io.rs` | PNG read/write |
| `assets/shaders/*.wgsl` | 12 WGSL shaders (G-buffer, deferred lighting, water, celestial, tone mapping, UI) |

## Verification

```bash
cargo test --locked --offline
cargo build --release --locked --offline
cargo run --release --locked --offline -- --smoke-test-ui
```

The smoke test renders the production chest and furnace screens through wgpu in a hidden test window, writes screenshots to `target/test-artifacts/`, and exits without opening or changing a world save. See [the survival playtest checklist](docs/survival-playtest.md) for interactive checks.

## Rendering

PNG assets and screenshots use `png` directly, with no general-purpose image processing dependency. wgpu enables native DirectX 12, Vulkan, Metal and OpenGL backends plus WGSL; the browser build uses WebGPU.

Fancy graphics uses linear-color lighting, ACES tone mapping, and sun/moon textures and lighting curves from the Bedrock Vibrant Visuals samples. See [celestial rendering and sneaking](docs/celestials-and-sneaking.md) for the reference sources, camera/pose behavior, moon phases and parity limits. It also provides directional sunlight, moonlight, and subtle distance haze. Water has a muted blue-green tint, view-dependent sky reflection, and animated normals that leave block edges in place. Empty clicks punch, holding attack repeats the swing, and the arm extends past the bottom of the viewport throughout the animation. Its square forearm points forward, and a separate depth pass keeps the arm and held item visible against nearby terrain in both graphics modes.

The renderer implements selected features from Microsoft's [Vibrant Visuals lighting](https://learn.microsoft.com/en-us/minecraft/creator/documents/vibrantvisuals/lightingcustomization?view=minecraft-bedrock-stable), [atmosphere](https://learn.microsoft.com/en-us/minecraft/creator/documents/vibrantvisuals/atmosphericscustomization?view=minecraft-bedrock-stable), and [water](https://learn.microsoft.com/en-us/minecraft/creator/documents/vibrantvisuals/watercustomization?view=minecraft-bedrock-stable) schemas: `water/water.json` supports bio-optical particle concentrations (CDOM, chlorophyll, suspended sediment) blended with Bedrock surface biome colors, frequency- and pull-controlled multi-octave waves, and procedural dynamic underwater caustics. Several wave parameters and `caustics.texture` remain unsupported. Shadow maps, screen-space reflections, and volumetric light shafts remain future work. Haze is an analytic approximation; water reflection samples the sky color rather than nearby geometry.

```bash
cargo run --release --locked --offline -- --smoke-test-hand
cargo run --release --locked --offline -- --smoke-test-lighting
```

These hidden-window tests leave saves unchanged and capture the actual presented frames under `target/test-artifacts/`. They exercise press, release, and held attack in Fancy and Fast modes; arm and held-item visibility against a near-plane wall; noon, sunset, midnight, and sunrise; and GPU probes for color conversion, distance haze, cave visibility, hidden sun glare, cloud tint, and stars being occluded by terrain.

## Direction

Current development focuses on closing the gap with Minecraft Bedrock survival gameplay. Near-term priorities:

- **Redstone and mechanics**: expanding signal propagation, repeaters, comparators, and observer blocks beyond the current wiring, torches, lamps, and pistons.
- **The Nether**: portal construction, nether terrain generation, nether-specific mobs and biomes.
- **Multiplayer**: networked sessions over WebRTC or WebSocket for both desktop and browser builds.
- **Audio**: spatial sound effects for block interactions, mob sounds, ambient music, and weather.
- **Weather**: rain, snow, thunder, and lightning with gameplay effects.
- **Enchanting**: enchantment table, anvil combining, and enchantment effects on tools and armor.
- **Renderer**: shadow maps, screen-space reflections, and volumetric fog.

## License

Released into the public domain (The Unlicense). See [LICENSE](./LICENSE).
