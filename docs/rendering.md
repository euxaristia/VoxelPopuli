# Rendering and quality

VoxelPopuli uses wgpu and WGSL, with GLFW desktop windows and WebGPU in browsers.
It does not use hardware ray tracing. Built-in textures and lighting artwork are
original; selected Vibrant Visuals JSON settings can supply desktop pack overrides.

## Quality controls and costs

Open Settings from Esc, then click Graphics Quality to cycle Cinematic, High,
Fast, and back to Cinematic. New desktop worlds default to Cinematic; new browser
worlds default to High. Loaded saves retain their migrated or explicit preset.

| Preset | World render-size policy | Configured shadow side | Effect step budget |
|---|---|---|---|
| Cinematic | Native framebuffer size | 4096 | 48 |
| High | 75% of framebuffer width and height | 2048 | 24 |
| Fast | Forward path, approximately 1125x633 pixel budget | Disabled | Disabled |

These are configuration budgets, not measured GPU costs or validation results.
Render-size policy never exceeds valid framebuffer dimensions; degenerate dimensions clamp to
at least one pixel. High rounds down after scaling. The HUD stays at output
resolution, independent of world resolution.

At 2560x1440, native resolution shades about 5.2 times as many pixels as the old
1125x633 budget before adding effects. Higher-resolution HDR attachments, shadow
maps, and additional passes increase GPU time and memory. There is no 60 FPS
guarantee. Device texture limits can require an explicit quality fallback; native
resolution is not a promise that every adapter can allocate every configured target.

## Pipeline and remaster status

Cinematic and High select deferred HDR rendering; Fast selects the cheaper
forward path. The existing renderer separates geometry, lighting, forward water
and celestials, tone mapping, and output-resolution UI. The first-person hand has
its own depth pass so it is not hidden by nearby terrain.

The quality model, settings controls, and version-8 persistence are implemented.
Cinematic and High also wire material MERS/normal maps, four stabilized shadow
cascades, contact occlusion, shadow-tested atmosphere and procedural clouds,
screen-space reflections, depth-aware water, temporal reconstruction, and bloom.
Fast keeps the forward path and block clouds. This source wiring is not visual
or GPU-performance validation.

Deferred emission is capped at two exposed radiance units before tone mapping.
This keeps nighttime torch, flame, and lava colours from clipping to white while
preserving the Cinematic shader's daytime emission strength. Bloom stays enabled.

Screen-space reflections can use only geometry represented in the scene buffers.
Offscreen surfaces and transparent objects missing from the opaque scene require
fallbacks; this is not full-scene ray tracing. Cave sky leaks, waterline artifacts,
shadow shimmer, temporal ghosting, and transparency ordering still need GPU checks.
The hidden generated-world regression reproduces seed 1074691402050369410 at
midnight across three camera directions and captures normals, lighting,
atmosphere, forward effects, temporal resolve, and bloom. Invalid normal vectors
previously became white HDR pixels which bloom expanded into floating blobs.
Normal fallback and derivative safeguards remove those sources; filtered bloom
extraction prevents highlights popping between alternating pixel positions.
The regression checks for invalid normals and unexplained white terrain glow.
This verifies that defect, not the entire remaster or its performance budget.

## Settings migration

Browser saves and native `voxelpopuli:session` records use save version 8. The
version-7 field layout remains unchanged, including the historical Fancy boolean,
16-bit block/item IDs, terrain generator ID, and day counter. Version 8 declares
its new format in the header and adds one required graphics preset byte after the
day counter: Cinematic = 0, High = 1, Fast = 2.

Versions 1-7 remain readable. Their Fancy setting maps to High, not the new desktop
Cinematic default; Fast remains Fast. Missing, invalid, truncated, or unversioned
preset extensions are rejected. UI changes survive native save/reopen and browser
save/reload through the same versioned settings codec. This does not alter native
Minecraft terrain or player record layouts. Older VoxelPopuli binaries cannot read
new version-8 records. Keep a backup before moving a save between game versions.

## Code-only verification

With the repository's existing toolchain and dependencies installed, on Windows:

```powershell
$env:RUSTFLAGS = '-Dwarnings'
cargo fmt --check
cargo clippy --all-targets --locked --offline -- -D warnings
cargo test --locked --offline
cargo build --release --locked --offline
cargo check --locked --offline --target wasm32-unknown-unknown
node --throw-deprecation --test web/*.test.mjs scripts/test-web-browser-guard.test.mjs
```

On Linux, set `export RUSTFLAGS=-Dwarnings` before the same Cargo commands. Browser
compilation requires the existing WebAssembly target; JavaScript tests use Node's
built-in runner and do not launch a browser. Run applicable checks on each available
development OS, including WSL, without installing missing tools implicitly.

Regressions cover preset defaults/cycling, render sizing, historical migrations,
all-preset roundtrips, rejected corrupt extensions, and native session persistence.
Code-only tests do not validate shader execution, adapter limits, visual quality,
or performance. The following GPU checks are ignored by default and must be
requested explicitly. The first requests an adapter without a window; the second
runs the actual generated-world renderer in an invisible, unfocused test window,
ignores desktop input, never opens a player save, and captures three camera views.

```powershell
$env:RUSTFLAGS = '-Dwarnings'
$env:WGPU_BACKEND = 'vulkan' # Use 'dx12' for the second Windows backend.
cargo test --locked --offline renderer::post_tests:: -- --ignored --test-threads=1
cargo test --locked --offline hidden_cinematic_rendering_regression -- --ignored --test-threads=1
```

Captures stay under `target/test-artifacts/`. Browser checks remain separate.
