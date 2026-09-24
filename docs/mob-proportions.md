# Mob proportions and reference colours

Goats, llamas, trader llamas, camels, camel husks, pigs, wolves, foxes, pandas
and polar bears use species-specific cuboids based on Mojang's Bedrock model
geometry. Their geometry uses 16 model pixels per block, independently of
collision height, with the polar bear's reference 1.2 visual multiplier.
Horns, ears and raised necks no longer shrink an entire
animal to fit its collision box. Babies retain the existing half-size body
and enlarged-head animation.

All mob textures are original procedural artwork. These ten animals have
per-part coat palettes, eye detail, fur markings, noses and hooves generated
in Rust. The atlas stores unshaded colours; scene lighting and exposure still
affect the final screen colour. No Mojang texture sheets are loaded or embedded.
The palette and patterns approximate the reference appearance, without claiming
pixel-identical colours or markings.

Adult collision dimensions are checked against the base or adult component
groups in the pinned Bedrock behavior pack, including the adult scale for
cats and rabbits and the tropical fish scale. The fixture records 74 species;
41 previously different height/width pairs are corrected. Slimes use the medium
size group. Pufferfish inflation and tadpole growth need separate stateful size
handling and are excluded; cod's existing dimensions are unchanged. This does
not implement every Bedrock scale variant, baby geometry or animation.

## Sources and adaptations

All references are pinned to
[Mojang/bedrock-samples revision 46ba6ea985fb5a92d79a9419198f10dda14c199d](https://github.com/Mojang/bedrock-samples/tree/46ba6ea985fb5a92d79a9419198f10dda14c199d).
See `resource_pack/models/entity/{goat,llama,camel,pig,wolf,fox,panda,polar_bear}.geo.json`
and `behavior_pack/entities/`. The procedural artwork is described in
[assets/mobs/README.md](../assets/mobs/README.md).

The renderer retains its own animation joints. Thin goat beard and camel tail
cards have a small thickness for visibility. Bear torsos and limb depth remain
simplified; trader llama cloth is a body overlay. One adult coat is selected per
species (creamy llama, pale wolf, red fox, normal panda); biome coat variants,
equipment and the newer dedicated baby models are not implemented here.

## Validation

`cargo test mob_` covers the collision fixture, distinguishing head and leg
dimensions, fixed model scale, atlas part separation, eye placement and procedural palettes.
`cargo run --release -- --smoke-test-mobs` renders all 77 species at rest, moving
and in side profile, then checks simulation and scene rendering. The collision
fixture test fails against the previous catalogue before these corrections.
