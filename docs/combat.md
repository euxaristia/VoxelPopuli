# Combat animation reference

Target: Bedrock 26.45, the latest stable release listed on 2026-09-12.
Mojang's latest stable sample pack is tagged `v1.26.40.05`; the 26.45
hotfix notes do not list animation changes.

References:

- [26.44/45 release notes](https://feedback.minecraft.net/hc/en-us/articles/48149564061965-Minecraft-Bedrock-Edition-26-44-45-Hotfix-Changelog)
- [First-person attack definitions](https://github.com/Mojang/bedrock-samples/blob/v1.26.40.05/resource_pack/animations/player_firstperson.animation.json)
- [Player controllers](https://github.com/Mojang/bedrock-samples/blob/v1.26.40.05/resource_pack/animation_controllers/player.animation_controllers.json)
- [Arm geometry and pivot](https://github.com/Mojang/bedrock-samples/blob/v1.26.40.05/resource_pack/models/entity/humanoid.custom.geo.json)
- [Additive channels and XYZ rotation order](https://learn.microsoft.com/en-us/minecraft/creator/documents/animations/animationsoverview)
- [Zombie arm poses](https://github.com/Mojang/bedrock-samples/blob/v1.26.40.05/resource_pack/animations/zombie.animation.json)
- [Iron golem arm poses](https://github.com/Mojang/bedrock-samples/blob/v1.26.40.05/resource_pack/animations/iron_golem.animation.json)

`combat_animation.rs` evaluates the published first-person attack bone,
adult bare-handed zombie arms, and iron golem attack triangle. Molang angles
are degrees; conversion happens before Rust trigonometry. Numeric pose tests
check independently calculated samples from these definitions.

## Limits of parity

This is not yet a verified 1:1 reproduction of Bedrock 26.45. The sample pack
does not define the engine-supplied `first_person_item_rotation_factor` or
the attack clock. The viewmodel currently uses `sin(pi * progress)` for that
factor and retains VoxelPopuli's 0.3-second swing. Pixel offsets are converted
at 16 pixels per block. The arm uses the source 4-by-12-pixel geometry,
shoulder pivot, and first-person base position and rotation. Base and attack
rotation channels are added before building one XYZ rotation; there are no
per-frame shoulder clamps or corrections to the strike's depth. Existing held items follow this forearm;
Bedrock's separate engine-rendered item transforms are not reproduced.

Hurt feedback currently multiplies the mob tint for 0.5 seconds. Death uses
a one-second visual lifetime and a square-root roll to 90 degrees. These
are explicit approximations, not values verified against the closed-source
Bedrock renderer. Original meshes, textures, lighting, and camera remain.
Other species-specific attacks, bow charging, and player camera hurt motion
still need reference captures and implementation before claiming full parity.

Dead mobs are removed from gameplay immediately and transferred to a visual
list. They cannot attack, breed, block another hit, or generate additional
drops. Melee, arrows, and explosions share the same death path. Visual timers
stop with the existing paused world update; nothing is added to the save format.

## Validation

```sh
cargo test --locked
cargo test --locked render_attack_pose_contact_sheet -- --ignored
cargo run --locked --release -- --smoke-test-hand
python scripts/build-web.py
```

The native smoke test checks real click/hold/release swings in both rendering
modes, viewmodel visibility against a near-plane wall, damage feedback,
corpse expiry, and exactly-once drops. Combat screenshots are written beneath
`target/test-artifacts`. Browser startup and gameplay can be checked with the
existing `scripts/test-web-browser.mjs` runner described in `web.md`.

The CPU contact sheet renders idle, 10%, 25%, and 75% swing progress directly
from the arm geometry and projection into `target/test-artifacts/attack-poses-cpu.png`.
It needs no window, graphics context, or cursor access. Direction tests check
that the strike rises toward the crosshair across aspect ratios, and a separate
channel-cancellation test distinguishes additive Euler channels from multiplying
independent base and attack rotation matrices.

For a 1:1 comparison, capture vanilla Bedrock 26.45 with keyboard/mouse,
default FOV, and an unmodified resource pack: empty-hand and sword attacks
(single click, repeated clicks, held attack), bow charge/release, incoming
damage, and each supported hostile mob's attack and death. Record frame rate,
camera position, attack start, impact, and return-to-idle frames. Engine-only
timing and transforms must be measured from that reference rather than inferred
from Java Edition or presented as verified Bedrock behavior.
