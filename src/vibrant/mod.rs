// Vibrant Visuals pack data: the JSON schemas that drive the deferred
// renderer's lighting, atmosphere and material parameters.
//
// Files are read from a pack directory laid out the way Bedrock resource
// packs are, so a pack authored against the Vibrant Visuals docs loads
// here unchanged:
//
//   manifest.json                          capabilities: ["pbr"]
//   lighting/global.json                   directional, ambient, sky
//   atmospherics/atmospherics.json         scattering and sky color
//   local_lighting/local_lighting.json     per-block light color and type
//   pbr/global.json                        MERS fallbacks
//   textures/**/<name>.texture_set.json    per-texture PBR maps
//
// Every file is optional: a missing or malformed one falls back to the
// documented default and appends a warning rather than failing the load,
// because a broken pack should not cost the player their world.
pub mod atmospherics;
pub mod frame;
pub mod json;
pub mod keyframe;
pub mod lighting;
pub mod texture_set;

use atmospherics::AtmosphereSettings;
use lighting::{LightingSettings, LocalLightSettings, PbrFallbackSettings};
use std::path::{Path, PathBuf};
use texture_set::TextureSet;

/// Where the built-in pack lives, relative to the working directory.
pub const DEFAULT_PACK_DIR: &str = "assets/vibrant";

const TEXTURE_SET_SUFFIX: &str = ".texture_set.json";
// A pack that walks a deep tree of its own is almost certainly pointed at
// the wrong directory; stop before we stat the whole disk.
const MAX_TEXTURE_SEARCH_DEPTH: usize = 8;

#[derive(Clone, Debug)]
pub struct VibrantPack {
    pub lighting: LightingSettings,
    pub atmospherics: AtmosphereSettings,
    pub local_lighting: LocalLightSettings,
    pub pbr_fallback: PbrFallbackSettings,
    /// Texture sets keyed by the texture base name, so `stone` resolves the
    /// set authored in `stone.texture_set.json`.
    texture_sets: Vec<(String, TextureSet)>,
    /// True when manifest.json declares the "pbr" or "raytraced" capability.
    pub pbr_capable: bool,
    /// Non-fatal problems found while loading, for the log.
    pub warnings: Vec<String>,
}

impl Default for VibrantPack {
    fn default() -> Self {
        Self {
            lighting: LightingSettings::default(),
            atmospherics: AtmosphereSettings::default(),
            local_lighting: LocalLightSettings::default(),
            pbr_fallback: PbrFallbackSettings::default(),
            texture_sets: Vec::new(),
            pbr_capable: false,
            warnings: Vec::new(),
        }
    }
}

impl VibrantPack {
    /// Loads a pack directory. A directory that does not exist yields the
    /// defaults with no warnings: running without a pack is normal.
    pub fn load(dir: impl AsRef<Path>) -> Self {
        let dir = dir.as_ref();
        let mut pack = VibrantPack::default();
        if !dir.is_dir() {
            return pack;
        }

        pack.pbr_capable = read_capabilities(dir, &mut pack.warnings);

        if let Some(value) = read_json(&dir.join("lighting/global.json"), &mut pack.warnings) {
            match LightingSettings::parse(&value) {
                Some(settings) => pack.lighting = settings,
                None => pack
                    .warnings
                    .push("lighting/global.json: no minecraft:lighting_settings object".into()),
            }
        }

        if let Some(value) = read_json(
            &dir.join("atmospherics/atmospherics.json"),
            &mut pack.warnings,
        ) {
            match AtmosphereSettings::parse(&value) {
                Some(settings) => pack.atmospherics = settings,
                None => pack.warnings.push(
                    "atmospherics/atmospherics.json: no minecraft:atmosphere_settings object"
                        .into(),
                ),
            }
        }

        let local_path = dir.join("local_lighting/local_lighting.json");
        let legacy_path = dir.join("point_lights/global.json");
        if let Some(value) = read_json(&local_path, &mut pack.warnings) {
            match LocalLightSettings::parse(&value) {
                Some(settings) => pack.local_lighting = settings,
                None => pack.warnings.push(
                    "local_lighting/local_lighting.json: no minecraft:local_light_settings object"
                        .into(),
                ),
            }
        } else if let Some(value) = read_json(&legacy_path, &mut pack.warnings) {
            pack.warnings.push(
                "point_lights/global.json is deprecated; move it to \
                 local_lighting/local_lighting.json"
                    .into(),
            );
            if let Some(settings) = parse_legacy_point_lights(&value) {
                pack.local_lighting = settings;
            }
        }

        if let Some(value) = read_json(&dir.join("pbr/global.json"), &mut pack.warnings) {
            match PbrFallbackSettings::parse(&value) {
                Some(settings) => pack.pbr_fallback = settings,
                None => pack
                    .warnings
                    .push("pbr/global.json: no minecraft:pbr_fallback_settings object".into()),
            }
        }

        let textures = dir.join("textures");
        if textures.is_dir() {
            let mut found = Vec::new();
            collect_texture_sets(&textures, 0, &mut found, &mut pack.warnings);
            for (name, path) in found {
                let Some(value) = read_json(&path, &mut pack.warnings) else {
                    continue;
                };
                match TextureSet::parse(&value) {
                    Some(set) => pack.insert_texture_set(name, set),
                    None => pack.warnings.push(format!(
                        "{}: no minecraft:texture_set object",
                        path.display()
                    )),
                }
            }
        }

        pack
    }

