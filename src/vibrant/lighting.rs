// lighting/global.json, local_lighting/local_lighting.json and
// pbr/global.json, as specified by the Vibrant Visuals Light Sources
// reference. Field names and value ranges match the published schemas so
// packs authored for Bedrock load unchanged.
use super::json::Json;
use super::keyframe::{Color, Keyframed, keyframed_or};
use crate::block::BlockType;
use crate::java_compat::java_block_name_to_block_type;

/// One celestial light in `directional_lights.orbital`.
#[derive(Clone, Debug)]
pub struct OrbitalLight {
    /// Brightness in lux. Real values: a clear noon sun is upwards of
    /// 100,000 lx, the full moon well under one.
    pub illuminance: Keyframed<f32>,
    pub color: Keyframed<Color>,
}

/// `directional_lights.flash`, the End's celestial flash. Not key-framed:
/// its illuminance is a peak that the dimension scales at random.
#[derive(Clone, Debug)]
pub struct FlashLight {
    pub illuminance: f32,
    pub color: Color,
}

#[derive(Clone, Debug)]
pub struct LightingSettings {
    pub identifier: String,
    pub sun: OrbitalLight,
    pub moon: OrbitalLight,
    /// Rotation of the sun/moon orbit off the standard axis, in degrees.
    pub orbital_offset_degrees: Keyframed<f32>,
    pub flash: FlashLight,
    /// How far albedo is desaturated when computing emissive light, [0, 1].
    pub emissive_desaturation: f32,
    /// Fallback light where nothing else lights a surface. Documented
    /// default is #FFFFFF at 0.02 lx, and the range is [0.0, 5.0].
    pub ambient_illuminance: Keyframed<f32>,
    pub ambient_color: Keyframed<Color>,
    /// How much energy the sky contributes to indirect diffuse and
    /// specular, [0.1, 1.0]. Lower values darken shadows.
    pub sky_intensity: Keyframed<f32>,
}

impl Default for LightingSettings {
    fn default() -> Self {
        Self {
            identifier: "voxelpopuli:default_lighting".to_string(),
            // A clear-sky day curve in real lux: full sun at noon (day
            // fraction 0.0), collapsing through civil twilight to
            // starlight at midnight (0.5).
            sun: OrbitalLight {
                illuminance: day_curve(&[
                    (0.0, 100_000.0),
                    (0.25, 20_000.0),
                    (0.35, 400.0),
                    (0.5, 1.0),
                    (0.65, 400.0),
                    (0.75, 20_000.0),
                ]),
                color: Keyframed::constant(Color::WHITE),
            },
            // Full moonlight measures about a quarter of a lux.
            moon: OrbitalLight {
                illuminance: Keyframed::constant(0.27),
                color: Keyframed::constant(Color::WHITE),
            },
            orbital_offset_degrees: Keyframed::constant(0.0),
            flash: FlashLight {
                illuminance: 5.0,
                color: Color::WHITE,
            },
            emissive_desaturation: 0.1,
            ambient_illuminance: Keyframed::constant(0.02),
            ambient_color: Keyframed::constant(Color::WHITE),
            sky_intensity: Keyframed::constant(1.0),
        }
    }
}

fn day_curve(stops: &[(f32, f32)]) -> Keyframed<f32> {
    let object = Json::Object(
        stops
            .iter()
            .map(|(t, v)| (t.to_string(), Json::Number(*v as f64)))
            .collect(),
    );
    Keyframed::parse(&object).expect("day curve stops are well formed")
}

impl LightingSettings {
    pub fn parse(root: &Json) -> Option<Self> {
        let settings = root.get("minecraft:lighting_settings")?;
        let mut out = LightingSettings::default();
        if let Some(id) = settings
            .get("description")
            .and_then(|d| d.get("identifier"))
            .and_then(Json::as_str)
        {
            out.identifier = id.to_string();
        }
        if let Some(directional) = settings.get("directional_lights") {
            if let Some(orbital) = directional.get("orbital") {
                if let Some(sun) = orbital.get("sun") {
                    out.sun = parse_orbital(sun, &out.sun);
                }
                if let Some(moon) = orbital.get("moon") {
                    out.moon = parse_orbital(moon, &out.moon);
                }
                out.orbital_offset_degrees = keyframed_or(orbital, "orbital_offset_degrees", 0.0);
            }
            if let Some(flash) = directional.get("flash") {
                out.flash = FlashLight {
                    illuminance: flash
                        .get("illuminance")
                        .and_then(Json::as_f32)
                        .unwrap_or(out.flash.illuminance),
                    color: flash
                        .get("color")
                        .and_then(Color::parse)
                        .unwrap_or(out.flash.color),
                };
            }
        }
        if let Some(emissive) = settings.get("emissive") {
            if let Some(desaturation) = emissive.get("desaturation").and_then(Json::as_f32) {
                out.emissive_desaturation = desaturation.clamp(0.0, 1.0);
            }
        }
        if let Some(ambient) = settings.get("ambient") {
            out.ambient_illuminance = keyframed_or(ambient, "illuminance", 0.02);
            out.ambient_color = keyframed_or(ambient, "color", Color::WHITE);
        }
        if let Some(sky) = settings.get("sky") {
            out.sky_intensity = keyframed_or(sky, "intensity", 1.0);
        }
        Some(out)
    }

