# Survival playtest

Use a fresh file so existing worlds stay intact:

```bash
cargo run --release -- --save playtest.vps
```

1. Confirm the hotbar is empty. Gather wood; craft planks, sticks, a crafting
   table, and a wooden pickaxe. Mine cobblestone and build a stone pickaxe and
   furnace. Double-tapping jump should not enable flight.
2. Smelt iron ore with coal. Check that fuel decreases and output appears after
   ten seconds. Close the furnace, wait, and reopen it; cooking should continue.
   Full or incompatible output must block another smelt without consuming fresh fuel.
3. Cook porkchops and beef, sprint until hungry, and eat. Put ingredients into
   the furnace with shift-click and right-click. Its output slot must reject items.
4. Craft and place a chest. Store a worn tool and several stacks. Test left-click,
   right-click, and shift-click transfers. Break the chest and collect its contents.
5. Drop one item with Q and a stack with Ctrl+Q. Walk away, return, and collect it.
   Repeat with a full inventory: the uncollected remainder should stay visible.
6. Leave items in both crafting grids and hold a worn tool on the cursor. Close
   the window, restart with the same `--save playtest.vps`, and check the items,
   furnace progress, chest contents, dropped stacks, tool wear, and time of day.
7. Close and reopen a crafting table after returning its ingredients to inventory.
   Its old output preview must be gone; clicking it must not grant free items.
8. Equip armor, take damage, swap it out and back. Its durability must not reset.
9. Set a bed spawn far from the starting area. Die and confirm terrain loads
   before movement resumes. Block the bed's standing space and repeat; the player
   should appear in nearby open space. Current death rules keep inventory and XP.
10. Pause during smelting. Fuel, cooking progress, hunger, and daylight should
    remain unchanged until play resumes. Check inventory clicks at 100% and
    200% display scaling, and close the game normally without a shutdown crash.

The world rendering check runs the real game loop in a hidden window, captures
the scene and actual swapchain image in both graphics modes in
`target/test-artifacts`, and exits without saving:

```sh
cargo run --release -- --smoke-test-world
```

It uses the coastal seed that previously spawned the player in a dark cave. New
worlds must start above the terrain; `--reset-spawn` recovers an affected saved
position while retaining inventory and world edits.

To check loading the affected coastal save without resetting its position or
writing back to it:

```sh
cargo run --release -- --smoke-test-world --smoke-saved-world --save survival.vps
```

Current limits: singleplayer, single chests, no furnace XP, no offline smelting,
and no automatic timed save. Java world import/export does not transfer native
container inventories. These checks do not establish full Minecraft parity.
