# Procedural sky and optional settings

`src/celestial.rs` generates the original sun glow and eight moon phases.
`src/vibrant/lighting.rs`, `src/vibrant/atmospherics.rs`, and `src/vibrant/water.rs` supply
authored lighting, atmosphere, and water defaults. Native and browser builds use the same code.
No Mojang PNG or sample JSON assets are bundled.

This directory may contain locally authored resource-pack settings using the
supported Bedrock JSON schemas. Each file is optional; absent settings use the
Rust defaults. See [rendering notes](../../docs/celestials-and-sneaking.md).