    /// Ambient illuminance clamped to the documented [0.0, 5.0] range.
    pub fn ambient_illuminance_at(&self, time: f32) -> f32 {
        self.ambient_illuminance.sample(time).clamp(0.0, 5.0)
    }

    /// Sky intensity clamped to the documented [0.1, 1.0] range.
    pub fn sky_intensity_at(&self, time: f32) -> f32 {
        self.sky_intensity.sample(time).clamp(0.1, 1.0)
    }
}

fn parse_orbital(value: &Json, fallback: &OrbitalLight) -> OrbitalLight {
    OrbitalLight {
        illuminance: value
            .get("illuminance")
            .and_then(Keyframed::parse)
            .unwrap_or_else(|| fallback.illuminance.clone()),
        color: value
            .get("color")
            .and_then(Keyframed::parse)
            .unwrap_or_else(|| fallback.color.clone()),
    }
}

/// How a light-emitting block contributes light. Static lights are baked
/// and flat; point lights are dynamic, with specular and shadows, and cost
/// considerably more.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LightType {
    Static,
    Point,
}

#[derive(Clone, Copy, Debug)]
pub struct LocalLight {
    pub color: Color,
    pub light_type: LightType,
}

/// local_lighting/local_lighting.json, resolved onto our own BlockType.
#[derive(Clone, Debug)]
pub struct LocalLightSettings {
    entries: Vec<(BlockType, LocalLight)>,
}

impl Default for LocalLightSettings {
    fn default() -> Self {
        // The blocks the engine treats as point lights out of the box.
        // Packs may override the color, and may promote their own blocks.
        let defaults: &[(&str, u32)] = &[
            ("minecraft:torch", 0xEFE39D),
            ("minecraft:redstone_torch", 0xFF0000),
            ("minecraft:end_rod", 0xFFFFFF),
            ("minecraft:lantern", 0xCE8133),
            ("minecraft:soul_lantern", 0x00FFFF),
            ("minecraft:soul_torch", 0x00FFFF),
            ("minecraft:candle", 0xEFE39D),
            ("minecraft:sea_pickle", 0xFFFFFF),
            ("minecraft:copper_torch", 0xB8EF8D),
            ("minecraft:copper_lantern", 0xB8EF8D),
        ];
        let mut entries = Vec::new();
        for (name, hex) in defaults {
            let block = java_block_name_to_block_type(name);
            // Names this build has no block for resolve to Air; skip them
            // rather than paint every empty voxel with a light.
            if block == BlockType::Air {
                continue;
            }
            insert(
                &mut entries,
                block,
                LocalLight {
                    color: color_from_hex(*hex),
                    light_type: LightType::Point,
                },
            );
        }
        Self { entries }
    }
}

fn color_from_hex(hex: u32) -> Color {
    Color::rgb(
        ((hex >> 16) & 0xff) as f32 / 255.0,
        ((hex >> 8) & 0xff) as f32 / 255.0,
        (hex & 0xff) as f32 / 255.0,
    )
}

fn insert(entries: &mut Vec<(BlockType, LocalLight)>, block: BlockType, light: LocalLight) {
    match entries.iter_mut().find(|(b, _)| *b == block) {
        Some((_, existing)) => *existing = light,
        None => entries.push((block, light)),
    }
}

