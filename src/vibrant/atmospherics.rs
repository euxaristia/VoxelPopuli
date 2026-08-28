// atmospherics/atmospherics.json, as specified by the Vibrant Visuals
// Atmospheric Effects reference. Every field is key-framable over the day.
//
// These terms drive an analytic sky: a Rayleigh term for the broad blue
// gradient and a forward-scattering Mie lobe for the glare around the sun
// and moon. The sun and moon colors from lighting/global.json feed the
// same calculation, so they move the sky as well as lit surfaces.
use super::json::Json;
use super::keyframe::{Color, Keyframed, keyframed_or};

/// `horizon_blend_stops`: how the sky is split between horizon and zenith.
#[derive(Clone, Debug)]
pub struct HorizonBlendStops {
    /// Minimum horizon height.
    pub min: Keyframed<f32>,
    /// Height relative to the horizon where the zenith color takes over.
    pub start: Keyframed<f32>,
    /// Height relative to the horizon where Mie scattering begins.
    pub mie_start: Keyframed<f32>,
    /// Maximum horizon height.
    pub max: Keyframed<f32>,
}

impl Default for HorizonBlendStops {
    fn default() -> Self {
        Self {
            min: Keyframed::constant(0.0),
            start: Keyframed::constant(0.25),
            mie_start: Keyframed::constant(0.5),
            max: Keyframed::constant(0.25),
        }
    }
}

#[derive(Clone, Debug)]
pub struct AtmosphereSettings {
    pub identifier: String,
    pub horizon_blend_stops: HorizonBlendStops,
    /// Strength of the Rayleigh scattering term.
    pub rayleigh_strength: Keyframed<f32>,
    /// Strength of the sun's Mie term.
    pub sun_mie_strength: Keyframed<f32>,
    /// Strength of the moon's Mie term.
    pub moon_mie_strength: Keyframed<f32>,
    /// Shape of the Mie lobe: larger is a tighter, harder glare.
    pub sun_glare_shape: Keyframed<f32>,
    pub sky_zenith_color: Keyframed<Color>,
    pub sky_horizon_color: Keyframed<Color>,
}

impl Default for AtmosphereSettings {
    fn default() -> Self {
        Self {
            identifier: "voxelpopuli:default_atmospherics".to_string(),
            horizon_blend_stops: HorizonBlendStops::default(),
            rayleigh_strength: Keyframed::constant(1.0),
            sun_mie_strength: Keyframed::constant(1.0),
            moon_mie_strength: Keyframed::constant(0.0),
            sun_glare_shape: Keyframed::constant(4.0),
            sky_zenith_color: Keyframed::constant(Color::rgb(0.0, 0.49, 0.64)),
            sky_horizon_color: Keyframed::constant(Color::rgb(0.77, 0.86, 1.0)),
        }
    }
}

impl AtmosphereSettings {
    pub fn parse(root: &Json) -> Option<Self> {
        let settings = root.get("minecraft:atmosphere_settings")?;
        let mut out = AtmosphereSettings::default();
        if let Some(id) = settings
            .get("description")
            .and_then(|d| d.get("identifier"))
            .and_then(Json::as_str)
        {
            out.identifier = id.to_string();
        }
        if let Some(stops) = settings.get("horizon_blend_stops") {
            let defaults = HorizonBlendStops::default();
            out.horizon_blend_stops = HorizonBlendStops {
                min: keyframed_or(stops, "min", defaults.min.sample(0.0)),
                start: keyframed_or(stops, "start", defaults.start.sample(0.0)),
                mie_start: keyframed_or(stops, "mie_start", defaults.mie_start.sample(0.0)),
                max: keyframed_or(stops, "max", defaults.max.sample(0.0)),
            };
        }
        out.rayleigh_strength = keyframed_or(settings, "rayleigh_strength", 1.0);
        out.sun_mie_strength = keyframed_or(settings, "sun_mie_strength", 1.0);
        out.moon_mie_strength = keyframed_or(settings, "moon_mie_strength", 0.0);
        out.sun_glare_shape = keyframed_or(settings, "sun_glare_shape", 4.0);
        if let Some(zenith) = settings.get("sky_zenith_color").and_then(Keyframed::parse) {
            out.sky_zenith_color = zenith;
        }
        if let Some(horizon) = settings.get("sky_horizon_color").and_then(Keyframed::parse) {
            out.sky_horizon_color = horizon;
        }
        Some(out)
    }

