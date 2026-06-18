# Repository Guidelines

## Project Structure & Module Organization

VoxelPopuli is a Rust 2024 voxel sandbox using OpenGL/GLFW. Core code lives in `src/`: `main.rs` owns the game loop, input, player physics, and UI; `world.rs` manages chunk streaming and world state; `chunk.rs` handles terrain generation, lighting, fluids, and meshing; `block.rs`, `item.rs`, `crafting.rs`, `mining.rs`, and `explosion.rs` contain gameplay rules. Rendering helpers are in `renderer.rs`, procedural textures in `atlas.rs`, and noise in `noise.rs`.

Assets are under `assets/`, with GLSL shaders in `assets/shaders/`. Platform raylib archives are in `lib/`. Tests are mostly inline Rust unit tests in the relevant modules, with small Python visual/helper scripts at the repository root.

## Build, Test, and Development Commands

- `cargo build --release`: builds the optimized game binary.
- `cargo run --release`: runs the game with release optimizations; prefer this for performance-sensitive rendering work.
- `cargo test`: runs Rust unit tests across gameplay and utility modules.
- `cargo fmt`: formats Rust code using standard rustfmt rules.
- `cargo clippy --all-targets`: runs lint checks when available locally.

Linux builds require GLFW/OpenGL system libraries as described in `README.md`.

## Coding Style & Naming Conventions

Use standard Rust formatting with 4-space indentation. Prefer small, focused modules and keep gameplay constants near the system that owns them. Rust items should follow idiomatic naming: `snake_case` for functions and variables, `PascalCase` for types and enum variants, and `SCREAMING_SNAKE_CASE` for constants.

Keep comments brief and useful. Avoid broad refactors when making targeted gameplay or rendering changes.

## Testing Guidelines

Add or update unit tests beside the code they validate, using `#[cfg(test)] mod tests`. Name tests after the behavior, for example `test_drop_diamond_ore_needs_iron_tier`. Run `cargo test` before submitting changes that affect gameplay rules, crafting, mining, block properties, terrain generation, or utility logic.

## Commit & Pull Request Guidelines

Recent history uses concise conventional prefixes such as `feat:`, `fix:`, `chore:`, `docs:`, and `refactor:`. Keep commit subjects imperative and specific, for example `fix: order clouds before water for transparency`.

Pull requests should include a short summary, testing performed, and screenshots or clips for visual/rendering changes. Link related issues when applicable, and call out platform-specific behavior or required system dependencies.

## Agent-Specific Instructions

Do not overwrite user changes. Check `git status --short` before committing or broad edits, and keep generated files out of commits unless they are intentional assets or fixtures.