    /// Loads the built-in pack directory.
    pub fn load_default() -> Self {
        Self::load(DEFAULT_PACK_DIR)
    }

    fn insert_texture_set(&mut self, name: String, set: TextureSet) {
        match self.texture_sets.iter_mut().find(|(n, _)| *n == name) {
            Some((_, existing)) => *existing = set,
            None => self.texture_sets.push((name, set)),
        }
    }

    pub fn texture_set(&self, name: &str) -> Option<&TextureSet> {
        self.texture_sets
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, set)| set)
    }

    pub fn texture_set_count(&self) -> usize {
        self.texture_sets.len()
    }
}

fn read_json(path: &Path, warnings: &mut Vec<String>) -> Option<json::Json> {
    if !path.is_file() {
        return None;
    }
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(e) => {
            warnings.push(format!("{}: {e}", path.display()));
            return None;
        }
    };
    match json::parse(&source) {
        Ok(value) => Some(value),
        Err(e) => {
            warnings.push(format!("{}: {e}", path.display()));
            None
        }
    }
}

fn read_capabilities(dir: &Path, warnings: &mut Vec<String>) -> bool {
    let Some(manifest) = read_json(&dir.join("manifest.json"), warnings) else {
        return false;
    };
    manifest
        .get("capabilities")
        .and_then(json::Json::as_array)
        .is_some_and(|caps| {
            caps.iter()
                .filter_map(json::Json::as_str)
                .any(|c| c == "pbr" || c == "raytraced")
        })
}

// The pre-1.21.120 file, whose only content was a table of colors. Every
// entry in it was a point light.
fn parse_legacy_point_lights(root: &json::Json) -> Option<LocalLightSettings> {
    // The published sample carries a doubled namespace; accept both.
    let settings = root
        .get("minecraft:point_light_settings")
        .or_else(|| root.get("minecraft:minecraft:point_light_settings"))?;
    let colors = settings.get("colors")?.as_object()?;
    let rewritten = json::Json::Object(vec![(
        "minecraft:local_light_settings".to_string(),
        json::Json::Object(
            colors
                .iter()
                .map(|(name, color)| {
                    (
                        name.clone(),
                        json::Json::Object(vec![
                            ("light_color".to_string(), color.clone()),
                            (
                                "light_type".to_string(),
                                json::Json::String("point_light".to_string()),
                            ),
                        ]),
                    )
                })
                .collect(),
        ),
    )]);
    LocalLightSettings::parse(&rewritten)
}

