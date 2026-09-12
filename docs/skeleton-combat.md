# Skeleton behavior specification

Reference: [Mojang's stable skeleton definition](https://github.com/Mojang/bedrock-samples/blob/v1.26.40.05/behavior_pack/entities/skeleton.json), with defaults from Microsoft's component documentation. Validation uses documentation and headless simulations.

## Implemented behavior

- Ranged priority 0: 15-block radius, three-second reload, two seconds on Hard; hold position throughout reloads when a clear shot is available.
- Retaliation priority 1, excluding Breeze attackers. Nearest targets include players, iron golems, and baby turtles outside water.
- Sun escape priority 2. Wolf avoidance priority 4, six-block acquisition distance, 1.2 movement multiplier.
- Underwater or unarmed skeletons use melee at 1.25 movement speed and base damage 2. Bow combat resumes only after leaving water with a bow.
- Pickup priority 5: three-block search, two-block collection, one item at a time, equipment preferences, durability, and equipment drops.
- Random stroll, player look, and random look priorities 6, 7, and 8.
- Twenty continuous seconds in powdered snow converts a skeleton to a stray while preserving health/equipment; leaving snow resets the timer.
- Collision dimensions 0.6 by 1.9 blocks; health 20.

[Targeting defaults](https://learn.microsoft.com/en-us/minecraft/creator/reference/content/entityreference/examples/entitygoals/minecraftbehavior_nearest_attackable_target?view=minecraft-bedrock-stable) provide 16-block acquisition, required visibility, three-second unseen timeout, and a 0.8 sneaking detection multiplier. Stable entity IDs preserve retaliation across mob-vector reordering. Pursuit uses last-observed positions; dead/creative players are excluded.

[Melee defaults](https://learn.microsoft.com/en-us/minecraft/creator/reference/content/entityreference/examples/entitygoals/minecraftbehavior_melee_box_attack?view=minecraft-bedrock-stable) provide a one-second cooldown and 0.8-block horizontal expansion of the bounding box. Attacks require vertical overlap and visibility.

[Ranged defaults](https://learn.microsoft.com/en-us/minecraft/creator/reference/content/entityreference/examples/entitygoals/minecraftbehavior_ranged_attack?view=minecraft-bedrock-stable) describe holding position in range and one second of sight before pursuit. Arrows identify their shooter, strike other mobs, and trigger retaliation. Aim compensates for VoxelPopuli's projectile gravity and drag.

[Sun escape](https://github.com/MicrosoftDocs/minecraft-creator/blob/main/creator/Reference/Content/EntityReference/Examples/EntityGoals/minecraftBehavior_flee_sun.md) seeks reachable shade or water. [Daylight burning](https://github.com/MicrosoftDocs/minecraft-creator/blob/main/creator/Reference/Content/EntityReference/Examples/EntityComponents/minecraftComponent_burns_in_daylight.md) is prevented by water, shade, or a helmet. Armor uses VoxelPopuli's existing damage reduction and durability rules.

[Despawn defaults](https://learn.microsoft.com/en-us/minecraft/creator/reference/content/entityreference/examples/entitycomponents/minecraftcomponent_despawn?view=minecraft-bedrock-stable) use 128/32-block outer/inner distances, a 30-second inactivity threshold, and a 1/800 random check. Picked-up gear makes skeletons persistent. Peaceful removes hostiles and prevents their spawning.

[Random stroll](https://learn.microsoft.com/en-us/minecraft/creator/reference/content/entityreference/examples/entitygoals/minecraftbehavior_random_stroll?view=minecraft-bedrock-stable) uses probability 1/120 with destination bounds of 10 horizontal and 7 vertical blocks. [Player look](https://learn.microsoft.com/en-us/minecraft/creator/reference/content/entityreference/examples/entitygoals/minecraftbehavior_look_at_player?view=minecraft-bedrock-stable) uses eight blocks, probability 0.02, and two to four seconds. [Random look](https://learn.microsoft.com/en-us/minecraft/creator/reference/content/entityreference/examples/entitygoals/minecraftbehavior_random_look_around?view=minecraft-bedrock-stable) uses probability 0.02 and the documented 20–40 second duration. Per-tick chances are converted to elapsed-time probabilities.

## Engine boundaries

This is documentation-based behavior, not a verified identical Bedrock simulation. Component definitions do not supply executable navigation, goal scheduling, movement integration, aiming, or random-number algorithms. Navigation here is bounded breadth-first search (768 nodes, 16-block extent), with one-block steps, drops up to three blocks, water avoidance on land, and water traversal when submerged or seeking refuge. Routes are reconsidered every half second. The 0.25 movement attribute uses the existing 10.75 conversion to blocks/second. Fire duration/damage and helmet wear remain local approximations.

The [pickup documentation](https://learn.microsoft.com/en-us/minecraft/creator/reference/content/entityreference/examples/entitygoals/minecraftbehavior_pickup_items?view=minecraft-bedrock-stable) omits difficulty-dependent eligibility probabilities. Pickup is enabled for all skeletons here; that probability is not matched. Regional-difficulty armor generation, enchantments, unsupported equipment tiers, ladders, name tags, vehicle-specific behavior, and stray arrow status effects require game systems outside this implementation. Native strays, bogged, parched, and pillagers retain their previous AI; converted skeletons retain the new state machine. Other creatures' own AI is unchanged.

## Persistence and verification

Settings includes Peaceful, Easy, Normal, and Hard. Save version 3 retains difficulty and skeleton health, position, equipment, durability, persistence, melee mode, and conversion/fire timers. Targets and paths are reacquired after loading. Versions 1 and 2 remain readable and default to Normal. Older builds cannot read version 3 saves. Other mob kinds retain their existing persistence behavior.

Run `cargo test --locked` for pure decision tests and actual world-update tests using simulation without graphics allocation. Tests cover reloads, sight, golem selection, wolves, water, shelter, pickup, routes, conversion, equipment, despawning, and save compatibility. Build WebAssembly with `python scripts/build-web.py`. Neither command opens a window or controls the cursor.
