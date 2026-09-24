// water/water.json, as specified by the Vibrant Visuals Water Customization reference.
// https://learn.microsoft.com/en-us/minecraft/creator/documents/vibrantvisuals/watercustomization?view=minecraft-bedrock-stable

use super::json::Json;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleConcentrations {
    /// 0.0 - 15.0 mg/L; higher concentrations produce yellow/yellow-brown colors (absorbs blue).
    pub cdom: f32,
    /// 0.0 - 10.0 mg/L; higher concentrations produce green colors (absorbs blue and red).
    pub chlorophyll: f32,
    /// 0.0 - 300.0 mg/L; higher concentrations produce red/red-brown colors (absorbs blue and green).
    pub suspended_sediment: f32,
}

impl Default for ParticleConcentrations {
    fn default() -> Self {
        Self {
            cdom: 1.0,
            chlorophyll: 0.5,
            suspended_sediment: 0.5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WaveSettings {
    pub enabled: bool,
    /// 0.0 - 3.0: wave displacement amount.
    pub depth: f32,
    /// 0.0 - 360.0: heading change in degrees between octaves.
    pub direction_increment: f32,
    /// 0.01 - 3.0: wave size / frequency per block.
    pub frequency: f32,
    /// 0.0 - 2.0: frequency scaling per octave.
    pub frequency_scaling: f32,
    /// 0.0 - 1.0: blend between neighboring octaves.
    pub mix: f32,
    /// 1 - 30: wave octaves.
    pub octaves: u32,
    /// -1.0 - 1.0: pull smaller waves into larger ones (concave > 0, convex < 0).
    pub pull: f32,
    /// 0.01 - 1.0: fractal resolution.
    pub sample_width: f32,
    /// 1.0 - 10.0: wave shape (1.0 = sine, > 1.0 = sharp crests).
    pub shape: f32,
    /// 0.01 - 10.0: starting speed.
    pub speed: f32,
    /// 0.0 - 2.0: speed multiplier per octave.
    pub speed_scaling: f32,
}

impl Default for WaveSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            depth: 1.0,
            direction_increment: 80.0,
            frequency: 1.0,
            frequency_scaling: 1.2,
            mix: 0.2,
            octaves: 8,
            pull: 0.38,
            sample_width: 0.01,
            shape: 1.5,
            speed: 2.0,
            speed_scaling: 1.03,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct CausticSettings {
    pub enabled: bool,
    /// 0.01 - 5.0 seconds per frame.
    pub frame_length: f32,
    /// 1 - 6: brightness power.
    pub power: u32,
    /// 0.1 - 5.0: tiling scale.
    pub scale: f32,
    pub texture: Option<String>,
}

impl Default for CausticSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            frame_length: 0.05,
            power: 2,
            scale: 0.5,
            texture: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct WaterSettings {
    pub identifier: String,
    pub particles: ParticleConcentrations,
    pub waves: WaveSettings,
    pub caustics: CausticSettings,
    /// 0.0 - 1.0: contribution of biome surface_color.
    pub biome_water_color_contribution: f32,
}

impl Default for WaterSettings {
    fn default() -> Self {
        Self {
            identifier: "minecraft:default_water".into(),
            particles: ParticleConcentrations::default(),
            waves: WaveSettings::default(),
            caustics: CausticSettings::default(),
            biome_water_color_contribution: 0.2,
        }
    }
}

impl WaterSettings {
    /// Computes the RGB water body color from particle concentrations (CDOM, chlorophyll, suspended sediment)
    /// combined with the biome water color.
    ///
    /// The physics follows bio-optical oceanography as specified by Bedrock Vibrant Visuals:
    /// - Pure water absorbs red strongly, blue weakly.
    /// - CDOM absorbs blue strongly, shifting light towards yellow/brown.
    /// - Chlorophyll absorbs blue and red, shifting light towards green.
    /// - Suspended sediment scatters and absorbs blue/green, shifting light towards red/clay-brown.
    pub fn compute_water_color(&self, biome_surface_color: [f32; 3]) -> [f32; 3] {
        // Base absorption coefficients per meter (R, G, B)
        // Pure water:
        let aw = [0.35f32, 0.035, 0.005];
        // CDOM absorption:
        let acdom = [
            self.particles.cdom * 0.01,
            self.particles.cdom * 0.05,
            self.particles.cdom * 0.18,
        ];
        // Chlorophyll absorption:
        let achl = [
            self.particles.chlorophyll * 0.08,
            self.particles.chlorophyll * 0.015,
            self.particles.chlorophyll * 0.09,
        ];
        // Suspended sediment absorption:
        let ased = [
            self.particles.suspended_sediment * 0.005,
            self.particles.suspended_sediment * 0.025,
            self.particles.suspended_sediment * 0.06,
        ];

        // Total absorption:
        let a = [
            aw[0] + acdom[0] + achl[0] + ased[0],
            aw[1] + acdom[1] + achl[1] + ased[1],
            aw[2] + acdom[2] + achl[2] + ased[2],
        ];

        // Transmittance over effective depth (approx 2.5 meters)
        let path = 2.5f32;
        let tr = (-a[0] * path).exp();
        let tg = (-a[1] * path).exp();
        let tb = (-a[2] * path).exp();

        // Scattering contribution from suspended sediment (backscattering adds diffuse tint)
        let bb = (self.particles.suspended_sediment * 0.02).clamp(0.0, 0.6);
        let sr = tr * (1.0 - bb) + bb * 0.55;
        let sg = tg * (1.0 - bb) + bb * 0.40;
        let sb = tb * (1.0 - bb) + bb * 0.25;

        let max_val = sr.max(sg).max(sb).max(1e-4);
        let optical_color = [sr / max_val, sg / max_val, sb / max_val];

        // Blend with biome water color
        let f = self.biome_water_color_contribution.clamp(0.0, 1.0);
        [
            optical_color[0] * (1.0 - f) + biome_surface_color[0] * f,
            optical_color[1] * (1.0 - f) + biome_surface_color[1] * f,
            optical_color[2] * (1.0 - f) + biome_surface_color[2] * f,
        ]
    }

    pub fn parse(json: &Json) -> Option<Self> {
        let settings = json.get("minecraft:water_settings")?;
        let mut out = WaterSettings::default();

        if let Some(desc) = settings.get("description") {
            if let Some(id) = desc.get("identifier").and_then(Json::as_str) {
                out.identifier = id.to_string();
            }
        }

        if let Some(p) = settings.get("particle_concentrations") {
            if let Some(v) = p.get("cdom").and_then(Json::as_f32) {
                out.particles.cdom = v.clamp(0.0, 15.0);
            }
            if let Some(v) = p.get("chlorophyll").and_then(Json::as_f32) {
                out.particles.chlorophyll = v.clamp(0.0, 10.0);
            }
            if let Some(v) = p.get("suspended_sediment").and_then(Json::as_f32) {
                out.particles.suspended_sediment = v.clamp(0.0, 300.0);
            }
        }

        if let Some(w) = settings.get("waves") {
            if let Some(v) = w.get("enabled").and_then(Json::as_bool) {
                out.waves.enabled = v;
            }
            if let Some(v) = w.get("depth").and_then(Json::as_f32) {
                out.waves.depth = v.clamp(0.0, 3.0);
            }
            if let Some(v) = w.get("direction_increment").and_then(Json::as_f32) {
                out.waves.direction_increment = v.clamp(0.0, 360.0);
            }
            if let Some(v) = w.get("frequency").and_then(Json::as_f32) {
                out.waves.frequency = v.clamp(0.01, 3.0);
            }
            if let Some(v) = w.get("frequency_scaling").and_then(Json::as_f32) {
                out.waves.frequency_scaling = v.clamp(0.0, 2.0);
            }
            if let Some(v) = w.get("mix").and_then(Json::as_f32) {
                out.waves.mix = v.clamp(0.0, 1.0);
            }
            if let Some(v) = w.get("octaves").and_then(Json::as_u32) {
                out.waves.octaves = v.clamp(1, 30);
            }
            if let Some(v) = w.get("pull").and_then(Json::as_f32) {
                out.waves.pull = v.clamp(-1.0, 1.0);
            }
            if let Some(v) = w.get("sampleWidth").and_then(Json::as_f32) {
                out.waves.sample_width = v.clamp(0.01, 1.0);
            }
            if let Some(v) = w.get("shape").and_then(Json::as_f32) {
                out.waves.shape = v.clamp(1.0, 10.0);
            }
            if let Some(v) = w.get("speed").and_then(Json::as_f32) {
                out.waves.speed = v.clamp(0.01, 10.0);
            }
            if let Some(v) = w.get("speed_scaling").and_then(Json::as_f32) {
                out.waves.speed_scaling = v.clamp(0.0, 2.0);
            }
        }

        if let Some(c) = settings.get("caustics") {
            if let Some(v) = c.get("enabled").and_then(Json::as_bool) {
                out.caustics.enabled = v;
            }
            if let Some(v) = c.get("frame_length").and_then(Json::as_f32) {
                out.caustics.frame_length = v.clamp(0.01, 5.0);
            }
            if let Some(v) = c.get("power").and_then(Json::as_u32) {
                out.caustics.power = v.clamp(1, 6);
            }
            if let Some(v) = c.get("scale").and_then(Json::as_f32) {
                out.caustics.scale = v.clamp(0.1, 5.0);
            }
            if let Some(v) = c.get("texture").and_then(Json::as_str) {
                out.caustics.texture = Some(v.to_string());
            }
        }

        if let Some(v) = settings
            .get("biome_water_color_contribution")
            .and_then(Json::as_f32)
        {
            out.biome_water_color_contribution = v.clamp(0.0, 1.0);
        }

        Some(out)
    }
}

/// Authentic Bedrock surface water colors for each biome in RGB [0.0, 1.0].
pub fn biome_surface_water_color(biome: crate::chunk::Biome) -> [f32; 3] {
    use crate::chunk::Biome;
    match biome {
        Biome::MangroveSwamp => [0.227, 0.478, 0.416], // #3A7A6A
        Biome::Desert => [0.196, 0.647, 1.000],        // #32A5FF
        Biome::SnowyTundra | Biome::SnowyTaiga => [0.125, 0.502, 0.753], // #2080C0
        Biome::CherryGrove => [0.365, 0.718, 0.937],   // #5DB7EF
        Biome::Plains
        | Biome::Forest
        | Biome::BirchForest
        | Biome::FlowerForest
        | Biome::SunflowerPlains
        | Biome::Meadow
        | Biome::Mountains
        | Biome::HighHills => [0.090, 0.529, 0.831], // #1787D4 (Bedrock default ocean)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_documented_water_example() {
        let json = crate::vibrant::json::parse(
            r#"{
            "format_version": "1.26.0",
            "minecraft:water_settings": {
                "description": {
                    "identifier": "my_pack:default_water"
                },
                "particle_concentrations": {
                    "chlorophyll": 0.5,
                    "suspended_sediment": 0.5,
                    "cdom": 1.0
                },
                "waves": {
                    "enabled": true,
                    "depth": 1.0,
                    "direction_increment": 80.0,
                    "frequency": 1.0,
                    "frequency_scaling": 1.2,
                    "mix": 0.2,
                    "octaves": 28,
                    "pull": 0.38,
                    "sampleWidth": 0.01,
                    "shape": 1.5,
                    "speed": 2.0,
                    "speed_scaling": 1.03
                },
                "caustics": {
                    "enabled": true,
                    "frame_length": 0.05,
                    "power": 2,
                    "scale": 0.5
                },
                "biome_water_color_contribution": 0.2
            }
        }"#,
        )
        .unwrap();

        let s = WaterSettings::parse(&json).unwrap();
        assert_eq!(s.identifier, "my_pack:default_water");
        assert_eq!(s.particles.cdom, 1.0);
        assert_eq!(s.particles.chlorophyll, 0.5);
        assert_eq!(s.particles.suspended_sediment, 0.5);
        assert!(s.waves.enabled);
        assert_eq!(s.waves.depth, 1.0);
        assert_eq!(s.waves.direction_increment, 80.0);
        assert_eq!(s.waves.frequency, 1.0);
        assert_eq!(s.waves.frequency_scaling, 1.2);
        assert_eq!(s.waves.octaves, 28);
        assert_eq!(s.waves.pull, 0.38);
        assert_eq!(s.waves.sample_width, 0.01);
        assert_eq!(s.waves.shape, 1.5);
        assert_eq!(s.waves.speed, 2.0);
        assert!(s.caustics.enabled);
        assert_eq!(s.caustics.power, 2);
        assert_eq!(s.caustics.scale, 0.5);
        assert_eq!(s.biome_water_color_contribution, 0.2);
    }

    #[test]
    fn particle_concentrations_shift_water_color_optically() {
        let mut settings = WaterSettings::default();
        let biome_blue = [0.090, 0.529, 0.831];

        // 1. Clear ocean (low CDOM, low chlorophyll, low sediment): azure blue
        settings.particles = ParticleConcentrations {
            cdom: 0.1,
            chlorophyll: 0.1,
            suspended_sediment: 0.0,
        };
        let c_ocean = settings.compute_water_color(biome_blue);
        assert!(
            c_ocean[2] > c_ocean[0],
            "ocean water must be blue-dominated"
        );

        // 2. High CDOM (tea-colored river/lake): absorbs blue, higher red/green
        settings.particles = ParticleConcentrations {
            cdom: 10.0,
            chlorophyll: 0.2,
            suspended_sediment: 0.1,
        };
        let c_cdom = settings.compute_water_color(biome_blue);
        assert!(
            c_cdom[0] > c_ocean[0],
            "CDOM water must have more yellow/red than clear ocean"
        );

        // 3. High Chlorophyll (algal bloom/marsh): green-dominated
        settings.particles = ParticleConcentrations {
            cdom: 0.5,
            chlorophyll: 8.0,
            suspended_sediment: 0.1,
        };
        let c_chl = settings.compute_water_color(biome_blue);
        assert!(
            c_chl[1] > c_chl[0] && c_chl[1] > c_chl[2],
            "high chlorophyll must be green-dominated"
        );

        // 4. High Suspended Sediment (muddy flood): red-brown/clay
        settings.particles = ParticleConcentrations {
            cdom: 0.5,
            chlorophyll: 0.2,
            suspended_sediment: 150.0,
        };
        let c_sed = settings.compute_water_color(biome_blue);
        assert!(
            c_sed[0] > c_sed[2],
            "sediment water must be red/brown-dominated"
        );
    }
}