impl LocalLightSettings {
    pub fn parse(root: &Json) -> Option<Self> {
        let settings = root.get("minecraft:local_light_settings")?;
        let mut out = LocalLightSettings::default();
        for (name, entry) in settings.as_object()? {
            let block = java_block_name_to_block_type(name);
            if block == BlockType::Air {
                continue;
            }
            // An entry with no light_type keeps whatever the block already
            // was, so a pack can recolor a torch without promoting it.
            let previous = out.get(block);
            let light_type = match entry.get("light_type").and_then(Json::as_str) {
                Some("point_light") => LightType::Point,
                Some("static_light") => LightType::Static,
                _ => previous.map_or(LightType::Static, |l| l.light_type),
            };
            let color = entry
                .get("light_color")
                .and_then(Color::parse)
                .or_else(|| previous.map(|l| l.color))
                .unwrap_or(Color::WHITE);
            insert(&mut out.entries, block, LocalLight { color, light_type });
        }
        Some(out)
    }

    pub fn get(&self, block: BlockType) -> Option<LocalLight> {
        self.entries
            .iter()
            .find(|(b, _)| *b == block)
            .map(|(_, light)| *light)
    }

    pub fn is_point_light(&self, block: BlockType) -> bool {
        self.get(block)
            .is_some_and(|l| l.light_type == LightType::Point)
    }
}

/// A metalness/emissive/roughness/subsurface tuple. Channel order matches
/// the packed MERS texture: R metalness, G emissive, B roughness,
/// A subsurface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mers {
    pub metalness: f32,
    pub emissive: f32,
    pub roughness: f32,
    pub subsurface: f32,
}

impl Mers {
    /// Fully rough dielectric: the sensible material for untextured
    /// geometry, and what the docs use as the sample fallback.
    pub const DEFAULT: Mers = Mers {
        metalness: 0.0,
        emissive: 0.0,
        roughness: 1.0,
        subsurface: 0.0,
    };

    pub fn from_color(color: Color) -> Self {
        let [metalness, emissive, roughness, subsurface] = color.raw();
        Self {
            metalness,
            emissive,
            roughness,
            subsurface,
        }
    }

    pub fn to_array(self) -> [f32; 4] {
        [
            self.metalness,
            self.emissive,
            self.roughness,
            self.subsurface,
        ]
    }
}

/// pbr/global.json: the MERS applied to anything with no texture set.
#[derive(Clone, Copy, Debug)]
pub struct PbrFallbackSettings {
    pub blocks: Mers,
    pub actors: Mers,
    pub particles: Mers,
    pub items: Mers,
}

impl Default for PbrFallbackSettings {
    fn default() -> Self {
        Self {
            blocks: Mers::DEFAULT,
            actors: Mers::DEFAULT,
            particles: Mers::DEFAULT,
            items: Mers::DEFAULT,
        }
    }
}

impl PbrFallbackSettings {
    pub fn parse(root: &Json) -> Option<Self> {
        let settings = root.get("minecraft:pbr_fallback_settings")?;
        let mut out = PbrFallbackSettings::default();
        let field = |name: &str, fallback: Mers| {
            settings
                .get(name)
                .and_then(|o| o.get("global_metalness_emissive_roughness_subsurface"))
                .and_then(Color::parse)
                .map(Mers::from_color)
                .unwrap_or(fallback)
        };
        out.blocks = field("blocks", out.blocks);
        out.actors = field("actors", out.actors);
        out.particles = field("particles", out.particles);
        out.items = field("items", out.items);
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vibrant::json;

    #[test]
    fn parses_the_documented_lighting_example() {
        let source = r##"{
            "format_version": "1.26.0",
            "minecraft:lighting_settings": {
                "description": { "identifier": "my_pack:default_lighting" },
                "directional_lights": {
                    "orbital": {
                        "sun": {
                            "illuminance": { "0.0": 109880.0, "0.5": 1.0, "1.0": 109880.0 },
                            "color": [ 255.0, 255.0, 255.0 ]
                        },
                        "moon": { "illuminance": 0.27, "color": "#ffffff" },
                        "orbital_offset_degrees": 3.0
                    },
                    "flash": { "illuminance": 5.0, "color": [ 255.0, 255.0, 255.0 ] }
                },
                "emissive": { "desaturation": 0.1 },
                "ambient": { "illuminance": 0.02, "color": "#ffffff" },
                "sky": { "intensity": 1.0 }
            }
        }"##;
        let settings = LightingSettings::parse(&json::parse(source).unwrap()).unwrap();
        assert_eq!(settings.identifier, "my_pack:default_lighting");
        assert_eq!(settings.sun.illuminance.sample(0.0), 109880.0);
        assert_eq!(settings.sun.illuminance.sample(0.5), 1.0);
        assert_eq!(settings.moon.illuminance.sample(0.3), 0.27);
        assert_eq!(settings.orbital_offset_degrees.sample(0.0), 3.0);
        assert_eq!(settings.flash.illuminance, 5.0);
        assert_eq!(settings.emissive_desaturation, 0.1);
        assert_eq!(settings.ambient_illuminance_at(0.0), 0.02);
        assert_eq!(settings.sky_intensity_at(0.0), 1.0);
    }