fn collect_texture_sets(
    dir: &Path,
    depth: usize,
    out: &mut Vec<(String, PathBuf)>,
    warnings: &mut Vec<String>,
) {
    if depth > MAX_TEXTURE_SEARCH_DEPTH {
        warnings.push(format!(
            "{}: texture search stopped at depth {MAX_TEXTURE_SEARCH_DEPTH}",
            dir.display()
        ));
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            warnings.push(format!("{}: {e}", dir.display()));
            return;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Follows symlinks by design so a pack can share a texture tree;
        // the depth cap is what stops a cycle.
        if path.is_dir() {
            collect_texture_sets(&path, depth + 1, out, warnings);
            continue;
        }
        let Some(file_name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if let Some(base) = file_name.strip_suffix(TEXTURE_SET_SUFFIX) {
            out.push((base.to_string(), path));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::block::BlockType;
    use crate::vibrant::keyframe::Color;
    use std::sync::atomic::{AtomicU32, Ordering};

    // Each test gets its own directory under the OS temp dir, removed on
    // drop so a failure does not leak fixtures.
    struct TempPack(PathBuf);

    impl TempPack {
        fn new() -> Self {
            static COUNTER: AtomicU32 = AtomicU32::new(0);
            let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "voxelpopuli_vibrant_{}_{unique}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn write(&self, relative: &str, contents: &str) {
            let path = self.0.join(relative);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, contents).unwrap();
        }

        fn load(&self) -> VibrantPack {
            VibrantPack::load(&self.0)
        }
    }

    impl Drop for TempPack {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_missing_directory_yields_silent_defaults() {
        let pack = VibrantPack::load("definitely/not/a/pack/dir");
        assert!(pack.warnings.is_empty());
        assert!(!pack.pbr_capable);
        assert_eq!(pack.lighting.sky_intensity_at(0.0), 1.0);
        assert_eq!(pack.texture_set_count(), 0);
    }

    #[test]
    fn loads_a_full_pack() {
        let pack = TempPack::new();
        pack.write(
            "manifest.json",
            r##"{ "format_version": 1, "capabilities": ["pbr"] }"##,
        );
        pack.write(
            "lighting/global.json",
            r##"{ "minecraft:lighting_settings": { "sky": { "intensity": 0.4 } } }"##,
        );
        pack.write(
            "atmospherics/atmospherics.json",
            r##"{ "minecraft:atmosphere_settings": { "rayleigh_strength": 2.0 } }"##,
        );
        pack.write(
            "local_lighting/local_lighting.json",
            r##"{ "minecraft:local_light_settings": { "minecraft:torch": { "light_color": "#FF0000" } } }"##,
        );
        pack.write(
            "pbr/global.json",
            r##"{ "minecraft:pbr_fallback_settings": { "blocks": { "global_metalness_emissive_roughness_subsurface": [255, 0, 0, 0] } } }"##,
        );
        pack.write(
            "textures/blocks/iron_block.texture_set.json",
            r##"{ "minecraft:texture_set": { "color": "iron_block", "metalness_emissive_roughness": "iron_block_mer" } }"##,
        );

        let loaded = pack.load();
        assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
        assert!(loaded.pbr_capable);
        assert_eq!(loaded.lighting.sky_intensity_at(0.0), 0.4);
        assert_eq!(loaded.atmospherics.sample(0.0).rayleigh_strength, 2.0);
        assert_eq!(
            loaded.local_lighting.get(BlockType::Torch).unwrap().color,
            Color::rgb(1.0, 0.0, 0.0)
        );
        assert_eq!(loaded.pbr_fallback.blocks.metalness, 1.0);
        assert!(loaded.texture_set("iron_block").is_some());
    }

    #[test]
    fn a_manifest_without_the_capability_reports_not_pbr_capable() {
        let pack = TempPack::new();
        pack.write("manifest.json", r##"{ "capabilities": ["chemistry"] }"##);
        assert!(!pack.load().pbr_capable);
    }

    #[test]
    fn malformed_json_warns_and_keeps_the_default() {
        let pack = TempPack::new();
        pack.write("lighting/global.json", "{ this is not json");
        let loaded = pack.load();
        assert_eq!(loaded.warnings.len(), 1);
        assert!(loaded.warnings[0].contains("global.json"));
        assert_eq!(loaded.lighting.sky_intensity_at(0.0), 1.0);
    }

    #[test]
    fn a_valid_file_with_the_wrong_root_object_warns() {
        let pack = TempPack::new();
        pack.write("pbr/global.json", r##"{ "format_version": "1.21.40" }"##);
        let loaded = pack.load();
        assert_eq!(loaded.warnings.len(), 1);
        assert!(loaded.warnings[0].contains("minecraft:pbr_fallback_settings"));
    }

    #[test]
    fn finds_texture_sets_nested_under_the_textures_tree() {
        let pack = TempPack::new();
        pack.write(
            "textures/blocks/deep/nested/stone.texture_set.json",
            r##"{ "minecraft:texture_set": { "color": "stone" } }"##,
        );
        pack.write("textures/blocks/ignore_me.json", r##"{ "nope": 1 }"##);
        let loaded = pack.load();
        assert_eq!(loaded.texture_set_count(), 1);
        assert!(loaded.texture_set("stone").is_some());
    }

    #[test]
    fn falls_back_to_the_deprecated_point_lights_file_with_a_warning() {
        let pack = TempPack::new();
        pack.write(
            "point_lights/global.json",
            r##"{
                "format_version": "1.21.40",
                "minecraft:minecraft:point_light_settings": {
                    "colors": { "minecraft:torch": "#00FF00" }
                }
            }"##,
        );
        let loaded = pack.load();
        assert!(loaded.warnings.iter().any(|w| w.contains("deprecated")));
        let torch = loaded.local_lighting.get(BlockType::Torch).unwrap();
        assert_eq!(torch.color, Color::rgb(0.0, 1.0, 0.0));
        assert_eq!(torch.light_type, lighting::LightType::Point);
    }

    #[test]
    fn the_current_local_lighting_file_takes_precedence_over_the_deprecated_one() {
        let pack = TempPack::new();
        pack.write(
            "point_lights/global.json",
            r##"{ "minecraft:point_light_settings": { "colors": { "minecraft:torch": "#00FF00" } } }"##,
        );
        pack.write(
            "local_lighting/local_lighting.json",
            r##"{ "minecraft:local_light_settings": { "minecraft:torch": { "light_color": "#0000FF" } } }"##,
        );
        let loaded = pack.load();
        assert!(!loaded.warnings.iter().any(|w| w.contains("deprecated")));
        assert_eq!(
            loaded.local_lighting.get(BlockType::Torch).unwrap().color,
            Color::rgb(0.0, 0.0, 1.0)
        );
    }
}
