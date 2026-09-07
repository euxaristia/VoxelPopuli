// Turns pack settings plus the current time and camera into the uniform
// block the deferred shaders read.
//
// This is where authored values become physical ones: illuminance stays in
// lux all the way to the shader, and the exposure that brings lux back
// into display range is derived here rather than metered from the frame.
use super::VibrantPack;
use crate::renderer::DeferredUniforms;
use glam::{Mat4, Vec3};

/// The game's own day counter maps onto the schema's day fraction, where
/// 0.0 is noon and 0.5 is midnight.
pub const SECONDS_PER_DAY: f32 = 1200.0;

/// Everything about this frame that is not in the pack.
#[derive(Clone, Copy, Debug)]
pub struct FrameInput {
    /// Fraction of the day in [0, 1); 0.0 is noon.
    pub day_fraction: f32,
    pub camera_pos: Vec3,
    /// The same view-projection the geometry pass drew with.
    pub view_proj: Mat4,
}

/// Converts the game's running clock to the schema's day fraction.
pub fn day_fraction(dusk_time: f32) -> f32 {
    // The world's sun angle is sin(2*pi*t/1200), so t/1200 == 0.25 is the
    // zenith. The schema puts the zenith at 0.0.
    (dusk_time / SECONDS_PER_DAY - 0.25).rem_euclid(1.0)
}

