// `*.texture_set.json` — the per-texture PBR maps described in Overview of
// Physically Based Rendering.
//
// Each layer is either a texture name to load or a flat value baked in
// place, so a pack can make a whole block mirror-smooth without authoring
// an image. A layer that is absent falls back to the pbr/global.json
// default for the object's category.
use super::json::Json;
use super::keyframe::Color;
use super::lighting::Mers;

/// A texture layer: a named image, or a constant standing in for one.
#[derive(Clone, Debug, PartialEq)]
pub enum Layer {
    Texture(String),
    Value(Color),
}

impl Layer {
    fn parse(value: &Json) -> Option<Layer> {
        match value {
            // A string is ambiguous in the schema: a texture name, or a
            // hex color. Hex is always 6 or 8 digits behind a '#', and
            // texture names never start with one.
            Json::String(text) if text.starts_with('#') => Color::parse(value).map(Layer::Value),
            Json::String(text) => Some(Layer::Texture(text.clone())),
            Json::Array(_) => Color::parse(value).map(Layer::Value),
            _ => None,
        }
    }

    pub fn texture_name(&self) -> Option<&str> {
        match self {
            Layer::Texture(name) => Some(name),
            Layer::Value(_) => None,
        }
    }
}

/// The surface maps for one texture, minus the base color.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct TextureSet {
    /// Base color / albedo.
    pub color: Option<Layer>,
    /// Packed metalness (R), emissive (G), roughness (B).
    pub metalness_emissive_roughness: Option<Layer>,
    /// The Vibrant Visuals four-channel form, adding subsurface (A). Takes
    /// precedence over the three-channel key when a pack supplies both.
    pub metalness_emissive_roughness_subsurface: Option<Layer>,
    /// Tangent-space normal map. RGB 128/128/255 is a flat surface.
    pub normal: Option<Layer>,
    /// Grayscale height, an alternative to `normal`. 0.5 is flush.
    pub heightmap: Option<Layer>,
}

impl TextureSet {
    pub fn parse(root: &Json) -> Option<Self> {
        let settings = root.get("minecraft:texture_set")?;
        let layer = |key: &str| settings.get(key).and_then(Layer::parse);
        Some(Self {
            color: layer("color"),
            metalness_emissive_roughness: layer("metalness_emissive_roughness"),
            metalness_emissive_roughness_subsurface: layer(
                "metalness_emissive_roughness_subsurface",
            ),
            normal: layer("normal"),
            heightmap: layer("heightmap"),
        })
    }

    /// The MERS layer a pack actually meant, preferring the four-channel
    /// key over the legacy three-channel one.
    pub fn mers_layer(&self) -> Option<&Layer> {
        self.metalness_emissive_roughness_subsurface
            .as_ref()
            .or(self.metalness_emissive_roughness.as_ref())
    }

    /// Constant MERS for this set, when it was authored as a value rather
    /// than a texture. `None` means the caller must sample the texture, or
    /// fall back to pbr/global.json.
    pub fn constant_mers(&self) -> Option<Mers> {
        match self.mers_layer()? {
            Layer::Value(color) => Some(Mers::from_color(*color)),
            Layer::Texture(_) => None,
        }
    }

    /// Whether this set needs a surface-detail texture bound at all.
    pub fn has_surface_detail(&self) -> bool {
        self.normal.is_some() || self.heightmap.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vibrant::json;

    #[test]
    fn parses_the_documented_mirror_example() {
        let source = r##"{
            "format_version": "1.16.100",
            "minecraft:texture_set": {
                "color": "iron_block",
                "metalness_emissive_roughness": "iron_block_mer"
            }
        }"##;
        let set = TextureSet::parse(&json::parse(source).unwrap()).unwrap();
        assert_eq!(set.color, Some(Layer::Texture("iron_block".to_string())));
        assert_eq!(
            set.mers_layer().and_then(Layer::texture_name),
            Some("iron_block_mer")
        );
        assert_eq!(set.constant_mers(), None);
        assert!(!set.has_surface_detail());
    }

    #[test]
    fn hardcoded_values_stand_in_for_textures() {
        // A mirror with no MER image: fully metallic, perfectly smooth.
        let source = r##"{
            "minecraft:texture_set": {
                "color": [200, 200, 210],
                "metalness_emissive_roughness_subsurface": [255, 0, 0, 0]
            }
        }"##;
        let set = TextureSet::parse(&json::parse(source).unwrap()).unwrap();
        let mers = set.constant_mers().unwrap();
        assert_eq!(mers.metalness, 1.0);
        assert_eq!(mers.roughness, 0.0);
        assert_eq!(mers.subsurface, 0.0);
    }

    #[test]
    fn the_four_channel_key_wins_over_the_three_channel_one() {
        let source = r##"{
            "minecraft:texture_set": {
                "metalness_emissive_roughness": "legacy_mer",
                "metalness_emissive_roughness_subsurface": "mers"
            }
        }"##;
        let set = TextureSet::parse(&json::parse(source).unwrap()).unwrap();
        assert_eq!(set.mers_layer().and_then(Layer::texture_name), Some("mers"));
    }

    #[test]
    fn distinguishes_a_hex_color_from_a_texture_name() {
        let source = r##"{
            "minecraft:texture_set": {
                "color": "#FF8800",
                "normal": "brick_normal"
            }
        }"##;
        let set = TextureSet::parse(&json::parse(source).unwrap()).unwrap();
        assert!(matches!(set.color, Some(Layer::Value(_))));
        assert_eq!(set.normal, Some(Layer::Texture("brick_normal".to_string())));
        assert!(set.has_surface_detail());
    }

    #[test]
    fn heightmap_counts_as_surface_detail() {
        let source = r##"{ "minecraft:texture_set": { "heightmap": "cobble_height" } }"##;
        let set = TextureSet::parse(&json::parse(source).unwrap()).unwrap();
        assert!(set.has_surface_detail());
        assert_eq!(set.normal, None);
    }

    #[test]
    fn rejects_a_file_without_the_settings_object() {
        let value = json::parse(r##"{ "format_version": "1.16.100" }"##).unwrap();
        assert!(TextureSet::parse(&value).is_none());
    }
}
