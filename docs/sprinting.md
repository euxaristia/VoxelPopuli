# Sprinting and hunger

Use Ctrl while moving forward, or tap W twice within 0.35 seconds and keep W held.
Gamepads use left-stick click or two forward pushes. Shift remains sneak. Sprinting
continues after releasing the sprint button until forward input stops or a stopping
condition applies. Backward and sideways-only movement cannot start a sprint.

Land movement retains the existing walking speed of 4.317 blocks/second and sprint
speed of 5.612 blocks/second. A sprint jump adds a 4-block/second horizontal impulse
in the facing direction. Sneaking, drawing a bow, opening inventory or pausing,
horizontal collision, entering water/lava, flight, and death interrupt land sprinting.
The existing swimming and flight movement modes retain their own speeds.

Survival requires more than six hunger points (three drumsticks) to sprint. The
food system uses hunger, saturation, and exhaustion:

- Ground sprinting costs 0.1 exhaustion per block actually traveled horizontally.
- A normal jump costs 0.05 exhaustion; a sprint jump costs 0.2 at takeoff.
- Swimming costs 0.01 exhaustion per block moved. Walking costs none.
- Every four exhaustion points consume one saturation point, then one hunger
  point once saturation is depleted. Fractional exhaustion is retained.
- Eating restores hunger and saturation. Sandbox movement does not consume food.

Airborne sprint movement does not also incur ground-distance exhaustion. Movement
rejected by collision costs nothing. Hunger, saturation, and exhaustion retain the
existing save format; active sprint input resets on reload and respawn.

## References

[Mojang's controls guide](https://www.minecraft.net/en-us/article/minecraft-controls)
documents Ctrl for sprint and Shift for sneak. The
[official sprint-window description](https://www.minecraft.net/en-us/article/minecraft-java-edition-1-21-9)
describes double-tapping the forward key. This implementation uses a fixed seven-tick
window at 20 ticks/second rather than adding a new settings control.

[Microsoft's exhaustion component documentation](https://learn.microsoft.com/en-us/minecraft/creator/reference/content/entityreference/examples/entitycomponents/minecraftcomponent_exhaustion_values?view=minecraft-bedrock-stable)
and [Mojang's player example](https://github.com/Mojang/bedrock-samples/blob/main/behavior_pack/entities/player.json)
provide the action costs. Use the player example's sprint value of 0.1, rather than
the generic component's default value of 0.01. Existing project hunger thresholds,
food conversion, and land speeds are preserved.

## Checks

```sh
cargo test --locked player::tests::
cargo test --locked sprint::tests::
cargo test --locked hunger_state_survives_save_reload_without_resuming_sprint
```

Tests cover double-tap and button activation, cancellation, actual-distance costs,
blocked movement, jumps, swimming, sandbox exemption, food thresholds, food recovery,
and save/reload. Follow the [survival playtest](survival-playtest.md) for keyboard
and controller checks in a running world.