    #[test]
    fn absent_objects_keep_documented_defaults() {
        let source = r##"{ "minecraft:lighting_settings": {} }"##;
        let settings = LightingSettings::parse(&json::parse(source).unwrap()).unwrap();
        assert_eq!(settings.ambient_illuminance_at(0.4), 0.02);
        assert_eq!(settings.ambient_color.sample(0.4), Color::WHITE);
        assert_eq!(settings.sky_intensity_at(0.4), 1.0);
        assert_eq!(settings.moon.illuminance.sample(0.4), 0.27);
    }

    #[test]
    fn out_of_range_values_are_clamped_to_the_schema() {
        let source = r##"{ "minecraft:lighting_settings": {
            "ambient": { "illuminance": 900.0 },
            "sky": { "intensity": 0.0 },
            "emissive": { "desaturation": 7.0 }
        } }"##;
        let settings = LightingSettings::parse(&json::parse(source).unwrap()).unwrap();
        assert_eq!(settings.ambient_illuminance_at(0.0), 5.0);
        assert_eq!(settings.sky_intensity_at(0.0), 0.1);
        assert_eq!(settings.emissive_desaturation, 1.0);
    }

    #[test]
    fn rejects_a_file_without_the_settings_object() {
        let value = json::parse(r##"{ "format_version": "1.26.0" }"##).unwrap();
        assert!(LightingSettings::parse(&value).is_none());
    }

    #[test]
    fn torch_is_a_point_light_by_default() {
        let settings = LocalLightSettings::default();
        assert!(settings.is_point_light(BlockType::Torch));
        assert!(!settings.is_point_light(BlockType::Stone));
    }

    #[test]
    fn pack_recolors_a_default_point_light_without_demoting_it() {
        let source = r##"{
            "format_version": "1.21.120",
            "minecraft:local_light_settings": {
                "minecraft:torch": { "light_color": "#FF0000" }
            }
        }"##;
        let settings = LocalLightSettings::parse(&json::parse(source).unwrap()).unwrap();
        let torch = settings.get(BlockType::Torch).unwrap();
        assert_eq!(torch.color, Color::rgb(1.0, 0.0, 0.0));
        assert_eq!(torch.light_type, LightType::Point);
    }

    #[test]
    fn pack_promotes_its_own_block_to_a_point_light() {
        let source = r##"{
            "minecraft:local_light_settings": {
                "minecraft:redstone_lamp": { "light_color": [0, 255, 0], "light_type": "point_light" }
            }
        }"##;
        let settings = LocalLightSettings::parse(&json::parse(source).unwrap()).unwrap();
        let lamp = settings.get(BlockType::RedstoneLamp).unwrap();
        assert_eq!(lamp.light_type, LightType::Point);
        assert_eq!(lamp.color, Color::rgb(0.0, 1.0, 0.0));
    }

    #[test]
    fn unknown_block_identifiers_are_ignored() {
        let source = r##"{
            "minecraft:local_light_settings": {
                "somepack:not_a_block": { "light_type": "point_light" }
            }
        }"##;
        let settings = LocalLightSettings::parse(&json::parse(source).unwrap()).unwrap();
        assert!(!settings.is_point_light(BlockType::Air));
    }

    #[test]
    fn parses_the_documented_pbr_fallback_example() {
        let source = r##"{
            "format_version": "1.21.40",
            "minecraft:pbr_fallback_settings": {
                "blocks": { "global_metalness_emissive_roughness_subsurface": [0.0, 0.0, 255.0, 0.0] },
                "actors": { "global_metalness_emissive_roughness_subsurface": "#00FF0000" }
            }
        }"##;
        let settings = PbrFallbackSettings::parse(&json::parse(source).unwrap()).unwrap();
        assert_eq!(settings.blocks, Mers::DEFAULT);
        assert_eq!(settings.actors.roughness, 0.0);
        assert_eq!(settings.actors.emissive, 1.0);
        // Unlisted categories keep the default.
        assert_eq!(settings.items, Mers::DEFAULT);
    }
}
