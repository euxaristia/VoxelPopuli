# Sneaking and celestial rendering

F5 cycles first-person, rear third-person, front third-person, then first-person,
following the [Minecraft perspective controls](https://edusupport.minecraft.net/hc/en-us/articles/360047116832-Minecraft-keyboard-and-mouse-controls).
Third-person uses a complete, animated player model with the selected skin
palette, head pitch, crouching and held item. The camera follows up to four
blocks away and pulls inward around terrain; movement, mining and attacks
continue to originate from the player. Perspective starts in first-person
when launching the game and is not stored in world saves.
The camera's collision sweep fits beside walls and under a crouching ceiling,
while its corners still stop it from moving through obstructing terrain.

Shift sneaks; Ctrl or double-tap W sprints. Sneaking lowers the camera from
1.62 to 1.27 blocks with a short transition, reduces ground movement to 30%,
uses a 1.5-block body height, and bends the visible torso forward 28 degrees.
Releasing Shift keeps the player crouched while standing would hit a ceiling.
Creative flight still uses Shift to descend. Camera aiming, mining, interaction,
arrows and water breathing use the same lowered eye position. The camera is
updated after movement, so it no longer follows the previous frame's position.

The torso bend follows [Mojang's sneak animation](https://github.com/Mojang/bedrock-samples/blob/46ba6ea985fb5a92d79a9419198f10dda14c199d/resource_pack/animations/player.animation.json).
The camera transition and first-person body proportions are local adaptations;
ledge protection and crawling are not implemented. The reduced collision size
follows [Bedrock's 1.20.10 sneaking change](https://www.minecraft.net/en-us/article/1-20-10-update-available-bedrock).

The sky draws an original procedural square sun with a warm glow and an
eight-frame moon with generated crater markings. Both sprites are generated
once in Rust and use additive blending. Geometry remains stable at the zenith,
follows the camera, and shares the lighting orbit including orbital offsets.
The sprite planes follow the sky orbit, not the camera's rotation. Looking
around does not turn them toward the screen. Their world-space geometry stays
square; normal perspective can make their projected outlines non-square.
Celestial sprites do not write depth and render at the far plane, so terrain
occludes them.

Fancy graphics uses VoxelPopuli's authored lighting and atmosphere defaults.
Optional local JSON resource-pack settings can override those defaults.
Fast graphics uses the same generated sprites with its simpler sky.
No Mojang PNGs or sample JSON files are distributed or embedded in the build.
Colours, patterns, scattering, angular sizing and tone mapping are local
recreations; pixel-identical Vibrant Visuals output is not claimed.
The local Mie lobe uses an exponent of `1 + sun_glare_shape` so the vanilla
fractional values have a continuous slope at the sun-facing hemisphere boundary.
This avoids the sharp vertical division produced by applying those values
directly as a power of a clamped cosine.
See [Microsoft's atmosphere documentation](https://learn.microsoft.com/en-us/minecraft/creator/documents/vibrantvisuals/atmosphericscustomization?view=minecraft-bedrock-stable).

The moon advances through eight phases on a 20-minute day cycle. Sleeping
advances the day too. Save format 7 stores the day counter; formats 1–6
default to day zero. Native Bedrock `Time` includes full elapsed days.
Generator-version compatibility remains unchanged.

Run `cargo run --release -- --smoke-test-celestial-sneak` to capture the sun,
eight phases, off-axis full moons at 35, 45 and 80 degrees elevation,
three camera headings around a fixed moon direction,
both first-person body poses, all third-person poses, and a GPU
sky-seam regression capture in `target/test-artifacts/`.
Run `cargo run --release -- --smoke-test-lighting` for the full lighting cycle.
