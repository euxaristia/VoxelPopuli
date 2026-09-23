# Bedrock storage

Desktop worlds use Bedrock LevelDB, little-endian NBT `level.dat`, and persistent
subchunk palettes. `.mcworld` is a ZIP package of that directory, not the live
database's file extension. The target is Bedrock 26.51. This integration is still
being completed; compatibility with every Bedrock gameplay feature is not claimed.

## Commands

```sh
cargo run --release -- --save survival
cargo run --release -- --export-bedrock generated-world --export-radius 1 --seed 42
cargo run --release -- --pack-mcworld generated-world --output generated-world.mcworld
cargo run --release -- --unpack-mcworld exported-world.mcworld --output imported-world
cargo run --release -- --save imported-world
```

Exports and unpacking require new destinations. Close the world in Minecraft
before opening or packaging its database. VoxelPopuli holds the native database
lock for its entire session and refuses concurrent access. Native terrain saves
are larger than old seed-plus-edits saves because generated chunks are materialized.

The default legacy `world.vps` migrates to `world/`. Passing `--save name.vps`
migrates to `name.bedrock/`. Migration writes into a temporary sibling directory,
materializes edited chunks, writes the player record, and renames the completed
directory. The original `.vps` remains intact. A native destination is never
intentionally overwritten. Browser saves continue using the existing browser format.

## Current boundary

- The engine simulates Y 0–255. Other saved sections are preserved. Players outside
  that range are rejected on load rather than silently relocated.
- Unsupported block identities appear as protected bedrock. Their original states,
  additional palette layers, unknown database records, and unknown metadata remain
  in the native world. Supported blocks retain imported states until changed.
- Player inventory, armor, health, hunger, and experience use native player records.
  Inventory items with unsupported identities, enchantments, custom names, or other
  unrepresented item tags are rejected explicitly. Offhand items and additional armor
  slots are also rejected. These limitations prevent silent inventory loss.
- Ordinary chests and furnaces use native block entities. Unopened loot-table
  containers are rejected. Unimplemented block entity types remain in storage.
- Crafting cursors, settings, and other engine bookkeeping use an additional
  `voxelpopuli:session` database record. Minecraft ignores that record.
- Native actor interchange and dimension travel are still under implementation.
  Existing unknown actor records remain in storage; engine-specific entity state
  currently remains in the extra session record.
- Newly explored chunks use VoxelPopuli terrain generation. Minecraft can generate
  different terrain beyond the saved area; sharing a seed does not imply terrain parity.

## Validation

The initial nine-chunk `.mcworld` export imported and opened with visible terrain
in the actual Bedrock client. That test exposed a fixed-height spawn bug; new exports
now choose terrain support and two blocks of headroom, covered by regression tests.
The corrected spawn still needs a repeated client check.

Local tests cover persistent palettes, negative coordinates, dimension key isolation,
unknown states and extra layers, record preservation, synced transactions, database
locking across processes, legacy migration, native player inventory changes, malformed
NBT, and archive traversal and path conflicts. Two private Bedrock 26.51 fixture
copies provide 5,336 subchunks for format round-trip tests; those fixtures are not
committed or distributed. Full bidirectional gameplay compatibility remains unverified.

```sh
cargo test --locked
cargo test --locked inspect_real_bedrock_worlds_without_writes -- --ignored --nocapture
```

The ignored fixture check expects local copies in `target/bedrock-fixtures/world-1`
and `target/bedrock-fixtures/world-2`. It must never point at a world open in Minecraft.

## Loading performance checks

Chunk reads and save comparisons batch their subchunk records so each candidate
database table is decoded once per batch. Reads use the database's existing
64 MiB block-cache budget with checksum verification enabled. Chunk NBT decoding
and lighting run after releasing the world storage lock. Writes still use synced
WAL transactions; generated terrain is durable before it enters the world.

The release-only storage profile covers 1,089 chunks, crossing the database's
4 MiB flush threshold to exercise table reads as well as the WAL overlay. It
reports generation, persistence, reload, and unchanged-save times and checks a
30-second persistence budget. It creates and removes an isolated temporary world.

```sh
cargo test --release --locked profile_world_loading -- --ignored --nocapture
```

For full loading and rendering, create a fresh test world (the destination must
not exist), then run the hidden-window smoke test. Repeat the second command to
measure reloads. This writes generated chunks only to the selected test world.

```sh
cargo run --release --locked -- --export-bedrock target/loading-check --export-radius 0 --seed 1074691402050369410
cargo run --release --locked -- --smoke-test-world --smoke-saved-world --profile-loading --save target/loading-check
```

`--profile-loading` logs loading-loop elapsed time, loaded chunks, pending meshes,
and update duration. In smoke tests it uses the saved/default view distance
(normally 16), rather than the usual smoke-test radius of 4. Timings exclude
window, shader, and renderer initialization.

## Dependencies and references

Native storage uses `bedrock-leveldb` with zlib/snappy, `zip` with deflate, and `libc`
on Unix for LevelDB-compatible POSIX record locks. `bedrock-world` was evaluated as
a broader parsing library; it is not a dependency. `bedrock-render` renders map tiles
and is not a replacement for the game's 3D renderer.

- [Microsoft world format differences](https://learn.microsoft.com/en-us/minecraft/creator/documents/differencesbetweenbedrockandjava?view=minecraft-bedrock-stable)
- [Microsoft world packaging](https://learn.microsoft.com/en-us/minecraft/creator/documents/createaworldtemplate?view=minecraft-bedrock-stable)
- [Official Minecraft samples](https://github.com/microsoft/minecraft-samples)
- [Mojang block metadata](https://github.com/Mojang/bedrock-samples/tree/main/metadata/vanilladata_modules)
- [bedrock-world](https://github.com/BE-Community-Dev/bedrock-world)
- [bedrock-render](https://github.com/BE-Community-Dev/bedrock-render)
