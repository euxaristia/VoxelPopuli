# Procedural sky and optional settings

`src/celestial.rs` generates the original sun glow and eight moon phases.
`src/vibrant/lighting.rs` and `src/vibrant/atmospherics.rs` supply authored
lighting and atmosphere defaults. Native and browser builds use the same code.
No Mojang PNG or sample JSON assets are bundled.

This directory may contain locally authored resource-pack settings using the
supported Bedrock JSON schemas. Each file is optional; absent settings use the
Rust defaults. See [rendering notes](../../docs/celestials-and-sneaking.md).