/// How much a celestial body contributes given the direction towards it.
///
/// The window matches the forward sky: full day above sun_y 0.3, night
/// below -0.2, and a smoothstep between so sunset is a dusk rather than
/// a frame-long snap when the body crosses y = 0.
pub fn horizon_fade(dir: Vec3) -> f32 {
    smoothstep(-0.2, 0.3, dir.y)
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Direction towards the sun for a day fraction, matching the world's own
/// sun angle so shadows agree with the drawn sun.
pub fn sun_direction(day_fraction: f32, orbital_offset_degrees: f32) -> Vec3 {
    let angle = (day_fraction + 0.25) * std::f32::consts::TAU;
    let tilt = orbital_offset_degrees.to_radians();
    // The orbit runs in the Y-Z plane; the offset tilts that plane so the
    // sun does not pass exactly through the zenith.
    let (sin_a, cos_a) = angle.sin_cos();
    Vec3::new(-sin_a * tilt.sin(), sin_a * tilt.cos(), cos_a).normalize_or(Vec3::Y)
}

// Exposure bounds, as EV100. The scene's key illuminance swings by five
// orders of magnitude across a day, and letting auto-exposure fully
// normalize that would make midnight look exactly as bright as noon.
// Clamping the low end keeps night dark while still lifting it out of
// black; the high end is a clear noon sky.
const MIN_EV100: f32 = -2.0;
const MAX_EV100: f32 = 17.0;
// ISO 100 with the standard incident-light calibration constant C = 250.
const INCIDENT_CALIBRATION: f32 = 2.5;
// Middle-grey placement of the tone curve.
const EXPOSURE_COMPENSATION: f32 = 1.2;

/// Exposure multiplier for a scene whose key illuminance is `lux`.
pub fn exposure_for_illuminance(lux: f32) -> f32 {
    let ev100 = (lux.max(1e-4) / INCIDENT_CALIBRATION)
        .log2()
        .clamp(MIN_EV100, MAX_EV100);
    1.0 / (EXPOSURE_COMPENSATION * ev100.exp2())
}

/// How bright block light (torches, lava) reads, in lux. Low enough to
/// vanish in daylight, high enough to carry a cave at night.
const BLOCK_LIGHT_LUX: f32 = 1.5;
/// Gentle aerial perspective shared by opaque geometry and transparent water.
pub const HAZE_DENSITY: f32 = 0.0018;

pub fn build_uniforms(pack: &VibrantPack, input: &FrameInput) -> DeferredUniforms {
    let time = input.day_fraction;
    let lighting = &pack.lighting;
    let atmosphere = pack.atmospherics.sample(time);

    let sun_dir = sun_direction(time, lighting.orbital_offset_degrees.sample(time));
    // The sun and moon sit at opposite points of the same orbit.
    let moon_dir = -sun_dir;

    let sun_lux = lighting.sun.illuminance.sample(time).max(0.0) * horizon_fade(sun_dir);
    let moon_lux = lighting.moon.illuminance.sample(time).max(0.0) * horizon_fade(moon_dir);
    let ambient_lux = lighting.ambient_illuminance_at(time);

    let key_illuminance = sun_lux + moon_lux + ambient_lux;
    let exposure = exposure_for_illuminance(key_illuminance);

    let block_light = pack
        .local_lighting
        .get(crate::block::BlockType::Torch)
        .map(|light| light.color)
        .unwrap_or(super::keyframe::Color::WHITE)
        .to_linear();

    DeferredUniforms {
        inv_view_proj: input.view_proj.inverse().to_cols_array(),
        camera_pos_exposure: [
            input.camera_pos.x,
            input.camera_pos.y,
            input.camera_pos.z,
            exposure,
        ],
        sun_direction_illuminance: [sun_dir.x, sun_dir.y, sun_dir.z, sun_lux],
        sun_color: rgb4(lighting.sun.color.sample(time).to_linear()),
        moon_direction_illuminance: [moon_dir.x, moon_dir.y, moon_dir.z, moon_lux],
        moon_color: rgb4(lighting.moon.color.sample(time).to_linear()),
        ambient_color_illuminance: {
            let [r, g, b] = lighting.ambient_color.sample(time).to_linear();
            [r, g, b, ambient_lux]
        },
        sky_params: [
            lighting.sky_intensity_at(time),
            lighting.emissive_desaturation,
            BLOCK_LIGHT_LUX,
            HAZE_DENSITY,
        ],
        zenith_color: rgb4(atmosphere.zenith_color),
        horizon_color: rgb4(atmosphere.horizon_color),
        atmosphere: [
            atmosphere.rayleigh_strength,
            atmosphere.sun_mie_strength,
            atmosphere.moon_mie_strength,
            atmosphere.sun_glare_shape,
        ],
        horizon_stops: [
            atmosphere.horizon_min,
            atmosphere.horizon_start,
            atmosphere.horizon_mie_start,
            atmosphere.horizon_max,
        ],
        block_light_color: rgb4(block_light),
    }
}

/// Scale that puts a forward-rendered, linear color into the
/// same lux-scaled space the deferred pass writes, so transparent
/// geometry survives tone mapping alongside the opaque scene.
pub fn hdr_scale(uniforms: &DeferredUniforms) -> f32 {
    let key = uniforms.sun_direction_illuminance[3]
        + uniforms.moon_direction_illuminance[3]
        + uniforms.ambient_color_illuminance[3];
    key / std::f32::consts::PI
}

/// Clouds reflect the current orbital light and sky instead of glowing white.
pub fn cloud_tint(uniforms: &DeferredUniforms) -> Vec3 {
    let sun_lux = uniforms.sun_direction_illuminance[3];
    let moon_lux = uniforms.moon_direction_illuminance[3];
    let sun_weight = sun_lux / (sun_lux + moon_lux).max(1e-4);
    let sun = Vec3::from_slice(&uniforms.sun_color);
    let moon = Vec3::from_slice(&uniforms.moon_color);
    let horizon = Vec3::from_slice(&uniforms.horizon_color);
    (sun * sun_weight + moon * (1.0 - sun_weight) * 0.2) * 0.65 + horizon * 0.35
}

fn rgb4(rgb: [f32; 3]) -> [f32; 4] {
    [rgb[0], rgb[1], rgb[2], 0.0]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noon_input() -> FrameInput {
        FrameInput {
            day_fraction: 0.0,
            camera_pos: Vec3::new(0.0, 70.0, 0.0),
            view_proj: Mat4::perspective_rh(1.2, 1.6, 0.1, 1000.0),
        }
    }

    #[test]
    fn day_fraction_puts_noon_at_zero() {
        // The world's sun is highest a quarter of the way through its cycle.
        assert!((day_fraction(SECONDS_PER_DAY * 0.25) - 0.0).abs() < 1e-6);
        assert!((day_fraction(SECONDS_PER_DAY * 0.75) - 0.5).abs() < 1e-6);
        // And it wraps rather than running off the end of the curves.
        assert!((0.0..1.0).contains(&day_fraction(SECONDS_PER_DAY * 12.3)));
        assert!((0.0..1.0).contains(&day_fraction(-SECONDS_PER_DAY * 0.4)));
    }

    #[test]
    fn the_sun_is_overhead_at_noon_and_below_at_midnight() {
        assert!(sun_direction(0.0, 0.0).y > 0.99);
        assert!(sun_direction(0.5, 0.0).y < -0.99);
        // Sunrise and sunset put it on the horizon.
        assert!(sun_direction(0.25, 0.0).y.abs() < 1e-6);
    }

    #[test]
    fn the_orbital_offset_tilts_the_orbit_off_the_zenith() {
        let tilted = sun_direction(0.0, 30.0);
        assert!(tilted.y < 0.99);
        assert!(tilted.x.abs() > 0.1);
        // Still a unit vector, so the lighting math stays energy-correct.
        assert!((tilted.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn exposure_falls_as_the_scene_brightens() {
        let noon = exposure_for_illuminance(100_000.0);
        let night = exposure_for_illuminance(0.29);
        assert!(noon < night);
        // A white surface at noon lands in display range rather than
        // blowing out: radiance is illuminance / pi.
        let lit = 100_000.0 / std::f32::consts::PI * noon;
        assert!((0.2..1.5).contains(&lit), "noon exposed value {lit}");
    }

    #[test]
    fn night_stays_darker_than_day_after_exposure() {
        let day = 100_000.0 / std::f32::consts::PI * exposure_for_illuminance(100_000.0);
        let night = 0.29 / std::f32::consts::PI * exposure_for_illuminance(0.29);
        assert!(
            night < day * 0.75,
            "auto exposure flattened the day/night difference: {night} vs {day}"
        );
    }

    #[test]
    fn exposure_is_finite_for_a_pitch_black_scene() {
        assert!(exposure_for_illuminance(0.0).is_finite());
        assert!(exposure_for_illuminance(0.0) > 0.0);
    }

    #[test]
    fn a_body_below_the_horizon_contributes_no_light() {
        let pack = VibrantPack::default();
        let midnight = build_uniforms(
            &pack,
            &FrameInput {
                day_fraction: 0.5,
                ..noon_input()
            },
        );
        assert_eq!(midnight.sun_direction_illuminance[3], 0.0);
        assert!(midnight.moon_direction_illuminance[3] > 0.0);

        let noon = build_uniforms(&pack, &noon_input());
        assert!(noon.sun_direction_illuminance[3] > 1000.0);
        assert_eq!(noon.moon_direction_illuminance[3], 0.0);
    }

    #[test]
    fn horizon_fade_matches_the_forward_dusk_window() {
        assert_eq!(horizon_fade(Vec3::Y), 1.0);
        assert_eq!(horizon_fade(Vec3::new(0.0, 0.3, 0.0)), 1.0);
        assert_eq!(horizon_fade(Vec3::new(0.0, -0.2, 0.0)), 0.0);
        assert_eq!(horizon_fade(-Vec3::Y), 0.0);
        let sunset = horizon_fade(Vec3::new(1.0, 0.0, 0.0));
        assert!(sunset > 0.2 && sunset < 0.8, "sunset fade {sunset}");
    }

    fn key_lux(day_fraction: f32) -> f32 {
        let uniforms = build_uniforms(
            &VibrantPack::default(),
            &FrameInput {
                day_fraction,
                ..noon_input()
            },
        );
        uniforms.sun_direction_illuminance[3] + uniforms.moon_direction_illuminance[3]
    }

    #[test]
    fn twilight_does_not_drop_illuminance_off_a_cliff() {
        // 0.01 of the day is 12 seconds. Adjacent samples through sunset
        // should stay within a small factor; the old y.clamp(0) cut sun
        // lux to zero in a single frame.
        let samples = [0.22, 0.23, 0.24, 0.25, 0.26, 0.27];
        let mut prev = key_lux(samples[0]);
        for &t in &samples[1..] {
            let lux = key_lux(t);
            assert!(
                prev / lux.max(1e-4) < 6.0,
                "sun+moon lux jumped at {t}: {prev} -> {lux}"
            );
            prev = lux;
        }
        assert!(key_lux(0.0) > key_lux(0.22));
        assert!(key_lux(0.28) > 0.0);
    }

    #[test]
    fn default_sky_shifts_from_day_blue_to_night() {
        let pack = VibrantPack::default();
        let noon = pack.atmospherics.sample(0.0);
        let midnight = pack.atmospherics.sample(0.5);
        // A dark blue night preserves silhouettes without looking like day.
        assert!(noon.zenith_color[2] > noon.zenith_color[0]);
        assert!(midnight.zenith_color[2] > midnight.zenith_color[0]);
        assert!(
            midnight.zenith_color.iter().sum::<f32>()
                < noon.zenith_color.iter().sum::<f32>() * 0.12
        );
        let sunset = pack.atmospherics.sample(0.25);
        assert!(sunset.horizon_color[0] > sunset.horizon_color[2]);
    }

    #[test]
    fn ambient_never_drops_to_zero_so_caves_are_not_pure_black() {
        let pack = VibrantPack::default();
        let uniforms = build_uniforms(&pack, &noon_input());
        assert!(uniforms.ambient_color_illuminance[3] > 0.0);
    }

    #[test]
    fn the_inverse_view_projection_round_trips() {
        let input = noon_input();
        let uniforms = build_uniforms(&VibrantPack::default(), &input);
        let inverse = Mat4::from_cols_array(&uniforms.inv_view_proj);
        let identity = input.view_proj * inverse;
        for i in 0..4 {
            for j in 0..4 {
                let want = if i == j { 1.0 } else { 0.0 };
                assert!(
                    (identity.col(i)[j] - want).abs() < 1e-4,
                    "not an inverse at {i},{j}"
                );
            }
        }
    }

    #[test]
    fn hdr_scale_tracks_the_key_illuminance() {
        let pack = VibrantPack::default();
        let noon = hdr_scale(&build_uniforms(&pack, &noon_input()));
        let midnight = hdr_scale(&build_uniforms(
            &pack,
            &FrameInput {
                day_fraction: 0.5,
                ..noon_input()
            },
        ));
        assert!(noon > midnight * 100.0);
        assert!(midnight > 0.0);
    }

    #[test]
    fn clouds_take_warm_sunset_and_dim_blue_moonlight() {
        let pack = VibrantPack::default();
        let tint_at = |day_fraction| {
            cloud_tint(&build_uniforms(
                &pack,
                &FrameInput {
                    day_fraction,
                    ..noon_input()
                },
            ))
        };
        let day = tint_at(0.0);
        let sunset = tint_at(0.25);
        let night = tint_at(0.5);
        assert!(sunset.x > sunset.z * 1.5);
        assert!(night.z > night.x);
        assert!(night.max_element() < day.max_element() * 0.25);
    }
}
