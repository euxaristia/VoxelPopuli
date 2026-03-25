# VoxelPopuli 🧱

A lightweight Minecraft-inspired voxel sandbox written in Rust with OpenGL.

## Highlights ✨
- Infinite-style chunk streaming around the player
- Procedural terrain and trees via Perlin noise
- Dynamic cloud and celestial rendering (Sun, Moon, Stars)
- PS1-style vertex snapping and visual aesthetic
- Procedural texture atlas generation
- Block placing and mining
- Survival physics: gravity, voxel collision, swimming, and jumping
- High performance via optimized Rust noise and mesh generation

## Stack 🛠️
- [Rust](https://www.rust-lang.org/) (2021 Edition)
- [OpenGL 3.3 Core](https://www.opengl.org/)
- [GLFW](https://www.glfw.org/) (Windowing & Input)
- [glam](https://github.com/bit_shifter/glam-rs) (Linear Algebra)

## Build

You will need system dependencies for GLFW and OpenGL:
- **Linux**: `libgl1-mesa-dev libx11-dev libxcursor-dev libxinerama-dev libxrandr-dev libxi-dev libwayland-dev wayland-protocols libxkbcommon-dev`

```bash
cargo build --release
```

## Run 🚀

For the best performance and to avoid visual artifacts, always run in release mode:

```bash
cargo run --release
```

## Controls 🎮
- `W A S D`: Move
- `Mouse`: Look
- `Space`: Jump / swim up
- `Left Ctrl`: Sprint
- `Left Click`: Break block
- `Right Click`: Place block (Stone)
- `Esc`: Quit

## Project Layout 📁
- `src/main.rs`: Entry point, window management, player physics, and celestial rendering.
- `src/renderer.rs`: OpenGL abstractions for Shaders, Textures, and Meshes.
- `src/world.rs`: World state, chunk pool management, raycasting, and clouds.
- `src/chunk.rs`: Procedural terrain generation and optimized mesh building.
- `src/noise.rs`: Thread-safe Perlin noise implementation.
- `src/block.rs`: Block types and properties.

## License 📜
Released into the **Public Domain** (The Unlicense). See [LICENSE](./LICENSE).
