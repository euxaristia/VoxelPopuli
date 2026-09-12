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
cargo install wasm-bindgen-cli --version 0.2.126 --locked
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

Click **Click to play** after terrain loads. Desktop controls work in the
browser: WASD, mouse look, Space to jump, Shift to sneak, Ctrl to sprint,
left/right click to mine/place, E for inventory, 1–9 for the hotbar, and Esc
to release the mouse and pause. Standard-mapped gamepads are supported during
play. Use **Fullscreen** or F11 to expand the game.

Worlds persist in IndexedDB in the current browser, using the desktop save
format. The game saves every 30 seconds, on pause, on **Save world**, and on
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
pool. The browser build includes the existing fancy/fast rendering modes.
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
reload. Verify both graphics modes and check the browser console for GPU errors.

For the automated WebGPU smoke test, run
`node scripts/test-web-browser.mjs <chromium-executable>` after building, using
an installed Chrome/Chromium executable. It starts its own localhost server
under `/VoxelPopuli/`, launches a temporary browser profile, and checks startup,
pointer capture, movement, inventory, graphics settings, resizing, durable
save/reload, and Save & Quit. Screenshots and diagnostics stay under `target/`.
The test requires a working WebGPU adapter and keeps the browser sandbox enabled.
