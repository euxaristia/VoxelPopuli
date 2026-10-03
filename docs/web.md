# Browser build and GitHub Pages

The browser build runs the same Rust terrain, gameplay, inventory, mobs, and
wgpu renderer as the desktop game. It requires WebGPU, a keyboard and mouse,
and HTTPS (or localhost for development). No server or special isolation
headers are required.

## Build and preview

Install the Rust target and the packager matching the pinned wasm-bindgen
dependency once:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.128 --locked
python scripts/build-web.py
python -m http.server 8080 --bind 127.0.0.1 --directory target/web
```

Open `http://127.0.0.1:8080`. The generated `target/web` directory is the entire
static deployment. JavaScript, WASM, shaders, and font assets use relative paths
so the build also works under a repository subpath such as `/VoxelPopuli/`.
Shaders and the font are embedded in the WASM binary; terrain textures remain
procedurally generated.

## Deploy

In the repository's **Settings → Pages**, select **GitHub Actions** as the build
source. The `WebAssembly / GitHub Pages` workflow builds pull requests without
deploying them, and deploys pushes to `main`. It can also be run manually on
`main` after enabling Pages. The deployment job reports the published URL.

See [GitHub's custom Pages workflow documentation](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).

## Controls and saves

F5 cycles first-person, rear third-person and front third-person while playing.
With the game canvas focused, it switches perspective instead of reloading the page.

Click **Click to play** after terrain loads. Desktop controls work in the
browser: WASD, mouse look, Space to jump, Shift to sneak, Ctrl or double-tap W to sprint,
left/right click to mine/place, E for inventory, 1–9 for the hotbar, and Esc
to release the mouse and pause. Standard-mapped gamepads are supported during
play. Use **Fullscreen** or F11 to expand the game.

Worlds persist in IndexedDB in the current browser using VoxelPopuli's version 8
save format with the historical 16-bit block/item layout and an explicit graphics
preset extension. Versions 1-7 remain readable. Old Fancy settings become High;
old Fast settings remain Fast. Older binaries cannot read newly written version 8
records. Existing
worlds keep their original terrain generator; new worlds get the additional
[bee habitats and trees](world-generation.md). Desktop
worlds use native Bedrock storage; browser saves do not contain a Bedrock database.
The game saves every 30 seconds, on pause, on **Save world**, and on
**Save & Quit**. Wait for the saved confirmation before closing a tab: browsers
do not guarantee asynchronous saves during page shutdown. A failed write leaves
the previous save intact; an unreadable save stops startup rather than replacing
it. A browser lock prevents simultaneous tabs overwriting the same world.
Clearing site data removes the save, and private browsing may discard it when
the session closes. Saves do not automatically sync with the desktop game or
other browsers/devices.

The browser defaults to a four-chunk view distance, with a maximum of eight.
Generation and meshing each process at most one job per frame on the main
thread; initial loading and streaming can be slower than the native worker
pool. New browser worlds default to High graphics. Open Settings from Esc and
click Graphics Quality to cycle Cinematic, High, and Fast. High targets 75% of
framebuffer width and height; Cinematic targets native resolution and can cost
substantially more GPU time and memory. Fast retains the cheaper forward path
and approximately 1125x633 pixel budget. The HUD stays at output resolution.
Existing worlds keep their migrated preset. See [rendering and limitations](rendering.md).
Java filesystem world import/export and custom resource-pack directories remain
desktop features. Touch-only controls and a WebGL fallback are not included.

## Validation

```sh
cargo test --locked
cargo check --locked --target wasm32-unknown-unknown
node --test web/*.test.mjs
python scripts/build-web.py
```

The JavaScript tests use Node's built-in test runner (Node 22 or later); no npm
dependencies are required. `bun test web/` also runs them. Before deployment,
verify the generated build in a WebGPU browser: load terrain, capture/release the
pointer, move and jump, mine/place blocks, open inventory, resize, save, and
reload. Verify all three quality presets and check the browser console for GPU
errors. These are future interactive checks, not results of the current code-only
remaster work; no browser or GPU smoke validation was performed for that work.

The automated WebGPU smoke test acquires pointer lock and sends input events.
Headless mode and a temporary profile do not isolate the physical mouse, and
this test can disrupt other desktop apps. It refuses to launch without explicit
`--allow-desktop-input` consent. Do not run it on an active desktop under a promise
of input isolation.

When that desktop-input risk is explicitly accepted, run
`node scripts/test-web-browser.mjs <chromium-executable> --allow-desktop-input`
after building, with Node 22+ or Bun and an installed Chromium browser. It uses
a temporary data profile and checks startup, movement, inventory, graphics,
resizing, save/reload, and Save & Quit. Screenshots stay under `target/`; the
browser sandbox remains enabled. Ordinary unit tests never acquire pointer lock.
