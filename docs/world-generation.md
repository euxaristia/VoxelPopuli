# World generation and bee habitats

New worlds use terrain generator 2. Existing saves without a generator version
keep generator 1, including after saving again, loading on the web, or migrating
to native Bedrock storage. Generator 1 preserves the original six biomes and
their vegetation. There is no automatic upgrade of existing worlds.

Generator 2 adds forests, birch forests, flower forests, sunflower plains,
meadows, mangrove swamps, and cherry groves. Forests mix oak and birch; birch
forests use birch trees. Meadows have sparse oak/birch trees and flowers.
Cherry trees have branching trunks and broad pink canopies. Mangroves have
raised trunks and spreading roots over mud and shallow water. Sunflowers occupy
two blocks; flower forests and meadows also contain cornflowers, alliums,
oxeye daisies, poppies, and dandelions. Cherry groves have pink petals.

Birch, mangrove, and cherry logs, leaves, and planks have distinct textures,
mining properties, fire behavior, and native block identities. Logs craft into
four matching planks; those planks work in the existing wood recipes. All new
flowers are bee food and pollination targets. Nest generation includes these
habitats, with two or three initial occupants subject to the creature cap.

The habitat associations follow Mojang's [bee guide](https://www.minecraft.net/en-us/article/bee)
and [Bedrock biome examples](https://github.com/Mojang/bedrock-samples/tree/main/behavior_pack/biomes).
These are VoxelPopuli implementations: climate thresholds, tree shapes, tree
density, flower distribution, and nest probabilities are not a reproduction of
Minecraft's proprietary terrain generator. Existing terrain height and village
layout rules remain in use. Public biome examples do not expose every vanilla
tree/nest feature rule. Sapling growth, propagules, waterlogged roots, and the
complete Minecraft flower roster are not implemented.
The legacy Java 1.17 exporter rejects chunks containing mangrove, cherry, mud,
or pink petals before writing an export, since that Minecraft version predates
those blocks. Use native Bedrock export for those habitats.

New native chunks store their biome palette rather than labeling everything
plains. Native chunk blocks already saved are never regenerated. Browser saves
retain their generator version because their terrain is rebuilt from the seed
and player edits. The supplemental/browser save format is version 7; versions
1 through 6 remain readable. Unknown future generator versions are rejected.

To explore the new generation, create a new world:

```sh
cargo run --release -- --seed 42 --save habitats
```

The hidden-window rendering check generates one example of each new habitat
using seed 42 and the production terrain mesher. It writes screenshots and
prints chunk coordinates without opening a world save:

```sh
cargo run --release -- --smoke-test-habitats
```
