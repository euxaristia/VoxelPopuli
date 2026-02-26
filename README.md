# VoxelPopuli 🧱

A lightweight Minecraft-inspired voxel sandbox written in C with raylib.

## Highlights ✨
- Infinite-style chunk streaming around the player
- Procedural terrain and trees via Perlin noise
- Dynamic cloud rendering
- Animated water texture with runtime atlas updates
- Block placing and mining with a 9-slot hotbar
- Inventory panel for quick block selection

## Stack 🛠️
- C99
- [raylib](https://www.raylib.com/)
- GNU Make
- Clang (default), GCC (optional)

## Build

```bash
make        # default build (clang)
make gcc    # build with gcc for comparison
make clang  # explicit clang build
```

## Run 🚀

```bash
./voxelpopuli
```

## Controls 🎮
- `W A S D`: Move
- `Mouse`: Look
- `Space`: Jump / swim up
- `Left Ctrl`: Sprint
- `Left Click`: Break block
- `Right Click`: Place selected block
- `1..9`: Select hotbar slot
- `E`: Toggle inventory
- `Esc`: Quit

## Project Layout 📁
- `src/main.c`: Game loop, input, player movement, UI
- `src/world.c`: World systems, texture atlas, clouds, water animation
- `src/chunk.c`: Chunk generation and mesh building
- `src/noise.c`: Perlin noise implementation

## License 📜
Licensed under **GNU AGPLv3**. See [LICENSE](./LICENSE).