    /// Everything the sky shader needs for one frame, already sampled.
    pub fn sample(&self, time: f32) -> AtmosphereFrame {
        AtmosphereFrame {
            horizon_min: self.horizon_blend_stops.min.sample(time),
            horizon_start: self.horizon_blend_stops.start.sample(time),
            horizon_mie_start: self.horizon_blend_stops.mie_start.sample(time),
            horizon_max: self.horizon_blend_stops.max.sample(time),
            rayleigh_strength: self.rayleigh_strength.sample(time).max(0.0),
            sun_mie_strength: self.sun_mie_strength.sample(time).max(0.0),
            moon_mie_strength: self.moon_mie_strength.sample(time).max(0.0),
            // A zero or negative exponent would blow the Mie lobe up to a
            // flat white sky, so hold it at a sane minimum.
            sun_glare_shape: self.sun_glare_shape.sample(time).max(1.0),
            zenith_color: self.sky_zenith_color.sample(time).to_linear(),
            horizon_color: self.sky_horizon_color.sample(time).to_linear(),
        }
    }
}

/// One frame's worth of atmosphere terms, in linear light.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AtmosphereFrame {
    pub horizon_min: f32,
    pub horizon_start: f32,
    pub horizon_mie_start: f32,
    pub horizon_max: f32,
    pub rayleigh_strength: f32,
    pub sun_mie_strength: f32,
    pub moon_mie_strength: f32,
    pub sun_glare_shape: f32,
    pub zenith_color: [f32; 3],
    pub horizon_color: [f32; 3],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vibrant::json;

    #[test]
    fn parses_the_documented_atmospherics_example() {
        let source = r##"{
            "format_version": "1.21.40",
            "minecraft:atmosphere_settings": {
                "description": { "identifier": "my_pack:default_atmospherics" },
                "horizon_blend_stops": {
                    "min": { "0.0": 0.0, "0.34": 0.11 },
                    "start": { "0.0": 0.25, "0.34": 0.401 },
                    "mie_start": { "0.0": 0.5, "0.34": 0.5009999871253967 },
                    "max": { "0.0": 0.25, "0.34": 0.467 }
                },
                "rayleigh_strength": { "0.0": 1.026124954, "0.25": 0.1624998152 },
                "sun_mie_strength": { "0.0": 1.0, "0.25": 3.0 },
                "moon_mie_strength": { "0.0": 0.0, "0.3249999880790710": 1.0 },
                "sun_glare_shape": { "0.0": 15.89900016784668, "0.6": 4.0 },
                "sky_zenith_color": { "0.0": [0, 125, 164], "0.5": [7, 10, 36] },
                "sky_horizon_color": { "0.0": [255, 255, 254], "0.25": [255, 85, 85] }
            }
        }"##;
        let settings = AtmosphereSettings::parse(&json::parse(source).unwrap()).unwrap();
        assert_eq!(settings.identifier, "my_pack:default_atmospherics");
        assert_eq!(settings.rayleigh_strength.sample(0.0), 1.026124954);
        assert_eq!(settings.sun_mie_strength.sample(0.25), 3.0);
        assert_eq!(settings.horizon_blend_stops.start.sample(0.0), 0.25);
        // Midway between the two zenith stops.
        let mid = settings.sky_zenith_color.sample(0.25);
        assert!((mid.g - (125.0 + 10.0) / 2.0 / 255.0).abs() < 1e-5);
    }

    #[test]
    fn absent_fields_keep_defaults() {
        let value = json::parse(r##"{ "minecraft:atmosphere_settings": {} }"##).unwrap();
        let settings = AtmosphereSettings::parse(&value).unwrap();
        let frame = settings.sample(0.0);
        assert_eq!(frame.rayleigh_strength, 1.0);
        assert_eq!(frame.sun_glare_shape, 4.0);
        assert_eq!(frame.moon_mie_strength, 0.0);
    }

    #[test]
    fn sampled_terms_are_clamped_to_usable_ranges() {
        let source = r##"{ "minecraft:atmosphere_settings": {
            "rayleigh_strength": -5.0,
            "sun_glare_shape": 0.0
        } }"##;
        let settings = AtmosphereSettings::parse(&json::parse(source).unwrap()).unwrap();
        let frame = settings.sample(0.5);
        assert_eq!(frame.rayleigh_strength, 0.0);
        assert_eq!(frame.sun_glare_shape, 1.0);
    }

    #[test]
    fn rejects_a_file_without_the_settings_object() {
        let value = json::parse(r##"{ "format_version": "1.21.40" }"##).unwrap();
        assert!(AtmosphereSettings::parse(&value).is_none());
    }
}
