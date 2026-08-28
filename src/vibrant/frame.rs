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

pub fn build_uniforms(pack: &VibrantPack, input: &FrameInput) -> DeferredUniforms {
    let time = input.day_fraction;
    let lighting = &pack.lighting;
    let atmosphere = pack.atmospherics.sample(time);

    let sun_dir = sun_direction(time, lighting.orbital_offset_degrees.sample(time));
    // The sun and moon sit at opposite points of the same orbit.
    let moon_dir = -sun_dir;

    // A body below the horizon contributes nothing, so its illuminance
    // fades out as it sets rather than lighting the world from underneath.
    let horizon_fade = |dir: Vec3| dir.y.clamp(0.0, 1.0);
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
            0.0,
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

/// Scale that puts a forward-rendered, display-referred color into the
/// same lux-scaled space the deferred pass writes, so transparent
/// geometry survives tone mapping alongside the opaque scene.
pub fn hdr_scale(uniforms: &DeferredUniforms) -> f32 {
    let key = uniforms.sun_direction_illuminance[3]
        + uniforms.moon_direction_illuminance[3]
        + uniforms.ambient_color_illuminance[3];
    key / std::f32::consts::PI
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
}
